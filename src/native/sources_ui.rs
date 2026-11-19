use super::*;
use controls::{self as ctl, ButtonStyle, Icon, muted, text_color};

impl Megalodon {
    pub(super) fn close_source_editor(&mut self) {
        self.source_editor.open = false;
        self.source_editor.existing = false;
        self.source_editor.error = None;
        self.source_editor
            .cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.source_editor.request = self.source_editor.request.wrapping_add(1);
        self.source_editor.busy = false;
    }
    pub(super) fn reset_library(&mut self, ctx: &Context) {
        self.stop();
        self.source_cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.source_cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.session_epoch = self.session_epoch.wrapping_add(1);
        self.metadata.patches.clear();
        self.metadata.uploads.clear();
        self.metadata.uploading = None;
        self.metadata.picker = None;
        self.metadata.busy = false;
        self.metadata.status.clear();
        let request = self.profiles.request.wrapping_add(1);
        self.profiles = profile_ui::Profiles::default();
        self.profiles.request = request;
        self.catalog = None;
        self.files = None;
        self.library = None;
        self.resume_items.clear();
        self.images.clear();
        self.profile = None;
        self.profile_id = None;
        self.avatar_picker = false;
        self.detail = None;
        self.episodes.clear();
        self.requested_detail.clear();
        self.search.clear();
        self.genre.clear();
        self.sort = 0;
        self.hero = 0;
        self.hero_old = None;
        self.hero_requested = None;
        self.hero_ready_at = None;
        self.scene = vec![egui::epaint::ClippedShape {
            clip_rect: ctx.content_rect(),
            shape: Shape::rect_filled(ctx.content_rect(), 0, ctl::cinema()),
        }];
        self.scene_key = None;
        self.outgoing_scene.clear();
        self.outgoing_textures.clear();
        self.rail_offsets.clear();
        self.rail_targets.clear();
        self.rail_limits.clear();
        self.error = None;
        let user = self.local_user();
        self.apply_profile_preferences(&user.id);
        self.load_local_portrait(ctx);
    }
    pub(super) fn select_source(&mut self, ctx: &Context, id: &str) {
        let Some(source) = self.sources.entries.iter().find(|s| s.id == id).cloned() else {
            return;
        };
        let config = match source.config() {
            Ok(c) => c,
            Err(e) => {
                self.error = Some(e);
                return;
            }
        };
        if let Some(config) = config {
            self.activate_profile(ctx, config);
            return;
        }
        self.stop();
        let previous = self.sources.active.clone();
        self.sources.active = Some(source.id.clone());
        if let Err(e) = self.sources.save() {
            self.sources.active = previous;
            self.error = Some(e);
            return;
        }
        self.reset_library(ctx);
        self.page = Page::Home;
        self.busy = true;
        self.services_status = String::new();
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        let epoch = self.session_epoch;
        let cancel = self.source_cancel.clone();
        let manager = self.services.clone();
        let handle = self.rt.handle().clone();
        let art_folder = self.metadata.settings.art_folder.clone();
        self.rt.spawn_blocking(move || {
            // Finish an old owned local service before releasing its mount. Manual services remain running.
            let result = handle
                .block_on(manager.lock().unwrap().stop())
                .and_then(|_| files::Files::load_with_art(source, cancel, &art_folder));
            let _ = tx.send(Message::Files(epoch, result));
            ctx.request_repaint();
        });
    }
    pub(super) fn refresh_files(&mut self, ctx: &Context) {
        let Some(files) = &self.files else { return };
        let user = self.local_user();
        let account = self.profile_key(&user.id);
        let mut catalog = files.catalog(user);
        for item in &mut catalog.items {
            self.local_state(item);
        }
        let mut resume: Vec<_> = files
            .items
            .iter()
            .filter_map(|item| {
                self.preferences
                    .history
                    .get(&format!("{account}|{}", item.id))
            })
            .filter(|w| history::resumable(&w.item))
            .cloned()
            .collect();
        // Next up: when the latest watched episode of a series is finished, the
        // following one (by season and number) continues the row, unless watched.
        let mut latest: std::collections::HashMap<&str, &history::Watch> = Default::default();
        for item in files.items.iter().filter(|i| i.kind == "Episode") {
            let (Some(series), Some(watch)) = (
                item.series_id.as_deref(),
                self.preferences
                    .history
                    .get(&format!("{account}|{}", item.id)),
            ) else {
                continue;
            };
            if latest.get(series).is_none_or(|w| watch.updated > w.updated) {
                latest.insert(series, watch);
            }
        }
        for (series, watch) in latest {
            if !watch.complete
                || resume
                    .iter()
                    .any(|w| w.item.series_id.as_deref() == Some(series))
            {
                continue;
            }
            let order = |i: &Item| {
                (
                    i.parent_index_number.unwrap_or(0),
                    i.index_number.unwrap_or(0),
                )
            };
            let current = order(&watch.item);
            let next = files
                .episodes(series)
                .into_iter()
                .filter(|e| e.parent_index_number.unwrap_or(0) > 0 || current.0 == 0)
                .find(|e| {
                    order(e) > current
                        && !self
                            .preferences
                            .history
                            .get(&format!("{account}|{}", e.id))
                            .is_some_and(|w| w.complete)
                });
            if let Some(next) = next {
                resume.push(history::Watch {
                    item: next,
                    position: 0.0,
                    duration: 0.0,
                    updated: watch.updated,
                    complete: false,
                });
            }
        }
        resume.sort_by_key(|w| std::cmp::Reverse(w.updated));
        self.resume_items = resume
            .into_iter()
            .map(|mut w| {
                if let Some(current) = files.items.iter().find(|i| i.id == w.item.id) {
                    w.item = current.clone();
                }
                self.local_state(&mut w.item);
                w.item
            })
            .collect();
        self.catalog = Some(Arc::new(catalog));
        let user = self.local_user();
        self.apply_profile_preferences(&user.id);
        self.load_local_portrait(ctx);
        self.busy = false;
    }
    pub(super) fn local_user(&self) -> jellyfin::User {
        jellyfin::User {
            id: self.preferences.local_profile.clone(),
            name: self
                .preferences
                .local_profiles
                .get(&self.preferences.local_profile)
                .cloned()
                .unwrap_or_else(|| "Mi perfil".into()),
            policy: jellyfin::Policy {
                is_administrator: true,
                ..Default::default()
            },
            ..Default::default()
        }
    }
    pub(super) fn open_file_detail(&mut self, id: &str) {
        let Some(files) = &self.files else { return };
        let Some(mut item) = files.items.iter().find(|i| i.id == id).cloned() else {
            return;
        };
        self.local_state(&mut item);
        self.episodes = files.episodes(id);
        self.previous = if self.page == Page::Detail {
            self.previous
        } else {
            self.page
        };
        self.page = Page::Detail;
        self.busy = false;
        self.requested_detail = id.into();
        self.detail = Some(item);
        self.season = self
            .episodes
            .iter()
            .filter_map(|i| i.parent_index_number)
            .find(|n| *n > 0)
            .unwrap_or(0);
        self.episode_page = 0;
        let mut episodes = std::mem::take(&mut self.episodes);
        for item in &mut episodes {
            self.local_state(item);
        }
        self.episodes = episodes;
    }
    pub(super) fn play_file(&mut self, id: &str, resume: bool) {
        let Some(files) = self.files.clone() else {
            return;
        };
        let Some(mut item) = files.items.iter().find(|i| i.id == id).cloned() else {
            return;
        };
        self.stop();
        self.local_state(&mut item);
        if let Some(series) = &item.series_id {
            self.episodes = files.episodes(series);
        }
        let start = if resume {
            item.user_data
                .as_ref()
                .filter(|d| !d.played)
                .map_or(0.0, |d| d.playback_position_ticks as f64 / 10_000_000.0)
        } else {
            0.0
        };
        match files.playback(item, start) {
            Ok(p) => self.start_playback(p),
            Err(e) => self.error = Some(e),
        }
    }
    pub(super) fn file_artwork(
        &mut self,
        ctx: &Context,
        item: &Item,
        kind: &str,
        width: u32,
    ) -> Option<TextureHandle> {
        let files = self.files.as_ref()?.clone();
        // An image saved from the app is shown from its local copy at once; the
        // library listing catches up on the next refresh.
        let uploaded = self
            .metadata
            .uploads
            .get(&(item.id.clone(), kind.into()))
            .cloned();
        let path = match &uploaded {
            Some(local) => local.to_string_lossy().into_owned(),
            None => files.pictures.get(&(item.id.clone(), kind.into()))?.clone(),
        };
        let base = format!(
            "{}|/Items/{}/Images/{kind}?SourceImage&{}&",
            files.source.id,
            item.id,
            sources::stable_id(&path)
        );
        let key = format!("{base}{width}");
        let cache = data::data_dir()
            .join("image-cache")
            .join(sources::stable_id(&format!(
                "{}|{}",
                base, files.art_revision
            )));
        self.load_library_image(ctx, key, path, uploaded.is_some(), cache, width)
    }
    /// Loads an image from the active file library (local path or its private
    /// stream), caching remote bytes on disk. `local` forces a disk read.
    pub(super) fn load_library_image(
        &mut self,
        ctx: &Context,
        key: String,
        path: String,
        local: bool,
        cache: std::path::PathBuf,
        width: u32,
    ) -> Option<TextureHandle> {
        let files = self.files.as_ref()?.clone();
        if let Some(slot) = self.images.get_mut(&key) {
            slot.touched = self.frame;
            return slot.texture.clone();
        }
        self.images.insert(
            key.clone(),
            ImageSlot {
                texture: None,
                ambient: None,
                failed: false,
                touched: self.frame,
                loaded_at: None,
            },
        );
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        let slots = self.image_slots.clone();
        self.rt.spawn(async move {
            let Ok(_permit) = slots.acquire_owned().await else {
                return;
            };
            let result = async {
                let bytes = if let Ok(bytes) = tokio::fs::read(&cache).await {
                    bytes
                } else if !local && let Some(stream) = &files.stream {
                    let auth = stream
                        .header
                        .strip_prefix("Authorization: ")
                        .ok_or("Imagen no disponible")?;
                    let client = reqwest::Client::builder()
                        .timeout(Duration::from_secs(15))
                        .redirect(reqwest::redirect::Policy::none())
                        .build()
                        .map_err(|_| "Imagen no disponible")?;
                    let mut response = client
                        .get(&path)
                        .header("Authorization", auth)
                        .send()
                        .await
                        .map_err(|_| "Imagen no disponible")?
                        .error_for_status()
                        .map_err(|_| "Imagen no disponible")?;
                    if response.content_length().unwrap_or(0) > 16 * 1024 * 1024 {
                        return Err("Imagen demasiado grande".into());
                    }
                    let mut bytes = vec![];
                    while let Some(chunk) =
                        response.chunk().await.map_err(|_| "Imagen no disponible")?
                    {
                        if bytes.len() + chunk.len() > 16 * 1024 * 1024 {
                            return Err("Imagen demasiado grande".into());
                        }
                        bytes.extend_from_slice(&chunk);
                    }
                    bytes
                } else {
                    if tokio::fs::metadata(&path)
                        .await
                        .map_err(|_| "Imagen no disponible")?
                        .len()
                        > 16 * 1024 * 1024
                    {
                        return Err("Imagen demasiado grande".into());
                    }
                    tokio::fs::read(&path)
                        .await
                        .map_err(|_| "Imagen no disponible")?
                };
                if bytes.len() > 16 * 1024 * 1024 {
                    return Err("Imagen demasiado grande".into());
                }
                tokio::task::spawn_blocking(move || {
                    let image = decode_fit(&bytes, width)?;
                    if files.stream.is_some() && !local && !cache.exists() {
                        if let Some(parent) = cache.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        let _ = std::fs::write(cache, bytes);
                    }
                    Ok(image)
                })
                .await
                .map_err(|_| "Imagen no disponible".to_owned())?
            }
            .await;
            let _ = tx.send(Message::Image(key, result));
            ctx.request_repaint();
        });
        None
    }
    pub(super) fn connections_ui(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Conexiones").size(21.0));
        ui.add_space(12.0);
        if self.sources.entries.is_empty() {
            ui.label(RichText::new("Conecta una carpeta, Drive o tu servidor.").color(muted()));
        }
        for source in self.sources.entries.clone() {
            let active = self.sources.active.as_deref() == Some(&source.id);
            let width = ui.available_width();
            let (row, _) = ui.allocate_exact_size(vec2(width, 80.0), Sense::hover());
            if active {
                ui.painter().rect_filled(
                    row.shrink2(vec2(0.0, 4.0)),
                    12,
                    Color32::from_white_alpha(8),
                );
            }
            let icon = match source.kind {
                sources::Kind::Folder { .. } => Icon::Folder,
                sources::Kind::Jellyfin { .. } => Icon::Server,
                _ => Icon::Cloud,
            };
            ctl::icon(
                ui.painter(),
                Rect::from_center_size(row.min + vec2(32.0, 39.0), Vec2::splat(26.0)),
                icon,
                if active { ctl::focus() } else { muted() },
            );
            let label = ui::paragraph(
                ui,
                &source.name,
                (width - 265.0).max(100.0),
                17.0,
                text_color(),
                1,
            );
            ui.painter()
                .galley(row.min + vec2(60.0, 14.0), label, text_color());
            let description = ui::paragraph(
                ui,
                &source.description(),
                (width - 265.0).max(100.0),
                13.0,
                muted(),
                1,
            );
            ui.painter()
                .galley(row.min + vec2(60.0, 40.0), description, muted());
            let button = Rect::from_min_size(
                pos2(row.right() - 176.0, row.top() + 17.0),
                vec2(116.0, 44.0),
            );
            ui.add_enabled_ui(!self.busy, |ui| {
                if ctl::button(
                    ui,
                    button,
                    if active { "En uso" } else { "Abrir" },
                    if active { Some(Icon::Check) } else { None },
                    if active {
                        ButtonStyle::Quiet
                    } else {
                        ButtonStyle::Soft
                    },
                )
                .clicked()
                    && !active
                {
                    self.select_source(ui.ctx(), &source.id);
                }
            });
            if ctl::icon_button(
                ui,
                Rect::from_min_size(
                    pos2(row.right() - 48.0, row.top() + 17.0),
                    Vec2::splat(44.0),
                ),
                Icon::Close,
                "Quitar conexión",
                false,
            )
            .clicked()
            {
                self.source_editor.remove = Some(source.id.clone());
            }
        }
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(184.0, 44.0), Sense::hover());
            if ctl::button(
                ui,
                r,
                "Añadir conexión",
                Some(Icon::Plus),
                ButtonStyle::Soft,
            )
            .clicked()
            {
                self.open_source_editor();
            }
            if self.catalog.is_some()
                && !self.busy
                && small_button(ui, "Actualizar biblioteca", false)
            {
                self.refresh(ui.ctx(), true);
            }
        });
        ui.add_space(28.0);
    }
    pub(super) fn remove_source_dialog(&mut self, ctx: &Context, id: &str) {
        let Some(source) = self.sources.entries.iter().find(|s| s.id == id).cloned() else {
            self.source_editor.remove = None;
            return;
        };
        let mut confirmed = false;
        let mut cancelled = false;
        let close = ctl::dialog(ctx, "remove-source", "Quitar conexión", 460.0, |ui| {
            ui.label(RichText::new(&source.name).size(17.0));
            ui.add_space(6.0);
            ui.label(
                RichText::new("Se quitará de Megalodon. Los archivos se conservarán.")
                    .color(muted())
                    .size(14.0),
            );
            ui.add_space(20.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let (r, _) = ui.allocate_exact_size(vec2(168.0, 44.0), Sense::hover());
                confirmed =
                    ctl::button(ui, r, "Quitar conexión", None, ButtonStyle::Primary).clicked();
                let (r, _) = ui.allocate_exact_size(vec2(112.0, 44.0), Sense::hover());
                cancelled = ctl::button(ui, r, "Cancelar", None, ButtonStyle::Quiet).clicked();
            });
        });
        if close || cancelled {
            self.source_editor.remove = None;
            return;
        }
        if !confirmed {
            return;
        }
        let active = self.sources.active.as_deref() == Some(&source.id);
        match self.sources.remove(&source.id) {
            Ok(()) => {
                if let sources::Kind::Jellyfin { server, .. } = &source.kind
                    && let Err(error) = accounts::forget_server(&data::data_dir(), server)
                {
                    self.error = Some(error);
                }
                if active {
                    self.reset_library(ctx);
                    self.page = Page::Settings;
                    self.busy = false;
                    self.server.clear();
                    let manager = self.services.clone();
                    let handle = self.rt.handle().clone();
                    self.rt.spawn_blocking(move || {
                        let _ = handle.block_on(manager.lock().unwrap().stop());
                    });
                    let _ = std::fs::remove_file(data::data_dir().join("connection.json"));
                }
            }
            Err(e) => self.error = Some(e),
        }
        self.source_editor.remove = None;
    }
    /// The body of the «Añadir conexión» dialog: source type, its fields and
    /// one primary action. The dialog keeps its size between types.
    pub(super) fn connection_editor(&mut self, ui: &mut Ui) {
        let width = ui.available_width();
        let (track, _) = ui.allocate_exact_size(vec2(width, 48.0), Sense::hover());
        ui.painter()
            .rect_filled(track, 14, Color32::from_white_alpha(10));
        let kinds = ["Carpeta", "Google Drive", "Jellyfin", "WebDAV"];
        let segment = (track.width() - 8.0) / kinds.len() as f32;
        for (i, label) in kinds.into_iter().enumerate() {
            let r = Rect::from_min_size(
                track.min + vec2(4.0 + i as f32 * segment, 4.0),
                vec2(segment, 40.0),
            );
            let mut cell = ui.new_child(UiBuilder::new().id_salt(("kind", i)).max_rect(r));
            if self.source_editor.busy || self.busy {
                cell.disable();
            }
            if ctl::button(
                &mut cell,
                r,
                label,
                None,
                if self.source_editor.kind == i {
                    ButtonStyle::Primary
                } else {
                    ButtonStyle::Quiet
                },
            )
            .clicked()
            {
                self.source_editor.kind = i;
                self.source_editor.existing = false;
                self.source_editor.error = None;
            }
        }
        ui.add_space(20.0);
        if self.source_editor.kind != 2 {
            field(
                ui,
                "Nombre (opcional)",
                &mut self.source_editor.name,
                "Mi biblioteca",
                false,
            );
        }
        match self.source_editor.kind {
            3 => self.webdav_form(ui),
            0 => self.folder_form(ui),
            1 if self.source_editor.existing => self.rclone_form(ui),
            1 => self.drive_form(ui),
            _ => self.jellyfin_form(ui),
        }
        if let Some(error) = &self.source_editor.error {
            ui.add_space(10.0);
            ui.label(RichText::new(error).color(ctl::focus()));
        }
    }
    fn folder_form(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Carpeta").size(13.0).color(muted()));
        ui.horizontal(|ui| {
            ui.add_sized(
                vec2((ui.available_width() - 132.0).max(160.0), 44.0),
                TextEdit::singleline(&mut self.source_editor.path)
                    .hint_text("D:\\Películas")
                    .margin(vec2(12.0, 12.0)),
            );
            let (r, _) = ui.allocate_exact_size(vec2(120.0, 44.0), Sense::hover());
            if ctl::button(ui, r, "Examinar…", Some(Icon::Folder), ButtonStyle::Soft).clicked() {
                let tx = self.tx.clone();
                let ctx = ui.ctx().clone();
                let request = self.source_editor.request;
                self.rt.spawn_blocking(move || {
                    let _ = tx.send(Message::FolderChosen(request, pick_folder()));
                    ctx.request_repaint();
                });
            }
        });
        let ready = !self.source_editor.path.trim().is_empty();
        if footer(ui, "Añadir carpeta", ready) {
            match sources::folder(
                std::path::Path::new(self.source_editor.path.trim()),
                &self.source_editor.name,
            ) {
                Ok(source) => self.add_source(ui.ctx(), source),
                Err(e) => self.source_editor.error = Some(e),
            }
        }
    }
    fn drive_form(&mut self, ui: &mut Ui) {
        ui.label(
            RichText::new("Permite leer tu biblioteca. No modifica ni elimina archivos de Drive.")
                .color(muted())
                .size(14.0),
        );
        if let Some(drive) = services::discover(&data::data_dir()).and_then(|s| s.drive)
            && self.source_editor.config.is_empty()
        {
            self.source_editor.config = drive.config.display().to_string();
            self.source_editor.remote = drive.remote;
        }
        ui.add_space(4.0);
        if !self.source_editor.busy
            && small_button(ui, "Usar una conexión de rclone existente", false)
        {
            self.source_editor.existing = true;
            self.source_editor.error = None;
        }
        if self.source_editor.busy {
            waiting(ui, "Completa la conexión en tu navegador…");
            if footer(ui, "Cancelar conexión", true) {
                self.source_editor
                    .cancel
                    .store(true, std::sync::atomic::Ordering::Relaxed);
            }
        } else if footer(ui, "Conectar con Google Drive", true) {
            self.source_editor.busy = true;
            self.source_editor.request = self.source_editor.request.wrapping_add(1);
            self.source_editor.cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let cancel = self.source_editor.cancel.clone();
            let request = self.source_editor.request;
            let name = self.source_editor.name.clone();
            let tx = self.tx.clone();
            let ctx = ui.ctx().clone();
            self.rt.spawn_blocking(move || {
                let result = drive::connect(&name, &cancel);
                let _ = tx.send(Message::SourceAdded(request, result));
                ctx.request_repaint();
            });
        }
    }
    fn rclone_form(&mut self, ui: &mut Ui) {
        let (r, _) = ui.allocate_exact_size(vec2(220.0, 40.0), Sense::hover());
        if ctl::button(
            ui,
            r,
            "Volver a Google Drive",
            Some(Icon::Back),
            ButtonStyle::Quiet,
        )
        .clicked()
        {
            self.source_editor.existing = false;
            self.source_editor.error = None;
        }
        ui.add_space(4.0);
        field(
            ui,
            "Archivo de configuración",
            &mut self.source_editor.config,
            "Ruta a rclone.conf",
            false,
        );
        field(
            ui,
            "Conexión y carpeta",
            &mut self.source_editor.remote,
            "drive:Películas",
            false,
        );
        let ready = !self.source_editor.config.trim().is_empty()
            && !self.source_editor.remote.trim().is_empty();
        if footer(ui, "Añadir conexión", ready) {
            match drive::existing(
                &self.source_editor.name,
                std::path::Path::new(self.source_editor.config.trim()),
                &self.source_editor.remote,
            ) {
                Ok(source) => self.add_source(ui.ctx(), source),
                Err(e) => self.source_editor.error = Some(e),
            }
        }
    }
    fn webdav_form(&mut self, ui: &mut Ui) {
        ui.label(
            RichText::new("La dirección WebDAV de tu NAS, Nextcloud o servidor de archivos.")
                .size(14.0)
                .color(muted()),
        );
        ui.add_space(4.0);
        field(
            ui,
            "Dirección de la carpeta",
            &mut self.source_editor.endpoint,
            "https://mi-servidor/archivos/",
            false,
        );
        field(
            ui,
            "Usuario (si lo necesita)",
            &mut self.source_editor.username,
            "Usuario del servidor",
            false,
        );
        field(
            ui,
            "Contraseña",
            &mut self.source_editor.password,
            "Contraseña del servidor",
            true,
        );
        if self.source_editor.busy {
            waiting(ui, "Comprobando conexión…");
            if footer(ui, "Cancelar", true) {
                self.source_editor
                    .cancel
                    .store(true, std::sync::atomic::Ordering::Relaxed);
            }
        } else if footer(
            ui,
            "Conectar",
            !self.source_editor.endpoint.trim().is_empty(),
        ) {
            self.source_editor.busy = true;
            self.source_editor.error = None;
            self.source_editor.request = self.source_editor.request.wrapping_add(1);
            self.source_editor.cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let cancel = self.source_editor.cancel.clone();
            let request = self.source_editor.request;
            let name = self.source_editor.name.clone();
            let endpoint = self.source_editor.endpoint.clone();
            let user = self.source_editor.username.clone();
            let password = std::mem::take(&mut self.source_editor.password);
            let tx = self.tx.clone();
            let ctx = ui.ctx().clone();
            self.rt.spawn_blocking(move || {
                let result = drive::connect_webdav(&name, &endpoint, &user, &password, &cancel);
                let _ = tx.send(Message::SourceAdded(request, result));
                ctx.request_repaint();
            });
        }
    }
    fn jellyfin_form(&mut self, ui: &mut Ui) {
        field(
            ui,
            "Servidor",
            &mut self.server,
            "https://jellyfin.ejemplo.com",
            false,
        );
        field(ui, "Usuario", &mut self.username, "", false);
        field(ui, "Contraseña", &mut self.password, "", true);
        if self.busy {
            waiting(ui, "Conectando…");
            return;
        }
        let ready = !self.server.trim().is_empty() && !self.username.trim().is_empty();
        if footer(ui, "Conectar", ready) {
            self.busy = true;
            let server = self.server.clone();
            let username = self.username.trim().to_owned();
            let password = std::mem::take(&mut self.password);
            let tx = self.tx.clone();
            let ctx = ui.ctx().clone();
            let epoch = self.session_epoch;
            self.rt.spawn(async move {
                let result = data::authenticate(server, username, password).await;
                let _ = tx.send(Message::Auth(epoch, result));
                ctx.request_repaint();
            });
        }
    }
    pub(super) fn add_source(&mut self, ctx: &Context, source: sources::Source) {
        let id = source.id.clone();
        match self.sources.add(source) {
            Ok(()) => {
                self.source_editor.open = false;
                self.source_editor.existing = false;
                self.select_source(ctx, &id);
            }
            Err(e) => self.source_editor.error = Some(e),
        }
    }
}
/// The dialog's primary action, aligned right after the fields.
fn footer(ui: &mut Ui, label: &str, enabled: bool) -> bool {
    ui.add_space(14.0);
    let width = (label.chars().count() as f32 * 8.2 + 40.0).max(120.0);
    let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::hover());
    let r = Rect::from_min_size(pos2(row.right() - width, row.top()), vec2(width, 44.0));
    let mut cell = ui.new_child(UiBuilder::new().id_salt(("footer", label)).max_rect(r));
    if !enabled {
        cell.disable();
    }
    ctl::button(&mut cell, r, label, None, ButtonStyle::Primary).clicked()
}
fn waiting(ui: &mut Ui, label: &str) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::hover());
        ctl::loading(ui, r.center(), 8.0);
        ui.label(RichText::new(label).color(muted()));
    });
}
fn small_button(ui: &mut Ui, label: &str, primary: bool) -> bool {
    let width = (label.chars().count() as f32 * 7.8 + 28.0).max(88.0);
    let (r, _) = ui.allocate_exact_size(vec2(width, 44.0), Sense::hover());
    ctl::button(
        ui,
        r,
        label,
        None,
        if primary {
            ButtonStyle::Primary
        } else {
            ButtonStyle::Quiet
        },
    )
    .clicked()
}
fn field(ui: &mut Ui, label: &str, value: &mut String, hint: &str, password: bool) {
    ui.label(RichText::new(label).size(13.0).color(muted()));
    ui.add_sized(
        vec2(ui.available_width(), 44.0),
        TextEdit::singleline(value)
            .password(password)
            .hint_text(hint)
            .margin(vec2(12.0, 12.0)),
    );
    ui.add_space(8.0);
}
/// Windows' own file picker, limited to images.
pub(super) fn pick_image() -> Option<std::path::PathBuf> {
    use windows_sys::Win32::UI::Controls::Dialogs::*;
    let filter: Vec<u16> = "Imágenes (JPG, PNG, WebP)\0*.jpg;*.jpeg;*.png;*.webp\0\0"
        .encode_utf16()
        .collect();
    let title: Vec<u16> = "Elige una imagen".encode_utf16().chain(Some(0)).collect();
    let mut buffer = vec![0u16; 32768];
    unsafe {
        let mut dialog: OPENFILENAMEW = std::mem::zeroed();
        dialog.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
        dialog.lpstrFilter = filter.as_ptr();
        dialog.lpstrFile = buffer.as_mut_ptr();
        dialog.nMaxFile = buffer.len() as u32;
        dialog.lpstrTitle = title.as_ptr();
        dialog.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR | OFN_EXPLORER;
        if GetOpenFileNameW(&mut dialog) == 0 {
            return None;
        }
    }
    let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    Some(std::path::PathBuf::from(String::from_utf16_lossy(
        &buffer[..len],
    )))
}
impl Megalodon {
    /// «+» in the image picker: choose a file, normalise it, save it in the
    /// library's image folder under the title's folder, and use it.
    pub(super) fn upload_artwork(&mut self, ctx: &Context, id: &str, kind: &str) {
        let Some(files) = self.files.clone() else {
            return;
        };
        let Some(relative) = files.art_target(id, kind) else {
            self.error = Some("No se encuentra este título en la biblioteca.".into());
            return;
        };
        self.metadata.uploading = Some((id.to_owned(), kind.to_owned()));
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        let cancel = self.source_cancel.clone();
        let (id, kind) = (id.to_owned(), kind.to_owned());
        self.rt.spawn_blocking(move || {
            let result = (|| -> Result<std::path::PathBuf, String> {
                let Some(chosen) = pick_image() else {
                    return Err(String::new());
                };
                let size = std::fs::metadata(&chosen)
                    .map_err(|_| "No se puede leer esa imagen.")?
                    .len();
                if size > 40 * 1024 * 1024 {
                    return Err("La imagen supera los 40 MB.".into());
                }
                let bytes = std::fs::read(&chosen).map_err(|_| "No se puede leer esa imagen.")?;
                let prepared = files::prepare_art(&bytes, &kind)?;
                let stamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_nanos());
                let ext = if kind == "Logo" { "png" } else { "jpg" };
                let local = data::data_dir().join("art-uploads").join(format!(
                    "{}.{ext}",
                    sources::stable_id(&format!("{}|{relative}|{stamp}", files.source.id))
                ));
                std::fs::create_dir_all(local.parent().unwrap())
                    .map_err(|_| "No se pudo preparar la imagen.")?;
                std::fs::write(&local, &prepared).map_err(|_| "No se pudo preparar la imagen.")?;
                files.save_art(&relative, &local, &cancel)?;
                Ok(local)
            })();
            let _ = tx.send(Message::ArtUploaded(id, kind, result));
            ctx.request_repaint();
        });
    }
    /// A frame from inside the video (at a fixed, title-specific point between 20 %
    /// and 70 %), extracted once in the background and kept on disk.
    /// Frames kept per title for the Seguir viendo hover.
    pub(super) const FRAME_SLOTS: usize = 4;
    fn frame_path(&self, item: &Item, slot: usize) -> Option<std::path::PathBuf> {
        let files = self.files.as_ref()?;
        let id = sources::stable_id(&format!("{}|{}", files.source.id, item.id));
        Some(
            data::data_dir()
                .join("frames")
                .join(format!("{id}-{slot}.jpg")),
        )
    }
    /// Slots already on disk; missing ones are extracted in the background, one
    /// title and one frame at a time, at spread-out points of the video.
    pub(super) fn ready_frames(&mut self, ctx: &Context, item: &Item) -> Vec<usize> {
        let Some(files) = self.files.clone() else {
            return vec![];
        };
        let ready: Vec<usize> = (0..Self::FRAME_SLOTS)
            .filter(|slot| self.frame_path(item, *slot).is_some_and(|p| p.is_file()))
            .collect();
        let busy = self.metadata.frames.values().any(|pending| *pending);
        let Some(slot) = (0..Self::FRAME_SLOTS).find(|slot| {
            !ready.contains(slot)
                && !self
                    .metadata
                    .frames
                    .contains_key(&format!("{}#{slot}", item.id))
        }) else {
            return ready;
        };
        if busy {
            return ready;
        }
        let Some(path) = self.frame_path(item, slot) else {
            return ready;
        };
        let Ok(playback) = files.playback(item.clone(), 0.0) else {
            return ready;
        };
        let key = format!("{}#{slot}", item.id);
        self.metadata.frames.insert(key.clone(), true);
        // 15 %, 35 %, 55 %, 75 % with a small per-title offset.
        let jitter = item
            .id
            .bytes()
            .fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32))
            % 8;
        let percent = 15 + slot as u32 * 20 + jitter;
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        let cancel = self.source_cancel.clone();
        self.rt.spawn_blocking(move || {
            let result = std::fs::create_dir_all(path.parent().unwrap())
                .map_err(|_| "No se pudo preparar el fotograma.".to_owned())
                .and_then(|_| {
                    player::Player::extract_frame(
                        &playback.url,
                        &playback.headers,
                        percent,
                        &path,
                        &cancel,
                    )
                });
            let _ = tx.send(Message::FrameReady(key, result));
            ctx.request_repaint();
        });
        ready
    }
    pub(super) fn frame_texture(
        &mut self,
        ctx: &Context,
        item: &Item,
        slot: usize,
    ) -> Option<TextureHandle> {
        let path = self.frame_path(item, slot)?;
        let width = artwork_width(ctx, 720);
        self.load_library_image(
            ctx,
            format!("frame:{}:{slot}", item.id),
            path.to_string_lossy().into_owned(),
            true,
            path,
            width,
        )
    }
    /// Asks Google for permission to save into a Drive connection made read-only.
    pub(super) fn allow_drive_writes(&mut self, ctx: &Context) {
        let Some(files) = &self.files else { return };
        let sources::Kind::Drive { config, .. } = &files.source.kind else {
            return;
        };
        let config = config.clone();
        self.metadata.status = "Autoriza el permiso en tu navegador…".into();
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        let cancel = self.source_cancel.clone();
        self.rt.spawn_blocking(move || {
            let _ = tx.send(Message::DriveWrites(drive::allow_writes(&config, &cancel)));
            ctx.request_repaint();
        });
    }
}
fn pick_folder() -> Option<std::path::PathBuf> {
    use windows_sys::Win32::{System::Com::*, UI::Shell::*};
    unsafe {
        if CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32) < 0 {
            return None;
        }
        let title: Vec<u16> = "Elige la carpeta de tu biblioteca"
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let info = BROWSEINFOW {
            lpszTitle: title.as_ptr(),
            ulFlags: BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE,
            ..std::mem::zeroed()
        };
        let pidl = SHBrowseForFolderW(&info);
        let mut buffer = vec![0u16; 32768];
        let selected = if !pidl.is_null()
            && SHGetPathFromIDListEx(pidl, buffer.as_mut_ptr(), buffer.len() as u32, 0) != 0
        {
            let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
            Some(std::path::PathBuf::from(String::from_utf16_lossy(
                &buffer[..len],
            )))
        } else {
            None
        };
        if !pidl.is_null() {
            CoTaskMemFree(pidl.cast());
        }
        CoUninitialize();
        selected
    }
}
