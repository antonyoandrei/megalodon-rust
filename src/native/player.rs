//! Small, dynamically loaded libmpv boundary. Video renders directly into the application's GL target.
use eframe::{
    egui,
    glow::{self, HasContext},
};
use std::{
    ffi::{CStr, CString, c_char, c_int, c_void},
    path::Path,
    ptr,
};

type Handle = *mut c_void;
#[repr(C)]
struct Param {
    kind: c_int,
    data: *mut c_void,
}
#[repr(C)]
struct GlInit {
    get_proc: unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void,
    context: *mut c_void,
}
#[repr(C)]
struct Fbo {
    id: c_int,
    width: c_int,
    height: c_int,
    format: c_int,
}
#[repr(C)]
struct Event {
    id: c_int,
    error: c_int,
    userdata: u64,
    data: *mut c_void,
}
#[repr(C)]
struct EndFile {
    reason: c_int,
    error: c_int,
}

struct Api {
    _library: libloading::Library,
    create: unsafe extern "C" fn() -> Handle,
    initialize: unsafe extern "C" fn(Handle) -> c_int,
    option: unsafe extern "C" fn(Handle, *const c_char, *const c_char) -> c_int,
    set: unsafe extern "C" fn(Handle, *const c_char, *const c_char) -> c_int,
    get: unsafe extern "C" fn(Handle, *const c_char) -> *mut c_char,
    free: unsafe extern "C" fn(*mut c_void),
    command: unsafe extern "C" fn(Handle, *const *const c_char) -> c_int,
    wait: unsafe extern "C" fn(Handle, f64) -> *const Event,
    destroy: unsafe extern "C" fn(Handle),
    render_create: unsafe extern "C" fn(*mut Handle, Handle, *mut Param) -> c_int,
    render_callback:
        unsafe extern "C" fn(Handle, Option<unsafe extern "C" fn(*mut c_void)>, *mut c_void),
    render: unsafe extern "C" fn(Handle, *mut Param) -> c_int,
    render_free: unsafe extern "C" fn(Handle),
}

impl Api {
    fn load(path: &Path) -> Result<Self, String> {
        // Resolve every symbol before creating a handle, and keep the DLL alive longer than all handles.
        unsafe {
            let dll = libloading::Library::new(path).map_err(
                |_| "Falta libmpv-2.dll. Reinstala Megalodon o usa el paquete portátil completo.",
            )?;
            macro_rules! symbol {
                ($name:literal) => {
                    *dll.get(concat!($name, "\0").as_bytes())
                        .map_err(|_| "La versión del motor de vídeo no es compatible.")?
                };
            }
            Ok(Self {
                create: symbol!("mpv_create"),
                initialize: symbol!("mpv_initialize"),
                option: symbol!("mpv_set_option_string"),
                set: symbol!("mpv_set_property_string"),
                get: symbol!("mpv_get_property_string"),
                free: symbol!("mpv_free"),
                command: symbol!("mpv_command"),
                wait: symbol!("mpv_wait_event"),
                destroy: symbol!("mpv_terminate_destroy"),
                render_create: symbol!("mpv_render_context_create"),
                render_callback: symbol!("mpv_render_context_set_update_callback"),
                render: symbol!("mpv_render_context_render"),
                render_free: symbol!("mpv_render_context_free"),
                _library: dll,
            })
        }
    }
}

#[derive(Clone)]
pub struct Track {
    pub id: String,
    pub title: String,
    pub language: String,
    pub codec: String,
    pub channels: usize,
    pub selected: bool,
}

pub struct Player {
    api: Api,
    handle: Handle,
    render: Handle,
    repaint: Box<egui::Context>,
    pub loaded: bool,
    pub error: Option<String>,
    pub ended: bool,
}

// libmpv's client API is thread-safe. A Mutex protects access; GL operations are called only by eframe's render thread.
unsafe impl Send for Player {}

unsafe extern "C" fn request_frame(data: *mut c_void) {
    if !data.is_null() {
        unsafe {
            (&*data.cast::<egui::Context>()).request_repaint();
        }
    }
}
unsafe extern "C" fn resolve_gl(context: *mut c_void, name: *const c_char) -> *mut c_void {
    let resolver = unsafe { &*context.cast::<&dyn Fn(&CStr) -> *const c_void>() };
    resolver(unsafe { CStr::from_ptr(name) }).cast_mut()
}

impl Player {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Result<Self, String> {
        let exe_dir = std::env::current_exe()
            .map_err(|_| "No se encuentra la aplicación.")?
            .parent()
            .unwrap()
            .to_owned();
        let dll = [
            exe_dir.join("libmpv-2.dll"),
            Path::new(".tools/mpv/libmpv-2.dll").to_owned(),
        ]
        .into_iter()
        .find(|p| p.is_file())
        .ok_or("No se encuentra el motor de reproducción libmpv-2.dll.")?;
        let api = Api::load(&dll)?;
        let handle = unsafe { (api.create)() };
        if handle.is_null() {
            return Err("No se pudo iniciar el reproductor.".into());
        }
        let mut player = Self {
            api,
            handle,
            render: ptr::null_mut(),
            repaint: Box::new(cc.egui_ctx.clone()),
            loaded: false,
            error: None,
            ended: false,
        };
        for (key, value) in [
            ("vo", "libmpv"),
            ("hwdec", "auto-safe"),
            ("config", "no"),
            ("terminal", "no"),
            ("osc", "no"),
            ("input-default-bindings", "no"),
            ("input-vo-keyboard", "no"),
            ("keep-open", "yes"),
            ("idle", "yes"),
            ("audio-display", "no"),
            ("background-color", "#000000"),
            ("demuxer-max-bytes", "134217728"),
            ("demuxer-max-back-bytes", "33554432"),
            ("cache", "yes"),
            ("network-timeout", "20"),
        ] {
            let key = CString::new(key).unwrap();
            let value = CString::new(value).unwrap();
            unsafe {
                (player.api.option)(handle, key.as_ptr(), value.as_ptr());
            }
        }
        if unsafe { (player.api.initialize)(handle) } < 0 {
            return Err("No se pudo inicializar el motor de vídeo.".into());
        }
        let resolver = cc
            .get_proc_address
            .ok_or("La tarjeta gráfica no ofrece un contexto OpenGL compatible.")?;
        let mut init = GlInit {
            get_proc: resolve_gl,
            context: (&resolver as *const &dyn Fn(&CStr) -> *const c_void)
                .cast_mut()
                .cast(),
        };
        let mut params = [
            Param {
                kind: 1,
                data: c"opengl".as_ptr().cast_mut().cast(),
            },
            Param {
                kind: 2,
                data: (&mut init as *mut GlInit).cast(),
            },
            Param {
                kind: 0,
                data: ptr::null_mut(),
            },
        ];
        if unsafe { (player.api.render_create)(&mut player.render, handle, params.as_mut_ptr()) }
            < 0
        {
            return Err("No se pudo preparar la salida de vídeo acelerada.".into());
        }
        unsafe {
            (player.api.render_callback)(
                player.render,
                Some(request_frame),
                (&mut *player.repaint as *mut egui::Context).cast(),
            );
        }
        Ok(player)
    }

    /// Saves one frame of a video as JPEG, without a window or audio: a separate,
    /// short-lived mpv that seeks to `percent` of the video, writes the frame
    /// (at most 1280 px wide) and exits. Used for Seguir viendo previews.
    pub fn extract_frame(
        url: &str,
        headers: &str,
        percent: u32,
        out: &Path,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<(), String> {
        let exe_dir = std::env::current_exe()
            .map_err(|_| "No se encuentra la aplicación.")?
            .parent()
            .unwrap()
            .to_owned();
        let dll = [
            exe_dir.join("libmpv-2.dll"),
            Path::new(".tools/mpv/libmpv-2.dll").to_owned(),
        ]
        .into_iter()
        .find(|p| p.is_file())
        .ok_or("No se encuentra libmpv-2.dll.")?;
        let api = Api::load(&dll)?;
        let folder = out
            .parent()
            .ok_or("Ruta de fotograma no válida.")?
            .join(format!(
                "tmp-{}",
                out.file_stem().unwrap_or_default().to_string_lossy()
            ));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).map_err(|_| "No se pudo preparar el fotograma.")?;
        let handle = unsafe { (api.create)() };
        if handle.is_null() {
            return Err("No se pudo iniciar mpv.".into());
        }
        let folder_text = folder.to_string_lossy().into_owned();
        let start = format!("{}%", percent.clamp(5, 90));
        let options: [(&str, &str); 16] = [
            ("config", "no"),
            ("terminal", "no"),
            ("vo", "image"),
            ("vo-image-format", "jpg"),
            ("vo-image-jpeg-quality", "88"),
            ("vo-image-outdir", &folder_text),
            ("ao", "null"),
            ("audio", "no"),
            ("sub", "no"),
            ("frames", "1"),
            ("start", &start),
            ("hr-seek", "no"),
            ("vf", "scale=1280:-2"),
            ("network-timeout", "15"),
            ("http-header-fields", headers),
            ("ytdl", "no"),
        ];
        let result = (|| {
            for (key, value) in options {
                let key = CString::new(key).map_err(|_| "Opción no válida.")?;
                let value = CString::new(value).map_err(|_| "Opción no válida.")?;
                unsafe { (api.option)(handle, key.as_ptr(), value.as_ptr()) };
            }
            if unsafe { (api.initialize)(handle) } < 0 {
                return Err("No se pudo iniciar mpv.".to_owned());
            }
            let args = [
                c"loadfile".to_owned(),
                CString::new(url).map_err(|_| "URL no válida.")?,
            ];
            let mut pointers: Vec<_> = args.iter().map(|s| s.as_ptr()).collect();
            pointers.push(ptr::null());
            if unsafe { (api.command)(handle, pointers.as_ptr()) } < 0 {
                return Err("No se pudo abrir el vídeo.".to_owned());
            }
            let started = std::time::Instant::now();
            loop {
                if cancel.load(std::sync::atomic::Ordering::Relaxed)
                    || started.elapsed() > std::time::Duration::from_secs(40)
                {
                    return Err("Fotograma cancelado.".to_owned());
                }
                let event = unsafe { &*(api.wait)(handle, 0.25) };
                // 7: end-file, 1: shutdown.
                if matches!(event.id, 1 | 7) {
                    break;
                }
            }
            let frame = std::fs::read_dir(&folder)
                .map_err(|_| "No se generó el fotograma.")?
                .filter_map(Result::ok)
                .map(|e| e.path())
                .find(|p| p.extension().is_some_and(|x| x == "jpg"))
                .ok_or("No se generó el fotograma.")?;
            let saved = trim_letterbox(&frame, out);
            let _ = std::fs::remove_file(&frame);
            saved
        })();
        unsafe { (api.destroy)(handle) };
        let _ = std::fs::remove_dir_all(&folder);
        result
    }

    pub fn set(&self, key: &str, value: &str) -> Result<(), String> {
        let key = CString::new(key).map_err(|_| "Propiedad no válida.")?;
        let value = CString::new(value).map_err(|_| "Valor no válido.")?;
        if self.handle.is_null()
            || unsafe { (self.api.set)(self.handle, key.as_ptr(), value.as_ptr()) } < 0
        {
            return Err("El reproductor no acepta ese ajuste.".into());
        }
        Ok(())
    }
    pub fn get(&self, key: &str) -> Option<String> {
        if self.handle.is_null() {
            return None;
        }
        let key = CString::new(key).ok()?;
        let value = unsafe { (self.api.get)(self.handle, key.as_ptr()) };
        if value.is_null() {
            return None;
        }
        let text = unsafe { CStr::from_ptr(value) }
            .to_string_lossy()
            .into_owned();
        unsafe {
            (self.api.free)(value.cast());
        }
        Some(text)
    }
    pub fn number(&self, key: &str) -> f64 {
        self.get(key)
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite())
            .unwrap_or(0.0)
    }
    pub fn flag(&self, key: &str) -> bool {
        self.get(key).is_some_and(|v| v == "yes")
    }
    pub fn command(&self, args: &[&str]) -> Result<(), String> {
        let strings = args
            .iter()
            .map(|s| CString::new(*s))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "Orden no válida.")?;
        let mut pointers = strings.iter().map(|s| s.as_ptr()).collect::<Vec<_>>();
        pointers.push(ptr::null());
        if self.handle.is_null()
            || unsafe { (self.api.command)(self.handle, pointers.as_ptr()) } < 0
        {
            return Err("No se pudo completar la orden de reproducción.".into());
        }
        Ok(())
    }
    pub fn load(&mut self, playback: &super::data::Playback, volume: f64) -> Result<(), String> {
        self.loaded = false;
        self.ended = false;
        self.error = None;
        self.set("http-header-fields", &playback.headers)?;
        self.set("start", &playback.start.to_string())?;
        self.set("pause", "no")?;
        self.set("volume", &volume.clamp(0.0, 100.0).to_string())?;
        self.command(&["loadfile", &playback.url, "replace"])
    }
    pub fn poll(&mut self) -> bool {
        let mut just_loaded = false;
        if self.handle.is_null() {
            return false;
        }
        for _ in 0..64 {
            let event = unsafe { &*(self.api.wait)(self.handle, 0.0) };
            match event.id {
                0 => break,
                8 => {
                    self.loaded = true;
                    just_loaded = true;
                }
                7 if !event.data.is_null() => {
                    let end = unsafe { &*event.data.cast::<EndFile>() };
                    if end.reason == 0 {
                        self.ended = true;
                    }
                    if end.error < 0 {
                        self.error=Some("No se pudo leer el vídeo. Comprueba que el servidor y el archivo siguen disponibles.".into());
                    }
                }
                _ => {}
            }
        }
        if self.loaded {
            self.ended = self.flag("eof-reached");
        }
        just_loaded
    }
    pub fn tracks(&self, kind: &str) -> Vec<Track> {
        (0..(self.number("track-list/count") as usize).min(100))
            .filter_map(|index| {
                let key = format!("track-list/{index}");
                if self.get(&format!("{key}/type")).as_deref() != Some(kind) {
                    return None;
                }
                let id = self.get(&format!("{key}/id"))?;
                let lang = self.get(&format!("{key}/lang")).unwrap_or_default();
                let title = self
                    .get(&format!("{key}/title"))
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| format!("Pista {id}"));
                Some(Track {
                    id,
                    title,
                    language: lang,
                    codec: self.get(&format!("{key}/codec")).unwrap_or_default(),
                    channels: self.number(&format!("{key}/demux-channel-count")) as usize,
                    selected: self.flag(&format!("{key}/selected")),
                })
            })
            .collect()
    }
    pub fn render(&self, gl: &glow::Context, size: [u32; 2]) {
        if self.render.is_null() || !self.loaded {
            return;
        }
        // eframe invokes this with its destination framebuffer current. Black bars belong to mpv, never the hero.
        let mut fbo = Fbo {
            id: unsafe { gl.get_parameter_i32(glow::DRAW_FRAMEBUFFER_BINDING) },
            width: size[0] as i32,
            height: size[1] as i32,
            format: 0,
        };
        let mut flip = 1i32;
        let mut params = [
            Param {
                kind: 3,
                data: (&mut fbo as *mut Fbo).cast(),
            },
            Param {
                kind: 4,
                data: (&mut flip as *mut i32).cast(),
            },
            Param {
                kind: 0,
                data: ptr::null_mut(),
            },
        ];
        unsafe {
            (self.api.render)(self.render, params.as_mut_ptr());
        }
    }
    pub fn shutdown(&mut self) {
        unsafe {
            if !self.render.is_null() {
                (self.api.render_callback)(self.render, None, ptr::null_mut());
                (self.api.render_free)(self.render);
                self.render = ptr::null_mut();
            }
            if !self.handle.is_null() {
                (self.api.destroy)(self.handle);
                self.handle = ptr::null_mut();
            }
        }
    }
}
impl Drop for Player {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Removes black bands (letterbox / pillarbox) from an extracted frame. Rows and
/// columns are trimmed only while they are uniformly near-black, and never
/// beyond a third of the picture, so dark scenes are left intact.
fn trim_letterbox(source: &Path, out: &Path) -> Result<(), String> {
    let image = image::open(source)
        .map_err(|_| "No se pudo leer el fotograma.")?
        .to_rgb8();
    let (w, h) = image.dimensions();
    let dark_row = |y: u32| {
        (0..w)
            .step_by(4)
            .all(|x| image.get_pixel(x, y).0.iter().all(|c| *c < 22))
    };
    let dark_col = |x: u32| {
        (0..h)
            .step_by(4)
            .all(|y| image.get_pixel(x, y).0.iter().all(|c| *c < 22))
    };
    let mut top = 0;
    while top < h / 3 && dark_row(top) {
        top += 1;
    }
    let mut bottom = h;
    while bottom > h - h / 3 && dark_row(bottom - 1) {
        bottom -= 1;
    }
    let mut left = 0;
    while left < w / 3 && dark_col(left) {
        left += 1;
    }
    let mut right = w;
    while right > w - w / 3 && dark_col(right - 1) {
        right -= 1;
    }
    // Where a band was found, 2 px more absorb its soft edge.
    let band = |found: bool, value: u32, step: i64| -> u32 {
        if found {
            (value as i64 + step).max(0) as u32
        } else {
            value
        }
    };
    let (top_found, left_found) = (top > 0, left > 0);
    let (bottom_found, right_found) = (bottom < h, right < w);
    let top = band(top_found, top, 2).min(h / 3);
    let left = band(left_found, left, 2).min(w / 3);
    let bottom = band(bottom_found, bottom, -2).max(top + 1);
    let right = band(right_found, right, -2).max(left + 1);
    let cropped = if top_found || left_found || bottom_found || right_found {
        image::imageops::crop_imm(&image, left, top, right - left, bottom - top).to_image()
    } else {
        image
    };
    cropped
        .save_with_format(out, image::ImageFormat::Jpeg)
        .map_err(|_| "No se pudo guardar el fotograma.".to_owned())
}

#[cfg(test)]
mod frame_tests {
    #[test]
    fn letterbox_bands_are_trimmed_but_picture_is_kept() {
        let dir = std::env::temp_dir().join(format!("megalodon-frame-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut image = image::RgbImage::new(400, 225);
        for y in 30..195 {
            for x in 0..400 {
                image.put_pixel(x, y, image::Rgb([180, 120, 60]));
            }
        }
        let source = dir.join("in.png");
        image.save(&source).unwrap();
        let out = dir.join("out.jpg");
        super::trim_letterbox(&source, &out).unwrap();
        let trimmed = image::open(&out).unwrap();
        assert_eq!(trimmed.width(), 400);
        assert!(
            (160..=166).contains(&trimmed.height()),
            "{}",
            trimmed.height()
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
