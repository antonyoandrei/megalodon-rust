use super::controls::{self as ctl, ButtonStyle, Icon, cinema, focus, muted, screen, text_color};
use super::*;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub(super) enum SceneKey {
    Library(Page, String),
    Avatars,
    ProfileCreate,
    Onboarding(usize),
    Preparing,
    Player,
    Remote,
}

/// Settings open over the page instead of growing it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Dialog {
    Appearance,
    Experience,
    Metadata,
    Updates,
    Advanced,
}

fn scene_opacities(elapsed: f32, reduced: bool) -> (f32, f32) {
    if reduced {
        return (0.0, 1.0);
    }
    // Let the old words leave before the new ones arrive; simultaneous fades
    // superimpose two unrelated headings and two sets of profile portraits.
    let outgoing = (1.0 - (elapsed / 0.09).clamp(0.0, 1.0)).powi(2);
    let incoming = 1.0 - (1.0 - ((elapsed - 0.06) / 0.18).clamp(0.0, 1.0)).powi(3);
    (outgoing, incoming)
}

fn rail_columns(usable: f32, poster: bool) -> f32 {
    ((usable + 20.0) / if poster { 190.0 } else { 390.0 })
        .ceil()
        .max(if poster { 3.0 } else { 2.0 })
}
/// Tall enough on large screens that a 16:9 backdrop is not reduced to a thin
/// band (about 2.4:1 at most), while rows stay close on laptops.
fn hero_height(viewport: f32, width: f32) -> f32 {
    (viewport * 0.68).clamp(
        if width < 740.0 { 540.0 } else { 490.0 },
        (width / 2.4).max(620.0),
    )
}
/// Panoramic art (backdrops, thumbs and provider banners) fills the hero.
/// Anything narrower is shown intact as a poster, never stretched.
fn panoramic(size: Vec2) -> bool {
    size.x >= 960.0 && (1.3..=5.2).contains(&(size.x / size.y))
}
/// Whether an item can lead the hero with a full-bleed image.
fn has_wide_art(item: &Item) -> bool {
    !item.backdrop_image_tags.is_empty() || item.image_tags.contains_key("Thumb")
}
/// Newest titles with panoramic art lead. Poster-only titles are used only when
/// the library has no panoramic art at all, so one composition holds per rotation.
fn featured_items(latest: &[Item]) -> Vec<Item> {
    let wide: Vec<_> = latest
        .iter()
        .filter(|i| has_wide_art(i))
        .take(4)
        .cloned()
        .collect();
    if !wide.is_empty() {
        return wide;
    }
    latest
        .iter()
        .filter(|i| i.image_tags.contains_key("Primary"))
        .take(4)
        .cloned()
        .collect()
}
/// A 12×18 average of portrait art. Linear sampling turns it into a soft colour
/// field behind the poster, in the title's own palette, without a blur shader.
pub(super) fn ambient_image(image: &ColorImage) -> Option<ColorImage> {
    let [w, h] = image.size;
    if w < 24 || (h as f32) < w as f32 * 1.15 {
        return None;
    }
    let (cols, rows) = (12, 18);
    let mut pixels = Vec::with_capacity(cols * rows);
    for row in 0..rows {
        for col in 0..cols {
            let (x0, x1) = (
                col * w / cols,
                ((col + 1) * w / cols).max(col * w / cols + 1),
            );
            let (y0, y1) = (
                row * h / rows,
                ((row + 1) * h / rows).max(row * h / rows + 1),
            );
            let mut sum = [0u32; 3];
            let mut n = 0;
            for y in (y0..y1).step_by(((y1 - y0) / 8).max(1)) {
                for x in (x0..x1).step_by(((x1 - x0) / 8).max(1)) {
                    let p = image.pixels[y * w + x];
                    sum[0] += p.r() as u32;
                    sum[1] += p.g() as u32;
                    sum[2] += p.b() as u32;
                    n += 1;
                }
            }
            let n = n.max(1);
            pixels.push(Color32::from_rgb(
                (sum[0] / n) as u8,
                (sum[1] / n) as u8,
                (sum[2] / n) as u8,
            ));
        }
    }
    Some(ColorImage::new([cols, rows], pixels))
}

/// Cards name the title; artwork already says what it is. Only episodes need
/// a second line, so a resumed chapter is identifiable.
fn card_copy(item: &Item) -> (&str, String) {
    if item.kind != "Episode" {
        return (&item.name, String::new());
    }
    let series = item.series_name.as_deref().filter(|s| !s.trim().is_empty());
    let mut context = Vec::new();
    if let Some(season) = item.parent_index_number {
        context.push(if season == 0 {
            "Especiales".into()
        } else {
            format!("T{season}")
        });
    }
    if let Some(episode) = item.index_number {
        context.push(format!("E{episode}"));
    }
    if series.is_some() {
        context.push(item.name.clone());
    }
    (series.unwrap_or(&item.name), context.join(" · "))
}

pub fn theme(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    for (name, path) in [
        ("Segoe UI", "C:/Windows/Fonts/segoeui.ttf"),
        ("Symbols", "C:/Windows/Fonts/seguisym.ttf"),
    ] {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert(name.into(), Arc::new(FontData::from_owned(bytes)));
            fonts
                .families
                .entry(FontFamily::Proportional)
                .or_default()
                .push(name.into());
        }
    }
    if fonts.font_data.contains_key("Segoe UI") {
        fonts
            .families
            .get_mut(&FontFamily::Proportional)
            .unwrap()
            .insert(0, "Segoe UI".into());
    }
    ctx.set_fonts(fonts);
    let mut style = (*ctx.style()).clone();
    style.visuals = Visuals::dark();
    style.visuals.panel_fill = cinema();
    style.visuals.window_fill = screen();
    style.visuals.extreme_bg_color = cinema();
    style.visuals.override_text_color = Some(text_color());
    style.visuals.selection.bg_fill = Color32::from_rgb(60, 98, 128);
    style.visuals.widgets.inactive.bg_fill = screen();
    style.visuals.widgets.inactive.weak_bg_fill = screen();
    style.visuals.widgets.inactive.bg_stroke = Stroke::NONE;
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(42, 44, 50);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(60, 64, 70);
    style.visuals.widgets.inactive.corner_radius = CornerRadius::same(10);
    style.visuals.widgets.hovered.corner_radius = CornerRadius::same(10);
    style.visuals.widgets.active.corner_radius = CornerRadius::same(10);
    style.visuals.widgets.open.corner_radius = CornerRadius::same(10);
    style.visuals.text_edit_bg_color = Some(screen());
    style.visuals.window_corner_radius = CornerRadius::same(16);
    style.visuals.interact_cursor = Some(CursorIcon::PointingHand);
    style.spacing.item_spacing = vec2(12.0, 12.0);
    style.spacing.button_padding = vec2(18.0, 12.0);
    style.spacing.interact_size = vec2(44.0, 44.0);
    style.animation_time = 0.15;
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(16.0));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::proportional(15.0));
    style
        .text_styles
        .insert(TextStyle::Heading, FontId::proportional(28.0));
    style
        .text_styles
        .insert(TextStyle::Small, FontId::proportional(13.0));
    ctx.set_style(style);
}

pub(super) fn text(
    painter: &Painter,
    pos: Pos2,
    value: impl Into<String>,
    size: f32,
    color: Color32,
) {
    painter.text(
        pos,
        Align2::LEFT_TOP,
        value.into(),
        FontId::proportional(size),
        color,
    );
}
pub(super) fn gradient(
    painter: &Painter,
    rect: Rect,
    start: Color32,
    end: Color32,
    vertical: bool,
) {
    let mut mesh = egui::Mesh::default();
    let colors = if vertical {
        [start, start, end, end]
    } else {
        [start, end, end, start]
    };
    for (pos, color) in [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ]
    .into_iter()
    .zip(colors)
    {
        mesh.colored_vertex(pos, color);
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(Shape::mesh(mesh));
}
fn image_cover(ui: &mut Ui, texture: &TextureHandle, rect: Rect, radius: u8, opacity: f32) {
    image_cover_at(ui, texture, rect, radius, opacity, vec2(0.5, 0.5));
}
/// Cover crop around a focal point (0..1 in each axis).
fn image_cover_at(
    ui: &mut Ui,
    texture: &TextureHandle,
    rect: Rect,
    radius: u8,
    opacity: f32,
    focus: Vec2,
) {
    let source = texture.size_vec2();
    let scale = (rect.width() / source.x).max(rect.height() / source.y);
    let fraction = rect.size() / (source * scale);
    let uv = Rect::from_min_size(
        pos2((1.0 - fraction.x) * focus.x, (1.0 - fraction.y) * focus.y),
        fraction,
    );
    ui.place(
        rect,
        egui::Image::new(texture)
            .uv(uv)
            .fit_to_exact_size(rect.size())
            .maintain_aspect_ratio(false)
            .corner_radius(radius)
            .tint(Color32::WHITE.gamma_multiply(opacity)),
    );
}
fn backdrop(ui: &mut Ui, texture: &TextureHandle, rect: Rect, opacity: f32) {
    let mut canvas = ui.new_child(UiBuilder::new().max_rect(rect));
    canvas.set_clip_rect(ui.clip_rect().intersect(rect));
    // Faces and titles sit in the upper part of most backdrops: when the stage
    // is wider than the image, keep the top third rather than the middle.
    image_cover_at(&mut canvas, texture, rect, 0, opacity, vec2(0.5, 0.3));
}
fn title_logo(ui: &Ui, texture: &TextureHandle, rect: Rect, alpha: f32) {
    let points = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ];
    let quad = |offset: Vec2, color: Color32| {
        let mut mesh = egui::Mesh::with_texture(texture.id());
        for (pos, uv) in points.into_iter().zip([
            pos2(0.0, 0.0),
            pos2(1.0, 0.0),
            pos2(1.0, 1.0),
            pos2(0.0, 1.0),
        ]) {
            mesh.vertices.push(egui::epaint::Vertex {
                pos: pos + offset,
                uv,
                color,
            });
        }
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        ui.painter().add(Shape::mesh(mesh));
    };
    for (offset, opacity) in [
        (vec2(0.0, 7.0), 0.24),
        (vec2(0.0, 10.0), 0.10),
        (vec2(0.0, 13.0), 0.04),
    ] {
        quad(
            offset,
            Color32::from_black_alpha((255.0 * opacity * alpha) as u8),
        );
    }
    quad(Vec2::ZERO, Color32::WHITE.gamma_multiply(alpha));
}
fn skeleton(ui: &Ui, rect: Rect, round: u8, animate: bool) {
    if !ui.is_rect_visible(rect) {
        return;
    }
    let pulse = if animate && ui.input(|i| i.focused) {
        ((ui.input(|i| i.time) as f32 * 2.2).sin() + 1.0) * 0.5
    } else {
        0.0
    };
    ui.painter().rect_filled(
        rect,
        round,
        screen().lerp_to_gamma(Color32::WHITE, pulse * 0.02),
    );
    if animate && ui.input(|i| i.focused) {
        ui.ctx().request_repaint_after(Duration::from_millis(60));
    }
}
fn action(ui: &mut Ui, rect: Rect, label: &str, primary: bool) -> Response {
    let (label, glyph) = if let Some(rest) = label.strip_prefix('▶') {
        (rest.trim(), Some(Icon::Play))
    } else if let Some(rest) = label.strip_prefix('‹') {
        (rest.trim(), Some(Icon::Back))
    } else if label == "›" {
        ("", Some(Icon::Forward))
    } else {
        (label, None)
    };
    ctl::button(
        ui,
        rect,
        label,
        glyph,
        if primary {
            ButtonStyle::Primary
        } else {
            ButtonStyle::Soft
        },
    )
}
pub(super) fn paragraph(
    ui: &Ui,
    value: &str,
    width: f32,
    size: f32,
    color: Color32,
    rows: usize,
) -> Arc<egui::Galley> {
    let mut job =
        egui::text::LayoutJob::simple(value.into(), FontId::proportional(size), color, width);
    job.wrap.max_rows = rows;
    job.wrap.break_anywhere = false;
    ui.fonts_mut(|fonts| fonts.layout_job(job))
}
pub(super) fn time_label(value: f64) -> String {
    let value = value.max(0.0) as u64;
    if value >= 3600 {
        format!("{}:{:02}:{:02}", value / 3600, value / 60 % 60, value % 60)
    } else {
        format!("{}:{:02}", value / 60, value % 60)
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;
    #[test]
    fn resume_cards_identify_real_episodes_without_inventing_missing_numbers() {
        let mut episode: Item = serde_json::from_value(serde_json::json!({
            "Type": "Episode", "Name": "El comienzo", "SeriesName": "Una serie",
            "ParentIndexNumber": 3, "IndexNumber": 12
        }))
        .unwrap();
        assert_eq!(
            card_copy(&episode),
            ("Una serie", "T3 · E12 · El comienzo".into())
        );
        episode.parent_index_number = Some(0);
        assert_eq!(card_copy(&episode).1, "Especiales · E12 · El comienzo");
        episode.parent_index_number = None;
        episode.index_number = None;
        episode.series_name = None;
        assert_eq!(card_copy(&episode), ("El comienzo", String::new()));
    }
    #[test]
    fn cards_name_titles_without_year_or_kind() {
        let movie: Item = serde_json::from_value(serde_json::json!({
            "Type": "Movie", "Name": "Dragon Ball Z - Dead Zone", "ProductionYear": 1989
        }))
        .unwrap();
        assert_eq!(
            card_copy(&movie),
            ("Dragon Ball Z - Dead Zone", String::new())
        );
    }
    #[test]
    fn hero_prefers_panoramic_art_and_keeps_one_composition() {
        let item = |id: &str, backdrop: bool| -> Item {
            let mut value = serde_json::json!({"Id": id, "Type": "Movie", "Name": id,
                "ImageTags": {"Primary": "p"}});
            if backdrop {
                value["BackdropImageTags"] = serde_json::json!(["b"]);
            }
            serde_json::from_value(value).unwrap()
        };
        let latest = vec![
            item("a", false),
            item("b", true),
            item("c", false),
            item("d", true),
        ];
        let ids: Vec<_> = featured_items(&latest).into_iter().map(|i| i.id).collect();
        assert_eq!(ids, ["b", "d"]);
        let posters = vec![item("a", false), item("c", false)];
        assert_eq!(featured_items(&posters).len(), 2);
        assert!(panoramic(vec2(1900.0, 400.0)));
        assert!(panoramic(vec2(1920.0, 1080.0)));
        assert!(!panoramic(vec2(1000.0, 1500.0)));
        assert!(!panoramic(vec2(640.0, 360.0)));
    }
    #[test]
    fn ambient_field_only_for_portrait_art() {
        let poster = ColorImage::filled([200, 300], Color32::from_rgb(200, 40, 10));
        let ambient = ambient_image(&poster).unwrap();
        assert_eq!(ambient.size, [12, 18]);
        assert_eq!(ambient.pixels[0], Color32::from_rgb(200, 40, 10));
        assert!(ambient_image(&ColorImage::filled([300, 200], Color32::BLACK)).is_none());
    }
    #[test]
    fn absolute_controls_preserve_space_before_following_content() {
        let ctx = Context::default();
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1280.0, 800.0))),
                ..Default::default()
            },
            |ctx| {
                CentralPanel::default().show(ctx, |ui| {
                    let (hero, _) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 560.0), Sense::hover());
                    let texture = ctx.load_texture(
                        "test",
                        ColorImage::filled([2, 2], Color32::BLACK),
                        TextureOptions::LINEAR,
                    );
                    image_cover(ui, &texture, hero, 0, 1.0);
                    let _ = action(
                        ui,
                        Rect::from_min_size(hero.min + vec2(48.0, 105.0), vec2(110.0, 40.0)),
                        "Volver",
                        false,
                    );
                    let episodes = ui.label("Episodios");
                    assert!(
                        episodes.rect.top() >= hero.bottom(),
                        "El contenido no puede ocupar el espacio del hero"
                    );
                });
            },
        );
    }
}

impl Megalodon {
    fn unavailable(&mut self, ui: &mut Ui) {
        let disconnected = self.sources.active.is_none();
        ui.add_space(160.0);
        ui.vertical_centered(|ui| {
            ui.heading(if disconnected {
                "Añade tu biblioteca"
            } else {
                "No se pudo cargar tu biblioteca"
            });
            ui.label(
                RichText::new(if disconnected {
                    "Conecta Google Drive, una carpeta local o un servidor Jellyfin."
                } else {
                    "Comprueba que esta conexión esté disponible e inténtalo de nuevo."
                })
                .color(muted()),
            );
            ui.add_space(20.0);
            let r = Rect::from_center_size(
                ui.cursor().min + vec2(ui.available_width() / 2.0, 24.0),
                vec2(180.0, 48.0),
            );
            if action(
                ui,
                r,
                if disconnected {
                    "Añadir conexión"
                } else {
                    "Volver a intentar"
                },
                true,
            )
            .clicked()
            {
                if disconnected {
                    self.open_source_editor();
                } else {
                    self.refresh(ui.ctx(), true);
                }
            }
            ui.add_space(56.0);
        });
    }
    fn library_skeleton(&self, ui: &mut Ui) {
        let width = ui.available_width();
        let gutter = if width < 900.0 { 32.0 } else { 60.0 };
        let height = hero_height(ui.ctx().content_rect().height(), width);
        let (hero, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
        gradient(ui.painter(), hero, screen(), cinema(), true);
        let animated = self.busy && !self.preferences.reduced_motion;
        for (x, y, w, h) in [
            (gutter, height - 320.0, 280.0, 72.0),
            (gutter, height - 215.0, 180.0, 14.0),
            (gutter, height - 174.0, 340.0, 14.0),
            (gutter, height - 148.0, 260.0, 14.0),
            (gutter, height - 74.0, 172.0, 48.0),
        ] {
            skeleton(
                ui,
                Rect::from_min_size(hero.min + vec2(x, y), vec2(w, h)),
                8,
                animated,
            );
        }
        ui.add_space(30.0);
        let (heading, _) = ui.allocate_exact_size(vec2(width, 44.0), Sense::hover());
        skeleton(
            ui,
            Rect::from_min_size(heading.min + vec2(gutter, 0.0), vec2(185.0, 24.0)),
            6,
            animated,
        );
        let columns = rail_columns(width - 2.0 * gutter, false) as usize;
        let card = (width - 2.0 * gutter - 20.0 * (columns - 1) as f32) / columns as f32;
        let (row, _) = ui.allocate_exact_size(vec2(width, card * 0.5625 + 48.0), Sense::hover());
        for i in 0..columns {
            let pos = row.min + vec2(gutter + i as f32 * (card + 20.0), 0.0);
            skeleton(
                ui,
                Rect::from_min_size(pos, vec2(card, card * 0.5625)),
                12,
                animated,
            );
            skeleton(
                ui,
                Rect::from_min_size(
                    pos + vec2(0.0, card * 0.5625 + 14.0),
                    vec2(card * 0.6, 14.0),
                ),
                5,
                animated,
            );
        }
    }
    pub(super) fn image_alpha(&self, ctx: &Context, texture: &TextureHandle) -> f32 {
        let duration = if self.preferences.reduced_motion {
            0.08
        } else {
            0.24
        };
        let alpha = self
            .images
            .get(&texture.name())
            .and_then(|s| s.loaded_at)
            .map_or(1.0, |since| {
                (since.elapsed().as_secs_f32() / duration).min(1.0)
            });
        if alpha < 1.0 {
            ctx.request_repaint();
        }
        alpha
    }
    fn placeholder(&self, ui: &Ui, item: &Item, kind: &str, width: u32, rect: Rect, round: u8) {
        // Still loading from the library, Jellyfin or a provider: pulse, no icon.
        let _ = width;
        let marker = format!("/Items/{}/Images/{kind}?", item.id);
        let provider = self
            .metadata
            .patches
            .get(&item.id)
            .and_then(|p| p.images.get(kind));
        let waiting = self.images.iter().any(|(key, slot)| {
            !slot.failed
                && slot.texture.is_none()
                && (key.contains(&marker)
                    || provider.is_some_and(|url| key.ends_with(url.as_str())))
        });
        skeleton(ui, rect, round, waiting && !self.preferences.reduced_motion);
        // Large stages (hero, detail) stay a calm surface: no icon in the middle.
        if !waiting && rect.width() < 900.0 {
            ctl::icon(
                ui.painter(),
                Rect::from_center_size(rect.center(), vec2(28.0, 28.0)),
                Icon::Picture,
                muted(),
            );
        }
    }
    pub(super) fn library_ui(&mut self, ctx: &Context) {
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            if Popup::is_any_open(ctx) {
                Popup::close_all(ctx);
            } else if self.metadata.picker.is_some() {
                self.metadata.picker = None;
            } else if self.source_editor.remove.is_some() {
                self.source_editor.remove = None;
            } else if self.source_editor.open {
                self.close_source_editor();
            } else if self.settings_dialog.is_some() {
                self.close_settings_dialog(ctx);
            } else if self.page == Page::Detail {
                self.page = self.previous;
            } else if self.avatar_picker {
                self.avatar_picker = false;
            } else if self.page == Page::Search {
                self.page = Page::Home;
            } else if self.page == Page::Profiles {
                if self.avatar_picker {
                    self.avatar_picker = false;
                } else if self.profiles.creating {
                    self.cancel_profile_form();
                } else {
                    self.page = Page::Home;
                }
            }
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::F)) {
            self.page = Page::Search;
            self.avatar_picker = false;
            self.genre.clear();
            ctx.data_mut(|d| d.insert_temp(Id::new("focus-search"), true));
        }
        let key = match self.page {
            _ if self.avatar_picker => SceneKey::Avatars,
            Page::Profiles if self.profiles.creating => SceneKey::ProfileCreate,
            Page::Detail => SceneKey::Library(self.page, self.requested_detail.clone()),
            _ => SceneKey::Library(self.page, String::new()),
        };
        // Dialogs float above the page; opening one never restarts the page transition.
        self.begin_scene(ctx, key);
        let scroll_key = self
            .scene_key
            .clone()
            .unwrap_or(SceneKey::Library(self.page, String::new()));
        let mut scroll_top = 0.0;
        CentralPanel::default()
            .frame(Frame::NONE.fill(cinema()))
            .show(ctx, |ui| {
                if self.avatar_picker {
                    self.avatar_carousel(ui);
                    return;
                }
                let scroll = ScrollArea::vertical()
                    .id_salt(("page", scroll_key))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        match self.page {
                            Page::Home => self.home(ui),
                            Page::Movies | Page::Series | Page::Search | Page::Favorites => {
                                self.browse(ui)
                            }
                            Page::Detail => self.detail_ui(ui),
                            Page::Settings if self.avatar_picker => self.avatar_carousel(ui),
                            Page::Settings => self.settings(ui),
                            Page::Profiles => self.profiles_ui(ui),
                        }
                        ui.add_space(50.0);
                    });
                scroll_top = scroll.state.offset.y;
            });
        self.header(ctx, scroll_top);
        self.dialogs(ctx);
    }
    pub(super) fn open_source_editor(&mut self) {
        self.settings_dialog = None;
        self.source_editor.open = true;
        self.source_editor.existing = false;
        self.source_editor.error = None;
    }
    fn close_settings_dialog(&mut self, ctx: &Context) {
        if self.settings_dialog == Some(Dialog::Metadata) {
            self.apply_metadata_changes(ctx);
        }
        self.settings_dialog = None;
    }
    pub(super) fn dialogs(&mut self, ctx: &Context) {
        if self.source_editor.open
            && ctl::dialog(ctx, "connection", "Añadir conexión", 620.0, |ui| {
                self.connection_editor(ui)
            })
        {
            self.close_source_editor();
        }
        if let Some(id) = self.source_editor.remove.clone() {
            self.remove_source_dialog(ctx, &id);
        }
        self.artwork_picker(ctx);
        let Some(dialog) = self.settings_dialog else {
            return;
        };
        let (title, width) = match dialog {
            Dialog::Appearance => ("Apariencia", 640.0),
            Dialog::Experience => ("Reproducción y movimiento", 520.0),
            Dialog::Metadata => ("Imágenes y fichas", 640.0),
            Dialog::Updates => ("Actualizaciones", 560.0),
            Dialog::Advanced => ("Más opciones", 560.0),
        };
        let close = ctl::dialog(ctx, "settings", title, width, |ui| match dialog {
            Dialog::Appearance => {
                ui.label(
                    RichText::new("Cambia el fondo y los acentos de toda la aplicación. Se aplica al momento.")
                        .size(14.0)
                        .color(muted()),
                );
                ui.add_space(16.0);
                if let Some(id) = super::onboarding::theme_cards(ui, ctl::theme_id()) {
                    self.apply_theme(ui.ctx(), id);
                }
                ui.add_space(8.0);
            }
            Dialog::Experience => {
                if ctl::toggle(
                    ui,
                    &mut self.preferences.reduced_motion,
                    "Reducir movimiento",
                )
                .changed()
                {
                    let _ = self.save_preferences();
                }
                if ctl::toggle(
                    ui,
                    &mut self.auto_next,
                    "Continuar con el siguiente episodio",
                )
                .changed()
                {
                    self.preferences.auto_next = self.auto_next;
                    let _ = self.save_preferences();
                }
            }
            Dialog::Metadata => self.metadata_settings(ui),
            Dialog::Updates => self.updates_settings(ui),
            Dialog::Advanced => self.advanced_settings(ui),
        });
        if close {
            self.close_settings_dialog(ctx);
        }
    }
    fn advanced_settings(&mut self, ui: &mut Ui) {
        if self.library.is_some() && services::local_server(&self.server) {
            ui.label(
                RichText::new(&self.services_status)
                    .size(14.0)
                    .color(muted()),
            );
            let (r, _) = ui.allocate_exact_size(vec2(184.0, 44.0), Sense::hover());
            if !self.busy
                && ctl::button(ui, r, "Preparar servicios", None, ButtonStyle::Quiet).clicked()
            {
                self.busy = true;
                let tx = self.tx.clone();
                let ctx = ui.ctx().clone();
                self.rt.spawn_blocking(move || {
                    let _ = tx.send(Message::ServicesPrepared(
                        services::prepare(&data::data_dir()).map(|_| ()),
                    ));
                    ctx.request_repaint();
                });
            }
            ui.add_space(12.0);
        }
        ui.label(RichText::new("Abrir un vídeo").size(13.0).color(muted()));
        ui.horizontal(|ui| {
            ui.add_sized(
                vec2((ui.available_width() - 100.0).max(180.0), 44.0),
                TextEdit::singleline(&mut self.local_path)
                    .margin(vec2(12.0, 12.0))
                    .hint_text("Ruta del archivo"),
            );
            let (r, _) = ui.allocate_exact_size(vec2(88.0, 44.0), Sense::hover());
            if action(ui, r, "Abrir", true).clicked() {
                self.settings_dialog = None;
                self.open_local(self.local_path.clone());
            }
        });
        ui.label(
            RichText::new("También puedes arrastrarlo sobre Megalodon.")
                .size(13.0)
                .color(muted()),
        );
    }

    pub(super) fn begin_scene(&mut self, ctx: &Context, key: SceneKey) {
        if self.scene_key.as_ref() != Some(&key) {
            if matches!(key, SceneKey::Library(Page::Home, _)) {
                self.hero_cycle_at = Instant::now();
            }
            #[cfg(feature = "motion-capture")]
            self.motion_capture
                .begin(ctx, &key, self.preferences.reduced_motion);
            // A video callback is live GPU work, never a reusable scene snapshot.
            // Enter and leave playback over opaque black so letterboxing stays clean.
            self.outgoing_scene = if self.scene_key.is_some()
                && (matches!(
                    key,
                    SceneKey::Player | SceneKey::Preparing | SceneKey::Remote
                ) || self.scene_key == Some(SceneKey::Player))
            {
                vec![egui::epaint::ClippedShape {
                    clip_rect: ctx.content_rect(),
                    shape: Shape::rect_filled(ctx.content_rect(), 0, Color32::BLACK),
                }]
            } else {
                std::mem::take(&mut self.scene)
            };
            self.outgoing_textures
                .extend(self.images.values().filter_map(|s| s.texture.clone()));
            self.scene_changed = Instant::now();
            self.scene_key = Some(key);
        }
        if self.scene_size != ctx.content_rect().size() || self.preferences.reduced_motion {
            self.outgoing_scene.clear();
            self.outgoing_textures.clear();
        }
        self.scene_size = ctx.content_rect().size();
    }

    pub(super) fn finish_scene(&mut self, ctx: &Context) {
        if self.scene_key.is_none() {
            return; // The session changed during this frame; its textures are gone.
        }
        let scene_layer = LayerId::background();
        let (alpha, incoming) = scene_opacities(
            self.scene_changed.elapsed().as_secs_f32(),
            self.preferences.reduced_motion,
        );
        if incoming < 1.0 && !self.outgoing_scene.is_empty() {
            ctx.layer_painter(scene_layer).rect_filled(
                ctx.content_rect(),
                0,
                cinema().gamma_multiply(1.0 - incoming),
            );
            for entry in &self.outgoing_scene {
                let mut painter = ctx
                    .layer_painter(scene_layer)
                    .with_clip_rect(entry.clip_rect);
                painter.set_opacity(alpha);
                painter.add(entry.shape.clone());
            }
            ctx.request_repaint();
        } else {
            self.outgoing_scene.clear();
            self.outgoing_textures.clear();
        }
        // Keep the visible composition, including an unfinished dissolve: a rapid
        // second navigation starts from this frame rather than an older destination.
        if self.scene_key != Some(SceneKey::Player) {
            self.scene = ctx.graphics(|g| {
                g.get(scene_layer)
                    .map(|list| list.all_entries().cloned().collect())
                    .unwrap_or_default()
            });
        } else {
            self.scene.clear();
        }
    }
    fn header(&mut self, ctx: &Context, scroll_top: f32) {
        let width = ctx.content_rect().width();
        Area::new(Id::new("header"))
            .fixed_pos(pos2(0.0, 0.0))
            .order(Order::Foreground)
            .show(ctx, |ui| {
                ui.set_min_size(vec2(width, 84.0));
                let rect = Rect::from_min_size(Pos2::ZERO, vec2(width, 100.0));
                gradient(
                    ui.painter(),
                    rect,
                    Color32::from_black_alpha(210),
                    Color32::TRANSPARENT,
                    true,
                );
                let cover = ctl::motion(ctx, Id::new("header-cover"), scroll_top > 16.0, 0.16);
                let backing = cinema().gamma_multiply(cover);
                ui.painter().rect_filled(
                    Rect::from_min_size(Pos2::ZERO, vec2(width, 76.0)),
                    0,
                    backing,
                );
                gradient(
                    ui.painter(),
                    Rect::from_min_size(pos2(0.0, 76.0), vec2(width, 44.0)),
                    backing,
                    Color32::TRANSPARENT,
                    true,
                );
                let gutter = if width < 900.0 { 24.0 } else { 48.0 };
                let brand_rect = Rect::from_min_size(pos2(gutter, 20.0), Vec2::splat(48.0));
                ui.place(
                    brand_rect,
                    Image::new(&self.brand).fit_to_exact_size(Vec2::splat(48.0)),
                );
                let brand_link = ui
                    .interact(brand_rect, Id::new("brand-home"), Sense::click())
                    .on_hover_cursor(CursorIcon::PointingHand);
                brand_link.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, "Inicio"));
                if brand_link.has_focus() {
                    ui.painter().rect_stroke(
                        brand_rect.expand(3.0),
                        14,
                        Stroke::new(2.0_f32, focus()),
                        StrokeKind::Outside,
                    );
                }
                if brand_link.clicked() {
                    self.page = Page::Home;
                    self.avatar_picker = false;
                }
                let stride = if width < 850.0 { 82.0 } else { 104.0 };
                let start = ((width - stride * 4.0) / 2.0)
                    .min(width - gutter - 160.0 - stride * 4.0)
                    .max(80.0);
                for (index, (page, label)) in [
                    (Page::Home, "Inicio"),
                    (Page::Movies, "Películas"),
                    (Page::Series, "Series"),
                    (Page::Favorites, "Favoritos"),
                ]
                .into_iter()
                .enumerate()
                {
                    let r = Rect::from_min_size(
                        pos2(start + index as f32 * stride, 22.0),
                        vec2(stride - 5.0, 44.0),
                    );
                    let selected =
                        self.page == page || (self.page == Page::Detail && self.previous == page);
                    let response = ctl::button(
                        ui,
                        r,
                        label,
                        None,
                        if selected {
                            ButtonStyle::Primary
                        } else {
                            ButtonStyle::Quiet
                        },
                    );
                    if response.clicked() {
                        self.page = page;
                        self.avatar_picker = false;
                        self.search.clear();
                        self.genre.clear();
                    }
                }
                for (offset, icon, label, page) in [
                    (
                        148.0,
                        Icon::Search,
                        "Buscar en todo el contenido",
                        Page::Search,
                    ),
                    (96.0, Icon::Settings, "Ajustes", Page::Settings),
                ] {
                    let r =
                        Rect::from_min_size(pos2(width - gutter - offset, 22.0), Vec2::splat(44.0));
                    if ctl::icon_button(ui, r, icon, label, self.page == page).clicked() {
                        self.page = page;
                        self.avatar_picker = false;
                        if page == Page::Search {
                            self.genre.clear();
                            ctx.data_mut(|d| d.insert_temp(Id::new("focus-search"), true));
                        }
                    }
                }
                let r = Rect::from_min_size(pos2(width - gutter - 44.0, 22.0), vec2(44.0, 44.0));
                let user = self.current_user();
                let shown = self.displayed_avatar(
                    &user.id,
                    self.profile_id.as_deref(),
                    self.profile.is_some(),
                );
                super::profile_ui::paint_avatar(
                    ui,
                    Some(&shown),
                    self.profile.as_ref(),
                    &user.name,
                    r,
                    1.0,
                );
                let profile_response = ui
                    .interact(r, Id::new("profile"), Sense::click())
                    .on_hover_cursor(CursorIcon::PointingHand);
                profile_response.widget_info(|| {
                    WidgetInfo::labeled(WidgetType::Button, true, "Cambiar perfil")
                });
                if profile_response.clicked() {
                    self.open_profiles(ctx);
                }
                let hover = ctl::motion(
                    ctx,
                    Id::new("profile-hover"),
                    profile_response.hovered() || profile_response.has_focus(),
                    0.12,
                );
                ui.painter().circle_stroke(
                    r.center(),
                    24.0,
                    Stroke::new(
                        1.5_f32,
                        if profile_response.has_focus() {
                            focus()
                        } else {
                            Color32::from_white_alpha((150.0 * hover) as u8)
                        },
                    ),
                );
            });
    }
    fn home(&mut self, ui: &mut Ui) {
        let Some(catalog) = self.catalog.clone() else {
            if self.busy {
                self.library_skeleton(ui);
            } else {
                self.unavailable(ui);
            }
            return;
        };
        if catalog.items.is_empty() {
            ui.add_space(170.0);
            ui.vertical_centered(|ui| {
                ui.heading("Tu biblioteca está vacía");
                ui.label(
                    RichText::new("Aquí aparecerán las películas y series de esta conexión.")
                        .color(muted()),
                );
            });
            return;
        }
        let mut latest = catalog.items.clone();
        latest.sort_by(|a, b| {
            b.date_created
                .cmp(&a.date_created)
                .then_with(|| a.name.cmp(&b.name))
        });
        let featured = featured_items(&latest);
        if !featured.is_empty() {
            let ids: Vec<_> = featured.iter().map(|i| i.id.clone()).collect();
            if self.hero_catalog_ids != ids {
                self.hero_catalog_ids = ids;
                self.hero = 0;
                self.hero_old = None;
                self.hero_requested = None;
                self.hero_ready_at = None;
                self.hero_cycle_at = Instant::now();
            }
            self.hero = self.hero.min(featured.len() - 1);
            self.hero_ui(ui, &featured);
        } else {
            ui.add_space(124.0);
            ui.horizontal(|ui| {
                ui.add_space(40.0);
                ui.heading(&catalog.server_name);
            });
            ui.add_space(16.0);
        }
        let resume = self.resume_items.clone();
        if !resume.is_empty() {
            self.rail(ui, "Seguir viendo", &resume, false, Page::Favorites);
        }
        let movies: Vec<_> = catalog
            .items
            .iter()
            .filter(|i| i.kind == "Movie")
            .cloned()
            .collect();
        self.rail(ui, "Tus películas", &movies, true, Page::Movies);
        let series: Vec<_> = catalog
            .items
            .iter()
            .filter(|i| i.kind == "Series")
            .cloned()
            .collect();
        self.rail(ui, "Tus series", &series, false, Page::Series);
    }
    fn hero_artwork(&mut self, ctx: &Context, item: &Item) -> Option<(TextureHandle, bool)> {
        let mut fallback = None;
        for kind in ["Backdrop", "Thumb"] {
            let present = if kind == "Backdrop" {
                !item.backdrop_image_tags.is_empty()
            } else {
                item.image_tags.contains_key(kind)
            };
            if !present {
                continue;
            }
            // Request the full stage width in physical pixels (4K on large screens).
            let stage = ((ctx.content_rect().width().max(1280.0) / 320.0).ceil() * 320.0) as u32;
            if let Some(texture) = self.artwork(ctx, item, kind, stage) {
                if panoramic(texture.size_vec2()) {
                    return Some((texture, true));
                }
                fallback = Some(texture);
            } else if !self.artwork_failed(item, kind) {
                return None;
            }
        }
        self.artwork(ctx, item, "Primary", 1920)
            .or(fallback)
            .map(|texture| (texture, false))
    }
    fn hero_ui(&mut self, ui: &mut Ui, featured: &[Item]) {
        let width = ui.available_width();
        let stacked = width < 740.0;
        let controls_offset = if stacked { 56.0 } else { 0.0 };
        // Keep one height and one full-width composition throughout the rotation.
        // Poster-only libraries get a slightly shorter stage with the poster's own
        // colour field behind it, never a narrow centred block or an enlarged poster.
        let portrait_collection = featured.iter().all(|item| !has_wide_art(item));
        let height = if portrait_collection {
            (ui.ctx().content_rect().height() * 0.62)
                .clamp(if stacked { 500.0 } else { 460.0 }, 560.0)
        } else {
            hero_height(ui.ctx().content_rect().height(), width)
        };
        let gutter = if width < 900.0 { 32.0 } else { 60.0 };
        let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
        let interacting = ui.input(|i| {
            !i.focused
                || i.pointer.any_down()
                || i.pointer
                    .hover_pos()
                    .is_some_and(|p| rect.contains(p) && p.y > rect.bottom() - 100.0)
        });
        if self.busy
            || interacting
            || self.preferences.reduced_motion
            || rect.bottom() < ui.clip_rect().top() + 180.0
        {
            self.hero_cycle_at = Instant::now();
        } else if featured.len() > 1 && self.hero_requested.is_none() {
            let interval = Duration::from_secs(9);
            if self.hero_cycle_at.elapsed() >= interval {
                self.hero_requested = Some((self.hero + 1) % featured.len());
                self.hero_cycle_at = Instant::now();
            } else {
                ui.ctx()
                    .request_repaint_after(interval.saturating_sub(self.hero_cycle_at.elapsed()));
            }
        }
        // The four featured scenes are bounded and prefetched; switching never waits
        // for a separate logo request or starts a second image entrance animation.
        for item in featured {
            self.hero_artwork(ui.ctx(), item);
            self.artwork(ui.ctx(), item, "Logo", 800);
        }
        let current = featured[self.hero].clone();
        let texture = self.hero_artwork(ui.ctx(), &current);
        if let Some(requested) = self.hero_requested.filter(|_| {
            self.hero_old.is_none() || self.hero_changed.elapsed().as_secs_f32() >= 0.34
        }) {
            let target = &featured[requested];
            let art_ready = self.hero_artwork(ui.ctx(), target).is_some();
            let logo_ready = self.artwork(ui.ctx(), target, "Logo", 800).is_some()
                || !target.image_tags.contains_key("Logo")
                || self.artwork_failed(target, "Logo");
            if art_ready && logo_ready {
                self.hero_old = Some(self.hero);
                self.hero = requested;
                self.hero_requested = None;
                self.hero_changed = Instant::now();
                ui.ctx().request_repaint();
            } else if self.artwork_failed(target, target.art_kind()) {
                self.hero_requested = None;
                self.error = Some("No se pudo cargar esta escena. Puedes volver a seleccionarla después de actualizar la biblioteca.".into());
            }
        }
        let t = if self.preferences.reduced_motion {
            1.0
        } else {
            (self.hero_changed.elapsed().as_secs_f32() / 0.34).min(1.0)
        };
        let eased = 1.0 - (1.0 - t).powi(3);
        let previous_art = self
            .hero_old
            .filter(|_| t < 1.0)
            .and_then(|old| self.hero_artwork(ui.ctx(), &featured[old]));
        let current = featured[self.hero].clone();
        let texture = if self.hero_old.is_some() {
            self.hero_artwork(ui.ctx(), &current)
        } else {
            texture
        };
        let logo_ready = !current.image_tags.contains_key("Logo")
            || self.artwork(ui.ctx(), &current, "Logo", 800).is_some()
            || self.artwork_failed(&current, "Logo");
        let entrance = if (texture.is_some() || self.artwork_failed(&current, current.art_kind()))
            && logo_ready
        {
            let since = self.hero_ready_at.get_or_insert_with(Instant::now);
            let progress = (since.elapsed().as_secs_f32()
                / if self.preferences.reduced_motion {
                    0.08
                } else {
                    0.24
                })
            .min(1.0);
            if progress < 1.0 {
                ui.ctx().request_repaint();
            }
            1.0 - (1.0 - progress).powi(3)
        } else {
            0.0
        };
        let artwork_alpha = if self.hero_old.is_some() {
            eased
        } else {
            entrance
        };
        let field = |this: &Self, texture: &TextureHandle| {
            this.images
                .get(&texture.name())
                .and_then(|slot| slot.ambient.clone())
        };
        if let Some((old, false)) = &previous_art
            && let Some(old_field) = field(self, old)
        {
            let stays = texture.as_ref().is_some_and(|(_, wide)| !*wide);
            backdrop(
                ui,
                &old_field,
                rect,
                0.8 * if stays { 1.0 } else { 1.0 - eased },
            );
        }
        if let Some((poster, false)) = &texture
            && let Some(current_field) = field(self, poster)
        {
            backdrop(ui, &current_field, rect, 0.8 * artwork_alpha);
        }
        if let Some((old, true)) = &previous_art {
            backdrop(
                ui,
                old,
                rect,
                if texture.as_ref().is_some_and(|(_, wide)| *wide) {
                    1.0
                } else {
                    1.0 - eased
                },
            );
        }
        if let Some((texture, true)) = &texture {
            backdrop(ui, texture, rect, artwork_alpha);
        } else if texture.is_none() {
            self.placeholder(ui, &current, current.art_kind(), 1920, rect, 0);
        }
        let left = Rect::from_min_size(rect.min, vec2(rect.width() * 0.83, rect.height()));
        gradient(
            ui.painter(),
            left,
            Color32::from_black_alpha(235),
            Color32::TRANSPARENT,
            false,
        );
        gradient(
            ui.painter(),
            Rect::from_min_max(
                pos2(rect.left(), rect.top() + height * 0.5),
                rect.right_bottom(),
            ),
            Color32::TRANSPARENT,
            cinema(),
            true,
        );
        // Portrait artwork stays intact; it never becomes an enlarged panorama.
        let paint_poster = |ui: &mut Ui, texture: &TextureHandle, alpha: f32| {
            let source = texture.size_vec2();
            let max_width = if stacked {
                width * 0.26
            } else {
                (width * 0.28).min(320.0)
            };
            let scale = (max_width / source.x).min((height - 200.0).clamp(180.0, 420.0) / source.y);
            let poster = Rect::from_center_size(
                pos2(
                    if portrait_collection {
                        rect.right() - gutter - (width * 0.05).min(96.0) - source.x * scale / 2.0
                    } else {
                        rect.left() + (width * 0.73).min(1450.0)
                    },
                    rect.top() + height * 0.48 + 30.0,
                ),
                source * scale,
            );
            ui.painter().add(
                egui::epaint::Shadow {
                    offset: [0, 18],
                    blur: 48,
                    spread: 0,
                    color: Color32::from_black_alpha((150.0 * alpha) as u8),
                }
                .as_shape(poster, 12),
            );
            image_cover(ui, texture, poster, 12, alpha);
        };
        if let Some((old, false)) = &previous_art {
            paint_poster(ui, old, 1.0 - eased);
        }
        if let Some((texture, false)) = &texture {
            paint_poster(ui, texture, artwork_alpha);
        }
        let content_width = (width * 0.47).clamp(310.0, 530.0);
        let copy_rect = Rect::from_min_size(
            pos2(
                rect.left() + gutter,
                rect.bottom() - 326.0 - controls_offset,
            ),
            vec2(content_width, 212.0),
        );
        let (copy_item, alpha) = if let Some(old) = self.hero_old.filter(|_| t < 0.24) {
            (&featured[old], (1.0 - t / 0.24).powi(2))
        } else {
            (
                &current,
                if self.hero_old.is_some() {
                    1.0 - (1.0 - ((t - 0.24) / 0.64).clamp(0.0, 1.0)).powi(3)
                } else {
                    entrance
                },
            )
        };
        if let Some(logo) = self.artwork(ui.ctx(), copy_item, "Logo", 800) {
            let size = logo.size_vec2();
            let scale = (content_width / size.x).min(120.0 / size.y);
            let r = Rect::from_min_size(
                copy_rect.min + vec2(0.0, 120.0 - size.y * scale),
                size * scale,
            );
            title_logo(ui, &logo, r, alpha);
        } else if copy_item.image_tags.contains_key("Logo")
            && !self.artwork_failed(copy_item, "Logo")
        {
            skeleton(
                ui,
                Rect::from_min_size(
                    copy_rect.min + vec2(0.0, 56.0),
                    vec2(content_width * 0.7, 56.0),
                ),
                8,
                !self.preferences.reduced_motion,
            );
        } else {
            // Without a logo, the title sits on the metadata line; long names
            // step down a size rather than collide with the year.
            let mut galley = paragraph(
                ui,
                &copy_item.name,
                content_width,
                44.0,
                text_color().gamma_multiply(alpha),
                3,
            );
            if galley.rows.len() > 1 {
                galley = paragraph(
                    ui,
                    &copy_item.name,
                    content_width,
                    34.0,
                    text_color().gamma_multiply(alpha),
                    2,
                );
            }
            ui.painter().galley(
                copy_rect.min + vec2(0.0, 128.0 - galley.size().y),
                galley,
                text_color().gamma_multiply(alpha),
            );
        }
        let meta = [
            copy_item
                .community_rating
                .map_or(String::new(), |r| format!("★ {r:.1}")),
            copy_item.year(),
            copy_item.runtime(),
        ]
        .into_iter()
        .filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>()
        .join("   ");
        text(
            ui.painter(),
            copy_rect.min + vec2(0.0, 142.0),
            meta,
            14.0,
            text_color().gamma_multiply(alpha),
        );
        let overview = paragraph(
            ui,
            copy_item.overview.as_deref().unwrap_or(""),
            content_width,
            16.0,
            Color32::from_rgb(213, 215, 220).gamma_multiply(alpha),
            2,
        );
        ui.painter().galley(
            copy_rect.min + vec2(0.0, 179.0),
            overview,
            text_color().gamma_multiply(alpha),
        );
        let action_item = copy_item.clone();
        let button = Rect::from_min_size(
            pos2(rect.left() + gutter, rect.bottom() - 74.0 - controls_offset),
            vec2(172.0, 48.0),
        );
        if action(
            ui,
            button,
            if action_item.kind == "Series" {
                "Ver episodios"
            } else {
                "▶  Reproducir"
            },
            true,
        )
        .clicked()
        {
            if action_item.kind == "Series" {
                self.open_detail(ui.ctx(), &action_item.id);
            } else {
                self.play(ui.ctx(), &action_item.id, true);
            }
        }
        if action(
            ui,
            button.translate(vec2(188.0, 0.0)),
            "Más información",
            false,
        )
        .clicked()
        {
            self.open_detail(ui.ctx(), &action_item.id);
        }
        let thumb_width = if width < 900.0 { 52.0 } else { 76.0 };
        for (index, item) in featured.iter().enumerate() {
            let r = Rect::from_min_size(
                pos2(
                    if stacked {
                        rect.left() + gutter + index as f32 * (thumb_width + 10.0)
                    } else {
                        rect.right()
                            - gutter
                            - (featured.len() - index) as f32 * (thumb_width + 10.0)
                    },
                    rect.bottom() - 68.0,
                ),
                vec2(thumb_width, 43.0),
            );
            if let Some((tex, wide)) = self.hero_artwork(ui.ctx(), item) {
                let target = if wide {
                    r
                } else {
                    let source = tex.size_vec2();
                    Rect::from_center_size(
                        r.center(),
                        source * (r.width() / source.x).min(r.height() / source.y),
                    )
                };
                image_cover(
                    ui,
                    &tex,
                    target,
                    6,
                    if index == self.hero { 1.0 } else { 0.55 },
                );
            } else {
                skeleton(ui, r, 6, !self.preferences.reduced_motion);
            }
            if index == self.hero {
                ui.painter().rect_stroke(
                    r.expand(3.0),
                    8,
                    Stroke::new(2.0_f32, text_color()),
                    StrokeKind::Outside,
                );
            }
            let response = ui
                .interact(
                    r.expand2(vec2(0.0, 2.0)),
                    Id::new(("feature", index)),
                    Sense::click(),
                )
                .on_hover_cursor(CursorIcon::PointingHand)
                .on_hover_text(&item.name);
            response.widget_info(|| {
                WidgetInfo::selected(
                    WidgetType::Button,
                    true,
                    index == self.hero,
                    format!("Destacado: {}", item.name),
                )
            });
            if response.has_focus() {
                ui.painter().rect_stroke(
                    r.expand(5.0),
                    10,
                    Stroke::new(2.0_f32, focus()),
                    StrokeKind::Outside,
                );
            }
            if response.clicked() {
                self.hero_cycle_at = Instant::now();
                self.hero_requested = if index == self.hero {
                    None
                } else {
                    Some(index)
                };
            }
            if self.hero_requested == Some(index)
                && (self
                    .artwork(ui.ctx(), item, item.art_kind(), 1920)
                    .is_none()
                    || (item.image_tags.contains_key("Logo")
                        && self.artwork(ui.ctx(), item, "Logo", 800).is_none()))
            {
                ctl::loading(ui, r.center(), 9.0);
            }
        }
        if t < 1.0 {
            ui.ctx().request_repaint();
        } else {
            self.hero_old = None;
        }
    }
    fn rail(&mut self, ui: &mut Ui, label: &str, items: &[Item], poster: bool, destination: Page) {
        if items.is_empty() {
            return;
        }
        ui.add_space(22.0);
        let gutter = if ui.available_width() < 900.0 {
            32.0
        } else {
            60.0
        };
        let width = ui.available_width();
        let (heading, _) = ui.allocate_exact_size(vec2(width, 46.0), Sense::hover());
        text(
            ui.painter(),
            heading.min + vec2(gutter, 6.0),
            label,
            25.0,
            text_color(),
        );
        if self.page != destination
            && ctl::button(
                ui,
                Rect::from_min_size(
                    pos2(heading.right() - gutter - 184.0, heading.top()),
                    vec2(88.0, 40.0),
                ),
                "Ver todo",
                None,
                ButtonStyle::Quiet,
            )
            .clicked()
        {
            self.page = destination;
            self.search.clear();
            self.genre.clear();
        }
        let stride = width - 2.0 * gutter;
        let limit = self.rail_limits.get(label).copied().unwrap_or(f32::MAX);
        for (direction, x, glyph) in [
            (-1.0, heading.right() - gutter - 88.0, Icon::Back),
            (1.0, heading.right() - gutter - 40.0, Icon::Forward),
        ] {
            let offset = self
                .rail_targets
                .get(label)
                .or_else(|| self.rail_offsets.get(label))
                .copied()
                .unwrap_or(0.0)
                .min(limit);
            let enabled = if direction < 0.0 {
                offset > 0.5
            } else {
                offset < limit - 0.5
            };
            let rect = Rect::from_min_size(pos2(x, heading.top()), Vec2::splat(44.0));
            let mut arrow_ui = ui.new_child(
                UiBuilder::new()
                    .id_salt((label, direction as i32))
                    .max_rect(rect),
            );
            if !enabled {
                arrow_ui.disable();
            }
            if ctl::icon_button(
                &mut arrow_ui,
                rect,
                glyph,
                if direction < 0.0 {
                    "Anteriores"
                } else {
                    "Siguientes"
                },
                false,
            )
            .clicked()
            {
                self.rail_targets.insert(
                    label.into(),
                    (offset + direction * stride).clamp(0.0, limit),
                );
            }
        }
        let columns = rail_columns(width - 2.0 * gutter, poster);
        let card_width = (width - 2.0 * gutter - 20.0 * (columns - 1.0)) / columns;
        if let Some(target) = self.rail_targets.get(label).copied() {
            let offset = self.rail_offsets.entry(label.into()).or_default();
            *offset = if self.preferences.reduced_motion {
                target
            } else {
                *offset
                    + (target - *offset)
                        * (1.0 - (-ui.input(|i| i.stable_dt).min(0.05) * 16.0).exp())
            };
            if (target - *offset).abs() < 0.5 {
                *offset = target;
                self.rail_targets.remove(label);
            } else {
                ui.ctx().request_repaint();
            }
        }
        let output = ScrollArea::horizontal()
            .id_salt(label)
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .horizontal_scroll_offset(*self.rail_offsets.get(label).unwrap_or(&0.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 20.0;
                    ui.add_space(gutter);
                    self.metadata.preview_frames = label == "Seguir viendo";
                    for item in items.iter().take(50) {
                        self.card(ui, item, card_width, poster);
                    }
                    self.metadata.preview_frames = false;
                    ui.add_space(gutter);
                });
            });
        self.rail_limits.insert(
            label.into(),
            (output.content_size.x - output.inner_rect.width()).max(0.0),
        );
        self.rail_offsets
            .insert(label.into(), output.state.offset.x);
        if let Some(target) = self.rail_targets.get_mut(label) {
            *target = target.min((output.content_size.x - output.inner_rect.width()).max(0.0));
        }
        let r = output.inner_rect;
        if output.state.offset.x > 1.0 {
            gradient(
                ui.painter(),
                Rect::from_min_size(r.min, vec2(gutter, r.height())),
                cinema(),
                Color32::TRANSPARENT,
                false,
            );
        }
        if output.state.offset.x + output.inner_rect.width() + 1.0 < output.content_size.x {
            gradient(
                ui.painter(),
                Rect::from_min_size(pos2(r.right() - gutter, r.top()), vec2(gutter, r.height())),
                Color32::TRANSPARENT,
                cinema(),
                false,
            );
        }
        ui.add_space(20.0);
    }
    fn card(&mut self, ui: &mut Ui, item: &Item, width: f32, poster: bool) {
        let height = if poster {
            width * 1.5
        } else {
            width * 9.0 / 16.0
        };
        let (title, metadata) = card_copy(item);
        let title_galley = paragraph(
            ui,
            title,
            width,
            15.0,
            text_color(),
            if poster { 2 } else { 1 },
        );
        let metadata_galley = paragraph(
            ui,
            &metadata,
            width,
            13.0,
            muted(),
            if !poster && item.kind == "Episode" {
                2
            } else {
                1
            },
        );
        // Layout keeps rows even; the clickable area hugs the art and its text.
        let (rect, _) = ui.allocate_exact_size(
            vec2(width, height + if poster { 56.0 } else { 74.0 }),
            Sense::hover(),
        );
        let text_bottom = if metadata.is_empty() {
            height + 14.0 + title_galley.size().y
        } else {
            height + 38.0 + metadata_galley.size().y
        };
        let hit = Rect::from_min_size(rect.min, vec2(width, text_bottom + 4.0));
        let response = ui
            .interact(hit, ui.id().with(("card", &item.id)), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        response.widget_info(|| {
            WidgetInfo::labeled(
                WidgetType::Button,
                true,
                if metadata.is_empty() {
                    title.to_owned()
                } else {
                    format!("{title}, {metadata}")
                },
            )
        });
        if ui.is_rect_visible(rect) {
            let hover = ctl::motion(
                ui.ctx(),
                response.id.with("art-hover"),
                response.hovered() || response.has_focus(),
                0.18,
            );
            let image_rect =
                Rect::from_min_size(rect.min + vec2(0.0, 4.0 - hover * 3.0), vec2(width, height));
            let kind = if poster { "Primary" } else { item.art_kind() };
            if let Some(texture) =
                self.artwork(ui.ctx(), item, kind, if poster { 400 } else { 720 })
            {
                let alpha = self.image_alpha(ui.ctx(), &texture);
                if alpha < 1.0 {
                    skeleton(ui, image_rect, 12, false);
                }
                image_cover(ui, &texture, image_rect, 12, alpha);
            } else {
                self.placeholder(
                    ui,
                    item,
                    kind,
                    if poster { 400 } else { 720 },
                    image_rect,
                    10,
                );
            }
            // Seguir viendo: on each hover a random frame of the video eases in
            // over the art (no zoom, nothing moves) and eases out on leaving.
            if self.metadata.preview_frames && !poster {
                let hovered = response.hovered() || response.has_focus();
                let reveal = ctl::motion(ui.ctx(), response.id.with("frame"), hovered, 0.38);
                let ready = self.ready_frames(ui.ctx(), item);
                let slot_id = response.id.with("frame-slot");
                let was = ui
                    .ctx()
                    .data(|d| d.get_temp::<bool>(response.id.with("was-hovered")));
                if hovered && was != Some(true) && !ready.is_empty() {
                    let pick = ready[(ui.input(|i| i.time) * 1000.0) as usize % ready.len()];
                    ui.ctx().data_mut(|d| d.insert_temp(slot_id, pick));
                }
                ui.ctx()
                    .data_mut(|d| d.insert_temp(response.id.with("was-hovered"), hovered));
                if reveal > 0.0
                    && let Some(slot) = ui.ctx().data(|d| d.get_temp::<usize>(slot_id))
                    && let Some(frame) = self.frame_texture(ui.ctx(), item, slot)
                {
                    image_cover(ui, &frame, image_rect, 12, reveal * reveal);
                }
            }
            if hover > 0.0 {
                ui.painter().rect_stroke(
                    image_rect.expand(2.0),
                    11,
                    Stroke::new(
                        1.5_f32,
                        if response.has_focus() {
                            focus()
                        } else {
                            Color32::from_white_alpha((125.0 * hover) as u8)
                        },
                    ),
                    StrokeKind::Outside,
                );
            }
            ui.painter().galley(
                pos2(rect.left(), image_rect.bottom() + 10.0),
                title_galley,
                text_color(),
            );
            ui.painter().galley(
                pos2(rect.left(), image_rect.bottom() + 34.0),
                metadata_galley,
                muted(),
            );
            if item.user_data.as_ref().is_some_and(|d| d.played) {
                ui.painter().circle_filled(
                    image_rect.right_top() + vec2(-17.0, 17.0),
                    10.0,
                    text_color(),
                );
                ctl::icon(
                    ui.painter(),
                    Rect::from_center_size(
                        image_rect.right_top() + vec2(-17.0, 17.0),
                        vec2(15.0, 15.0),
                    ),
                    Icon::Check,
                    cinema(),
                );
            } else if let Some(data) = &item.user_data
                && data.playback_position_ticks > 0
                && let Some(total) = item.run_time_ticks.filter(|n| *n > 0)
            {
                let rail = Rect::from_min_size(
                    image_rect.left_bottom() + vec2(10.0, -10.0),
                    vec2(width - 20.0, 3.0),
                );
                ui.painter()
                    .rect_filled(rail, 2, Color32::from_black_alpha(100));
                ui.painter().rect_filled(
                    Rect::from_min_size(
                        rail.min,
                        vec2(
                            rail.width()
                                * (data.playback_position_ticks as f32 / total as f32)
                                    .clamp(0.0, 1.0),
                            3.0,
                        ),
                    ),
                    2,
                    ctl::palette().primary,
                );
            }
        }
        if response.clicked() {
            self.open_detail(ui.ctx(), &item.id);
        }
    }
    fn browse(&mut self, ui: &mut Ui) {
        ui.add_space(122.0);
        if self.page == Page::Favorites {
            let resume = self.resume_items.clone();
            self.rail(ui, "Seguir viendo", &resume, false, Page::Favorites);
            if !resume.is_empty() {
                ui.add_space(26.0);
            }
        }
        let gutter = if ui.available_width() < 900.0 {
            32.0
        } else {
            60.0
        };
        let width = ui.available_width();
        ui.scope_builder(
            UiBuilder::new().max_rect(Rect::from_min_max(
                ui.cursor().min + vec2(gutter, 0.0),
                pos2(ui.max_rect().right() - gutter, ui.max_rect().bottom()),
            )),
            |ui| {
                ui.heading(match self.page {
                    Page::Favorites => "Favoritos",
                    Page::Movies => "Películas",
                    Page::Series => "Series",
                    _ => "Buscar",
                });
                ui.add_space(16.0);
                let compact = width < 900.0;
                let draw_search = |this: &mut Self, ui: &mut Ui, w: f32| {
                    let (r, _) = ui.allocate_exact_size(vec2(w, 46.0), Sense::hover());
                    let edit = ui.place(
                        r,
                        TextEdit::singleline(&mut this.search)
                            .id(Id::new("library-search"))
                            .margin(Margin {
                                left: 42,
                                right: 40,
                                top: 13,
                                bottom: 13,
                            })
                            .hint_text("Películas, series…"),
                    );
                    ctl::icon(
                        ui.painter(),
                        Rect::from_center_size(r.left_center() + vec2(22.0, 0.0), vec2(19.0, 19.0)),
                        Icon::Search,
                        muted(),
                    );
                    if ui
                        .ctx()
                        .data_mut(|d| d.remove_temp::<bool>(Id::new("focus-search")))
                        .unwrap_or(false)
                    {
                        edit.request_focus();
                    }
                    if !this.search.is_empty()
                        && ctl::icon_button(
                            ui,
                            Rect::from_center_size(
                                r.right_center() - vec2(21.0, 0.0),
                                vec2(40.0, 40.0),
                            ),
                            Icon::Close,
                            "Borrar búsqueda",
                            false,
                        )
                        .clicked()
                    {
                        this.search.clear();
                        edit.request_focus();
                    }
                };
                if compact && self.page == Page::Search {
                    draw_search(self, ui, ui.available_width());
                    ui.add_space(8.0);
                }
                ui.horizontal(|ui| {
                    if !compact && self.page == Page::Search {
                        draw_search(self, ui, (ui.available_width() - 420.0).clamp(220.0, 500.0));
                    }
                    let mut genres = vec![("Todos los géneros".into(), self.genre.is_empty())];
                    if let Some(catalog) = &self.catalog {
                        genres.extend(catalog.genres().into_keys().map(|g| {
                            let selected = self.genre == g;
                            (g, selected)
                        }));
                    }
                    let label = if self.genre.is_empty() {
                        "Todos los géneros"
                    } else {
                        &self.genre
                    };
                    if let Some(index) = ctl::picker(ui, "genre", label, 218.0, &genres) {
                        self.genre = if index == 0 {
                            String::new()
                        } else {
                            genres[index].0.clone()
                        };
                    }
                    let labels = ["Título", "Añadidas", "Año", "Valoración"];
                    let sorts = labels
                        .iter()
                        .enumerate()
                        .map(|(i, s)| (s.to_string(), i == self.sort))
                        .collect::<Vec<_>>();
                    if let Some(index) = ctl::picker(ui, "sort", labels[self.sort], 160.0, &sorts) {
                        self.sort = index;
                    }
                });
                ui.add_space(22.0);
                let Some(catalog) = self.catalog.clone() else {
                    if !self.busy {
                        self.unavailable(ui);
                        return;
                    }
                    let (r, _) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 240.0), Sense::hover());
                    skeleton(ui, r, 12, !self.preferences.reduced_motion);
                    return;
                };
                let search = jellyfin::search_key(&self.search);
                let mut items: Vec<_> = catalog
                    .items
                    .iter()
                    .filter(|item| match self.page {
                        Page::Favorites => item.user_data.as_ref().is_some_and(|d| d.is_favorite),
                        Page::Movies => item.kind == "Movie",
                        Page::Series => item.kind == "Series",
                        _ => true,
                    })
                    .filter(|item| {
                        self.genre.is_empty()
                            || item.display_genres().contains(&self.genre.as_str())
                    })
                    .filter(|item| {
                        search.is_empty()
                            || jellyfin::search_key(&format!(
                                "{} {} {}",
                                item.name,
                                item.year(),
                                item.display_genres().join(" ")
                            ))
                            .contains(&search)
                    })
                    .cloned()
                    .collect();
                match self.sort {
                    1 => items.sort_by(|a, b| b.date_created.cmp(&a.date_created)),
                    2 => items.sort_by_key(|a| std::cmp::Reverse(a.production_year)),
                    3 => items.sort_by(|a, b| {
                        b.community_rating
                            .unwrap_or(0.0)
                            .total_cmp(&a.community_rating.unwrap_or(0.0))
                    }),
                    _ => items.sort_by(|a, b| a.name.cmp(&b.name)),
                };
                if items.is_empty() {
                    ui.add_space(50.0);
                    ui.heading(
                        if self.page == Page::Favorites
                            && self.search.is_empty()
                            && self.genre.is_empty()
                        {
                            "Tus favoritos, aquí"
                        } else {
                            "No hay coincidencias"
                        },
                    );
                    ui.label(
                        if self.page == Page::Favorites
                            && self.search.is_empty()
                            && self.genre.is_empty()
                        {
                            "Guarda películas y series desde su ficha para encontrarlas fácilmente."
                        } else {
                            "Prueba otro título o elimina un filtro."
                        },
                    );
                    if !self.search.is_empty() || !self.genre.is_empty() {
                        ui.add_space(12.0);
                        let (r, _) = ui.allocate_exact_size(vec2(180.0, 44.0), Sense::hover());
                        if action(ui, r, "Quitar filtros", false).clicked() {
                            self.search.clear();
                            self.genre.clear();
                        }
                    }
                }
                let columns = ((ui.available_width() + 20.0) / 180.0).floor().max(2.0) as usize;
                let card_width =
                    (ui.available_width() - 20.0 * (columns - 1) as f32) / columns as f32;
                for row in items.chunks(columns) {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 20.0;
                        for item in row {
                            self.card(ui, item, card_width, true);
                        }
                    });
                    ui.add_space(22.0);
                }
            },
        );
    }
    fn episodes_ui(&mut self, ui: &mut Ui) {
        let mut seasons = BTreeMap::<u32, usize>::new();
        for episode in &self.episodes {
            *seasons
                .entry(episode.parent_index_number.unwrap_or(0))
                .or_default() += 1;
        }
        let (heading, _) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::hover());
        let title = ui.painter().layout_no_wrap(
            "Episodios".into(),
            FontId::proportional(26.0),
            text_color(),
        );
        ui.painter().galley(
            heading.left_center() - vec2(0.0, title.size().y / 2.0),
            title.clone(),
            text_color(),
        );
        if !self.busy {
            ui.painter().text(
                heading.left_center() + vec2(title.size().x + 14.0, 0.0),
                Align2::LEFT_CENTER,
                seasons.get(&self.season).copied().unwrap_or(0).to_string(),
                FontId::proportional(16.0),
                muted(),
            );
        }
        ui.add_space(12.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
            for season in seasons.keys() {
                let label = if *season == 0 {
                    "Especiales".into()
                } else {
                    format!("T{season}")
                };
                let width = if *season == 0 { 108.0 } else { 52.0 };
                let (rect, _) = ui.allocate_exact_size(vec2(width, 44.0), Sense::hover());
                let selected = self.season == *season;
                let response = ctl::button(
                    ui,
                    rect,
                    &label,
                    None,
                    if selected {
                        ButtonStyle::Primary
                    } else {
                        ButtonStyle::Quiet
                    },
                );
                response.widget_info(|| {
                    WidgetInfo::selected(
                        WidgetType::SelectableLabel,
                        true,
                        selected,
                        if *season == 0 {
                            "Especiales".into()
                        } else {
                            format!("Temporada {season}")
                        },
                    )
                });
                if response.clicked() {
                    self.season = *season;
                    self.episode_page = 0;
                }
            }
        });
        ui.add_space(20.0);
        let columns = ((ui.available_width() + 20.0) / 280.0)
            .floor()
            .clamp(1.0, 4.0) as usize;
        let width = (ui.available_width() - 20.0 * (columns - 1) as f32) / columns as f32;
        let picture_height = width * 9.0 / 16.0;
        if self.busy {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 20.0;
                for _ in 0..columns {
                    let (r, _) =
                        ui.allocate_exact_size(vec2(width, picture_height + 72.0), Sense::hover());
                    skeleton(
                        ui,
                        Rect::from_min_size(r.min, vec2(width, picture_height)),
                        12,
                        !self.preferences.reduced_motion,
                    );
                    skeleton(
                        ui,
                        Rect::from_min_size(
                            r.min + vec2(0.0, picture_height + 16.0),
                            vec2(width * 0.7, 18.0),
                        ),
                        5,
                        !self.preferences.reduced_motion,
                    );
                }
            });
            return;
        }
        let episodes: Vec<_> = self
            .episodes
            .iter()
            .filter(|e| e.parent_index_number.unwrap_or(0) == self.season)
            .cloned()
            .collect();
        let pages = episodes.len().div_ceil(24).max(1);
        self.episode_page = self.episode_page.min(pages - 1);
        for row in episodes
            .iter()
            .skip(self.episode_page * 24)
            .take(24)
            .collect::<Vec<_>>()
            .chunks(columns)
        {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 20.0;
                for episode in row {
                    let (r, response) =
                        ui.allocate_exact_size(vec2(width, picture_height + 100.0), Sense::click());
                    let response = response.on_hover_cursor(CursorIcon::PointingHand);
                    response.widget_info(|| {
                        WidgetInfo::labeled(
                            WidgetType::Button,
                            true,
                            format!("Reproducir {}", episode.name),
                        )
                    });
                    if ui.is_rect_visible(r) {
                        let picture = Rect::from_min_size(r.min, vec2(width, picture_height));
                        if let Some(texture) = self.artwork(ui.ctx(), episode, "Primary", 720) {
                            image_cover(
                                ui,
                                &texture,
                                picture,
                                12,
                                self.image_alpha(ui.ctx(), &texture),
                            );
                        } else {
                            self.placeholder(ui, episode, "Primary", 720, picture, 12);
                        }
                        let hover = ctl::motion(
                            ui.ctx(),
                            response.id.with("episode-hover"),
                            response.hovered() || response.has_focus(),
                            0.12,
                        );
                        if hover > 0.0 {
                            ui.painter().rect_stroke(
                                picture.expand(2.0),
                                13,
                                Stroke::new(
                                    1.5_f32,
                                    if response.has_focus() {
                                        focus()
                                    } else {
                                        Color32::from_white_alpha((160.0 * hover) as u8)
                                    },
                                ),
                                StrokeKind::Outside,
                            );
                            ui.painter().circle_filled(
                                picture.center(),
                                24.0,
                                Color32::from_black_alpha((175.0 * hover) as u8),
                            );
                            ctl::icon(
                                ui.painter(),
                                Rect::from_center_size(picture.center(), Vec2::splat(24.0)),
                                Icon::Play,
                                text_color().gamma_multiply(hover),
                            );
                        }
                        let duration = episode.runtime();
                        if !duration.is_empty() {
                            let label = ui.painter().layout_no_wrap(
                                duration,
                                FontId::proportional(12.0),
                                text_color(),
                            );
                            let badge = Rect::from_min_size(
                                picture.right_bottom() - label.size() - vec2(22.0, 16.0),
                                label.size() + vec2(12.0, 8.0),
                            );
                            ui.painter()
                                .rect_filled(badge, 5, Color32::from_black_alpha(180));
                            ui.painter()
                                .galley(badge.min + vec2(6.0, 4.0), label, text_color());
                        }
                        let name = paragraph(
                            ui,
                            &format!("{}. {}", episode.index_number.unwrap_or(0), episode.name),
                            width,
                            16.0,
                            text_color(),
                            2,
                        );
                        ui.painter().galley(
                            r.min + vec2(0.0, picture_height + 12.0),
                            name,
                            text_color(),
                        );
                        let description = paragraph(
                            ui,
                            episode.overview.as_deref().unwrap_or(""),
                            width,
                            13.0,
                            muted(),
                            2,
                        );
                        ui.painter().galley(
                            r.min + vec2(0.0, picture_height + 56.0),
                            description,
                            muted(),
                        );
                    }
                    if response.clicked() {
                        self.play(ui.ctx(), &episode.id, true);
                    }
                }
            });
            ui.add_space(20.0);
        }
        if pages > 1 {
            ui.horizontal(|ui| {
                for (back, icon, label) in [
                    (true, Icon::Back, "Anterior"),
                    (false, Icon::Forward, "Siguiente"),
                ] {
                    let (rect, _) = ui.allocate_exact_size(vec2(44.0, 44.0), Sense::hover());
                    let enabled = if back {
                        self.episode_page > 0
                    } else {
                        self.episode_page + 1 < pages
                    };
                    if ui
                        .add_enabled_ui(enabled, |ui| {
                            ctl::icon_button(ui, rect, icon, label, false).clicked()
                        })
                        .inner
                    {
                        if back {
                            self.episode_page -= 1;
                        } else {
                            self.episode_page += 1;
                        }
                    }
                    if back {
                        ui.label(format!("{} / {pages}", self.episode_page + 1));
                    }
                }
            });
        }
    }

    fn detail_ui(&mut self, ui: &mut Ui) {
        let Some(item) = self.detail.clone() else {
            self.library_skeleton(ui);
            return;
        };
        let width = ui.available_width();
        let viewport = ui.ctx().content_rect().height();
        let series = item.kind == "Series";
        let wide_art = has_wide_art(&item);
        // The stage takes most of the screen; everything sits on it, bottom-left.
        let height =
            (viewport * if series { 0.74 } else { 0.88 }).clamp(520.0, (width / 1.55).max(560.0));
        let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
        let stage_width =
            ((ui.ctx().content_rect().width().max(1280.0) / 320.0).ceil() * 320.0) as u32;
        let gutter = if width < 900.0 { 32.0 } else { 60.0 };
        if wide_art {
            if let Some(texture) = self.artwork(ui.ctx(), &item, item.art_kind(), stage_width) {
                backdrop(ui, &texture, rect, self.image_alpha(ui.ctx(), &texture));
            } else {
                self.placeholder(ui, &item, item.art_kind(), stage_width, rect, 0);
            }
        } else if let Some(poster) = self.artwork(ui.ctx(), &item, "Primary", 700) {
            // Poster-only titles: the poster's own colour field fills the stage
            // and the poster stands intact on the right.
            if let Some(field) = self
                .images
                .get(&poster.name())
                .and_then(|slot| slot.ambient.clone())
            {
                backdrop(ui, &field, rect, 0.8);
            }
            let size = poster.size_vec2();
            let scale = (height * 0.62 / size.y).min(width * 0.26 / size.x);
            let frame = Rect::from_min_size(
                pos2(
                    rect.right() - gutter - (width * 0.06).min(110.0) - size.x * scale,
                    rect.top() + height * 0.2,
                ),
                size * scale,
            );
            ui.painter().add(
                egui::epaint::Shadow {
                    offset: [0, 18],
                    blur: 48,
                    spread: 0,
                    color: Color32::from_black_alpha(150),
                }
                .as_shape(frame, 12),
            );
            image_cover(ui, &poster, frame, 12, self.image_alpha(ui.ctx(), &poster));
        }
        gradient(
            ui.painter(),
            Rect::from_min_size(rect.min, vec2(width * 0.72, height)),
            Color32::from_black_alpha(232),
            Color32::TRANSPARENT,
            false,
        );
        gradient(
            ui.painter(),
            Rect::from_min_max(rect.min + vec2(0.0, height * 0.42), rect.max),
            Color32::TRANSPARENT,
            cinema(),
            true,
        );
        if action(
            ui,
            Rect::from_min_size(rect.min + vec2(gutter, 100.0), vec2(110.0, 40.0)),
            "‹  Volver",
            false,
        )
        .clicked()
        {
            self.page = self.previous;
        }

        // Layout from the bottom up, so long or short texts never overlap.
        let content_width = (width * 0.46).clamp(340.0, 640.0);
        let mut y = rect.bottom() - 56.0;
        let actions_top = y - 52.0;
        y = actions_top - 22.0;
        let user_data = item.user_data.clone().unwrap_or_default();
        let resume = !series && user_data.playback_position_ticks > 0 && !user_data.played;
        let progress = item
            .run_time_ticks
            .filter(|t| *t > 0 && resume)
            .map(|total| {
                let done = user_data.playback_position_ticks as f64 / total as f64;
                let left =
                    (total.saturating_sub(user_data.playback_position_ticks)) as f64 / 1e7 / 60.0;
                (done.clamp(0.0, 1.0) as f32, left.ceil() as u64)
            });
        if let Some((done, left)) = progress {
            let bar = Rect::from_min_size(pos2(rect.left() + gutter, y - 4.0), vec2(220.0, 4.0));
            ui.painter()
                .rect_filled(bar, 2, Color32::from_white_alpha(46));
            ui.painter().rect_filled(
                Rect::from_min_size(bar.min, vec2(bar.width() * done, 4.0)),
                2,
                ctl::palette().primary,
            );
            text(
                ui.painter(),
                bar.right_top() + vec2(14.0, -8.0),
                if left >= 60 {
                    format!("Quedan {} h {:02} min", left / 60, left % 60)
                } else {
                    format!("Quedan {left} min")
                },
                13.0,
                text_color(),
            );
            y -= 30.0;
        }
        let overview = item.overview.clone().unwrap_or_default();
        let synopsis = paragraph(
            ui,
            overview.trim(),
            content_width,
            17.0,
            text_color().gamma_multiply(0.92),
            5,
        );
        let truncated = synopsis.elided;
        if truncated {
            let more = Rect::from_min_size(
                pos2(rect.left() + gutter - 10.0, y - 30.0),
                vec2(110.0, 32.0),
            );
            if ctl::button(ui, more, "Leer más", None, ButtonStyle::Quiet).clicked() {
                ui.ctx()
                    .data_mut(|d| d.insert_temp(Id::new(("synopsis", &item.id)), true));
            }
            y -= 38.0;
        }
        if !overview.trim().is_empty() {
            y -= synopsis.size().y;
            ui.painter()
                .galley(pos2(rect.left() + gutter, y), synopsis, text_color());
            y -= 18.0;
        }
        let genres = item.display_genres();
        let meta = [
            item.year(),
            item.runtime(),
            item.community_rating
                .map_or(String::new(), |r| format!("★ {r:.1}")),
            genres
                .iter()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .join(", "),
        ]
        .into_iter()
        .filter(|s| !s.trim().is_empty())
        .collect::<Vec<_>>()
        .join("   ·   ");
        if !meta.is_empty() {
            y -= 20.0;
            text(
                ui.painter(),
                pos2(rect.left() + gutter, y),
                meta,
                14.0,
                muted(),
            );
            y -= 22.0;
        }
        if let Some(logo) = self.artwork(ui.ctx(), &item, "Logo", 800) {
            let size = logo.size_vec2();
            // Never reach the back button: shrink the logo into the free space.
            let room = (y - (rect.top() + 100.0 + 40.0 + 28.0)).clamp(48.0, 130.0);
            let scale = (460.0_f32.min(content_width) / size.x).min(room / size.y);
            let r =
                Rect::from_min_size(pos2(rect.left() + gutter, y - size.y * scale), size * scale);
            title_logo(ui, &logo, r, self.image_alpha(ui.ctx(), &logo));
        } else {
            let mut galley = paragraph(ui, &item.name, content_width, 46.0, text_color(), 3);
            if galley.rows.len() > 1 {
                galley = paragraph(ui, &item.name, content_width, 36.0, text_color(), 2);
            }
            ui.painter().galley(
                pos2(rect.left() + gutter, y - galley.size().y),
                galley,
                text_color(),
            );
        }

        // Actions: play (or the next episode of a series), restart, favourite, images.
        let mut x = rect.left() + gutter;
        let next_episode = series.then(|| self.next_episode_for_series()).flatten();
        let primary_label = if series {
            next_episode.as_ref().map(|(episode, started)| {
                let code = format!(
                    "T{} · E{}",
                    episode.parent_index_number.unwrap_or(0),
                    episode.index_number.unwrap_or(0)
                );
                if *started {
                    format!("▶  Continuar {code}")
                } else {
                    format!("▶  Empezar {code}")
                }
            })
        } else {
            Some(if resume {
                "▶  Continuar".to_owned()
            } else {
                "▶  Reproducir".to_owned()
            })
        };
        if let Some(label) = primary_label {
            let w = (label.chars().count() as f32 * 9.0 + 56.0).max(200.0);
            if action(
                ui,
                Rect::from_min_size(pos2(x, actions_top), vec2(w, 52.0)),
                &label,
                true,
            )
            .clicked()
            {
                match &next_episode {
                    Some((episode, _)) => self.play(ui.ctx(), &episode.id, true),
                    None => self.play(ui.ctx(), &item.id, true),
                }
            }
            x += w + 14.0;
        }
        if resume {
            if action(
                ui,
                Rect::from_min_size(pos2(x, actions_top), vec2(190.0, 52.0)),
                "Desde el principio",
                false,
            )
            .clicked()
            {
                self.play(ui.ctx(), &item.id, false);
            }
            x += 204.0;
        }
        let favorite = user_data.is_favorite;
        ui.scope(|ui| {
            if ctl::icon_button(
                ui,
                Rect::from_min_size(pos2(x, actions_top), Vec2::splat(52.0)),
                Icon::Heart,
                if favorite {
                    "Quitar de favoritos"
                } else {
                    "Añadir a favoritos"
                },
                favorite,
            )
            .clicked()
            {
                self.toggle_favorite(ui.ctx(), item.clone());
            }
            if self.has_artwork_choices(&item.id)
                && ctl::icon_button(
                    ui,
                    Rect::from_min_size(pos2(x + 64.0, actions_top), Vec2::splat(52.0)),
                    Icon::Picture,
                    "Cambiar imágenes",
                    false,
                )
                .clicked()
            {
                self.open_artwork_picker(&item.id, 0);
            }
        });

        // Full synopsis in a dialog, only when it does not fit on the stage.
        let open_id = Id::new(("synopsis", &item.id));
        if ui
            .ctx()
            .data(|d| d.get_temp::<bool>(open_id))
            .unwrap_or(false)
        {
            let close = ctl::dialog(ui.ctx(), "synopsis", &item.name, 640.0, |ui| {
                ui.label(RichText::new(overview.trim()).size(16.0));
            });
            if close {
                ui.ctx().data_mut(|d| d.remove::<bool>(open_id));
            }
        }

        if series {
            ui.add_space(8.0);
            ui.scope_builder(
                UiBuilder::new().max_rect(Rect::from_min_max(
                    ui.cursor().min + vec2(gutter, 0.0),
                    pos2(ui.max_rect().right() - gutter, ui.max_rect().bottom()),
                )),
                |ui| self.episodes_ui(ui),
            );
        }
    }
    /// The episode a series continues with: one in progress, else the one after
    /// the last finished, else the first. `true` when the series was started.
    fn next_episode_for_series(&self) -> Option<(Item, bool)> {
        let regular: Vec<&Item> = self
            .episodes
            .iter()
            .filter(|e| e.parent_index_number.unwrap_or(0) > 0)
            .collect();
        let list = if regular.is_empty() {
            self.episodes.iter().collect()
        } else {
            regular
        };
        if let Some(current) = list.iter().find(|e| history::resumable(e)) {
            return Some(((*current).clone(), true));
        }
        let last_played = list
            .iter()
            .rposition(|e| e.user_data.as_ref().is_some_and(|d| d.played));
        match last_played {
            Some(index) => list.get(index + 1).map(|e| ((*e).clone(), true)),
            None => list.first().map(|e| ((*e).clone(), false)),
        }
    }
    fn settings(&mut self, ui: &mut Ui) {
        ui.add_space(112.0);
        let width = ui.available_width();
        let inset = ((width - 800.0) / 2.0).max(32.0);
        let area = Rect::from_min_max(
            ui.cursor().min + vec2(inset, 0.0),
            pos2(ui.max_rect().right() - inset, ui.max_rect().bottom()),
        );
        ui.scope_builder(UiBuilder::new().max_rect(area), |ui| {
            ui.heading("Ajustes");
            ui.add_space(24.0);
            self.profile_settings_ui(ui);
            self.connections_ui(ui);
            ui.add_space(8.0);
            let providers: Vec<_> = self
                .metadata
                .settings
                .providers
                .iter()
                .filter(|e| self.metadata.settings.active(e.provider))
                .map(|e| e.provider.name())
                .collect();
            let theme_name = ctl::THEMES
                .iter()
                .find(|t| t.0 == ctl::theme_id())
                .map_or("Cine", |t| t.1);
            let rows = [
                (
                    Dialog::Appearance,
                    "Apariencia",
                    format!("Tema {theme_name}"),
                ),
                (
                    Dialog::Experience,
                    "Reproducción y movimiento",
                    format!(
                        "Movimiento {} · siguiente episodio {}",
                        if self.preferences.reduced_motion {
                            "reducido"
                        } else {
                            "completo"
                        },
                        if self.auto_next {
                            "automático"
                        } else {
                            "manual"
                        }
                    ),
                ),
                (
                    Dialog::Metadata,
                    "Imágenes y fichas",
                    if providers.is_empty() {
                        "Sin proveedores activos".into()
                    } else {
                        providers.join(" · ")
                    },
                ),
                (
                    Dialog::Updates,
                    "Actualizaciones",
                    if self.updates.release.is_some() {
                        "Nueva versión disponible".into()
                    } else {
                        concat!("Megalodon ", env!("CARGO_PKG_VERSION")).into()
                    },
                ),
                (
                    Dialog::Advanced,
                    "Más opciones",
                    "Abrir un vídeo y servicios locales".into(),
                ),
            ];
            for (dialog, title, summary) in rows {
                if settings_row(ui, title, &summary, self.settings_dialog == Some(dialog)) {
                    self.settings_dialog = Some(dialog);
                }
            }
        });
    }
}

fn settings_row(ui: &mut Ui, title: &str, summary: &str, open: bool) -> bool {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 72.0), Sense::hover());
    let row = r.shrink2(vec2(0.0, 3.0));
    let response = ui
        .interact(row, Id::new(("settings-row", title)), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    response.widget_info(|| {
        WidgetInfo::labeled(WidgetType::Button, true, format!("{title}. {summary}"))
    });
    let hover = ctl::motion(
        ui.ctx(),
        response.id.with("hover"),
        response.hovered() || response.has_focus() || open,
        0.16,
    );
    ui.painter().rect_filled(
        row,
        14,
        Color32::from_white_alpha((7.0 + hover * 9.0) as u8),
    );
    text(
        ui.painter(),
        row.left_top() + vec2(20.0, 13.0),
        title,
        17.0,
        text_color(),
    );
    let summary = paragraph(ui, summary, row.width() - 90.0, 13.0, muted(), 1);
    ui.painter()
        .galley(row.left_top() + vec2(20.0, 39.0), summary, muted());
    ctl::icon(
        ui.painter(),
        Rect::from_center_size(row.right_center() - vec2(28.0, 0.0), Vec2::splat(20.0)),
        Icon::Forward,
        muted(),
    );
    if response.has_focus() {
        ui.painter()
            .rect_stroke(row, 14, Stroke::new(2.0_f32, focus()), StrokeKind::Inside);
    }
    response.clicked()
}
