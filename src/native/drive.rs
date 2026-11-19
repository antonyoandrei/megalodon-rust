//! Read-only Drive access. Helpers are children of this app, never discovered and killed by name.
use super::{
    data, services,
    sources::{self, Kind, Source},
};
use std::os::windows::process::CommandExt;
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

fn command(exe: &Path) -> Command {
    let mut command = Command::new(exe);
    command
        .creation_flags(0x08000000)
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    command
}
pub fn executable() -> Option<PathBuf> {
    if let Some(path) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("rclone.exe")))
        .filter(|p| p.is_file())
    {
        return Some(path);
    }
    if let Some(drive) = services::discover(&data::data_dir())
        .and_then(|s| s.drive)
        .filter(|d| d.executable.is_file())
    {
        return Some(drive.executable);
    }
    let local = PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default())
        .join("Microsoft/WinGet/Links/rclone.exe");
    if local.is_file() {
        return Some(local);
    }
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|p| p.join("rclone.exe"))
            .find(|p| p.is_file())
    })
}
pub fn prepare() -> Result<PathBuf, String> {
    if let Some(path) = executable() {
        return Ok(path);
    }
    services::install_package("Rclone.Rclone")?;
    executable().ok_or_else(||"No se encuentra rclone después de instalarlo. Reabre Megalodon para actualizar el entorno.".into())
}
fn output(mut command: Command, cancel: &AtomicBool, timeout: Duration) -> Result<Vec<u8>, String> {
    let mut child = command
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|_| "No se pudo iniciar la conexión de Drive.")?;
    let stdout = child.stdout.take().unwrap();
    let reader = std::thread::spawn(move || {
        let mut bytes = vec![];
        stdout
            .take(64 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let start = Instant::now();
    let status = loop {
        if cancel.load(Ordering::Relaxed) || start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return Err(if cancel.load(Ordering::Relaxed) {
                "Conexión cancelada."
            } else {
                "Drive no ha respondido a tiempo. Puedes volver a intentarlo."
            }
            .into());
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(80)),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Se interrumpió la conexión con Drive.".into());
            }
        }
    };
    let bytes = reader
        .join()
        .map_err(|_| "No se pudo leer Drive.")?
        .map_err(|_| "No se pudo leer Drive.")?;
    if !status.success() {
        return Err(
            "No se pudo conectar con Drive. Revisa la cuenta, la carpeta y su autorización.".into(),
        );
    }
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("Esta biblioteca es demasiado grande para indexarla en una sola lectura. Conecta una subcarpeta.".into());
    }
    Ok(bytes)
}
pub fn connect(name: &str, cancel: &AtomicBool) -> Result<Source, String> {
    let exe = prepare()?;
    let id = random_id()?;
    let config = data::data_dir().join("drive").join(format!("{id}.conf"));
    std::fs::create_dir_all(config.parent().unwrap())
        .map_err(|_| "No se pudo guardar la conexión.")?;
    let mut cmd = command(&exe);
    cmd.args([
        "config",
        "create",
        "megalodon",
        "drive",
        "scope",
        "drive.readonly",
        "config_is_local",
        "true",
        "--no-output",
        "--config",
    ])
    .arg(&config);
    if let Err(error) = output(cmd, cancel, Duration::from_secs(300)) {
        let _ = std::fs::remove_file(&config);
        return Err(error);
    }
    Ok(Source {
        id: format!("drive-{id}"),
        name: if name.trim().is_empty() {
            "Google Drive".into()
        } else {
            name.trim().into()
        },
        kind: Kind::Drive {
            executable: exe,
            config,
            remote: "megalodon:".into(),
        },
    })
}
pub fn existing(name: &str, config: &Path, remote: &str) -> Result<Source, String> {
    if !config.is_file() {
        return Err("No se encuentra el archivo de conexión de rclone.".into());
    }
    let remote = remote.trim();
    let (account, _) = remote
        .split_once(':')
        .ok_or("Indica una conexión y carpeta, por ejemplo drive:Películas.")?;
    if account.is_empty() || account.starts_with('-') || account.contains(['/', '\\', '\n', '\r']) {
        return Err("El nombre de la conexión no es válido.".into());
    }
    let config = config
        .canonicalize()
        .map_err(|_| "La configuración no está disponible.")?;
    Ok(Source {
        id: format!(
            "drive-{}",
            sources::stable_id(&format!("{}|{remote}", config.display()))
        ),
        name: if name.trim().is_empty() {
            "Google Drive".into()
        } else {
            name.trim().into()
        },
        kind: Kind::Drive {
            executable: prepare()?,
            config,
            remote: remote.into(),
        },
    })
}
#[cfg(test)]
pub fn list(
    exe: &Path,
    config: &Path,
    remote: &str,
    cancel: &AtomicBool,
) -> Result<Vec<String>, String> {
    list_with_env(exe, config, remote, cancel, &[])
}
#[cfg(test)]
pub fn list_with_env(
    exe: &Path,
    config: &Path,
    remote: &str,
    cancel: &AtomicBool,
    environment: &[(String, String)],
) -> Result<Vec<String>, String> {
    Ok(inventory(exe, config, remote, cancel, environment)?
        .into_iter()
        .map(|e| e.path)
        .collect())
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Entry {
    pub path: String,
    #[serde(default)]
    pub mod_time: String,
}

pub fn inventory(
    exe: &Path,
    config: &Path,
    remote: &str,
    cancel: &AtomicBool,
    environment: &[(String, String)],
) -> Result<Vec<Entry>, String> {
    let mut cmd = command(exe);
    cmd.envs(environment.iter().cloned());
    cmd.args([
        "lsjson",
        remote,
        "--recursive",
        "--files-only",
        "--no-mimetype",
        "--config",
    ])
    .arg(config);
    let list: Vec<Entry> = serde_json::from_slice(&output(cmd, cancel, Duration::from_secs(180))?)
        .map_err(|_| "Drive devolvió un catálogo no válido.")?;
    Ok(list)
}

pub fn sync_metadata(
    exe: &Path,
    config: &Path,
    remote: &str,
    destination: &Path,
    cancel: &AtomicBool,
    environment: &[(String, String)],
) -> Result<(), String> {
    std::fs::create_dir_all(destination).map_err(|_| "No se puede guardar la caché de fichas.")?;
    let mut cmd = command(exe);
    cmd.envs(environment.iter().cloned());
    cmd.arg("copy")
        .arg(remote)
        .arg(destination)
        .args([
            "--include",
            "*.nfo",
            "--max-size",
            "1M",
            "--transfers",
            "8",
            "--checkers",
            "8",
            "--config",
        ])
        .arg(config);
    output(cmd, cancel, Duration::from_secs(900))?;
    Ok(())
}

pub const WEBDAV_REMOTE: &str = "megalodon_cloud:";

pub fn webdav_environment(
    exe: &Path,
    url: &str,
    user: &str,
    password: &str,
) -> Result<Vec<(String, String)>, String> {
    // rclone expects its own obscured password format. Feed the plaintext through
    // stdin, never an argument, URL or a configuration file. The saved form is DPAPI.
    let mut child = command(exe)
        .args(["obscure", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|_| "No se pudo preparar la conexión WebDAV.")?;
    let mut input = child
        .stdin
        .take()
        .ok_or("No se pudo preparar la conexión WebDAV.")?;
    if input.write_all(password.as_bytes()).is_err() {
        drop(input);
        let _ = child.kill();
        let _ = child.wait();
        return Err("No se pudo preparar la conexión WebDAV.".into());
    }
    drop(input);
    let output = child
        .wait_with_output()
        .map_err(|_| "No se pudo preparar la conexión WebDAV.")?;
    if !output.status.success() {
        return Err("No se pudo preparar la conexión WebDAV.".into());
    }
    let obscured = String::from_utf8(output.stdout).map_err(|_| "Credencial no válida.")?;
    Ok([
        ("RCLONE_CONFIG_MEGALODON_CLOUD_TYPE".into(), "webdav".into()),
        ("RCLONE_CONFIG_MEGALODON_CLOUD_URL".into(), url.into()),
        (
            "RCLONE_CONFIG_MEGALODON_CLOUD_VENDOR".into(),
            "other".into(),
        ),
        ("RCLONE_CONFIG_MEGALODON_CLOUD_USER".into(), user.into()),
        (
            "RCLONE_CONFIG_MEGALODON_CLOUD_PASS".into(),
            obscured.trim().into(),
        ),
    ]
    .into())
}

pub fn connect_webdav(
    name: &str,
    url: &str,
    user: &str,
    password: &str,
    cancel: &AtomicBool,
) -> Result<Source, String> {
    let mut url = data::server_url(url)?;
    if !url.ends_with('/') {
        url.push('/');
    }
    if user.contains(['\r', '\n', '\0']) || password.contains(['\r', '\n', '\0']) {
        return Err("El usuario o la contraseña contienen caracteres no válidos.".into());
    }
    let exe = prepare()?;
    let environment = webdav_environment(&exe, &url, user, password)?;
    let mut probe = command(&exe);
    probe
        .args([
            "lsjson",
            WEBDAV_REMOTE,
            "--max-depth",
            "1",
            "--dirs-only",
            "--config",
            "NUL",
        ])
        .envs(environment);
    output(probe, cancel, Duration::from_secs(30)).map_err(
        |_| "No se pudo abrir WebDAV. Revisa la dirección de la carpeta y sus credenciales.",
    )?;
    Ok(Source {
        id: format!("webdav-{}", sources::stable_id(&format!("{url}|{user}"))),
        name: if name.trim().is_empty() {
            "Mi servidor".into()
        } else {
            name.trim().into()
        },
        kind: Kind::WebDav {
            url,
            user: user.into(),
            credential: data::protect(password.as_bytes(), false)?,
        },
    })
}
/// Copies one local file to `relative` inside the connection (Drive or WebDAV),
/// creating folders as needed. Used only for the library's image folder.
pub fn upload(
    exe: &Path,
    config: &Path,
    remote: &str,
    relative: &str,
    local: &Path,
    cancel: &AtomicBool,
    environment: &[(String, String)],
) -> Result<(), String> {
    if relative
        .split('/')
        .any(|s| s.is_empty() || s == "." || s == "..")
    {
        return Err("Ruta de imagen no válida.".into());
    }
    let destination = if remote.ends_with(':') || remote.ends_with('/') {
        format!("{remote}{relative}")
    } else {
        format!("{remote}/{relative}")
    };
    let mut cmd = command(exe);
    cmd.envs(environment.iter().cloned());
    cmd.arg("copyto")
        .arg(local)
        .arg(destination)
        .arg("--config")
        .arg(config);
    output(cmd, cancel, Duration::from_secs(120)).map(|_| ()).map_err(|_| {
        "No se pudo guardar la imagen en la conexión. Si es de solo lectura, permite guardar imágenes en Ajustes → Imágenes y fichas.".into()
    })
}
/// Connections created by Megalodon start read-only. This asks Google, in the
/// browser, for permission to also save files (used for the image folder).
pub fn allow_writes(config: &Path, cancel: &AtomicBool) -> Result<(), String> {
    let owned = config.starts_with(data::data_dir().join("drive"));
    if !owned {
        return Err(
            "Esta conexión usa tu propia configuración de rclone; sus permisos se gestionan allí."
                .into(),
        );
    }
    let mut cmd = command(&prepare()?);
    cmd.args([
        "config",
        "create",
        "megalodon",
        "drive",
        "scope",
        "drive",
        "config_is_local",
        "true",
        "--no-output",
        "--config",
    ])
    .arg(config);
    output(cmd, cancel, Duration::from_secs(300)).map(|_| ())
}
fn random_id() -> Result<String, String> {
    let mut bytes = [0u8; 24];
    getrandom::fill(&mut bytes).map_err(|_| "Windows no pudo crear una sesión segura.")?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
pub struct Stream {
    child: Child,
    base: String,
    pub header: String,
}
impl Stream {
    pub fn start(exe: &Path, config: &Path, remote: &str) -> Result<Self, String> {
        Self::start_with_env(exe, config, remote, &[])
    }
    pub fn start_with_env(
        exe: &Path,
        config: &Path,
        remote: &str,
        environment: &[(String, String)],
    ) -> Result<Self, String> {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")
            .map_err(|_| "No se pudo preparar el reproductor de Drive.")?;
        let port = listener
            .local_addr()
            .map_err(|_| "No se pudo preparar Drive.")?
            .port();
        drop(listener);
        let password = random_id()?;
        let base = format!("http://127.0.0.1:{port}/");
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(1))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| "No se pudo preparar Drive.")?;
        let request = client
            .get(&base)
            .basic_auth("megalodon", Some(&password))
            .build()
            .map_err(|_| "No se pudo preparar Drive.")?;
        let header = format!(
            "Authorization: {}",
            request.headers()[reqwest::header::AUTHORIZATION]
                .to_str()
                .map_err(|_| "No se pudo preparar Drive.")?
        );
        let child = command(exe)
            .args([
                "serve",
                "http",
                remote,
                "--read-only",
                "--addr",
                &format!("127.0.0.1:{port}"),
                "--config",
            ])
            .arg(config)
            .envs(environment.iter().cloned())
            .env("RCLONE_USER", "megalodon")
            .env("RCLONE_PASS", &password)
            .stdout(Stdio::null())
            .spawn()
            .map_err(|_| "No se pudo iniciar Drive.")?;
        let mut stream = Self {
            child,
            base,
            header,
        };
        for _ in 0..40 {
            if stream
                .child
                .try_wait()
                .map_err(|_| "Se interrumpió Drive.")?
                .is_some()
            {
                return Err("No se pudo abrir la conexión de Drive.".into());
            }
            if client
                .get(&stream.base)
                .basic_auth("megalodon", Some(&password))
                .send()
                .is_ok_and(|r| r.status().is_success())
            {
                return Ok(stream);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Err("Drive no terminó de prepararse. Vuelve a intentarlo.".into())
    }
    pub fn url(&self, path: &str) -> Result<String, String> {
        let mut url = reqwest::Url::parse(&self.base).map_err(|_| "Ruta de Drive no válida.")?;
        {
            let mut segments = url
                .path_segments_mut()
                .map_err(|_| "Ruta de Drive no válida.")?;
            segments.clear();
            for segment in path.replace('\\', "/").split('/') {
                if matches!(segment, "." | "..") {
                    return Err("Ruta de Drive no válida.".into());
                }
                segments.push(segment);
            }
        }
        Ok(url.into())
    }
}
impl Drop for Stream {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "integration: requires scripts/setup-rclone.ps1"]
    fn webdav_source_authenticates_indexes_and_streams_without_saved_plaintext() {
        let exe = Path::new(env!("CARGO_MANIFEST_DIR")).join(".tools/rclone/rclone.exe");
        let root =
            std::env::temp_dir().join(format!("megalodon-webdav-test-{}", random_id().unwrap()));
        std::fs::create_dir_all(root.join("Serie/Season 01")).unwrap();
        let relative = "Serie/Season 01/Serie.S01E02.á #.mp4";
        let bytes: Vec<u8> = (0..=255).collect();
        std::fs::write(root.join(relative), &bytes).unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let password = "fixture-only-secret";
        let child = command(&exe)
            .args(["serve", "webdav"])
            .arg(&root)
            .args([
                "--addr",
                &addr.to_string(),
                "--read-only",
                "--config",
                "NUL",
            ])
            .env("RCLONE_USER", "fixture")
            .env("RCLONE_PASS", password)
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        // The same owned-child guard also cleans up a test failure.
        let server = Stream {
            child,
            base: format!("http://{addr}/"),
            header: String::new(),
        };
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap();
        for _ in 0..40 {
            if client
                .get(&server.base)
                .basic_auth("fixture", Some(password))
                .send()
                .is_ok()
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let cancel = AtomicBool::new(false);
        assert!(
            connect_webdav(
                "Prueba",
                &format!("{}?secret=x", server.base),
                "fixture",
                password,
                &cancel
            )
            .is_err()
        );
        let source = connect_webdav("Prueba", &server.base, "fixture", password, &cancel).unwrap();
        let saved = serde_json::to_string(&source).unwrap();
        assert!(!saved.contains(password));
        assert!(source.config().unwrap().is_none());
        let files =
            super::super::files::Files::load(source, std::sync::Arc::new(AtomicBool::new(false)))
                .unwrap();
        let item = files.items.iter().find(|i| i.kind == "Episode").unwrap();
        assert_eq!(
            (item.parent_index_number, item.index_number),
            (Some(1), Some(2))
        );
        let playback = files.playback(item.clone(), 0.0).unwrap();
        assert!(!playback.url.contains(password));
        let response = client
            .get(&playback.url)
            .header(
                "Authorization",
                playback.headers.strip_prefix("Authorization: ").unwrap(),
            )
            .header("Range", "bytes=32-63")
            .send()
            .unwrap();
        assert_eq!(response.status(), 206);
        assert_eq!(response.bytes().unwrap().as_ref(), &bytes[32..64]);
        drop(files);
        assert!(client.get(&playback.url).send().is_err());
        assert!(
            client
                .get(&server.base)
                .basic_auth("fixture", Some(password))
                .send()
                .is_ok()
        );
        assert_eq!(std::fs::read(root.join(relative)).unwrap(), bytes);
        drop(server);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "integration: requires scripts/setup-rclone.ps1"]
    fn bundled_drive_transport_lists_streams_ranges_and_closes_only_its_child() {
        let exe = Path::new(env!("CARGO_MANIFEST_DIR")).join(".tools/rclone/rclone.exe");
        assert!(exe.is_file(), "Prepare the bundled rclone first");
        let root =
            std::env::temp_dir().join(format!("megalodon-drive-test-{}", random_id().unwrap()));
        let media = root.join("library");
        std::fs::create_dir_all(&media).unwrap();
        let filename = "Película #1.mp4";
        let bytes: Vec<u8> = (0..=255).collect();
        std::fs::write(media.join(filename), &bytes).unwrap();
        let config = root.join("test.conf");
        std::fs::write(
            &config,
            format!("[fixture]\ntype = alias\nremote = {}\n", media.display()),
        )
        .unwrap();
        let cancel = AtomicBool::new(false);
        assert_eq!(
            list(&exe, &config, "fixture:", &cancel).unwrap(),
            [filename]
        );
        let stream = Stream::start(&exe, &config, "fixture:").unwrap();
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap();
        let url = stream.url(filename).unwrap();
        assert!(url.contains("%23"));
        assert!(stream.url("../test.conf").is_err());
        assert_eq!(client.get(&url).send().unwrap().status(), 401);
        let response = client
            .get(&url)
            .header(
                "Authorization",
                stream.header.strip_prefix("Authorization: ").unwrap(),
            )
            .header("Range", "bytes=32-63")
            .send()
            .unwrap();
        assert_eq!(response.status(), 206);
        assert_eq!(response.bytes().unwrap().as_ref(), &bytes[32..64]);
        drop(stream);
        assert!(client.get(url).send().is_err());
        // The fixture and configuration survive ending playback; only the owned helper closes.
        assert_eq!(std::fs::read(media.join(filename)).unwrap(), bytes);
        assert!(config.is_file());
        std::fs::remove_dir_all(root).unwrap();
    }
}
