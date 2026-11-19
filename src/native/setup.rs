// A windowed app in every build: Megalodon never opens a console.
#![windows_subsystem = "windows"]
#[allow(dead_code)]
mod updates;
use eframe::egui;
use std::{
    fs,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
};

const MAGIC: &[u8; 16] = b"MEGALODON-RUST01";
const REGISTRY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\MegalodonRust";

fn install_root() -> Result<PathBuf, String> {
    Ok(PathBuf::from(
        std::env::var_os("LOCALAPPDATA")
            .ok_or("No se encontró la carpeta de aplicaciones de Windows.")?,
    )
    .join("Programs/Megalodon"))
}
fn safe_name(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && !path.to_string_lossy().contains(':')
        && path
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
}
fn payload() -> Result<(Vec<u8>, u64), String> {
    let mut file = fs::File::open(std::env::current_exe().map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let size = file.metadata().map_err(|e| e.to_string())?.len();
    if size < 24 {
        return Err("Instalador incompleto.".into());
    }
    file.seek(SeekFrom::End(-24)).map_err(|e| e.to_string())?;
    let mut footer = [0u8; 24];
    file.read_exact(&mut footer).map_err(|e| e.to_string())?;
    if &footer[8..] != MAGIC {
        return Err("Este ejecutable no contiene el paquete de instalación.".into());
    }
    let len = u64::from_le_bytes(footer[..8].try_into().unwrap());
    if len > size - 24 || len > 1_000_000_000 {
        return Err("El paquete no tiene un tamaño válido.".into());
    }
    let offset = size - 24 - len;
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| e.to_string())?;
    let mut bytes = vec![0; len as usize];
    file.read_exact(&mut bytes).map_err(|e| e.to_string())?;
    Ok((bytes, offset))
}
fn shortcuts() -> Vec<PathBuf> {
    let mut paths = vec![];
    if let Some(roaming) = std::env::var_os("APPDATA") {
        paths.push(
            PathBuf::from(roaming).join("Microsoft/Windows/Start Menu/Programs/Megalodon.lnk"),
        );
    }
    // Respect a redirected Desktop (including OneDrive), without changing the user's shell settings.
    let shell = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
        .open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Shell Folders")
        .ok();
    if let Some(desktop) = shell.and_then(|k| k.get_value::<String, _>("Desktop").ok()) {
        paths.push(PathBuf::from(desktop).join("Megalodon.lnk"));
    }
    paths
}
fn install(progress: Arc<AtomicUsize>, register: bool) -> Result<PathBuf, String> {
    let (bytes, stub_size) = payload()?;
    let root = install_root()?;
    let parent = root.parent().unwrap();
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let allowed = parent.canonicalize().map_err(|e| e.to_string())?;
    if root.exists() && root.canonicalize().map_err(|e| e.to_string())? != allowed.join("Megalodon")
    {
        return Err("La carpeta de instalación está redirigida fuera del destino esperado.".into());
    }
    if root.exists() && !root.join("installation.json").is_file() {
        return Err("La carpeta de destino ya existe y no pertenece a esta instalación.".into());
    }
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let legacy = parent.join("Megalodon Rust");
    let migrate = !root.exists()
        && legacy.join("installation.json").is_file()
        && legacy
            .canonicalize()
            .is_ok_and(|p| p == allowed.join("Megalodon Rust"));
    let previous_root = if migrate { &legacy } else { &root };
    let staging = parent.join(format!("Megalodon.install-{unique}"));
    fs::create_dir(&staging).map_err(|e| e.to_string())?;
    if staging.canonicalize().map_err(|e| e.to_string())?
        != allowed.join(staging.file_name().unwrap())
    {
        return Err("La carpeta temporal está fuera del destino esperado.".into());
    }
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|_| "El paquete de instalación está dañado.")?;
    let total = archive.len();
    let mut files = vec![];
    let mut expanded = 0u64;
    for index in 0..total {
        let mut entry = archive
            .by_index(index)
            .map_err(|_| "No se pudo abrir un archivo del paquete.")?;
        let name = entry
            .enclosed_name()
            .filter(|p| safe_name(p))
            .ok_or("El paquete contiene una ruta no válida.")?;
        if entry.is_dir() {
            fs::create_dir_all(staging.join(&name)).map_err(|e| e.to_string())?;
            continue;
        }
        expanded = expanded
            .checked_add(entry.size())
            .ok_or("Paquete demasiado grande.")?;
        if expanded > 1_000_000_000 {
            return Err("Paquete demasiado grande.".into());
        }
        let target = staging.join(&name);
        fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
        let mut output = fs::File::create(&target).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut output)
            .map_err(|_| "Un archivo del instalador está dañado o no se puede escribir.")?;
        output.flush().map_err(|e| e.to_string())?;
        files.push(name.to_string_lossy().into_owned());
        progress.store((index + 1) * 85 / total.max(1), Ordering::Relaxed);
    }
    if !staging.join("Megalodon.exe").is_file() || !staging.join("libmpv-2.dll").is_file() {
        return Err("Faltan archivos esenciales en el paquete.".into());
    }
    let mut source = fs::File::open(std::env::current_exe().unwrap()).map_err(|e| e.to_string())?;
    let mut uninstaller =
        fs::File::create(staging.join("Uninstall.exe")).map_err(|e| e.to_string())?;
    std::io::copy(&mut (&mut source).take(stub_size), &mut uninstaller)
        .map_err(|e| e.to_string())?;
    drop(uninstaller);
    files.push("Uninstall.exe".into());
    files.push("installation.json".into());
    fs::write(
        staging.join("installation.json"),
        serde_json::to_vec_pretty(&files).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    let previous = parent.join(format!("Megalodon.previous-{unique}"));
    if previous_root.exists() {
        fs::rename(previous_root, &previous)
            .map_err(|_| "Cierra Megalodon antes de actualizar la aplicación.")?;
    }
    if let Err(error) = fs::rename(&staging, &root) {
        if previous.exists() {
            let _ = fs::rename(&previous, previous_root);
        }
        return Err(error.to_string());
    }
    if previous.exists() {
        // Only remove our own previous package, after resolving its path inside the fixed Programs directory.
        let allowed = parent.canonicalize().map_err(|e| e.to_string())?;
        let resolved = previous.canonicalize().map_err(|e| e.to_string())?;
        if resolved.starts_with(&allowed) && resolved != allowed {
            // Remove only recorded package files; preserve any user-added files in the backup.
            if let Ok(bytes) = fs::read(resolved.join("installation.json"))
                && let Ok(files) = serde_json::from_slice::<Vec<String>>(&bytes)
            {
                let mut directories = std::collections::BTreeSet::new();
                for name in files {
                    let name = Path::new(&name);
                    if !safe_name(name) {
                        continue;
                    }
                    let file = resolved.join(name);
                    if file
                        .canonicalize()
                        .is_ok_and(|path| path.starts_with(&resolved))
                    {
                        let _ = fs::remove_file(&file);
                    }
                    let mut parent = name.parent();
                    while let Some(dir) = parent {
                        if !dir.as_os_str().is_empty() {
                            directories.insert(resolved.join(dir));
                        }
                        parent = dir.parent();
                    }
                }
                for directory in directories.into_iter().rev() {
                    let _ = fs::remove_dir(directory);
                }
                let _ = fs::remove_dir(resolved);
            }
        }
    }
    if register {
        for shortcut in shortcuts() {
            if migrate {
                let old = shortcut.with_file_name("Megalodon Rust.lnk");
                // These shortcuts were registered by the migrated package, not the old Electron app.
                let _ = fs::remove_file(old);
            }
            if let Some(parent) = shortcut.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut link =
                mslnk::ShellLink::new(root.join("Megalodon.exe")).map_err(|e| e.to_string())?;
            link.set_working_dir(Some(root.to_string_lossy().into_owned()));
            link.set_icon_location(Some(
                root.join("Megalodon.exe").to_string_lossy().into_owned(),
            ));
            link.create_lnk(shortcut).map_err(|e| e.to_string())?;
        }
        refresh_icons();
        let (key, _) = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
            .create_subkey(REGISTRY)
            .map_err(|e| e.to_string())?;
        for (name, value) in [
            ("DisplayName", "Megalodon".to_owned()),
            ("DisplayVersion", env!("CARGO_PKG_VERSION").to_owned()),
            ("Publisher", "Megalodon".to_owned()),
            ("InstallLocation", root.to_string_lossy().into_owned()),
            (
                "DisplayIcon",
                root.join("Megalodon.exe").to_string_lossy().into_owned(),
            ),
            (
                "UninstallString",
                format!("\"{}\" --uninstall", root.join("Uninstall.exe").display()),
            ),
        ] {
            key.set_value(name, &value).map_err(|e| e.to_string())?;
        }
        key.set_value("NoModify", &1u32)
            .map_err(|e| e.to_string())?;
        key.set_value("NoRepair", &1u32)
            .map_err(|e| e.to_string())?;
        key.set_value("EstimatedSize", &((expanded / 1024) as u32))
            .map_err(|e| e.to_string())?;
    }
    progress.store(100, Ordering::Relaxed);
    Ok(root)
}
fn install_latest(progress: Arc<AtomicUsize>) -> Result<PathBuf, String> {
    if !std::env::args().any(|a| a == "--skip-update-check")
        && let Some(repo) = option_env!("MEGALODON_UPDATE_REPO")
        && let Ok(Some(release)) = updates::check(repo)
    {
        let folder = PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default())
            .join("MegalodonRust/updates");
        let path = updates::download(&release, &folder, |done, total| {
            if let Some(percent) = done.saturating_mul(80).checked_div(total) {
                progress.store(percent as usize, Ordering::Relaxed);
            }
        })?;
        use std::os::windows::process::CommandExt;
        let status = std::process::Command::new(path)
            .args(["--install", "--skip-update-check"])
            .creation_flags(0x08000000)
            .status()
            .map_err(|_| "No se pudo abrir la versión más reciente del instalador.")?;
        if !status.success() {
            return Err(
                "No se pudo instalar la actualización. Tu instalación anterior se conserva.".into(),
            );
        }
        progress.store(100, Ordering::Relaxed);
        return install_root();
    }
    install(progress, true)
}
fn wait_for_app(pid: u32) -> Result<(), String> {
    if pid == 0 || pid == std::process::id() {
        return Err("No se pudo identificar la aplicación que se está actualizando.".into());
    }
    use windows_sys::Win32::{Foundation::*, System::Threading::*};
    unsafe {
        let process = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        if process.is_null() {
            return Ok(());
        }
        let result = WaitForSingleObject(process, 90_000);
        CloseHandle(process);
        if result != WAIT_OBJECT_0 {
            return Err("Megalodon sigue cerrando sus servicios. Espera a que termine y vuelve a actualizar.".into());
        }
    }
    Ok(())
}
fn message(title: &str, text: &str, confirm: bool) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let title: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
    let text: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            if confirm {
                MB_YESNO | MB_ICONQUESTION
            } else {
                MB_OK | MB_ICONINFORMATION
            },
        ) == IDYES
    }
}
fn remove_installed(parent_pid: u32) -> Result<(), String> {
    use windows_sys::Win32::{Foundation::CloseHandle, System::Threading::*};
    unsafe {
        let process = OpenProcess(PROCESS_SYNCHRONIZE, 0, parent_pid);
        if !process.is_null() {
            WaitForSingleObject(process, 30_000);
            CloseHandle(process);
        }
    }
    let root = install_root()?;
    let allowed = root
        .parent()
        .unwrap()
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let resolved = root.canonicalize().map_err(|e| e.to_string())?;
    if resolved != allowed.join("Megalodon") {
        return Err("La carpeta de instalación no es válida.".into());
    }
    let files: Vec<String> = serde_json::from_slice(
        &fs::read(root.join("installation.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|_| "No se encontró el registro de instalación.")?;
    let mut directories = std::collections::BTreeSet::new();
    for name in files {
        let name = Path::new(&name);
        if !safe_name(name) {
            return Err("El registro contiene una ruta no válida.".into());
        }
        let file = root.join(name);
        if file.exists() {
            let actual = file.canonicalize().map_err(|e| e.to_string())?;
            if !actual.starts_with(&resolved) {
                return Err("La instalación contiene un enlace fuera de su carpeta.".into());
            }
            fs::remove_file(file).map_err(|_| "Cierra Megalodon antes de desinstalarlo.")?;
        }
        let mut parent = name.parent();
        while let Some(dir) = parent {
            if !dir.as_os_str().is_empty() {
                directories.insert(root.join(dir));
            }
            parent = dir.parent();
        }
    }
    for dir in directories.into_iter().rev() {
        let _ = fs::remove_dir(dir);
    }
    let _ = fs::remove_dir(root);
    for shortcut in shortcuts() {
        let _ = fs::remove_file(shortcut);
    }
    let _ = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).delete_subkey_all(REGISTRY);
    Ok(())
}

/// Windows caches shortcut icons by path; after an update the old artwork can
/// linger. Tell the shell icons changed and ask it to rebuild its cache.
fn refresh_icons() {
    use std::os::windows::process::CommandExt;
    unsafe {
        windows_sys::Win32::UI::Shell::SHChangeNotify(
            windows_sys::Win32::UI::Shell::SHCNE_ASSOCCHANGED as i32,
            windows_sys::Win32::UI::Shell::SHCNF_IDLIST,
            std::ptr::null(),
            std::ptr::null(),
        );
    }
    if let Some(system) = std::env::var_os("SystemRoot") {
        let _ = std::process::Command::new(PathBuf::from(system).join("System32/ie4uinit.exe"))
            .arg("-show")
            .creation_flags(0x08000000)
            .status();
    }
}

struct Installer {
    progress: Arc<AtomicUsize>,
    receiver: Option<mpsc::Receiver<Result<PathBuf, String>>>,
    result: Option<Result<PathBuf, String>>,
    logo: egui::TextureHandle,
}
impl eframe::App for Installer {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(receiver) = &self.receiver
            && let Ok(result) = receiver.try_recv()
        {
            self.result = Some(result);
            self.receiver = None;
        }
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(10, 11, 14))
                    .inner_margin(32),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add(egui::Image::new(&self.logo).fit_to_exact_size(egui::vec2(48.0, 48.0)));
                    ui.heading("Megalodon");
                });
                ui.add_space(20.0);
                if let Some(result) = &self.result {
                    match result {
                        Ok(root) => {
                            ui.heading("Listo para abrir");
                            ui.label("Megalodon se ha instalado para tu cuenta de Windows.");
                            ui.add_space(24.0);
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new("Abrir Megalodon")
                                            .color(egui::Color32::BLACK),
                                    )
                                    .fill(egui::Color32::WHITE)
                                    .corner_radius(10),
                                )
                                .clicked()
                            {
                                let _ = std::process::Command::new(root.join("Megalodon.exe"))
                                    .current_dir(root)
                                    .spawn();
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        }
                        Err(error) => {
                            ui.label(error);
                            ui.add_space(16.0);
                            if ui.button("Cerrar").clicked() {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        }
                    }
                } else if self.receiver.is_some() {
                    ui.heading("Instalando…");
                    ui.add_space(20.0);
                    ui.add(
                        egui::ProgressBar::new(
                            self.progress.load(Ordering::Relaxed) as f32 / 100.0,
                        )
                        .show_percentage(),
                    );
                    ctx.request_repaint_after(std::time::Duration::from_millis(100));
                } else {
                    ui.heading("Tu cine, en una aplicación");
                    ui.label("Cliente nativo en Rust con reproducción integrada.");
                    ui.add_space(20.0);
                    ui.label(
                        egui::RichText::new(
                            install_root()
                                .unwrap_or_default()
                                .display()
                                .to_string()
                                .replace('/', "\\"),
                        )
                        .small()
                        .color(egui::Color32::from_gray(170)),
                    );
                    ui.add_space(22.0);
                    ui.label("Incluye el reproductor y sus dependencias, y crea los accesos directos.");
                    ui.label(egui::RichText::new("Tus bibliotecas (carpetas, Drive, WebDAV o Jellyfin) se conectan después, desde Megalodon.").small().color(egui::Color32::from_gray(170)));
                    ui.add_space(20.0);
                    if ui
                        .add_sized(
                            [180.0, 44.0],
                            egui::Button::new(
                                egui::RichText::new("Instalar Megalodon")
                                    .color(egui::Color32::BLACK),
                            )
                            .fill(egui::Color32::WHITE)
                            .corner_radius(10),
                        )
                        .clicked()
                    {
                        let (tx, rx) = mpsc::channel();
                        self.receiver = Some(rx);
                        let progress = self.progress.clone();
                        std::thread::spawn(move || {
                            let _ = tx.send(install_latest(progress));
                        });
                    }
                }
            });
    }
}
fn main() -> eframe::Result {
    let args: Vec<_> = std::env::args().collect();
    if args.iter().any(|a| a == "--update") {
        let pid = args
            .iter()
            .position(|a| a == "--wait-pid")
            .and_then(|i| args.get(i + 1))
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let result = wait_for_app(pid).and_then(|_| install(Arc::new(AtomicUsize::new(0)), true));
        match result {
            Ok(root) => {
                use std::os::windows::process::CommandExt;
                let _ = std::process::Command::new(root.join("Megalodon.exe"))
                    .current_dir(&root)
                    .creation_flags(0x08000000)
                    .spawn();
            }
            Err(error) => {
                message("No se pudo actualizar Megalodon", &error, false);
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    if let Some(index) = args.iter().position(|a| a == "--remove-installed") {
        let result = remove_installed(
            args.get(index + 1)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
        );
        message(
            "Megalodon",
            result
                .err()
                .as_deref()
                .unwrap_or("Megalodon se ha desinstalado. Tu configuración personal se conserva."),
            false,
        );
        return Ok(());
    }
    if args.iter().any(|a| a == "--uninstall") {
        if message(
            "Desinstalar Megalodon",
            "¿Quieres desinstalar Megalodon? Tu biblioteca y configuración personal se conservarán.",
            true,
        ) {
            let helper = std::env::temp_dir().join(format!(
                "MegalodonRust-Uninstall-{}.exe",
                std::process::id()
            ));
            if std::env::current_exe()
                .and_then(|exe| fs::copy(exe, &helper))
                .is_ok()
            {
                use std::os::windows::process::CommandExt;
                let _ = std::process::Command::new(helper)
                    .args(["--remove-installed", &std::process::id().to_string()])
                    .creation_flags(0x08000000)
                    .spawn();
            }
        }
        return Ok(());
    }
    if args.iter().any(|a| a == "--install") {
        match install_latest(Arc::new(AtomicUsize::new(0))) {
            Ok(root) => println!("Instalado: {}", root.display()),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    if args.iter().any(|a| a == "--verify") {
        let result = payload().and_then(|(bytes, _)| {
            let mut archive =
                zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
            for i in 0..archive.len() {
                let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
                if !file.enclosed_name().is_some_and(|p| safe_name(&p)) {
                    return Err("Ruta no válida".into());
                }
                std::io::copy(&mut file, &mut std::io::sink()).map_err(|e| e.to_string())?;
            }
            Ok(archive.len())
        });
        match result {
            Ok(count) => println!("Paquete verificado: {count} archivos"),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    let icon = image::load_from_memory(include_bytes!("../../static/brand/mark.png"))
        .expect("Bundled icon")
        .into_rgba8();
    let icon = egui::IconData {
        width: icon.width(),
        height: icon.height(),
        rgba: icon.into_raw(),
    };
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Glow,
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([560.0, 400.0])
            .with_icon(icon)
            .with_resizable(false)
            .with_title("Instalar Megalodon"),
        ..Default::default()
    };
    eframe::run_native(
        "Instalar Megalodon",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            let mut fonts = egui::FontDefinitions::default();
            if let Ok(bytes) = std::fs::read("C:/Windows/Fonts/segoeui.ttf") {
                fonts.font_data.insert(
                    "Segoe UI".into(),
                    Arc::new(egui::FontData::from_owned(bytes)),
                );
                fonts
                    .families
                    .get_mut(&egui::FontFamily::Proportional)
                    .unwrap()
                    .insert(0, "Segoe UI".into());
            }
            cc.egui_ctx.set_fonts(fonts);
            cc.egui_ctx.style_mut(|style| {
                style.visuals.override_text_color = Some(egui::Color32::from_gray(245));
                style
                    .text_styles
                    .insert(egui::TextStyle::Body, egui::FontId::proportional(15.0));
                style
                    .text_styles
                    .insert(egui::TextStyle::Button, egui::FontId::proportional(15.0));
                style
                    .text_styles
                    .insert(egui::TextStyle::Heading, egui::FontId::proportional(25.0));
                style.spacing.button_padding = egui::vec2(20.0, 12.0);
            });
            let pixels = image::load_from_memory(include_bytes!("../../static/brand/mark.png"))?
                .into_rgba8();
            let logo = cc.egui_ctx.load_texture(
                "Megalodon",
                egui::ColorImage::from_rgba_unmultiplied(
                    [pixels.width() as usize, pixels.height() as usize],
                    &pixels,
                ),
                egui::TextureOptions::LINEAR,
            );
            Ok(Box::new(Installer {
                progress: Arc::new(AtomicUsize::new(0)),
                receiver: None,
                result: None,
                logo,
            }))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn package_paths_stay_in_installation() {
        assert!(safe_name(Path::new("static/brand/mark.png")));
        for path in [
            "../a",
            "C:/Windows/a",
            "/Windows/a",
            "static/../../a",
            "file:stream",
            "",
        ] {
            assert!(!safe_name(Path::new(path)));
        }
    }
}
