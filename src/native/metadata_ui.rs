use super::*;
use controls::{self as ctl, ButtonStyle, Icon, muted, text_color};

/// Picker tile that opens the file chooser instead of selecting an image.
const UPLOAD: &str = "upload";

impl Megalodon {
    pub(super) fn artwork_failed(&self, item: &Item, kind: &str) -> bool {
        let marker = format!("/Items/{}/Images/{kind}?", item.id);
        let external = self
            .metadata
            .patches
            .get(&item.id)
            .and_then(|p| p.images.get(kind));
        let mut failed = false;
        for (key, slot) in &self.images {
            if key.contains(&marker) || external.is_some_and(|url| key.ends_with(url)) {
                if !slot.failed {
                    return false;
                }
                failed = true;
            }
        }
        failed
    }
    pub(super) fn start_metadata(&mut self, ctx: &Context) {
        if self.metadata.busy || !self.metadata.settings.configured() {
            return;
        }
        let Some(catalog) = self.catalog.clone() else {
            return;
        };
        self.metadata.busy = true;
        self.metadata.status = "Completando fichas e imágenes…".into();
        let settings = self.metadata.settings.clone();
        let items = catalog.items.clone();
        // Seasons present in the library, so episode pictures are fetched only
        // for what the person actually has.
        let mut seasons: std::collections::HashMap<String, Vec<u32>> = Default::default();
        if let Some(files) = &self.files {
            for episode in files.items.iter().filter(|i| i.kind == "Episode") {
                if let (Some(series), Some(season)) =
                    (&episode.series_id, episode.parent_index_number)
                {
                    let list = seasons.entry(series.clone()).or_default();
                    if !list.contains(&season) {
                        list.push(season);
                    }
                }
            }
        }
        let cancel = self.source_cancel.clone();
        let epoch = self.session_epoch;
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        self.rt.spawn_blocking(move || {
            let result = metadata::run(items, seasons, settings, &cancel, |patch| {
                let _ = tx.send(Message::Metadata(epoch, patch));
                ctx.request_repaint();
            });
            let _ = tx.send(Message::MetadataDone(epoch, result));
            ctx.request_repaint();
        });
    }
    /// Restart enrichment once, after the dialog closes or on request, rather
    /// than on every switch or reorder.
    pub(super) fn apply_metadata_changes(&mut self, ctx: &Context) {
        if !self.metadata.dirty || self.metadata.busy {
            return;
        }
        self.metadata.dirty = false;
        self.metadata.patches.clear();
        // A different image folder needs the library read again (from cache).
        if let Some(files) = &self.files
            && files.art_folder != self.metadata.settings.art_folder
            && !self.metadata.settings.art_folder.is_empty()
        {
            let id = files.source.id.clone();
            self.select_source(ctx, &id);
            return;
        }
        if self.files.is_some() {
            self.refresh_files(ctx);
            self.start_metadata(ctx);
        } else {
            self.refresh(ctx, true);
        }
    }
    fn save_metadata_settings(&mut self) {
        match self.metadata.settings.save() {
            Ok(()) => self.metadata.dirty = true,
            Err(e) => self.metadata.status = e,
        }
    }
    /// The library's own image folder: what is found, how to organise it and,
    /// for Drive, the one-time permission to save from the app.
    fn image_folder_settings(&mut self, ui: &mut Ui) {
        let Some(files) = self.files.clone() else {
            return;
        };
        let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 84.0), Sense::hover());
        ui.painter()
            .rect_filled(row, 12, Color32::from_white_alpha(12));
        ctl::icon(
            ui.painter(),
            Rect::from_center_size(row.left_center() + vec2(30.0, 0.0), Vec2::splat(26.0)),
            Icon::Folder,
            ctl::focus(),
        );
        ui.painter().text(
            row.left_top() + vec2(60.0, 14.0),
            Align2::LEFT_TOP,
            "Tus imágenes",
            FontId::proportional(16.0),
            text_color(),
        );
        let titles = files.art_dirs.len();
        let status = if files.art_folder_exists {
            format!(
                "Carpeta «{}» · {titles} {} · tienen prioridad sobre todo lo demás",
                files.art_folder,
                if titles == 1 { "título" } else { "títulos" }
            )
        } else {
            format!(
                "Se creará «{}» al subir tu primera imagen con «+» en la ficha",
                files.art_folder
            )
        };
        let galley = ui::paragraph(ui, &status, row.width() - 80.0, 13.0, muted(), 2);
        ui.painter()
            .galley(row.left_top() + vec2(60.0, 40.0), galley, muted());
        ui.add_space(10.0);
        ui.label(
            RichText::new(format!(
                "Dentro de tu biblioteca: {}/Título (Año)/ con poster.jpg, background.jpg, logo.png y thumb.jpg (miniatura de Seguir viendo).",
                files.art_folder
            ))
            .size(13.0)
            .color(muted()),
        );
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Nombre de la carpeta")
                    .size(13.0)
                    .color(muted()),
            );
            let response = ui.add_sized(
                vec2(220.0, 40.0),
                TextEdit::singleline(&mut self.metadata.settings.art_folder)
                    .margin(vec2(12.0, 10.0))
                    .hint_text(format!("{} (automático)", files.art_folder)),
            );
            if response.lost_focus() {
                let clean = self
                    .metadata
                    .settings
                    .art_folder
                    .trim()
                    .trim_matches(['/', '\\'])
                    .to_owned();
                if clean.contains("..") || clean.contains(['<', '>', ':', '"', '|', '?', '*']) {
                    self.metadata.status = "Ese nombre de carpeta no es válido.".into();
                    self.metadata.settings.art_folder.clear();
                } else {
                    self.metadata.settings.art_folder = clean;
                    self.save_metadata_settings();
                }
            }
        });
        if matches!(files.source.kind, sources::Kind::Drive { ref config, .. } if config.starts_with(data::data_dir().join("drive")))
        {
            ui.add_space(6.0);
            let (r, _) = ui.allocate_exact_size(vec2(280.0, 40.0), Sense::hover());
            if ctl::button(
                ui,
                r,
                "Permitir guardar imágenes en Drive",
                Some(Icon::Cloud),
                ButtonStyle::Quiet,
            )
            .clicked()
            {
                self.allow_drive_writes(ui.ctx());
            }
        }
    }
    pub(super) fn metadata_settings(&mut self, ui: &mut Ui) {
        ui.label(
            RichText::new(
                "Gratis y sin cuentas: al conectar una biblioteca se completan carteles, fondos y logotipos que falten. Ordena la lista: el primero con una imagen adecuada tiene prioridad. Las coincidencias dudosas se ignoran.",
            )
            .size(14.0)
            .color(muted()),
        );
        ui.add_space(14.0);
        if self.files.is_some() {
            self.image_folder_settings(ui);
            ui.add_space(18.0);
            ui.label(RichText::new("Fuentes en internet").size(17.0));
            ui.add_space(6.0);
        }
        let busy = self.metadata.busy;
        let mut changed = false;
        let count = self.metadata.settings.providers.len();
        let mut swap = None;
        for index in 0..count {
            let entry = self.metadata.settings.providers[index];
            let provider = entry.provider;
            let usable = self.metadata.settings.has_key(provider);
            let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 68.0), Sense::hover());
            let tile = row.shrink2(vec2(0.0, 3.0));
            ui.painter().rect_filled(
                tile,
                12,
                Color32::from_white_alpha(if entry.enabled { 12 } else { 6 }),
            );
            ui.painter().text(
                tile.left_center() + vec2(18.0, 0.0),
                Align2::CENTER_CENTER,
                (index + 1).to_string(),
                FontId::proportional(14.0),
                muted(),
            );
            let name_color = if entry.enabled { text_color() } else { muted() };
            ui.painter().text(
                tile.left_top() + vec2(40.0, 12.0),
                Align2::LEFT_TOP,
                provider.name(),
                FontId::proportional(16.0),
                name_color,
            );
            let personal = match provider {
                metadata::Provider::Tmdb => !self.metadata.settings.tmdb.is_empty(),
                metadata::Provider::Fanart => !self.metadata.settings.fanart.is_empty(),
                _ => false,
            };
            let detail = if provider.needs_key() && !usable {
                "Opcional · requiere cuenta · ".to_owned() + provider.summary()
            } else if personal {
                "Tu clave · ".to_owned() + provider.summary()
            } else if provider.built_in() {
                "Incluido · ".to_owned() + provider.summary()
            } else if provider.needs_key() {
                "Autorizado · ".to_owned() + provider.summary()
            } else {
                "Sin clave · ".to_owned() + provider.summary()
            };
            let galley = ui::paragraph(ui, &detail, tile.width() - 210.0, 13.0, muted(), 1);
            ui.painter()
                .galley(tile.left_top() + vec2(40.0, 36.0), galley, muted());
            ui.add_enabled_ui(!busy, |ui| {
                for (offset, icon, label, target) in [
                    (144.0, Icon::Up, "Subir prioridad", index.checked_sub(1)),
                    (
                        104.0,
                        Icon::Down,
                        "Bajar prioridad",
                        Some(index + 1).filter(|i| *i < count),
                    ),
                ] {
                    let r = Rect::from_center_size(
                        pos2(tile.right() - offset, tile.center().y),
                        Vec2::splat(36.0),
                    );
                    let mut cell = ui.new_child(
                        UiBuilder::new()
                            .id_salt(("provider-move", index, offset as u32))
                            .max_rect(r),
                    );
                    if target.is_none() {
                        cell.disable();
                    }
                    if ctl::icon_button(
                        &mut cell,
                        r,
                        icon,
                        &format!("{label}: {}", provider.name()),
                        false,
                    )
                    .clicked()
                        && let Some(target) = target
                    {
                        swap = Some((index, target));
                    }
                }
                let mut enabled = entry.enabled;
                if ctl::switch(
                    ui,
                    Rect::from_center_size(
                        pos2(tile.right() - 40.0, tile.center().y),
                        vec2(44.0, 26.0),
                    ),
                    &mut enabled,
                    provider.name(),
                )
                .changed()
                {
                    self.metadata.settings.providers[index].enabled = enabled;
                    changed = true;
                }
            });
        }
        if let Some((a, b)) = swap {
            self.metadata.settings.providers.swap(a, b);
            changed = true;
        }
        ui.add_space(16.0);
        ui.label(RichText::new("Claves opcionales").size(17.0));
        ui.label(
            RichText::new("No hacen falta. Solo si quieres fichas en español de TMDB o fanart.tv.")
                .size(13.0)
                .color(muted()),
        );
        ui.add_space(6.0);
        for provider in [metadata::Provider::Tmdb, metadata::Provider::Fanart] {
            let connected = self.metadata.settings.has_key(provider);
            let input = if provider == metadata::Provider::Tmdb {
                &mut self.metadata.tmdb_input
            } else {
                &mut self.metadata.fanart_input
            };
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(match provider {
                        metadata::Provider::Tmdb => "TMDB · token de lectura",
                        _ => "fanart.tv · clave personal",
                    })
                    .size(13.0)
                    .color(muted()),
                );
                if let Some(url) = provider.key_url() {
                    ui.hyperlink_to(RichText::new("Obtener").size(13.0), url);
                }
            });
            ui.add_sized(
                vec2(ui.available_width(), 44.0),
                TextEdit::singleline(input)
                    .password(true)
                    .margin(vec2(12.0, 12.0))
                    .hint_text(if connected {
                        "Conectado · escribe para cambiarlo"
                    } else {
                        "Pega aquí la autorización"
                    }),
            );
            ui.add_space(6.0);
        }
        ui.add_space(6.0);
        if ctl::toggle(
            ui,
            &mut self.metadata.settings.replace_images,
            "Mejorar también las imágenes existentes",
        )
        .changed()
        {
            changed = true;
        }
        if changed {
            self.save_metadata_settings();
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(200.0, 44.0), Sense::hover());
            let mut cell = ui.new_child(UiBuilder::new().id_salt("metadata-apply").max_rect(r));
            if busy {
                cell.disable();
            }
            if ctl::button(
                &mut cell,
                r,
                "Guardar y completar",
                None,
                ButtonStyle::Primary,
            )
            .clicked()
            {
                let result = (|| -> Result<(), String> {
                    let mut settings = self.metadata.settings.clone();
                    for (input, destination, provider) in [
                        (
                            &mut self.metadata.tmdb_input,
                            &mut settings.tmdb,
                            metadata::Provider::Tmdb,
                        ),
                        (
                            &mut self.metadata.fanart_input,
                            &mut settings.fanart,
                            metadata::Provider::Fanart,
                        ),
                    ] {
                        let value = input.trim();
                        if !value.is_empty() {
                            if value.contains(['\r', '\n']) || value.len() > 4096 {
                                return Err("La autorización no tiene un formato válido.".into());
                            }
                            *destination = data::protect(value.as_bytes(), false)?;
                            if let Some(entry) = settings
                                .providers
                                .iter_mut()
                                .find(|e| e.provider == provider)
                            {
                                entry.enabled = true;
                            }
                        }
                    }
                    if !settings.configured() {
                        return Err("Activa al menos un proveedor.".into());
                    }
                    settings.save()?;
                    self.metadata.settings = settings;
                    self.metadata.tmdb_input.clear();
                    self.metadata.fanart_input.clear();
                    Ok(())
                })();
                match result {
                    Ok(()) => {
                        self.metadata.dirty = true;
                        self.apply_metadata_changes(ui.ctx());
                    }
                    Err(e) => self.metadata.status = e,
                }
            }
            if busy {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
                ctl::loading(ui, r.center(), 8.0);
            }
        });
        if !self.metadata.status.is_empty() {
            ui.label(RichText::new(&self.metadata.status).size(14.0));
        }
        ui.add_space(18.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Créditos:").size(13.0).color(muted()));
            ui.hyperlink_to(RichText::new("Metahub").size(13.0), "https://metahub.space");
            for provider in metadata::Provider::ALL {
                ui.hyperlink_to(
                    RichText::new(provider.name()).size(13.0),
                    provider.credits_url(),
                );
            }
        });
        ui.label(
            RichText::new("Imágenes de TVmaze bajo CC BY-SA.")
                .size(12.0)
                .color(muted()),
        );
        let logo_id = Id::new("tmdb-attribution");
        let logo = ui.ctx().data_mut(|d| d.get_temp::<TextureHandle>(logo_id));
        let logo = logo.or_else(|| {
            let image = decode_image(include_bytes!("../../static/providers/tmdb.png")).ok()?;
            let logo = ui.ctx().load_texture("TMDB", image, TextureOptions::LINEAR);
            ui.ctx().data_mut(|d| d.insert_temp(logo_id, logo.clone()));
            Some(logo)
        });
        if let Some(logo) = logo {
            let size = logo.size_vec2();
            ui.image((logo.id(), size * (96.0 / size.x)));
        }
        ui.label(
            RichText::new(
                "This product uses the TMDB API but is not endorsed or certified by TMDB.",
            )
            .size(12.0)
            .color(muted()),
        );
    }
    /// Whether the library itself supplies this kind of image for the title.
    fn has_local_art(&self, item: &Item, kind: &str) -> bool {
        if self
            .metadata
            .uploads
            .contains_key(&(item.id.clone(), kind.to_owned()))
        {
            return true;
        }
        if let Some(files) = &self.files {
            return files
                .pictures
                .contains_key(&(item.id.clone(), kind.to_owned()));
        }
        if kind == "Backdrop" {
            item.backdrop_image_tags.iter().any(|t| t != "metadata")
        } else {
            item.image_tags.get(kind).is_some_and(|t| t != "metadata")
        }
    }
    pub(super) fn has_artwork_choices(&self, id: &str) -> bool {
        // File libraries can always receive the person's own images.
        self.files.is_some()
            || self
                .metadata
                .patches
                .get(id)
                .is_some_and(|p| p.alternatives.values().any(|a| !a.is_empty()))
    }
    pub(super) fn open_artwork_picker(&mut self, id: &str, kind: usize) {
        self.metadata.picker = Some(id.to_owned());
        self.metadata.picker_kind = kind;
    }
    /// Saves an override (None = automatic) and refreshes everything showing the title.
    pub(super) fn choose_artwork(&mut self, id: &str, kind: &str, choice: Option<String>) {
        let entry = self.metadata.choices.entry(id.to_owned()).or_default();
        match choice {
            Some(choice) => {
                entry.insert(kind.to_owned(), choice);
            }
            None => {
                entry.remove(kind);
            }
        }
        if entry.is_empty() {
            self.metadata.choices.remove(id);
        }
        if let Err(e) = metadata::save_choices(&self.metadata.choices) {
            self.error = Some(e);
        }
        if let Some(patch) = self.metadata.patches.remove(id) {
            let patch = patch.with_choices(&self.metadata.choices);
            self.metadata.patches.insert(id.to_owned(), patch);
        }
        let item = self
            .catalog
            .as_ref()
            .and_then(|c| c.items.iter().find(|i| i.id == id))
            .or(self.detail.as_ref().filter(|i| i.id == id))
            .cloned();
        if let Some(mut item) = item {
            self.local_state(&mut item);
            self.update_item(&item);
        }
    }
    pub(super) fn artwork_picker(&mut self, ctx: &Context) {
        let Some(id) = self.metadata.picker.clone() else {
            return;
        };
        let patch = self.metadata.patches.get(&id).cloned().unwrap_or_default();
        let can_upload = self.files.is_some();
        let uploading = self
            .metadata
            .uploading
            .as_ref()
            .filter(|(item, _)| *item == id)
            .map(|(_, kind)| kind.clone());
        let item = self
            .catalog
            .as_ref()
            .and_then(|c| c.items.iter().find(|i| i.id == id))
            .or(self.detail.as_ref().filter(|i| i.id == id))
            .cloned();
        let Some(item) = item else {
            self.metadata.picker = None;
            return;
        };
        let kinds = [
            ("Primary", "Cartel"),
            ("Backdrop", "Fondo"),
            ("Logo", "Logotipo"),
        ];
        let mut picked: Option<(&str, Option<String>)> = None;
        let mut upload: Option<&str> = None;
        let close = ctl::dialog(ctx, "artwork-picker", "Imágenes", 820.0, |ui| {
            ui.label(
                RichText::new(format!(
                    "{} · Megalodon elige la mejor de forma automática; puedes fijar otra.",
                    item.name
                ))
                .size(14.0)
                .color(muted()),
            );
            ui.add_space(14.0);
            let width = ui.available_width();
            let (track, _) = ui.allocate_exact_size(vec2(width.min(540.0), 44.0), Sense::hover());
            ui.painter()
                .rect_filled(track, 13, Color32::from_white_alpha(10));
            let segment = (track.width() - 8.0) / kinds.len() as f32;
            for (index, (kind, label)) in kinds.iter().enumerate() {
                let r = Rect::from_min_size(
                    track.min + vec2(4.0 + index as f32 * segment, 4.0),
                    vec2(segment, 36.0),
                );
                let count = patch.alternatives.get(*kind).map_or(0, Vec::len);
                let mut cell =
                    ui.new_child(UiBuilder::new().id_salt(("art-kind", index)).max_rect(r));
                if count == 0 && !can_upload && !self.has_local_art(&item, kind) {
                    cell.disable();
                }
                if ctl::button(
                    &mut cell,
                    r,
                    label,
                    None,
                    if self.metadata.picker_kind == index {
                        ButtonStyle::Primary
                    } else {
                        ButtonStyle::Quiet
                    },
                )
                .clicked()
                {
                    self.metadata.picker_kind = index;
                }
            }
            ui.add_space(18.0);
            let (kind, _) = kinds[self.metadata.picker_kind.min(kinds.len() - 1)];
            let current = self
                .metadata
                .choices
                .get(&id)
                .and_then(|c| c.get(kind))
                .cloned();
            let (columns, aspect) = match kind {
                "Primary" => (5.0_f32, 1.5_f32),
                "Backdrop" => (3.0, 9.0 / 16.0),
                _ => (3.0, 0.42),
            };
            let gap = 14.0;
            let tile_w = ((width - gap * (columns - 1.0)) / columns).floor();
            let tile_h = (tile_w * aspect).round();
            // Tiles: automatic recommendation, the library's own image, then every alternative.
            let mut tiles: Vec<(Option<String>, String, String, Option<String>)> = vec![];
            tiles.push((
                None,
                "Automático".into(),
                "Recomendada".into(),
                patch
                    .auto_images
                    .get(kind)
                    .cloned()
                    .or_else(|| self.has_local_art(&item, kind).then(|| "local".into())),
            ));
            if can_upload {
                tiles.push((
                    Some(UPLOAD.into()),
                    "Subir imagen".into(),
                    if uploading.as_deref() == Some(kind) {
                        "Guardando…".into()
                    } else {
                        "Desde tu equipo".into()
                    },
                    None,
                ));
            }
            if self.has_local_art(&item, kind) {
                tiles.push((
                    Some(metadata::LOCAL_CHOICE.into()),
                    "Original".into(),
                    "De tu biblioteca".into(),
                    Some("local".into()),
                ));
            }
            for alternative in patch.alternatives.get(kind).into_iter().flatten() {
                let lang = match alternative.lang.as_str() {
                    "" | "00" | "null" => "sin texto".to_owned(),
                    other => other.to_uppercase(),
                };
                tiles.push((
                    Some(alternative.url.clone()),
                    alternative.source.clone(),
                    if alternative.width == 0 {
                        "Tu elección".to_owned()
                    } else {
                        format!("{lang} · {}×{}", alternative.width, alternative.height)
                    },
                    Some(alternative.url.clone()),
                ));
            }
            let rows = tiles.len().div_ceil(columns as usize);
            let (grid, _) = ui.allocate_exact_size(
                vec2(width, rows as f32 * (tile_h + 52.0 + gap)),
                Sense::hover(),
            );
            for (index, (choice, title, detail, preview)) in tiles.into_iter().enumerate() {
                let column = (index % columns as usize) as f32;
                let row = (index / columns as usize) as f32;
                let tile = Rect::from_min_size(
                    grid.min + vec2(column * (tile_w + gap), row * (tile_h + 52.0 + gap)),
                    vec2(tile_w, tile_h),
                );
                if !ui.is_rect_visible(tile.expand(60.0)) {
                    continue;
                }
                let selected = choice == current;
                let response = ui
                    .interact(
                        tile.union(tile.translate(vec2(0.0, 46.0))),
                        Id::new(("art-tile", kind, index)),
                        Sense::click(),
                    )
                    .on_hover_cursor(CursorIcon::PointingHand);
                response.widget_info(|| {
                    WidgetInfo::selected(
                        WidgetType::Button,
                        true,
                        selected,
                        format!("{title}, {detail}"),
                    )
                });
                ui.painter().rect_filled(
                    tile,
                    10,
                    if kind == "Logo" {
                        Color32::from_rgb(38, 41, 48)
                    } else {
                        ctl::screen()
                    },
                );
                let texture = match preview.as_deref() {
                    Some("local") => {
                        if self.files.is_some() {
                            self.file_artwork(ctx, &item, kind, 480)
                        } else {
                            None
                        }
                    }
                    Some(url) => self.metadata_artwork(ctx, url, 480),
                    None => None,
                };
                if let Some(texture) = texture {
                    let size = texture.size_vec2();
                    let fit = if kind == "Logo" {
                        let inner = tile.shrink(14.0);
                        Rect::from_center_size(
                            inner.center(),
                            size * (inner.width() / size.x).min(inner.height() / size.y),
                        )
                    } else {
                        tile
                    };
                    ui.put(
                        fit,
                        egui::Image::new(&texture)
                            .fit_to_exact_size(fit.size())
                            .maintain_aspect_ratio(kind == "Logo")
                            .corner_radius(if kind == "Logo" { 0 } else { 10 }),
                    );
                } else if preview.is_some() {
                    ctl::loading(ui, tile.center(), 9.0);
                } else if choice.as_deref() == Some(UPLOAD) {
                    if uploading.as_deref() == Some(kind) {
                        ctl::loading(ui, tile.center(), 11.0);
                    } else {
                        ui.painter().rect_stroke(
                            tile.shrink(1.0),
                            10,
                            Stroke::new(1.5_f32, Color32::from_white_alpha(40)),
                            StrokeKind::Inside,
                        );
                        ui.painter().circle_filled(
                            tile.center(),
                            24.0,
                            Color32::from_white_alpha(18),
                        );
                        ctl::icon(
                            ui.painter(),
                            Rect::from_center_size(tile.center(), Vec2::splat(22.0)),
                            Icon::Plus,
                            text_color(),
                        );
                    }
                } else {
                    ctl::icon(
                        ui.painter(),
                        Rect::from_center_size(tile.center(), Vec2::splat(26.0)),
                        Icon::Picture,
                        muted(),
                    );
                }
                let hover = ctl::motion(
                    ctx,
                    response.id.with("hover"),
                    response.hovered() || response.has_focus(),
                    0.14,
                );
                if selected || hover > 0.0 {
                    ui.painter().rect_stroke(
                        tile.expand(3.0),
                        12,
                        Stroke::new(
                            2.0_f32,
                            if selected {
                                ctl::focus()
                            } else {
                                Color32::from_white_alpha((140.0 * hover) as u8)
                            },
                        ),
                        StrokeKind::Outside,
                    );
                }
                if selected {
                    let badge = tile.right_top() + vec2(-16.0, 16.0);
                    ui.painter().circle_filled(badge, 11.0, ctl::focus());
                    ctl::icon(
                        ui.painter(),
                        Rect::from_center_size(badge, Vec2::splat(14.0)),
                        Icon::Check,
                        ctl::cinema(),
                    );
                }
                ui.painter().text(
                    tile.left_bottom() + vec2(0.0, 10.0),
                    Align2::LEFT_TOP,
                    &title,
                    FontId::proportional(14.0),
                    text_color(),
                );
                ui.painter().text(
                    tile.left_bottom() + vec2(0.0, 29.0),
                    Align2::LEFT_TOP,
                    &detail,
                    FontId::proportional(12.0),
                    muted(),
                );
                if response.clicked() && !selected {
                    if choice.as_deref() == Some(UPLOAD) {
                        if uploading.is_none() {
                            upload = Some(kind);
                        }
                    } else {
                        picked = Some((kind, choice));
                    }
                }
            }
        });
        if let Some((kind, choice)) = picked {
            self.choose_artwork(&id, kind, choice);
        }
        if let Some(kind) = upload {
            self.upload_artwork(ctx, &id, kind);
        }
        if close {
            self.metadata.picker = None;
        }
    }
    pub(super) fn metadata_artwork(
        &mut self,
        ctx: &Context,
        url: &str,
        width: u32,
    ) -> Option<TextureHandle> {
        if !metadata::allowed_image(url) {
            return None;
        }
        let key = format!("metadata-image:{width}:{url}");
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
        let url = url.to_owned();
        self.rt.spawn(async move {
            let Ok(_permit) = slots.acquire_owned().await else {
                return;
            };
            let cache = data::data_dir()
                .join("image-cache")
                .join(sources::stable_id(&url));
            let result = async {
                let bytes = if let Ok(bytes) = tokio::fs::read(&cache).await {
                    bytes
                } else {
                    // This client has NO library or provider credentials. Only the public image CDN is reachable;
                    // redirects are followed only between allowed image hosts (Metahub serves from a mirror).
                    let client = reqwest::Client::builder()
                        .timeout(Duration::from_secs(18))
                        .redirect(reqwest::redirect::Policy::custom(|attempt| {
                            if attempt.previous().len() < 3
                                && metadata::allowed_image(attempt.url().as_str())
                            {
                                attempt.follow()
                            } else {
                                attempt.stop()
                            }
                        }))
                        .build()
                        .map_err(|_| "Imagen no disponible")?;
                    let first = client.get(&url).send().await;
                    let retry = first
                        .as_ref()
                        .map_or(true, |r| r.status().is_server_error());
                    let response = if retry {
                        tokio::time::sleep(Duration::from_millis(750)).await;
                        client.get(&url).send().await
                    } else {
                        first
                    };
                    let mut response = response
                        .map_err(|_| "Imagen no disponible")?
                        .error_for_status()
                        .map_err(|_| "Imagen no disponible")?;
                    let mut bytes = vec![];
                    while let Some(chunk) =
                        response.chunk().await.map_err(|_| "Imagen incompleta")?
                    {
                        if bytes.len() + chunk.len() > 16 * 1024 * 1024 {
                            return Err("Imagen demasiado grande".into());
                        }
                        bytes.extend_from_slice(&chunk);
                    }
                    if let Some(parent) = cache.parent() {
                        let _ = tokio::fs::create_dir_all(parent).await;
                    }
                    let _ = tokio::fs::write(&cache, &bytes).await;
                    bytes
                };
                tokio::task::spawn_blocking(move || {
                    decode_fit(&bytes, width).inspect_err(|_| {
                        let _ = std::fs::remove_file(&cache);
                    })
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
}
