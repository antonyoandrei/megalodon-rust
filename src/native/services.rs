//! Own only processes launched by this app. Existing servers and mounts are borrowed.
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    net::{SocketAddr, TcpStream},
    os::windows::{fs::MetadataExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

const HIDDEN: u32 = 0x08000000;

#[derive(Clone, Serialize, Deserialize)]
pub struct LocalConfig {
    pub jellyfin: PathBuf,
    pub data: PathBuf,
    pub cache: PathBuf,
    pub drive: Option<DriveConfig>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct DriveConfig {
    pub executable: PathBuf,
    pub config: PathBuf,
    pub remote: String,
    pub mount: PathBuf,
    pub cache_gb: u32,
}

pub fn local_server(base: &str) -> bool {
    reqwest::Url::parse(base).is_ok_and(|url| {
        url.scheme() == "http"
            && url.port_or_known_default() == Some(8096)
            && matches!(
                url.host_str(),
                Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
            )
    })
}

pub fn instance_lock(dir: &Path) -> Result<Option<File>, String> {
    fs::create_dir_all(dir).map_err(|_| "No se puede abrir la configuración local.")?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join("native.lock"))
        .map_err(|_| "No se puede abrir el bloqueo de la aplicación.")?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(_) => Err("No se puede comprobar si Megalodon ya está abierto.".into()),
    }
}

fn program_files() -> PathBuf {
    PathBuf::from(std::env::var_os("ProgramFiles").unwrap_or_else(|| "C:\\Program Files".into()))
}
pub fn discover(dir: &Path) -> Option<LocalConfig> {
    if let Ok(bytes) = fs::read(dir.join("services.json")) {
        return serde_json::from_slice(&bytes).ok();
    }
    let legacy = PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("Megalodon");
    let json: serde_json::Value =
        serde_json::from_slice(&fs::read(legacy.join("launcher.json")).ok()?).ok()?;
    let path = |key| json.get(key).and_then(|v| v.as_str()).map(PathBuf::from);
    let drive = if json.get("Mode").and_then(|v| v.as_str()) == Some("Drive") {
        Some(DriveConfig {
            executable: path("Rclone")?,
            config: legacy.join("rclone.conf"),
            remote: json.get("Remote")?.as_str()?.to_owned(),
            mount: path("MountPath")?,
            cache_gb: json
                .get("CacheGB")
                .and_then(|v| v.as_u64())
                .unwrap_or(30)
                .clamp(1, 500) as u32,
        })
    } else {
        None
    };
    Some(LocalConfig {
        jellyfin: path("Jellyfin")?,
        data: path("JellyfinData")?,
        cache: legacy.join("jellyfin-cache"),
        drive,
    })
}

fn running(executable: &Path) -> bool {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        System::Diagnostics::ToolHelp::*,
    };
    let Some(name) = executable.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    // Read-only inventory. Never use a matching name as authority to terminate a process.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return true;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = Process32FirstW(snapshot, &mut entry);
        let mut found = false;
        while ok != 0 {
            let end = entry
                .szExeFile
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(entry.szExeFile.len());
            if String::from_utf16_lossy(&entry.szExeFile[..end]).eq_ignore_ascii_case(name) {
                found = true;
                break;
            }
            ok = Process32NextW(snapshot, &mut entry);
        }
        CloseHandle(snapshot);
        found
    }
}
fn port_open() -> bool {
    TcpStream::connect_timeout(
        &SocketAddr::from(([127, 0, 0, 1], 8096)),
        Duration::from_millis(250),
    )
    .is_ok()
}
fn mount_ready(path: &Path) -> bool {
    // A folder can exist before WinFsp attaches. Do not let Jellyfin scan it empty.
    // A configured drive letter instead becomes available only after it is mounted.
    let raw = path.as_os_str().to_string_lossy();
    if raw.len() == 2 && raw.ends_with(':') {
        return PathBuf::from(format!("{raw}\\")).is_dir();
    }
    fs::symlink_metadata(path).is_ok_and(|meta| {
        meta.file_attributes() & 0x400 != 0 || (path.has_root() && path.parent().is_none())
    })
}
fn command(executable: &Path) -> Command {
    let mut command = Command::new(executable);
    command
        .creation_flags(HIDDEN)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

pub struct Services {
    pub status: String,
    config: Option<LocalConfig>,
    server: Option<Child>,
    owned_base: String,
    shutdown_token: String,
    drive: Option<Child>,
    pub cancel: Arc<AtomicBool>,
}
impl Services {
    pub fn update_auth(&mut self, base: &str, token: &str) {
        if base.trim_end_matches('/') == self.owned_base.trim_end_matches('/') {
            self.shutdown_token = token.to_owned();
        }
    }
    pub fn new(dir: &Path) -> Self {
        Self {
            status: String::new(),
            config: discover(dir),
            server: None,
            owned_base: String::new(),
            shutdown_token: String::new(),
            drive: None,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }
    pub async fn start(&mut self, base: &str, token: &str) -> Result<(), String> {
        if !self.owned_base.is_empty() && self.owned_base != base {
            self.stop().await?;
            self.cancel.store(false, Ordering::Relaxed);
        }
        if !local_server(base) {
            self.status = "Servidor remoto".into();
            return Ok(());
        }
        let Some(config) = self.config.clone() else {
            self.status = "Conexión directa al servidor".into();
            return Ok(());
        };
        let external_server = port_open() || running(&config.jellyfin);
        if let Some(drive) = &config.drive {
            let external_drive = running(&drive.executable);
            if !mount_ready(&drive.mount) && !external_drive {
                if !drive.executable.is_file() || !drive.config.is_file() {
                    return Err(
                        "Falta rclone o su conexión. Usa Preparar servicios en Ajustes.".into(),
                    );
                }
                self.status = "Conectando Drive…".into();
                let mut process = command(&drive.executable);
                process
                    .arg("mount")
                    .arg(&drive.remote)
                    .arg(&drive.mount)
                    .arg("--config")
                    .arg(&drive.config)
                    .arg("--read-only")
                    .args([
                        "--vfs-cache-mode",
                        "full",
                        "--vfs-cache-max-age",
                        "12h",
                        "--vfs-cache-min-free-space",
                        "5G",
                    ])
                    .arg("--vfs-cache-max-size")
                    .arg(format!("{}G", drive.cache_gb))
                    .arg("--cache-dir")
                    .arg(config.cache.join("drive"))
                    .args([
                        "--buffer-size",
                        "32M",
                        "--vfs-read-ahead",
                        "256M",
                        "--log-level",
                        "ERROR",
                    ]);
                self.drive = Some(
                    process
                        .spawn()
                        .map_err(|_| "No se pudo iniciar rclone. Comprueba WinFsp.")?,
                );
            }
            let deadline = Instant::now() + Duration::from_secs(30);
            while !mount_ready(&drive.mount) {
                if self.cancel.load(Ordering::Relaxed) {
                    return Err("Arranque cancelado.".into());
                }
                if self
                    .drive
                    .as_mut()
                    .is_some_and(|child| child.try_wait().ok().flatten().is_some())
                    || Instant::now() > deadline
                {
                    return Err(if external_drive {
                            "rclone ya está abierto, pero la unidad configurada no está disponible. No se ha iniciado otra copia."
                        } else {
                            "No se pudo montar Drive. Comprueba la conexión de rclone y WinFsp."
                        }.into());
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
        if !external_server {
            if !config.jellyfin.is_file() {
                return Err("Falta Jellyfin. Usa Preparar servicios en Ajustes.".into());
            }
            fs::create_dir_all(&config.cache)
                .map_err(|_| "No se pudo abrir la caché de Jellyfin.")?;
            let folder = config
                .jellyfin
                .parent()
                .ok_or("Ruta de Jellyfin no válida.")?;
            self.status = "Iniciando Jellyfin…".into();
            let mut process = command(&config.jellyfin);
            process
                .arg("--datadir")
                .arg(&config.data)
                .arg("--cachedir")
                .arg(&config.cache)
                .arg("--webdir")
                .arg(folder.join("jellyfin-web"))
                .arg("--ffmpeg")
                .arg(folder.join("ffmpeg.exe"));
            self.owned_base = base.to_owned();
            self.shutdown_token = token.to_owned();
            self.server = Some(
                process
                    .spawn()
                    .map_err(|_| "No se pudo iniciar Jellyfin.")?,
            );
        }
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| e.to_string())?;
        let deadline = Instant::now() + Duration::from_secs(120);
        loop {
            if self.cancel.load(Ordering::Relaxed) {
                return Err("Arranque cancelado.".into());
            }
            if let Ok(response) = client.get(format!("{base}/health")).send().await
                && response.status().is_success()
                && response.text().await.unwrap_or_default().trim() == "Healthy"
            {
                break;
            }
            if self
                .server
                .as_mut()
                .is_some_and(|p| p.try_wait().ok().flatten().is_some())
            {
                return Err(
                    "Jellyfin se ha cerrado durante el arranque. Revisa su configuración.".into(),
                );
            }
            if Instant::now() > deadline {
                return Err("Jellyfin sigue preparándose. Puedes volver a actualizar la biblioteca cuando termine.".into());
            }
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
        self.status = if self
            .server
            .as_mut()
            .is_some_and(|child| child.try_wait().ok().flatten().is_none())
        {
            "Jellyfin se cerrará con Megalodon"
        } else {
            "Jellyfin ya estaba abierto · se conservará al salir"
        }
        .into();
        Ok(())
    }
    pub async fn stop(&mut self) -> Result<(), String> {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(server) = &mut self.server
            && server.try_wait().map_err(|e| e.to_string())?.is_none()
        {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|e| e.to_string())?;
            let response = client
                .post(format!("{}/System/Shutdown", self.owned_base))
                .header(
                    "Authorization",
                    super::jellyfin::authorization(&self.shutdown_token),
                )
                .send()
                .await
                .map_err(
                    |_| "Jellyfin no ha respondido al cierre. Se conserva el servidor y la unidad.",
                )?;
            if !response.status().is_success() {
                return Err("La cuenta de Jellyfin necesita permiso de administrador para cerrar el servidor local. Se conserva la unidad.".into());
            }
            let deadline = Instant::now() + Duration::from_secs(30);
            while server.try_wait().map_err(|e| e.to_string())?.is_none() {
                if Instant::now() >= deadline {
                    return Err(
                        "Jellyfin aún está cerrando. Se conserva la unidad hasta que termine."
                            .into(),
                    );
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
        if let Some(mut drive) = self.drive.take() {
            if port_open() {
                self.drive = Some(drive);
                return Err(
                    "La unidad se conserva porque Jellyfin sigue abierto fuera de Megalodon."
                        .into(),
                );
            }
            if drive.try_wait().map_err(|e| e.to_string())?.is_none() {
                drive.kill().map_err(|e| e.to_string())?;
            }
            let _ = drive.wait();
        }
        Ok(())
    }
}

pub fn install_package(id: &str) -> Result<(), String> {
    let status = command(Path::new("winget.exe"))
        .args([
            "install",
            "--id",
            id,
            "--exact",
            "--source",
            "winget",
            "--silent",
            "--accept-source-agreements",
            "--accept-package-agreements",
        ])
        .status()
        .map_err(|_| "No se encuentra el Instalador de aplicaciones de Windows (winget).")?;
    if !status.success() {
        return Err(format!(
            "No se pudo preparar {id}. Reabre el instalador para reintentar."
        ));
    }
    Ok(())
}

pub fn prepare(dir: &Path) -> Result<LocalConfig, String> {
    let mut config = discover(dir).unwrap_or_else(|| LocalConfig {
        jellyfin: program_files().join("Jellyfin/Server/jellyfin.exe"),
        data: dir.join("jellyfin"),
        cache: dir.join("jellyfin-cache"),
        drive: None,
    });
    if !config.jellyfin.is_file() {
        install_package("Jellyfin.Server")?;
        config.jellyfin = program_files().join("Jellyfin/Server/jellyfin.exe");
    }
    if !config.jellyfin.is_file() {
        return Err("Jellyfin no aparece en su carpeta de instalación.".into());
    }
    if let Some(drive) = &mut config.drive {
        if !drive.executable.is_file() {
            install_package("Rclone.Rclone")?;
            drive.executable = PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default())
                .join("Microsoft/WinGet/Links/rclone.exe");
            if !drive.executable.is_file() {
                return Err("No se ha localizado rclone después de instalarlo.".into());
            }
        }
        let winfsp = PathBuf::from(std::env::var_os("ProgramFiles(x86)").unwrap_or_default())
            .join("WinFsp/bin/winfsp-x64.dll");
        if !winfsp.is_file() {
            install_package("WinFsp.WinFsp")?;
        }
    }
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let temporary = dir.join("services.tmp");
    fs::write(
        &temporary,
        serde_json::to_vec_pretty(&config).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::rename(temporary, dir.join("services.json")).map_err(|e| e.to_string())?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordinary_folder_is_not_a_ready_mount() {
        let dir = std::env::temp_dir().join(format!("megalodon-mount-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        assert!(!mount_ready(&dir));
        assert!(!mount_ready(&dir.join("absent")));
        fs::remove_dir(dir).unwrap();
    }
    #[test]
    fn remote_servers_are_never_managed() {
        assert!(local_server("http://127.0.0.1:8096"));
        assert!(local_server("http://localhost:8096"));
        for base in [
            "http://192.168.1.8:8096",
            "https://jellyfin.example.com",
            "http://localhost:9000",
        ] {
            assert!(!local_server(base));
        }
    }
    #[test]
    fn instance_lease_prevents_duplicates_and_releases_on_exit() {
        let dir = std::env::temp_dir().join(format!("megalodon-lock-test-{}", std::process::id()));
        let lease = instance_lock(&dir).unwrap().unwrap();
        assert!(instance_lock(&dir).unwrap().is_none());
        drop(lease);
        assert!(instance_lock(&dir).unwrap().is_some());
        let _ = fs::remove_file(dir.join("native.lock"));
        let _ = fs::remove_dir(dir);
    }
    #[test]
    #[ignore = "helper spawned by the owned-service lifecycle test"]
    fn owned_server_helper() {
        use std::io::{Read, Write};
        let file = std::env::var_os("MEGALODON_TEST_PORT_FILE").expect("test-only port file");
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        fs::write(file, listener.local_addr().unwrap().port().to_string()).unwrap();
        let (mut stream, _) = listener.accept().unwrap();
        let mut bytes = [0u8; 4096];
        let count = stream.read(&mut bytes).unwrap();
        let request = String::from_utf8_lossy(&bytes[..count]).to_ascii_lowercase();
        assert!(request.starts_with("post /system/shutdown "));
        assert!(request.contains("authorization: mediabrowser "));
        assert!(request.contains("token=\"lifecycle-test\""));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .unwrap();
    }
    #[test]
    fn owned_server_closes_gracefully_borrowed_server_is_untouched() {
        let dir =
            std::env::temp_dir().join(format!("megalodon-services-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("port.txt");
        let _ = fs::remove_file(&file);
        let child = command(&std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "services::tests::owned_server_helper",
            ])
            .env("MEGALODON_TEST_PORT_FILE", &file)
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !file.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        let port = fs::read_to_string(&file).unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let mut borrowed = Services::new(&dir);
        borrowed.owned_base = format!("http://127.0.0.1:{port}");
        assert!(rt.block_on(borrowed.stop()).is_ok());
        let mut owned = Services::new(&dir);
        owned.server = Some(child);
        owned.owned_base = borrowed.owned_base;
        owned.shutdown_token = "lifecycle-test".into();
        assert!(rt.block_on(owned.stop()).is_ok());
        assert!(
            owned
                .server
                .as_mut()
                .unwrap()
                .try_wait()
                .unwrap()
                .unwrap()
                .success()
        );
        let _ = fs::remove_file(&file);
        let _ = fs::remove_dir(dir);
    }
}
