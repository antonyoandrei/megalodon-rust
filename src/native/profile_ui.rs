use super::controls::{self as ctl, ButtonStyle, Icon, focus, muted, text_color};
use super::*;

/// Built-in avatars: a colour with the profile's initial. Generic, light and
/// free of third-party characters; the person's own images come from the library.
pub(super) const AVATAR_COLORS: [(&str, [u8; 3]); 8] = [
    ("Azul", [76, 150, 232]),
    ("Turquesa", [38, 166, 154]),
    ("Verde", [92, 170, 92]),
    ("Ámbar", [222, 152, 48]),
    ("Coral", [228, 104, 88]),
    ("Rosa", [210, 92, 150]),
    ("Violeta", [140, 104, 214]),
    ("Grafito", [84, 90, 104]),
];
/// Category shown while the library has no avatars yet.
pub(super) const DEFAULT_CATEGORY: &str = "Mis avatares";
/// Gallery choice meaning «no photo»: the profile's automatic colour.
pub(super) const AUTO_AVATAR: &str = "auto";
fn color_index(id: &str) -> Option<usize> {
    id.strip_prefix("color-")?
        .parse::<usize>()
        .ok()
        .filter(|i| *i < AVATAR_COLORS.len())
}

/// Library images fill the circle (cover), centred.
pub(super) fn avatar_image(ui: &Ui, texture: &TextureHandle, rect: Rect, alpha: f32) {
    let size = texture.size_vec2();
    let cover = size * (rect.width() / size.x).max(rect.height() / size.y);
    let image_min = rect.center() - cover / 2.0;
    ui.painter().circle_filled(
        rect.center(),
        rect.width() / 2.0,
        Color32::from_rgb(32, 44, 61).gamma_multiply(alpha),
    );
    let mut mesh = egui::Mesh::with_texture(texture.id());
    let color = Color32::WHITE.gamma_multiply(alpha);
    let vertex = |pos: Pos2| egui::epaint::Vertex {
        pos,
        uv: pos2(
            (pos.x - image_min.x) / cover.x,
            (pos.y - image_min.y) / cover.y,
        ),
        color,
    };
    mesh.vertices.push(vertex(rect.center()));
    for index in 0..=64 {
        let angle = index as f32 / 64.0 * std::f32::consts::TAU;
        mesh.vertices.push(vertex(
            rect.center() + vec2(angle.cos(), angle.sin()) * (rect.width() / 2.0),
        ));
        if index > 0 {
            mesh.add_triangle(0, index, index + 1);
        }
    }
    ui.painter().add(Shape::mesh(mesh));
}

/// Paints any avatar choice: a library image, a colour with the initial, or a
/// neutral initial while nothing is chosen or an image is unavailable.
pub(super) fn paint_avatar(
    ui: &Ui,
    id: Option<&str>,
    texture: Option<&TextureHandle>,
    name: &str,
    rect: Rect,
    alpha: f32,
) {
    if let Some(texture) = texture {
        avatar_image(ui, texture, rect, alpha);
        return;
    }
    let fill = id.and_then(color_index).map_or(ctl::screen(), |i| {
        let [r, g, b] = AVATAR_COLORS[i].1;
        Color32::from_rgb(r, g, b)
    });
    ui.painter().circle_filled(
        rect.center(),
        rect.width() / 2.0,
        fill.gamma_multiply(alpha),
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        name.chars()
            .next()
            .unwrap_or('M')
            .to_uppercase()
            .to_string(),
        FontId::proportional(rect.width() * 0.42),
        Color32::WHITE.gamma_multiply(alpha),
    );
}

impl Megalodon {
    pub(super) fn current_user(&self) -> jellyfin::User {
        self.local_user()
    }

    pub(super) fn identity_key(&self, user: &str) -> String {
        format!("local|{user}")
    }

    pub(super) fn apply_profile_preferences(&mut self, user: &str) {
        let key = self.identity_key(user);
        if self.active_preferences == key {
            return;
        }
        if !self.active_preferences.is_empty() {
            self.preferences.remember_settings(&self.active_preferences);
        }
        self.preferences.activate_settings(&key);
        self.auto_next = self.preferences.auto_next;
        self.active_preferences = key;
    }

    pub(super) fn save_preferences(&mut self) -> Result<(), String> {
        if !self.active_preferences.is_empty() {
            self.preferences.remember_settings(&self.active_preferences);
        }
        data::save_json("preferences.json", &self.preferences)
    }

    pub(super) fn load_local_portrait(&mut self, ctx: &Context) {
        let user = self.local_user();
        let key = format!("local|{}", user.id);
        if !self.preferences.avatars.contains_key(&key) {
            let legacy = self
                .preferences
                .avatars
                .get(&self.profile_key(&user.id))
                .cloned()
                .or_else(|| {
                    self.preferences
                        .avatars
                        .iter()
                        .filter(|(k, _)| k.ends_with(&format!("|{}", user.id)))
                        .min_by_key(|(k, _)| *k)
                        .map(|(_, id)| id.clone())
                });
            if let Some(id) = legacy {
                self.preferences.avatars.insert(key.clone(), id);
            }
        }
        self.profile_id = self.preferences.avatars.get(&key).cloned();
        // A character from the retired built-in set becomes the image with the
        // same name in the person's avatars folder, once the library is read.
        if let Some(legacy) = self.profile_id.clone()
            && let Some(found) = self.resolve_legacy_avatar(&legacy)
        {
            self.preferences.avatars.insert(key, found.clone());
            let _ = self.save_preferences();
            self.profile_id = Some(found);
        }
        self.profile = self
            .profile_id
            .clone()
            .and_then(|id| self.avatar_texture(ctx, &id));
    }

    pub(super) fn profile_settings_ui(&mut self, ui: &mut Ui) {
        let user = self.current_user();
        let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 88.0), Sense::hover());
        self.user_portrait(
            ui,
            &user,
            Rect::from_min_size(row.min, Vec2::splat(72.0)),
            1.0,
        );
        let label = super::ui::paragraph(
            ui,
            &user.name,
            (row.width() - 244.0).max(120.0),
            24.0,
            text_color(),
            1,
        );
        ui.painter()
            .galley(row.min + vec2(92.0, 7.0), label, text_color());
        super::ui::text(
            ui.painter(),
            row.min + vec2(92.0, 42.0),
            "Perfil de Megalodon",
            14.0,
            muted(),
        );
        let r = Rect::from_min_size(
            pos2(row.right() - 140.0, row.top() + 14.0),
            vec2(140.0, 44.0),
        );
        if ctl::button(ui, r, "Personalizar", None, ButtonStyle::Soft).clicked() {
            self.open_avatar_carousel();
        }
        ui.add_space(12.0);
    }

    pub(super) fn profile_key(&self, user: &str) -> String {
        format!(
            "{}|{user}",
            self.library.as_ref().map_or_else(
                || self.sources.active.as_deref().unwrap_or("local"),
                |l| l.config.base_url.trim_end_matches('/')
            )
        )
    }

    fn resolve_legacy_avatar(&self, id: &str) -> Option<String> {
        if id.starts_with("color-") || id.starts_with("library:") {
            return None;
        }
        let files = self.files.as_ref()?;
        let name = [
            "dragon-ball-",
            "naruto-",
            "pokemon-",
            "marvel-",
            "dc-",
            "celebrities-",
        ]
        .iter()
        .find_map(|prefix| id.strip_prefix(prefix))
        .unwrap_or(id);
        let key = |s: &str| -> String {
            s.chars()
                .filter(|c| c.is_alphanumeric())
                .flat_map(char::to_lowercase)
                .collect()
        };
        let wanted = key(name);
        files
            .avatars
            .iter()
            .find(|a| key(&a.label) == wanted)
            .map(|a| a.id.clone())
    }
    /// Every avatar on offer: the person's library images by category, then colours.
    pub(super) fn avatar_options(&self) -> Vec<files::LibraryAvatar> {
        let mut options: Vec<files::LibraryAvatar> = vec![];
        if let Some(files) = &self.files {
            options.extend(files.avatars.iter().cloned());
        }
        for avatar in &self.profiles.uploaded_avatars {
            if !options.iter().any(|a| a.id == avatar.id) {
                options.push(avatar.clone());
            }
        }
        options.sort_by(|a, b| {
            (a.category.to_lowercase(), a.label.to_lowercase())
                .cmp(&(b.category.to_lowercase(), b.label.to_lowercase()))
        });
        options
    }

    /// Without a chosen photo each profile gets its own colour: profiles are
    /// numbered in order, so no two of the first eight share one.
    pub(super) fn auto_avatar(&self, user_id: &str) -> String {
        let index = self
            .preferences
            .local_profiles
            .keys()
            .position(|id| id == user_id)
            .unwrap_or(0);
        format!("color-{}", index % AVATAR_COLORS.len())
    }

    /// The id to paint: the chosen image when it is loaded, else the automatic colour.
    pub(super) fn displayed_avatar(
        &self,
        user_id: &str,
        chosen: Option<&str>,
        loaded: bool,
    ) -> String {
        match chosen {
            Some(id) if loaded || id.starts_with("color-") => id.to_owned(),
            _ => self.auto_avatar(user_id),
        }
    }

    /// Texture for a library avatar; colours are drawn, never loaded.
    pub(super) fn avatar_texture(&mut self, ctx: &Context, id: &str) -> Option<TextureHandle> {
        if !id.starts_with("library:") {
            return None;
        }
        let avatar = self.avatar_options().into_iter().find(|a| a.id == id)?;
        let files = self.files.as_ref()?.clone();
        let cache = data::data_dir()
            .join("image-cache")
            .join(sources::stable_id(&format!(
                "avatar|{}|{}|{}",
                files.source.id, avatar.id, files.art_revision
            )));
        // Decoded once at the largest size it is shown, in physical pixels, with a
        // high-quality filter: small circles stay crisp instead of soft or jagged.
        let width = artwork_width(ctx, 128);
        self.load_library_image(
            ctx,
            format!("avatar:{id}"),
            avatar.location,
            avatar.local,
            cache,
            width,
        )
    }

    fn choose_avatar(&mut self, ctx: &Context, id: &str) -> bool {
        let user = self.current_user();
        let key = self.identity_key(&user.id);
        if id == AUTO_AVATAR {
            self.preferences.avatars.remove(&key);
            if let Err(error) = self.save_preferences() {
                self.error = Some(error);
                return false;
            }
            self.profile = None;
            self.profile_id = None;
            return true;
        }
        if !self.avatar_options().iter().any(|a| a.id == id) {
            return false;
        }
        let id = id.to_owned();
        let previous = self.preferences.avatars.insert(key.clone(), id.clone());
        if let Err(error) = self.save_preferences() {
            match previous {
                Some(previous) => {
                    self.preferences.avatars.insert(key, previous);
                }
                None => {
                    self.preferences.avatars.remove(&key);
                }
            }
            self.error = Some(error);
            return false;
        }
        self.profile = self.avatar_texture(ctx, &id);
        self.profile_id = Some(id);
        true
    }

    /// «+» in the avatar gallery: square-crop an image and save it in the
    /// library's avatars folder under the chosen category.
    fn upload_avatar(&mut self, ctx: &Context, category: &str) {
        let Some(files) = self.files.clone() else {
            return;
        };
        let category = if category.is_empty() {
            DEFAULT_CATEGORY.to_owned()
        } else {
            category.to_owned()
        };
        self.profiles.avatar_uploading = true;
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        let cancel = self.source_cancel.clone();
        self.rt.spawn_blocking(move || {
            let result = (|| -> Result<files::LibraryAvatar, String> {
                let Some(chosen) = super::sources_ui::pick_image() else {
                    return Err(String::new());
                };
                let bytes = std::fs::read(&chosen).map_err(|_| "No se puede leer esa imagen.")?;
                if bytes.len() > 40 * 1024 * 1024 {
                    return Err("La imagen supera los 40 MB.".into());
                }
                let prepared = files::prepare_avatar(&bytes)?;
                let name = chosen
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "Avatar".into());
                let relative = files.avatar_target(&category, &name);
                let local = data::data_dir().join("art-uploads").join(format!(
                    "{}.png",
                    sources::stable_id(&format!("{}|{relative}", files.source.id))
                ));
                std::fs::create_dir_all(local.parent().unwrap())
                    .map_err(|_| "No se pudo preparar el avatar.")?;
                std::fs::write(&local, &prepared).map_err(|_| "No se pudo preparar el avatar.")?;
                files.save_art(&relative, &local, &cancel)?;
                Ok(files::LibraryAvatar {
                    id: format!("library:{relative}"),
                    label: name.replace(['_', '-'], " "),
                    category,
                    location: local.to_string_lossy().into_owned(),
                    local: true,
                })
            })();
            let _ = tx.send(Message::AvatarUploaded(result));
            ctx.request_repaint();
        });
    }

    pub(super) fn avatar_uploaded(&mut self, result: Result<files::LibraryAvatar, String>) {
        self.profiles.avatar_uploading = false;
        match result {
            Ok(avatar) => {
                if let Some(files) = &self.files {
                    files::invalidate_inventory(&files.source.id);
                }
                self.profiles.avatar_category = avatar.category.clone();
                self.profiles.avatar_selected = Some(avatar.id.clone());
                self.profiles.uploaded_avatars.push(avatar);
            }
            Err(e) if e.is_empty() => {}
            Err(e) => self.error = Some(e),
        }
    }

    pub(super) fn activate_profile(&mut self, ctx: &Context, config: Config) {
        let next = match Library::new(config.clone()) {
            Ok(l) => Arc::new(l),
            Err(e) => {
                self.error = Some(e);
                return;
            }
        };
        self.stop(); // Capture the old profile/source before changing any credential or scope.
        if let Err(e) = data::save_connection(&config) {
            self.profiles.error = Some(e);
            return;
        }
        if let Err(e) = self.sources.remember(&config, "Jellyfin") {
            self.error = Some(e);
            return;
        }
        self.source_editor.open = false;
        self.source_cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.source_cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.metadata.patches.clear();
        self.metadata.busy = false;
        self.metadata.status.clear();
        self.files = None;
        self.session_epoch = self.session_epoch.wrapping_add(1);
        let request = self.profiles.request.wrapping_add(1);
        self.profiles = Profiles {
            request,
            ..Default::default()
        };
        self.library = Some(next);
        self.server = config.base_url;
        self.catalog = None;
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
        self.page = Page::Home;
        self.start_services(ctx);
    }

    pub(super) fn open_profiles(&mut self, _ctx: &Context) {
        self.page = Page::Profiles;
        self.avatar_picker = false;
        self.cancel_profile_form();
        self.profiles.users = self
            .preferences
            .local_profiles
            .iter()
            .map(|(id, name)| jellyfin::User {
                id: id.clone(),
                name: name.clone(),
                ..Default::default()
            })
            .collect();
    }

    fn select_profile(&mut self, ctx: &Context, user: jellyfin::User) {
        if self.current_user().id == user.id {
            self.page = Page::Home;
            return;
        }
        self.stop();
        let previous = std::mem::replace(&mut self.preferences.local_profile, user.id.clone());
        if let Err(e) = self.save_preferences() {
            self.preferences.local_profile = previous;
            self.profiles.error = Some(e);
            return;
        }
        self.apply_profile_preferences(&user.id);
        self.load_local_portrait(ctx);
        self.refresh_profile_state();
        self.page = Page::Home;
        self.detail = None;
        self.episodes.clear();
        self.rail_offsets.clear();
        self.rail_targets.clear();
    }

    fn user_portrait(&mut self, ui: &Ui, user: &jellyfin::User, rect: Rect, alpha: f32) {
        let avatar_id = self
            .preferences
            .avatars
            .get(&self.identity_key(&user.id))
            .cloned()
            .or_else(|| user.avatar_id.clone());
        let texture = match &avatar_id {
            Some(id) => self.avatar_texture(ui.ctx(), id),
            None => None,
        };
        let shown = self.displayed_avatar(&user.id, avatar_id.as_deref(), texture.is_some());
        paint_avatar(ui, Some(&shown), texture.as_ref(), &user.name, rect, alpha);
    }

    pub(super) fn profiles_ui(&mut self, ui: &mut Ui) {
        if self.avatar_picker {
            self.avatar_carousel(ui);
            return;
        }
        let current_user = self.current_user();
        let compact = ui.ctx().content_rect().height() < 680.0;
        let form = self.profiles.creating;
        ui.add_space(if compact && form {
            96.0
        } else if compact {
            104.0
        } else {
            ((ui.ctx().content_rect().height() - 520.0) * 0.34).clamp(128.0, 260.0)
        });
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(if self.profiles.creating {
                    "Añadir perfil"
                } else {
                    "¿Quién está viendo?"
                })
                .size(if compact && form { 28.0 } else { 36.0 })
                .color(text_color()),
            );
        });
        ui.add_space(if compact && form { 18.0 } else { 40.0 });
        if self.profiles.creating {
            self.profile_form(ui);
            return;
        }
        let mut users: Vec<_> = self.profiles.users.clone().into_iter().map(Some).collect();
        users.push(None); // Adding a person belongs beside the people, not in a toolbar.
        let compact = ui.available_width() < 900.0;
        let preferred_width = if compact { 144.0 } else { 176.0 };
        let row_height = if compact { 192.0 } else { 224.0 };
        let columns = ((ui.available_width() - 48.0) / preferred_width)
            .floor()
            .clamp(2.0, 6.0) as usize;
        let cell_width = preferred_width.min((ui.available_width() - 48.0) / columns as f32);
        for row in users.chunks(columns) {
            let row_width = row.len() as f32 * cell_width;
            let (row_rect, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), row_height), Sense::hover());
            let left = row_rect.center().x - row_width / 2.0;
            for (index, user) in row.iter().enumerate() {
                let hit = Rect::from_min_size(
                    pos2(left + index as f32 * cell_width, row_rect.top()),
                    vec2(cell_width, row_height - 16.0),
                );
                let Some(user) = user else {
                    let center = pos2(
                        hit.center().x,
                        hit.top() + if compact { 68.0 } else { 76.0 },
                    );
                    let r = Rect::from_center_size(
                        center,
                        Vec2::splat(if compact { 104.0 } else { 120.0 }),
                    );
                    let response = ui
                        .interact(r, Id::new("add-profile"), Sense::click())
                        .on_hover_cursor(CursorIcon::PointingHand);
                    response.widget_info(|| {
                        WidgetInfo::labeled(WidgetType::Button, true, "Añadir perfil")
                    });
                    let hover = ctl::motion(
                        ui.ctx(),
                        response.id.with("hover"),
                        response.hovered() || response.has_focus(),
                        0.16,
                    );
                    ui.painter().circle_filled(
                        center,
                        r.width() / 2.0,
                        Color32::from_white_alpha((12.0 + 12.0 * hover) as u8),
                    );
                    ctl::icon(
                        ui.painter(),
                        Rect::from_center_size(center, Vec2::splat(32.0)),
                        Icon::Plus,
                        muted().lerp_to_gamma(text_color(), hover),
                    );
                    if response.has_focus() {
                        ui.painter().circle_stroke(
                            center,
                            r.width() / 2.0 + 5.0,
                            Stroke::new(2.0_f32, focus()),
                        );
                    }
                    ui.painter().text(
                        pos2(
                            hit.center().x,
                            hit.top() + if compact { 142.0 } else { 164.0 },
                        ),
                        Align2::CENTER_CENTER,
                        "Añadir perfil",
                        FontId::proportional(if compact { 18.0 } else { 20.0 }),
                        muted(),
                    );
                    if response.clicked() && !self.profiles.busy {
                        self.cancel_profile_form();
                        self.profiles.creating = true;
                    }
                    continue;
                };
                let response = ui
                    .interact(
                        hit,
                        Id::new(("select-profile", &user.id)),
                        if self.profiles.busy {
                            Sense::hover()
                        } else {
                            Sense::click()
                        },
                    )
                    .on_hover_cursor(CursorIcon::PointingHand);
                response.widget_info(|| {
                    WidgetInfo::labeled(
                        WidgetType::Button,
                        !self.profiles.busy,
                        format!("Ver como {}", user.name),
                    )
                });
                let hover = ctl::motion(
                    ui.ctx(),
                    response.id.with("hover"),
                    response.hovered() || response.has_focus(),
                    0.16,
                );
                let pressed = response.is_pointer_button_down_on();
                let diameter = (if compact { 112.0 } else { 128.0 }) + 8.0 * hover
                    - if pressed { 5.0 } else { 0.0 };
                let circle = Rect::from_center_size(
                    pos2(
                        hit.center().x,
                        hit.top() + if compact { 68.0 } else { 76.0 },
                    ),
                    Vec2::splat(diameter),
                );
                self.user_portrait(ui, user, circle, 1.0);
                let active = user.id == current_user.id;
                if active || hover > 0.0 {
                    ui.painter().circle_stroke(
                        circle.center(),
                        diameter / 2.0 + 5.0,
                        Stroke::new(
                            2.0_f32,
                            if response.has_focus() {
                                focus()
                            } else {
                                text_color().gamma_multiply(if active { 0.75 } else { hover })
                            },
                        ),
                    );
                }
                let name = super::ui::paragraph(
                    ui,
                    &user.name,
                    cell_width - 16.0,
                    if compact { 18.0 } else { 20.0 },
                    text_color(),
                    1,
                );
                ui.painter().galley(
                    pos2(
                        hit.center().x - name.size().x / 2.0,
                        hit.top() + if compact { 142.0 } else { 164.0 } - name.size().y / 2.0,
                    ),
                    name,
                    text_color(),
                );
                if active {
                    ui.painter().text(
                        pos2(hit.center().x, hit.top() + row_height - 32.0),
                        Align2::CENTER_CENTER,
                        "En uso",
                        FontId::proportional(13.0),
                        muted(),
                    );
                }
                if response.clicked() {
                    self.select_profile(ui.ctx(), user.clone());
                }
            }
        }
        if let Some(error) = &self.profiles.error {
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(error).color(focus()));
            });
        }
        ui.add_space(8.0);
    }

    pub(super) fn cancel_profile_form(&mut self) {
        self.profiles.request = self.profiles.request.wrapping_add(1);
        self.profiles.busy = false;
        self.profiles.name.clear();
        self.profiles.creating = false;
        self.profiles.error = None;
    }

    fn profile_form(&mut self, ui: &mut Ui) {
        let width = 360.0_f32.min(ui.available_width() - 48.0);
        let left = (ui.available_width() - width) / 2.0;
        ui.scope_builder(
            UiBuilder::new().max_rect(Rect::from_min_size(
                ui.cursor().min + vec2(left, 0.0),
                vec2(width, 200.0),
            )),
            |ui| {
                ui.label("Nombre");
                ui.add_sized(
                    vec2(width, 46.0),
                    TextEdit::singleline(&mut self.profiles.name).margin(vec2(14.0, 13.0)),
                );
                ui.add_space(16.0);
                let (r, _) = ui.allocate_exact_size(vec2(width, 44.0), Sense::hover());
                if ctl::button(ui, r, "Crear perfil", None, ButtonStyle::Primary).clicked() {
                    let name = self.profiles.name.trim().to_owned();
                    if (1..=64).contains(&name.chars().count()) {
                        let id = format!(
                            "local-{}",
                            sources::stable_id(&format!(
                                "{}-{:?}",
                                name,
                                std::time::SystemTime::now()
                            ))
                        );
                        self.preferences
                            .local_profiles
                            .insert(id.clone(), name.clone());
                        match self.save_preferences() {
                            Ok(()) => {
                                self.cancel_profile_form();
                                let user = jellyfin::User {
                                    id,
                                    name,
                                    ..Default::default()
                                };
                                self.select_profile(ui.ctx(), user);
                                self.open_avatar_carousel();
                            }
                            Err(e) => {
                                self.preferences.local_profiles.remove(&id);
                                self.profiles.error = Some(e);
                            }
                        }
                    } else {
                        self.profiles.error =
                            Some("Escribe un nombre de hasta 64 caracteres.".into());
                    }
                }
                let (r, _) = ui.allocate_exact_size(vec2(width, 44.0), Sense::hover());
                if ctl::button(ui, r, "Volver a perfiles", None, ButtonStyle::Quiet).clicked() {
                    self.cancel_profile_form();
                }
                if let Some(error) = &self.profiles.error {
                    ui.label(RichText::new(error).color(focus()));
                }
            },
        );
    }

    pub(super) fn open_avatar_carousel(&mut self) {
        let options = self.avatar_options();
        let current = self
            .profile_id
            .clone()
            .filter(|id| options.iter().any(|a| &a.id == id));
        self.profiles.avatar_category = current
            .as_ref()
            .and_then(|id| options.iter().find(|a| &a.id == id))
            .or_else(|| options.first())
            .map(|a| a.category.clone())
            .unwrap_or_else(|| DEFAULT_CATEGORY.into());
        self.profiles.avatar_selected = current.or_else(|| Some(AUTO_AVATAR.into()));
        self.avatar_picker = true;
        self.page = Page::Settings;
        self.error = None;
    }

    pub(super) fn avatar_carousel(&mut self, ui: &mut Ui) {
        let options = self.avatar_options();
        let mut categories: Vec<String> = vec![];
        for avatar in &options {
            if !categories.contains(&avatar.category) {
                categories.push(avatar.category.clone());
            }
        }
        if categories.is_empty() {
            categories.push(DEFAULT_CATEGORY.into());
        }
        if !categories.contains(&self.profiles.avatar_category) {
            self.profiles.avatar_category = categories.first().cloned().unwrap_or_default();
        }
        let has_library = !options.is_empty();
        let auto = self.auto_avatar(&self.current_user().id);
        let can_upload = self.files.is_some();
        let user = self.current_user();
        let viewport = ui.ctx().content_rect();
        let compact = viewport.height() < 650.0 || ui.available_width() < 800.0;
        ui.add_space(if compact { 88.0 } else { 112.0 });
        let width = ui.available_width().min(1080.0);
        let left = ui.cursor().left() + (ui.available_width() - width) / 2.0;
        let area = Rect::from_min_size(
            pos2(left + 24.0, ui.cursor().top()),
            vec2(
                width - 48.0,
                viewport.height() - if compact { 108.0 } else { 140.0 },
            ),
        );
        ui.scope_builder(UiBuilder::new().max_rect(area), |ui| {
            let (head, _) = ui.allocate_exact_size(
                vec2(area.width(), if compact { 68.0 } else { 84.0 }),
                Sense::hover(),
            );
            let selected_id = self.profiles.avatar_selected.clone();
                        let selected_label = match selected_id.as_deref() {
                Some(AUTO_AVATAR) | None => "Automático".to_owned(),
                Some(id) => options
                    .iter()
                    .find(|a| a.id == id)
                    .map_or(String::new(), |a| a.label.clone()),
            };
            let portrait =
                Rect::from_min_size(head.min, Vec2::splat(if compact { 60.0 } else { 76.0 }));
            let texture = selected_id
                .as_deref()
                .and_then(|id| self.avatar_texture(ui.ctx(), id));
            let shown = if texture.is_some() {
                selected_id.clone().unwrap_or_else(|| auto.clone())
            } else {
                auto.clone()
            };
            paint_avatar(ui, Some(&shown), texture.as_ref(), &user.name, portrait, 1.0);
            let x = portrait.right() + 20.0;
            super::ui::text(
                ui.painter(),
                pos2(x, head.top() + 2.0),
                "Tu avatar",
                if compact { 24.0 } else { 30.0 },
                text_color(),
            );
            super::ui::text(
                ui.painter(),
                pos2(x, head.top() + if compact { 36.0 } else { 46.0 }),
                                &selected_label,
                15.0,
                muted(),
            );
            let save = Rect::from_min_size(
                pos2(head.right() - 132.0, head.top() + 10.0),
                vec2(132.0, 44.0),
            );
            if ctl::button(ui, save, "Guardar", Some(Icon::Check), ButtonStyle::Primary).clicked()
                && let Some(id) = &selected_id
                && self.choose_avatar(ui.ctx(), id)
            {
                self.avatar_picker = false;
            }
            let cancel = Rect::from_min_size(save.min - vec2(116.0, 0.0), vec2(104.0, 44.0));
            if ctl::button(ui, cancel, "Cancelar", None, ButtonStyle::Quiet).clicked() {
                self.avatar_picker = false;
            }
            ui.add_space(if compact { 8.0 } else { 20.0 });
            // Category tabs wrap onto extra rows when a library has many folders.
            let tab_width = (area.width() / categories.len().max(1) as f32).clamp(120.0, 200.0);
            let per_row = ((area.width() / tab_width).floor() as usize).max(1);
            let rows = categories.len().div_ceil(per_row);
            let (tabs, _) = ui.allocate_exact_size(
                vec2(area.width(), rows as f32 * 48.0),
                Sense::hover(),
            );
            for (i, label) in categories.iter().enumerate() {
                let r = Rect::from_min_size(
                    tabs.min
                        + vec2(
                            (i % per_row) as f32 * tab_width,
                            (i / per_row) as f32 * 48.0,
                        ),
                    vec2(tab_width - 4.0, 44.0),
                );
                if ctl::button(
                    ui,
                    r,
                    label,
                    None,
                    if &self.profiles.avatar_category == label {
                        ButtonStyle::Primary
                    } else {
                        ButtonStyle::Quiet
                    },
                )
                .clicked()
                {
                    self.profiles.avatar_category = label.clone();
                    self.profiles.avatar_category_changed = Some(Instant::now());
                }
            }
            if !has_library {
                ui.add_space(10.0);
                let folder = self
                    .files
                    .as_ref()
                    .map_or("posters".to_owned(), |f| f.art_folder.clone());
                ui.label(
                    RichText::new(if can_upload {
                        format!("Usa tus propias imágenes: súbelas con «+» o déjalas en {folder}/avatars/<categoría>/ dentro de tu biblioteca.")
                    } else {
                        "Conecta una carpeta, Drive o WebDAV para usar tus propias imágenes.".to_owned()
                    })
                    .size(13.0)
                    .color(muted()),
                );
            }
            ui.add_space(16.0);
            let alpha = if ctl::reduced(ui.ctx()) {
                1.0
            } else {
                self.profiles
                    .avatar_category_changed
                    .map_or(1.0, |at| (at.elapsed().as_secs_f32() / 0.18).min(1.0))
            };
            if alpha < 1.0 {
                ui.ctx().request_repaint();
            }
            let available_height = (viewport.bottom() - ui.cursor().top() - 28.0).max(150.0);
            let clip_margin = ui.visuals().clip_rect_margin;
            ui.visuals_mut().clip_rect_margin = 0.0;
            let category = self.profiles.avatar_category.clone();
                        // «Automático» (the profile's own colour) leads every category.
            let mut members: Vec<Option<files::LibraryAvatar>> = vec![Some(files::LibraryAvatar {
                id: AUTO_AVATAR.into(),
                label: "Automático".into(),
                category: category.clone(),
                location: String::new(),
                local: true,
            })];
            members.extend(
                options
                    .iter()
                    .filter(|a| a.category == category)
                    .cloned()
                    .map(Some),
            );
            if can_upload {
                members.push(None); // «+» tile
            }
            let mut upload = false;
            let gallery = ScrollArea::vertical()
                .id_salt(("avatar-gallery", &category))
                .max_height(available_height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.multiply_opacity(1.0 - (1.0 - alpha).powi(3));
                    let columns = (ui.available_width() / if compact { 106.0 } else { 132.0 })
                        .floor()
                        .max(3.0) as usize;
                    let cell = ui.available_width() / columns as f32;
                    let diameter = (cell - 28.0).clamp(64.0, 108.0);
                    for row in members.chunks(columns) {
                        let (line, _) = ui.allocate_exact_size(
                            vec2(ui.available_width(), diameter + 48.0),
                            Sense::hover(),
                        );
                        for (col, member) in row.iter().enumerate() {
                            let center = pos2(
                                line.left() + cell * (col as f32 + 0.5),
                                line.top() + diameter / 2.0 + 8.0,
                            );
                            let hit = Rect::from_min_size(
                                pos2(line.left() + cell * col as f32, line.top()),
                                vec2(cell, diameter + 44.0),
                            );
                            let id_text = member.as_ref().map_or("upload", |a| a.id.as_str());
                            let response = ui
                                .interact(hit, Id::new(("avatar-choice", id_text)), Sense::click())
                                .on_hover_cursor(CursorIcon::PointingHand);
                            let is_selected = member
                                .as_ref()
                                .is_some_and(|a| Some(&a.id) == selected_id.as_ref());
                            let label = member
                                .as_ref()
                                .map_or("Subir", |a| a.label.as_str())
                                .to_owned();
                            response.widget_info(|| {
                                WidgetInfo::selected(
                                    WidgetType::RadioButton,
                                    true,
                                    is_selected,
                                    &label,
                                )
                            });
                            let hover = ctl::motion(
                                ui.ctx(),
                                response.id.with("hover"),
                                response.hovered() || response.has_focus(),
                                0.14,
                            );
                            let d = diameter + 4.0 * hover
                                - if response.is_pointer_button_down_on() {
                                    3.0
                                } else {
                                    0.0
                                };
                            let portrait = Rect::from_center_size(center, Vec2::splat(d));
                            match member {
                                                                Some(avatar) if avatar.id == AUTO_AVATAR => {
                                    paint_avatar(ui, Some(&auto), None, &user.name, portrait, 1.0);
                                }
                                Some(avatar) => {
                                    let texture = self.avatar_texture(ui.ctx(), &avatar.id);
                                    if avatar.id.starts_with("library:") && texture.is_none() {
                                        ui.painter().circle_filled(center, d / 2.0, ctl::screen());
                                        ctl::loading(ui, center, 9.0);
                                    } else {
                                        paint_avatar(
                                            ui,
                                            Some(&avatar.id),
                                            texture.as_ref(),
                                            &user.name,
                                            portrait,
                                            1.0,
                                        );
                                    }
                                }
                                None => {
                                    ui.painter().circle_filled(
                                        center,
                                        d / 2.0,
                                        Color32::from_white_alpha(14),
                                    );
                                    if self.profiles.avatar_uploading {
                                        ctl::loading(ui, center, 11.0);
                                    } else {
                                        ctl::icon(
                                            ui.painter(),
                                            Rect::from_center_size(center, Vec2::splat(26.0)),
                                            Icon::Plus,
                                            text_color(),
                                        );
                                    }
                                }
                            }
                            if is_selected || hover > 0.0 {
                                ui.painter().circle_stroke(
                                    center,
                                    d / 2.0 + 4.0,
                                    Stroke::new(
                                        2.0_f32,
                                        if response.has_focus() {
                                            focus()
                                        } else {
                                            text_color().gamma_multiply(if is_selected {
                                                1.0
                                            } else {
                                                hover * 0.5
                                            })
                                        },
                                    ),
                                );
                            }
                            if is_selected {
                                let check = center + vec2(d * 0.35, d * 0.35);
                                ui.painter().circle_filled(check, 12.0, text_color());
                                ctl::icon(
                                    ui.painter(),
                                    Rect::from_center_size(check, Vec2::splat(15.0)),
                                    Icon::Check,
                                    ctl::cinema(),
                                );
                            }
                            let name = super::ui::paragraph(
                                ui,
                                &label,
                                cell - 8.0,
                                13.0,
                                if is_selected { text_color() } else { muted() },
                                1,
                            );
                            ui.painter().galley(
                                pos2(center.x - name.size().x / 2.0, line.top() + diameter + 20.0),
                                name,
                                if is_selected { text_color() } else { muted() },
                            );
                            if response.clicked() {
                                match member {
                                    Some(avatar) => {
                                        self.profiles.avatar_selected = Some(avatar.id.clone());
                                    }
                                    None if !self.profiles.avatar_uploading => upload = true,
                                    None => {}
                                }
                            }
                        }
                    }
                });
            if upload {
                self.upload_avatar(ui.ctx(), &category);
            }
            ui.visuals_mut().clip_rect_margin = clip_margin;
            let visible = gallery.inner_rect;
            if gallery.state.offset.y > 1.0 {
                super::ui::gradient(
                    ui.painter(),
                    Rect::from_min_size(visible.min, vec2(visible.width(), 36.0)),
                    ctl::cinema(),
                    Color32::TRANSPARENT,
                    true,
                );
            }
            if gallery.content_size.y > gallery.state.offset.y + visible.height() + 1.0 {
                super::ui::gradient(
                    ui.painter(),
                    Rect::from_min_max(
                        pos2(visible.left(), visible.bottom() - 44.0),
                        visible.right_bottom(),
                    ),
                    Color32::TRANSPARENT,
                    ctl::cinema(),
                    true,
                );
            }
        });
    }
}

#[derive(Default)]
pub(super) struct Profiles {
    pub users: Vec<jellyfin::User>,
    pub busy: bool,
    pub error: Option<String>,
    pub request: u64,
    pub creating: bool,
    pub name: String,
    avatar_selected: Option<String>,
    avatar_category: String,
    avatar_category_changed: Option<Instant>,
    /// Avatars saved from the app this session, shown before the next listing.
    uploaded_avatars: Vec<files::LibraryAvatar>,
    avatar_uploading: bool,
}
