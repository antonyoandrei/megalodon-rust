// A windowed app in every build: Megalodon never opens a console.
#![windows_subsystem = "windows"]

mod accounts;
#[path = "../avatars.rs"]
mod avatars;
mod casting;
mod casting_ui;
mod controls;
mod data;
mod drive;
mod files;
mod history;
#[path = "../jellyfin.rs"]
#[allow(dead_code)]
mod jellyfin;
mod metadata;
mod metadata_ui;
#[cfg(feature = "motion-capture")]
mod motion_capture;
mod onboarding;
mod playback_ui;
mod player;
mod profile_ui;
mod services;
mod sources;
mod sources_ui;
mod ui;
mod updates;
mod updates_ui;

use eframe::egui::{self, *};
use jellyfin::{Catalog, Config, Item, Library};
use std::{
    collections::{BTreeMap, HashMap},
    hash::{Hash, Hasher},
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Page {
    Home,
    Movies,
    Series,
    Search,
    Favorites,
    Detail,
    Settings,
    Profiles,
}
enum Message {
    Metadata(u64, metadata::Patch),
    ArtUploaded(String, String, Result<std::path::PathBuf, String>),
    AvatarUploaded(Result<files::LibraryAvatar, String>),
    FrameReady(String, Result<(), String>),
    DriveWrites(Result<(), String>),
    MetadataDone(u64, Result<usize, String>),
    UpdateChecked(u64, Result<Option<updates::Release>, String>),
    UpdateProgress(u64, u64, u64),
    UpdateDownloaded(u64, Result<std::path::PathBuf, String>),
    Catalog(u64, Result<(Arc<Catalog>, Option<String>), String>),
    Detail(u64, String, Result<(Item, Vec<Item>), String>),
    Image(String, Result<ColorImage, String>),
    Auth(u64, Result<Config, String>),
    Files(u64, Result<files::Files, String>),
    SourceAdded(u64, Result<sources::Source, String>),
    FolderChosen(u64, Option<std::path::PathBuf>),
    Playback(u64, Result<data::Playback, String>),
    ServicesReady(u64, Result<String, String>),
    ServicesPrepared(Result<(), String>),
    Receivers(u64, Result<Vec<casting::Device>, String>),
    Remote(u64, Result<Box<casting::Remote>, String>),
    Handoff(u64, Result<casting::Relay, String>),
}
struct ImageSlot {
    texture: Option<TextureHandle>,
    /// A tiny, softly interpolated copy of portrait art for cinematic hero backgrounds.
    ambient: Option<TextureHandle>,
    failed: bool,
    touched: u64,
    loaded_at: Option<Instant>,
}
struct Megalodon {
    metadata: metadata::State,
    updates: updates::State,
    services: Arc<Mutex<services::Services>>,
    services_cancel: Arc<std::sync::atomic::AtomicBool>,
    services_task: Option<tokio::task::JoinHandle<()>>,
    services_status: String,
    rt: Arc<tokio::runtime::Runtime>,
    tx: mpsc::Sender<Message>,
    rx: mpsc::Receiver<Message>,
    library: Option<Arc<Library>>,
    sources: sources::Store,
    source_editor: sources::Editor,
    settings_dialog: Option<ui::Dialog>,
    /// First-run step, while the welcome is shown.
    onboarding: Option<usize>,
    onboarding_name: String,
    #[cfg(feature = "motion-capture")]
    qa_picker: bool,
    #[cfg(feature = "motion-capture")]
    qa_detail: Option<&'static str>,
    files: Option<Arc<files::Files>>,
    source_cancel: Arc<std::sync::atomic::AtomicBool>,
    catalog: Option<Arc<Catalog>>,
    session_epoch: u64,
    profiles: profile_ui::Profiles,
    resume_items: Vec<Item>,
    page: Page,
    previous: Page,
    detail: Option<Item>,
    requested_detail: String,
    episodes: Vec<Item>,
    season: u32,
    episode_page: usize,
    images: HashMap<String, ImageSlot>,
    image_slots: Arc<tokio::sync::Semaphore>,
    frame: u64,
    last_open_request_check: Instant,
    brand: TextureHandle,
    profile: Option<TextureHandle>,
    profile_id: Option<String>,
    avatar_picker: bool,
    search: String,
    genre: String,
    sort: usize,
    hero: usize,
    hero_old: Option<usize>,
    hero_requested: Option<usize>,
    hero_changed: Instant,
    hero_ready_at: Option<Instant>,
    hero_cycle_at: Instant,
    hero_catalog_ids: Vec<String>,
    scene_key: Option<ui::SceneKey>,
    scene: Vec<egui::epaint::ClippedShape>,
    outgoing_scene: Vec<egui::epaint::ClippedShape>,
    outgoing_textures: Vec<TextureHandle>,
    scene_changed: Instant,
    scene_size: Vec2,
    #[cfg(feature = "motion-capture")]
    motion_capture: motion_capture::Capture,
    rail_offsets: HashMap<String, f32>,
    rail_targets: HashMap<String, f32>,
    rail_limits: HashMap<String, f32>,
    busy: bool,
    error: Option<String>,
    server: String,
    username: String,
    password: String,
    local_path: String,
    player: Option<Arc<Mutex<player::Player>>>,
    player_init_error: Option<String>,
    playback: Option<data::Playback>,
    preparing: bool,
    playback_request: u64,
    playback_started: Instant,
    preparing_title: String,
    feedback: Option<(String, Instant)>,
    skip_burst: Option<playback_ui::SeekBurst>,
    controls_seen: Instant,
    controls_keyboard: bool,
    last_mouse: Option<Pos2>,
    fullscreen: bool,
    playback_menu_open: bool,
    playback_menu_tab: usize,
    casting: casting_ui::Casting,
    playback_tracks: [Vec<player::Track>; 2],
    last_report: Instant,
    seek: f64,
    seek_dragging: bool,
    subtitle_position: i32,
    preferences: data::Preferences,
    active_preferences: String,
    no_report: bool,
    auto_next: bool,
    ended_at: Option<Instant>,
}

fn main() -> eframe::Result {
    if std::env::args().any(|a| a == "--setup-services") {
        let result = services::prepare(&data::data_dir());
        if let Err(error) = &result {
            let _ = std::fs::write(data::data_dir().join("services-setup-error.txt"), error);
        }
        std::process::exit(if result.is_ok() { 0 } else { 1 });
    }
    let _instance_lease = match services::instance_lock(&data::data_dir()) {
        Ok(Some(lease)) => lease,
        Ok(None) => {
            let args: Vec<_> = std::env::args().collect();
            if let Some(index) = args.iter().position(|a| a == "--open")
                && let Some(path) = args.get(index + 1)
            {
                let _ = data::save_json("open-request.json", path);
            }
            use windows_sys::Win32::UI::WindowsAndMessaging::*;
            let title: Vec<u16> = "Megalodon".encode_utf16().chain(Some(0)).collect();
            unsafe {
                let window = FindWindowW(std::ptr::null(), title.as_ptr());
                if !window.is_null() {
                    ShowWindow(window, SW_RESTORE);
                    SetForegroundWindow(window);
                }
            }
            return Ok(());
        }
        Err(error) => {
            return Err(eframe::Error::AppCreation(Box::new(std::io::Error::other(
                error,
            ))));
        }
    };
    let icon = image::load_from_memory(include_bytes!("../../static/brand/mark.png"))
        .ok()
        .map(|image| {
            let pixels = image.into_rgba8();
            egui::IconData {
                width: pixels.width(),
                height: pixels.height(),
                rgba: pixels.into_raw(),
            }
        });
    let mut viewport = ViewportBuilder::default()
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([640.0, 480.0])
        .with_title("Megalodon");
    if let Some(icon) = icon {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Glow,
        vsync: true,
        ..Default::default()
    };
    eframe::run_native(
        "Megalodon",
        options,
        Box::new(|cc| Ok(Box::new(Megalodon::new(cc)))),
    )
}

impl Megalodon {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        ui::theme(&cc.egui_ctx);
        let rt = Arc::new(tokio::runtime::Runtime::new().expect("Runtime de la aplicación"));
        let (tx, rx) = mpsc::channel();
        // The in-app symbol is the white master over the dark interface; the
        // window and taskbar icon keep the blue version (mark.png).
        let brand_image =
            image::load_from_memory(include_bytes!("../../static/brand/mark-white.png"))
                .unwrap()
                .into_rgba8();
        let brand = cc.egui_ctx.load_texture(
            "Megalodon",
            ColorImage::from_rgba_unmultiplied(
                [brand_image.width() as usize, brand_image.height() as usize],
                &brand_image,
            ),
            ARTWORK,
        );
        let existing_preferences = data::data_dir().join("preferences.json").is_file();
        let mut preferences: data::Preferences =
            std::fs::read(data::data_dir().join("preferences.json"))
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
                .unwrap_or_else(|| data::Preferences {
                    volume: 75.0,
                    auto_next: true,
                    automatic_updates: true,
                    ..Default::default()
                });
        accounts::migrate_local_profiles(&mut preferences);
        controls::set_theme(&preferences.theme);
        ui::theme(&cc.egui_ctx);
        let auto_next = preferences.auto_next;
        if preferences.local_profiles.is_empty() {
            preferences
                .local_profiles
                .insert("local".into(), "Mi perfil".into());
        }
        if !preferences
            .local_profiles
            .contains_key(&preferences.local_profile)
        {
            preferences.local_profile = preferences.local_profiles.keys().next().unwrap().clone();
        }
        let (sources, source_error) = match sources::Store::load() {
            Ok(s) => (s, None),
            Err(e) => (sources::Store::default(), Some(e)),
        };
        let config = sources.current().and_then(|s| s.config().ok().flatten());
        let server = config
            .as_ref()
            .map_or("http://127.0.0.1:8096".into(), |c| c.base_url.clone());
        let library = config
            .filter(|c| !c.token.is_empty())
            .and_then(|c| Library::new(c).ok())
            .map(Arc::new);
        let (player, player_init_error) = match player::Player::new(cc) {
            Ok(p) => (Some(Arc::new(Mutex::new(p))), None),
            Err(e) => (None, Some(e)),
        };
        let service_manager = services::Services::new(&data::data_dir());
        let services_cancel = service_manager.cancel.clone();
        let mut app = Self {
            metadata: metadata::State::default(),
            updates: updates::State::default(),
            services: Arc::new(Mutex::new(service_manager)),
            services_cancel,
            services_task: None,
            services_status: String::new(),
            rt,
            tx,
            rx,
            library,
            sources,
            source_editor: sources::Editor::default(),
            settings_dialog: None,
            onboarding: None,
            onboarding_name: String::new(),
            #[cfg(feature = "motion-capture")]
            qa_picker: false,
            #[cfg(feature = "motion-capture")]
            qa_detail: None,
            files: None,
            source_cancel: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            catalog: None,
            session_epoch: 0,
            profiles: profile_ui::Profiles::default(),
            resume_items: vec![],
            page: Page::Home,
            previous: Page::Home,
            detail: None,
            requested_detail: String::new(),
            episodes: vec![],
            season: 1,
            episode_page: 0,
            images: HashMap::new(),
            image_slots: Arc::new(tokio::sync::Semaphore::new(6)),
            frame: 0,
            last_open_request_check: Instant::now(),
            brand,
            profile: None,
            profile_id: None,
            avatar_picker: false,
            search: String::new(),
            genre: String::new(),
            sort: 0,
            hero: 0,
            hero_old: None,
            hero_requested: None,
            hero_changed: Instant::now(),
            hero_ready_at: None,
            hero_cycle_at: Instant::now(),
            hero_catalog_ids: vec![],
            scene_key: None,
            scene: vec![],
            outgoing_scene: vec![],
            outgoing_textures: vec![],
            scene_changed: Instant::now(),
            scene_size: Vec2::ZERO,
            #[cfg(feature = "motion-capture")]
            motion_capture: motion_capture::Capture::default(),
            rail_offsets: HashMap::new(),
            rail_targets: HashMap::new(),
            rail_limits: HashMap::new(),
            busy: false,
            error: source_error,
            server,
            username: String::new(),
            password: String::new(),
            local_path: String::new(),
            player,
            player_init_error,
            playback: None,
            preparing: false,
            playback_request: 0,
            playback_started: Instant::now(),
            preparing_title: String::new(),
            feedback: None,
            skip_burst: None,
            controls_seen: Instant::now(),
            controls_keyboard: false,
            last_mouse: None,
            fullscreen: false,
            playback_menu_open: false,
            playback_menu_tab: 0,
            casting: casting_ui::Casting::default(),
            playback_tracks: [vec![], vec![]],
            last_report: Instant::now(),
            seek: 0.0,
            seek_dragging: false,
            subtitle_position: -1,
            preferences,
            active_preferences: String::new(),
            no_report: std::env::args().any(|a| a == "--no-report"),
            auto_next,
            ended_at: None,
        };
        {
            let user = app.local_user();
            app.apply_profile_preferences(&user.id);
            app.load_local_portrait(&cc.egui_ctx);
            // People updating from an earlier version already set Megalodon up.
            let set_up = existing_preferences
                && (!app.sources.entries.is_empty()
                    || !app.preferences.history.is_empty()
                    || app
                        .preferences
                        .local_profiles
                        .values()
                        .any(|n| n != "Mi perfil"));
            if set_up {
                app.preferences.onboarded = true;
            }
            if !app.preferences.onboarded {
                app.onboarding = Some(0);
            } else if app.sources.active.is_none() {
                app.page = Page::Settings;
            }
        }
        if let Some(source) = app.sources.current().cloned() {
            if matches!(source.kind, sources::Kind::Jellyfin { .. }) {
                app.start_services(&cc.egui_ctx);
            } else {
                app.select_source(&cc.egui_ctx, &source.id);
            }
        }
        if app.preferences.automatic_updates {
            app.check_updates(&cc.egui_ctx);
        }
        let args: Vec<_> = std::env::args().collect();
        if let Some(index) = args.iter().position(|a| a == "--open")
            && let Some(path) = args.get(index + 1)
        {
            app.open_local(path.clone());
        }
        // Render evidence only (compiled out of release packages): start on a
        // given settings scene so dialogs can be captured without input automation.
        #[cfg(feature = "motion-capture")]
        if let Ok(scene) = std::env::var("MEGALODON_QA_SCENE") {
            app.page = Page::Settings;
            match scene.as_str() {
                "connection" => app.open_source_editor(),
                "metadata" => app.settings_dialog = Some(ui::Dialog::Metadata),
                "experience" => app.settings_dialog = Some(ui::Dialog::Experience),
                "picker" => app.qa_picker = true,
                "detail" => app.qa_detail = Some("Movie"),
                "detailseries" => app.qa_detail = Some("Series"),
                "appearance" => app.settings_dialog = Some(ui::Dialog::Appearance),
                "avatars" => app.open_avatar_carousel(),
                step if step.starts_with("welcome") => {
                    app.onboarding = step[7..].parse().ok().or(Some(0));
                }
                _ => {}
            }
        }
        app
    }
    fn start_services(&mut self, ctx: &Context) {
        self.busy = true;
        self.services_status = "Preparando conexión…".into();
        let manager = self.services.clone();
        let base = self.server.clone();
        let token = accounts::management_token(&data::data_dir(), &base).unwrap_or_else(|| {
            self.library
                .as_ref()
                .map_or(String::new(), |library| library.config.token.clone())
        });
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        let handle = self.rt.handle().clone();
        let epoch = self.session_epoch;
        self.services_task = Some(self.rt.spawn_blocking(move || {
            let mut manager = manager.lock().unwrap();
            manager
                .cancel
                .store(false, std::sync::atomic::Ordering::Relaxed);
            let result = handle
                .block_on(manager.start(&base, &token))
                .map(|_| manager.status.clone());
            let _ = tx.send(Message::ServicesReady(epoch, result));
            ctx.request_repaint();
        }));
    }
    fn refresh(&mut self, ctx: &Context, force: bool) {
        if self.library.is_none()
            && self.files.is_none()
            && let Some(id) = self.sources.active.clone()
        {
            self.select_source(ctx, &id);
            return;
        }
        if self.files.is_some() {
            if force {
                if let Some(id) = self.sources.active.clone() {
                    files::invalidate_inventory(&id);
                    self.select_source(ctx, &id);
                }
            } else {
                self.refresh_files(ctx);
            }
            return;
        }
        let Some(library) = self.library.clone() else {
            return;
        };
        self.busy = true;
        if force {
            self.images.retain(|_, slot| !slot.failed);
        }
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        let epoch = self.session_epoch;
        self.rt.spawn(async move {
            let result = library.catalog(force).await;
            let _ = tx.send(Message::Catalog(epoch, result));
            ctx.request_repaint();
        });
    }
    fn open_detail(&mut self, ctx: &Context, id: &str) {
        if self.files.is_some() {
            self.open_file_detail(id);
            return;
        }
        let (Some(library), Some(catalog)) = (self.library.clone(), self.catalog.clone()) else {
            return;
        };
        self.previous = if self.page == Page::Detail {
            self.previous
        } else {
            self.page
        };
        self.page = Page::Detail;
        self.busy = true;
        self.detail = catalog.items.iter().find(|i| i.id == id).cloned();
        self.episodes.clear();
        self.requested_detail = id.into();
        self.episode_page = 0;
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        let id = id.to_owned();
        let epoch = self.session_epoch;
        self.rt.spawn(async move {
            let result = async {
                let item = library.detail(&catalog.user.id, &id).await?;
                let parent = if item.kind == "Series" {
                    Some(item.id.as_str())
                } else {
                    item.series_id.as_deref()
                };
                let episodes = if let Some(parent) = parent {
                    library
                        .all_items(
                            &catalog.user.id,
                            "Episode",
                            Some(parent),
                            "ParentIndexNumber,IndexNumber,SortName",
                        )
                        .await?
                } else {
                    vec![]
                };
                Ok((item, episodes))
            }
            .await;
            let _ = tx.send(Message::Detail(epoch, id, result));
            ctx.request_repaint();
        });
    }
    fn play(&mut self, ctx: &Context, id: &str, resume: bool) {
        if self.preparing {
            return;
        }
        if let Some(error) = &self.player_init_error {
            self.error = Some(error.clone());
            return;
        }
        if self.files.is_some() {
            self.play_file(id, resume);
            return;
        }
        let (Some(library), Some(catalog)) = (self.library.clone(), self.catalog.clone()) else {
            return;
        };
        self.stop();
        self.preparing = true;
        self.playback_started = Instant::now();
        self.preparing_title = catalog
            .items
            .iter()
            .chain(self.episodes.iter())
            .chain(self.resume_items.iter())
            .find(|i| i.id == id)
            .map_or(String::new(), |i| i.name.clone());
        let request = self.playback_request;
        self.error = None;
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        let id = id.to_owned();
        let start = if resume {
            self.preferences
                .history
                .get(&format!(
                    "{}|{id}",
                    self.profile_key(&self.preferences.local_profile)
                ))
                .filter(|w| !w.complete)
                .map_or(0.0, |w| w.position)
        } else {
            0.0
        };
        self.rt.spawn(async move {
            let result = data::prepare(&library, &catalog.user.id, &id, start).await;
            let _ = tx.send(Message::Playback(request, result));
            ctx.request_repaint();
        });
    }
    fn open_local(&mut self, path: String) {
        let path = std::path::PathBuf::from(path);
        if !path.is_file() {
            self.error = Some("El archivo seleccionado no está disponible.".into());
            return;
        }
        self.stop();
        let playback = data::Playback {
            item: Item {
                name: path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                kind: "Movie".into(),
                ..Default::default()
            },
            url: path.to_string_lossy().into_owned(),
            headers: String::new(),
            source_id: String::new(),
            session_id: String::new(),
            start: 0.0,
            subtitles: vec![],
        };
        self.start_playback(playback);
    }
    fn start_playback(&mut self, playback: data::Playback) {
        let Some(player) = &self.player else {
            self.error = self.player_init_error.clone();
            return;
        };
        match player
            .lock()
            .unwrap()
            .load(&playback, self.preferences.volume)
        {
            Ok(()) => {
                self.playback = Some(playback);
                self.playback_started = Instant::now();
                self.feedback = None;
                self.subtitle_position = -1;
                self.controls_seen = Instant::now();
                self.ended_at = None;
                self.last_report = Instant::now();
            }
            Err(error) => self.error = Some(error),
        }
    }
    fn sync_progress(&mut self, _endpoint: &str) {
        let (Some(playback), Some(player)) = (self.playback.clone(), self.player.as_ref()) else {
            return;
        };
        let player = player.lock().unwrap();
        if !player.loaded {
            return;
        }
        let position = player.number("time-pos");
        let duration = player.number("duration");
        let ended = player.flag("eof-reached");
        drop(player);
        self.record_watch(playback.item.clone(), position, duration, ended);
        if !self.no_report {
            let _ = self.save_preferences();
        }
        self.last_report = Instant::now();
    }
    fn stop(&mut self) {
        let remote = self.casting.remote.as_ref().is_some_and(|r| r.connected)
            || self
                .casting
                .handoff
                .as_ref()
                .is_some_and(|r| r.progress().is_some());
        self.end_cast(false);
        self.playback_request = self.playback_request.wrapping_add(1);
        self.preparing = false;
        if !remote {
            self.sync_progress("/Stopped");
        }
        if let Some(player) = &self.player {
            let _ = player.lock().unwrap().command(&["stop"]);
        }
        self.playback = None;
        self.skip_burst = None;
        self.playback_menu_open = false;
        self.ended_at = None;
    }
    fn messages(&mut self, ctx: &Context) {
        while let Ok(message) = self.rx.try_recv() {
            match message {
                Message::Metadata(epoch, patch) => {
                    if epoch != self.session_epoch {
                        continue;
                    }
                    let id = patch.id.clone();
                    let patch = patch.with_choices(&self.metadata.choices);
                    self.metadata.patches.insert(id.clone(), patch);
                    if let Some(mut item) = self
                        .catalog
                        .as_ref()
                        .and_then(|c| c.items.iter().find(|i| i.id == id))
                        .cloned()
                    {
                        self.local_state(&mut item);
                        self.update_item(&item);
                    }
                }
                Message::ArtUploaded(id, kind, result) => {
                    self.metadata.uploading = None;
                    match result {
                        Ok(path) => {
                            self.metadata
                                .uploads
                                .insert((id.clone(), kind.clone()), path);
                            if let Some(files) = &self.files {
                                files::invalidate_inventory(&files.source.id);
                            }
                            self.choose_artwork(&id, &kind, Some(metadata::LOCAL_CHOICE.into()));
                        }
                        Err(e) if e.is_empty() => {}
                        Err(e) => self.error = Some(e),
                    }
                }
                Message::AvatarUploaded(result) => self.avatar_uploaded(result),
                Message::FrameReady(id, result) => {
                    if result.is_ok() {
                        self.metadata.frames.remove(&id);
                    } else {
                        self.metadata.frames.insert(id, false);
                    }
                }
                Message::DriveWrites(result) => {
                    self.metadata.status = match result {
                        Ok(()) => "Listo: Megalodon ya puede guardar imágenes en tu Drive.".into(),
                        Err(e) => e,
                    };
                }
                Message::MetadataDone(epoch, result) => {
                    if epoch != self.session_epoch {
                        continue;
                    }
                    #[cfg(feature = "motion-capture")]
                    if let Some(kind) = self.qa_detail.take() {
                        let resume = self
                            .resume_items
                            .iter()
                            .find(|i| i.kind == kind)
                            .map(|i| i.id.clone());
                        let id = resume.or_else(|| {
                            self.catalog.as_ref().and_then(|c| {
                                c.items
                                    .iter()
                                    .find(|i| i.kind == kind)
                                    .map(|i| i.id.clone())
                            })
                        });
                        if let Some(id) = id {
                            self.open_detail(ctx, &id);
                        }
                    }
                    #[cfg(feature = "motion-capture")]
                    if self.qa_picker
                        && let Some(id) = self.catalog.as_ref().and_then(|c| {
                            c.items
                                .iter()
                                .filter(|i| i.kind == "Series")
                                .find(|i| self.has_artwork_choices(&i.id))
                                .map(|i| i.id.clone())
                        })
                    {
                        self.qa_picker = false;
                        self.open_detail(ctx, &id);
                        self.open_artwork_picker(&id, 1);
                    }
                    self.metadata.busy = false;
                    self.metadata.status = match result {
                        Ok(0) => {
                            "Consulta terminada. No hay coincidencias seguras que añadir.".into()
                        }
                        Ok(count) => format!("Consulta terminada · {count} fichas completadas."),
                        Err(e) => e,
                    };
                }
                Message::UpdateChecked(request, result) => {
                    if request != self.updates.request {
                        continue;
                    }
                    self.updates.checking = false;
                    self.updates.checked = true;
                    match result {
                        Ok(release) => {
                            self.updates.release = release;
                            self.updates.dismissed = false;
                        }
                        Err(e) => self.updates.error = Some(e),
                    }
                }
                Message::UpdateProgress(request, done, total) => {
                    if request == self.updates.request {
                        self.updates.progress = (done, total);
                    }
                }
                Message::UpdateDownloaded(request, result) => {
                    if request != self.updates.request {
                        continue;
                    }
                    self.updates.downloading = false;
                    match result {
                        Ok(path) => self.updates.ready = Some(path),
                        Err(e) => self.updates.error = Some(e),
                    }
                }
                Message::Files(epoch, result) => {
                    if epoch != self.session_epoch {
                        continue;
                    }
                    self.busy = false;
                    match result {
                        Ok(files) => {
                            history::import_direct_history(
                                &mut self.preferences,
                                &files.source.id,
                                &files.items,
                            );
                            if let Err(e) = self.save_preferences() {
                                self.error = Some(e);
                            }
                            self.files = Some(Arc::new(files));
                            self.refresh_files(ctx);
                            self.start_metadata(ctx);
                        }
                        Err(e) => {
                            self.error = Some(e);
                            self.page = Page::Settings;
                        }
                    }
                }
                Message::SourceAdded(request, result) => {
                    if request != self.source_editor.request {
                        continue;
                    }
                    self.source_editor.busy = false;
                    match result {
                        Ok(source) => self.add_source(ctx, source),
                        Err(e) => self.source_editor.error = Some(e),
                    }
                }
                Message::FolderChosen(request, path) => {
                    if request == self.source_editor.request
                        && let Some(path) = path
                    {
                        self.source_editor.path = path.display().to_string();
                    }
                }
                Message::Receivers(request, result) => {
                    if request == self.casting.request {
                        self.casting.searching = false;
                        match result {
                            Ok(devices) => self.casting.devices = devices,
                            Err(error) => self.casting.error = Some(error),
                        }
                    }
                }
                Message::Remote(request, result) => {
                    if request == self.casting.request {
                        self.casting.connecting = false;
                        match result {
                            Ok(remote) => self.casting.remote = Some(*remote),
                            Err(error) => self.casting.error = Some(error),
                        }
                    }
                }
                Message::Handoff(request, result) => {
                    if request == self.casting.request {
                        self.casting.connecting = false;
                        match result {
                            Ok(relay) => self.casting.handoff = Some(relay),
                            Err(error) => self.casting.error = Some(error),
                        }
                    }
                }
                Message::ServicesReady(epoch, result) => {
                    if epoch != self.session_epoch {
                        continue;
                    }
                    self.busy = false;
                    match result {
                        Ok(status) => {
                            self.services_status = status;
                            self.refresh(ctx, false);
                        }
                        Err(error) => {
                            self.services_status = error.clone();
                            self.error = Some(error);
                        }
                    }
                }
                Message::ServicesPrepared(result) => {
                    self.busy = false;
                    match result {
                        Ok(()) => {
                            self.services_status =
                                "Servicios preparados para el próximo inicio".into();
                        }
                        Err(error) => self.error = Some(error),
                    }
                }
                Message::Catalog(epoch, result) => {
                    if epoch != self.session_epoch {
                        continue;
                    }
                    self.busy = false;
                    match result {
                        Ok((catalog, warning)) => {
                            self.error = warning;
                            self.hero_old = None;
                            self.hero_requested = None;
                            self.hero_ready_at = None;
                            if let Some(library) = &self.library {
                                if catalog.user.policy.is_administrator
                                    && let Ok(mut services) = self.services.try_lock()
                                {
                                    services.update_auth(
                                        &library.config.base_url,
                                        &library.config.token,
                                    );
                                }
                                if let Err(error) = accounts::remember(
                                    &data::data_dir(),
                                    &library.config,
                                    &catalog.user,
                                ) {
                                    self.error = Some(error);
                                }
                            }
                            self.catalog = Some(catalog);
                            self.refresh_profile_state();
                            self.load_local_portrait(ctx);
                            self.start_metadata(ctx);
                        }
                        Err(e) => {
                            self.error = Some(e);
                            if self.catalog.is_none() {
                                self.page = Page::Settings;
                            }
                        }
                    }
                }
                Message::Detail(epoch, id, result) => {
                    if epoch != self.session_epoch || self.requested_detail != id {
                        continue;
                    }
                    self.busy = false;
                    match result {
                        Ok((mut item, mut episodes)) => {
                            self.local_state(&mut item);
                            for episode in &mut episodes {
                                self.local_state(episode);
                            }
                            self.season = episodes
                                .iter()
                                .filter_map(|e| e.parent_index_number)
                                .find(|n| *n > 0)
                                .unwrap_or(0);
                            self.detail = Some(item);
                            self.episodes = episodes;
                        }
                        Err(e) => self.error = Some(e),
                    }
                }
                Message::Image(key, result) => {
                    if let Some(slot) = self.images.get_mut(&key) {
                        match result {
                            Ok(image) => {
                                slot.ambient = ui::ambient_image(&image).map(|small| {
                                    ctx.load_texture(
                                        format!("{key}#ambient"),
                                        small,
                                        TextureOptions::LINEAR,
                                    )
                                });
                                slot.texture = Some(ctx.load_texture(&key, image, ARTWORK));
                                slot.loaded_at = Some(Instant::now());
                                if self
                                    .profile_id
                                    .as_ref()
                                    .is_some_and(|id| key == format!("avatar:{id}"))
                                {
                                    self.profile = slot.texture.clone();
                                }
                            }
                            Err(_) => slot.failed = true,
                        }
                    }
                }
                Message::Auth(epoch, result) => {
                    if epoch != self.session_epoch {
                        continue;
                    }
                    self.busy = false;
                    match result {
                        Ok(config) => self.activate_profile(ctx, config),
                        Err(e) => self.error = Some(e),
                    }
                }
                Message::Playback(request, result) => {
                    if request != self.playback_request {
                        continue;
                    }
                    self.preparing = false;
                    match result {
                        Ok(playback) => self.start_playback(playback),
                        Err(e) => self.error = Some(e),
                    }
                }
            }
        }
        if self.images.len() > 110 {
            let mut old: Vec<_> = self
                .images
                .iter()
                .filter(|(_, s)| s.texture.is_some() && s.touched + 2 < self.frame)
                .map(|(k, s)| (k.clone(), s.touched))
                .collect();
            old.sort_by_key(|(_, t)| *t);
            for (key, _) in old.into_iter().take(self.images.len().saturating_sub(90)) {
                self.images.remove(&key);
            }
        }
    }
    fn artwork(
        &mut self,
        ctx: &Context,
        item: &Item,
        kind: &str,
        width: u32,
    ) -> Option<TextureHandle> {
        // Request physical pixels: a 1920 px hero on a 150 % display is 2880 px
        // wide, and stretching a smaller image is what made art look soft.
        let width = artwork_width(ctx, width);
        if let Some(url) = self
            .metadata
            .patches
            .get(&item.id)
            .and_then(|p| p.images.get(kind))
            .or_else(|| {
                // Episodes without their own thumbnail use the series artwork.
                // A supplied episode still always keeps its own frame.
                if item.kind != "Episode"
                    || kind == "Logo"
                    || item.image_tags.contains_key(kind)
                    || (kind == "Backdrop" && !item.backdrop_image_tags.is_empty())
                {
                    return None;
                }
                let series = self.metadata.patches.get(item.series_id.as_ref()?)?;
                // The episode's own TMDB picture first, then the series art.
                let still = item
                    .parent_index_number
                    .zip(item.index_number)
                    .and_then(|(s, e)| series.episode_stills.get(&format!("S{s}E{e}")));
                still
                    .filter(|_| kind == "Primary")
                    .or_else(|| series.images.get("Backdrop"))
                    .or_else(|| series.images.get("Primary"))
            })
            .cloned()
            && let Some(texture) = self.metadata_artwork(ctx, &url, width)
        {
            return Some(texture);
        }
        if self.files.is_some() {
            return self.file_artwork(ctx, item, kind, width);
        }
        let library = self.library.as_ref()?.clone();
        let tag = if kind == "Backdrop" {
            item.backdrop_image_tags.first()
        } else {
            item.image_tags.get(kind)
        };
        let tag = tag?;
        let url = format!(
            "{}/Items/{}/Images/{kind}?MaxWidth={width}&Quality=90&Tag={tag}",
            library.config.base_url, item.id
        );
        if let Some(slot) = self.images.get_mut(&url) {
            slot.touched = self.frame;
            return slot.texture.clone();
        }
        self.images.insert(
            url.clone(),
            ImageSlot {
                texture: None,
                ambient: None,
                failed: false,
                touched: self.frame,
                loaded_at: None,
            },
        );
        let ctx = ctx.clone();
        let tx = self.tx.clone();
        let slots = self.image_slots.clone();
        self.rt.spawn(async move {
            let Ok(_permit) = slots.acquire_owned().await else {
                return;
            };
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            url.hash(&mut hasher);
            let cache = data::data_dir()
                .join("image-cache")
                .join(format!("{:016x}", hasher.finish()));
            let result = async {
                let bytes = if let Ok(bytes) = tokio::fs::read(&cache).await {
                    bytes
                } else {
                    let response = library
                        .client
                        .get(&url)
                        .send()
                        .await
                        .map_err(|_| "Imagen no disponible")?
                        .error_for_status()
                        .map_err(|_| "Imagen no disponible")?;
                    if response.content_length().unwrap_or(0) > 16 * 1024 * 1024 {
                        return Err("Imagen demasiado grande".to_owned());
                    }
                    let bytes = response
                        .bytes()
                        .await
                        .map_err(|_| "Imagen no disponible")?
                        .to_vec();
                    if bytes.len() > 16 * 1024 * 1024 {
                        return Err("Imagen demasiado grande".to_owned());
                    }
                    let _ = tokio::fs::create_dir_all(cache.parent().unwrap()).await;
                    let _ = tokio::fs::write(&cache, &bytes).await;
                    bytes
                };
                tokio::task::spawn_blocking(move || decode_image(&bytes))
                    .await
                    .map_err(|_| "Imagen no disponible".to_owned())?
            }
            .await;
            let _ = tx.send(Message::Image(url, result));
            ctx.request_repaint();
        });
        None
    }
    fn next_episode(&self) -> Option<Item> {
        let current = &self.playback.as_ref()?.item.id;
        let index = self.episodes.iter().position(|e| &e.id == current)?;
        self.episodes.get(index + 1).cloned()
    }
}

/// Linear filtering with mipmaps: posters and stills drawn smaller than
/// their source stay smooth instead of shimmering with jagged edges.
const ARTWORK: TextureOptions = TextureOptions {
    magnification: TextureFilter::Linear,
    minification: TextureFilter::Linear,
    wrap_mode: TextureWrapMode::ClampToEdge,
    mipmap_mode: Some(TextureFilter::Linear),
};

/// Physical width for an artwork request, in 160 px steps so a DPI change
/// does not multiply cache entries.
fn artwork_width(ctx: &Context, logical: u32) -> u32 {
    let physical = (logical as f32 * ctx.pixels_per_point().max(1.0)).ceil() as u32;
    physical.div_ceil(160).saturating_mul(160).clamp(160, 3840)
}

/// Decodes and, when the source is far larger than its use, downsamples with
/// a high-quality filter before upload (less memory, crisper result).
fn decode_fit(bytes: &[u8], max_width: u32) -> Result<ColorImage, String> {
    let image = decode_image(bytes)?;
    let max = max_width.clamp(160, 3840) as usize;
    if image.width() <= max * 5 / 4 {
        return Ok(image);
    }
    let height = (image.height() as f32 * max as f32 / image.width() as f32).round() as u32;
    let rgba = image::RgbaImage::from_raw(
        image.width() as u32,
        image.height() as u32,
        image
            .pixels
            .iter()
            .flat_map(|p| p.to_srgba_unmultiplied())
            .collect(),
    )
    .ok_or("No se pudo decodificar la imagen")?;
    let resized = image::imageops::resize(
        &rgba,
        max as u32,
        height.max(1),
        image::imageops::FilterType::Lanczos3,
    );
    Ok(ColorImage::from_rgba_unmultiplied(
        [resized.width() as usize, resized.height() as usize],
        &resized,
    ))
}

fn decode_image(bytes: &[u8]) -> Result<ColorImage, String> {
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| "Formato de imagen no válido")?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|_| "No se pudo decodificar la imagen")?
        .into_rgba8();
    Ok(ColorImage::from_rgba_unmultiplied(
        [image.width() as usize, image.height() as usize],
        &image,
    ))
}

impl eframe::App for Megalodon {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.frame += 1;
        if self.last_open_request_check.elapsed() >= Duration::from_millis(250) {
            self.last_open_request_check = Instant::now();
            let request = data::data_dir().join("open-request.json");
            if let Ok(bytes) = std::fs::read(&request) {
                let _ = std::fs::remove_file(&request);
                if let Ok(path) = serde_json::from_slice::<String>(&bytes) {
                    self.open_local(path);
                }
            }
        }
        ctx.request_repaint_after(Duration::from_millis(250));
        ctx.data_mut(|d| d.insert_temp(Id::new("reduced-motion"), self.preferences.reduced_motion));
        ctx.style_mut(|s| {
            s.animation_time = if self.preferences.reduced_motion {
                0.0
            } else {
                0.16
            }
        });
        self.messages(ctx);
        self.poll_cast(ctx);
        for file in ctx.input(|i| i.raw.dropped_files.clone()) {
            if let Some(path) = file.path {
                self.open_local(path.to_string_lossy().into_owned());
            }
        }
        if self.playback.is_none() && !self.preparing && self.fullscreen {
            self.fullscreen = false;
            ctx.send_viewport_cmd(ViewportCommand::Fullscreen(false));
        }
        if self.casting.remote.as_ref().is_some_and(|r| r.connected)
            || self
                .casting
                .handoff
                .as_ref()
                .is_some_and(|r| r.progress().is_some())
        {
            self.begin_scene(ctx, ui::SceneKey::Remote);
            self.remote_ui(ctx);
        } else if self.preparing {
            self.begin_scene(ctx, ui::SceneKey::Preparing);
            self.preparing_ui(ctx);
        } else if self.playback.is_some() {
            self.begin_scene(ctx, ui::SceneKey::Player);
            self.player_ui(ctx);
        } else {
            if self.onboarding.is_some() {
                self.begin_scene(ctx, ui::SceneKey::Onboarding(self.onboarding.unwrap_or(0)));
                self.onboarding_ui(ctx);
                self.dialogs(ctx);
            } else {
                self.library_ui(ctx);
            }
        }
        self.finish_scene(ctx);
        self.update_notice(ctx);
        #[cfg(feature = "motion-capture")]
        self.motion_capture.frame(ctx);
        if self.casting.open {
            self.cast_menu(ctx);
        }
        if let Some(error) = self.error.clone() {
            let modal = egui::Modal::new(Id::new("error"))
                .frame(controls::menu_frame().inner_margin(24))
                .show(ctx, |ui| {
                    ui.set_width(380.0_f32.min(ctx.content_rect().width() - 80.0));
                    ui.label(RichText::new("No se ha podido completar").size(21.0));
                    ui.add_space(12.0);
                    ui.label(error);
                    ui.add_space(16.0);
                    let (rect, _) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::hover());
                    if controls::button(ui, rect, "Entendido", None, controls::ButtonStyle::Primary)
                        .clicked()
                    {
                        self.error = None;
                    }
                });
            if modal.should_close() {
                self.error = None;
            }
        }
    }
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.source_cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.source_editor
            .cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.stop();
        self.files = None;
        let _ = self.save_preferences();
        if let Some(player) = &self.player {
            player.lock().unwrap().shutdown();
        }
        self.services_cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(task) = self.services_task.take() {
            let _ = self.rt.block_on(task);
        }
        let result = self.rt.block_on(self.services.lock().unwrap().stop());
        if let Err(error) = result {
            let _ = std::fs::write(data::data_dir().join("services-shutdown.txt"), error);
        }
    }
}
