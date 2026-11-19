//! First run: name and avatar, a way of seeing the app, then a library.
//! Four short steps; every one can be changed later in Ajustes.
use super::controls::{self as ctl, ButtonStyle, Icon};
use super::profile_ui::paint_avatar;
use super::*;

pub(super) const STEPS: usize = 4;

/// Theme choice cards, shared by the first run and Ajustes → Apariencia.
/// Returns the id picked this frame.
pub(super) fn theme_cards(ui: &mut Ui, current: &str) -> Option<&'static str> {
    let width = ui.available_width();
    let columns = if width < 560.0 { 1.0 } else { 2.0 };
    let gap = 16.0;
    let card_w = (width - gap * (columns - 1.0)) / columns;
    let card_h = 148.0;
    let rows = (ctl::THEMES.len() as f32 / columns).ceil();
    let (grid, _) = ui.allocate_exact_size(
        vec2(width, rows * card_h + (rows - 1.0) * gap),
        Sense::hover(),
    );
    let mut picked = None;
    for (index, (id, name, description, palette)) in ctl::THEMES.iter().enumerate() {
        let col = (index % columns as usize) as f32;
        let row = (index / columns as usize) as f32;
        let card = Rect::from_min_size(
            grid.min + vec2(col * (card_w + gap), row * (card_h + gap)),
            vec2(card_w, card_h),
        );
        let response = ui
            .interact(card, Id::new(("theme-card", *id)), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        let selected = current == *id;
        response
            .widget_info(|| WidgetInfo::selected(WidgetType::RadioButton, true, selected, *name));
        let hover = ctl::motion(
            ui.ctx(),
            response.id.with("hover"),
            response.hovered() || response.has_focus(),
            0.14,
        );
        // A miniature of the interface in that theme: canvas, a hero band,
        // two cards, a line of text and the focus accent.
        ui.painter().rect_filled(card, 14, palette.cinema);
        let preview =
            Rect::from_min_size(card.min + vec2(14.0, 14.0), vec2(card.width() - 28.0, 74.0));
        ui.painter().rect_filled(preview, 8, palette.screen);
        for i in 0..3 {
            ui.painter().rect_filled(
                Rect::from_min_size(
                    preview.min + vec2(10.0 + i as f32 * 46.0, 34.0),
                    vec2(38.0, 30.0),
                ),
                5,
                palette.cinema.lerp_to_gamma(palette.muted, 0.18),
            );
        }
        ui.painter().rect_filled(
            Rect::from_min_size(preview.min + vec2(10.0, 12.0), vec2(80.0, 8.0)),
            4,
            palette.text,
        );
        ui.painter().rect_filled(
            Rect::from_min_size(preview.right_top() + vec2(-58.0, 12.0), vec2(46.0, 14.0)),
            7,
            palette.primary,
        );
        ui.painter().text(
            card.left_top() + vec2(16.0, 100.0),
            Align2::LEFT_TOP,
            *name,
            FontId::proportional(16.0),
            palette.text,
        );
        ui.painter().text(
            card.left_top() + vec2(16.0, 122.0),
            Align2::LEFT_TOP,
            *description,
            FontId::proportional(12.5),
            palette.muted,
        );
        ui.painter().rect_stroke(
            card,
            14,
            Stroke::new(
                if selected { 2.5_f32 } else { 1.0 },
                if selected {
                    ctl::focus()
                } else {
                    Color32::from_white_alpha((28.0 + 60.0 * hover) as u8)
                },
            ),
            StrokeKind::Inside,
        );
        if selected {
            let badge = card.right_top() + vec2(-22.0, card_h - 30.0);
            ui.painter().circle_filled(badge, 11.0, ctl::focus());
            ctl::icon(
                ui.painter(),
                Rect::from_center_size(badge, Vec2::splat(14.0)),
                Icon::Check,
                palette.cinema,
            );
        }
        if response.clicked() {
            picked = Some(*id);
        }
    }
    picked
}

impl Megalodon {
    pub(super) fn apply_theme(&mut self, ctx: &Context, id: &str) {
        ctl::set_theme(id);
        self.preferences.theme = ctl::theme_id().into();
        ui::theme(ctx);
        let _ = self.save_preferences();
    }

    fn finish_onboarding(&mut self) {
        self.onboarding = None;
        self.preferences.onboarded = true;
        let _ = self.save_preferences();
        self.page = Page::Home;
    }

    pub(super) fn onboarding_ui(&mut self, ctx: &Context) {
        let Some(step) = self.onboarding else {
            return;
        };
        // A connection added from the last step completes the welcome.
        if step == STEPS - 1 && self.sources.active.is_some() && !self.source_editor.open {
            self.finish_onboarding();
            return;
        }
        CentralPanel::default()
            .frame(Frame::NONE.fill(ctl::cinema()))
            .show(ctx, |ui| {
                let viewport = ui.max_rect();
                let width = (viewport.width() - 64.0).min(680.0);
                let column = Rect::from_min_size(
                    pos2(
                        viewport.center().x - width / 2.0,
                        viewport.top() + (viewport.height() * 0.12).clamp(40.0, 120.0),
                    ),
                    vec2(width, viewport.height()),
                );
                ui.scope_builder(UiBuilder::new().max_rect(column), |ui| {
                    // Progress: four short segments.
                    let (bar, _) = ui.allocate_exact_size(vec2(width, 4.0), Sense::hover());
                    let segment = (width - 3.0 * 8.0) / STEPS as f32;
                    for i in 0..STEPS {
                        ui.painter().rect_filled(
                            Rect::from_min_size(
                                bar.min + vec2(i as f32 * (segment + 8.0), 0.0),
                                vec2(segment, 4.0),
                            ),
                            2,
                            if i <= step {
                                ctl::text_color()
                            } else {
                                Color32::from_white_alpha(28)
                            },
                        );
                    }
                    ui.add_space(40.0);
                    match step {
                        0 => self.welcome_step(ui),
                        1 => self.profile_step(ui),
                        2 => self.theme_step(ui),
                        _ => self.library_step(ui),
                    }
                });
            });
    }

    fn step_actions(&mut self, ui: &mut Ui, back: bool, next: &str, enabled: bool) -> bool {
        ui.add_space(32.0);
        let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 48.0), Sense::hover());
        if back {
            let r = Rect::from_min_size(row.min, vec2(120.0, 48.0));
            if ctl::button(ui, r, "Atrás", Some(Icon::Back), ButtonStyle::Quiet).clicked() {
                self.onboarding = self.onboarding.map(|s| s.saturating_sub(1));
            }
        }
        let width = (next.chars().count() as f32 * 8.5 + 56.0).max(160.0);
        let r = Rect::from_min_size(pos2(row.right() - width, row.top()), vec2(width, 48.0));
        let mut cell = ui.new_child(
            UiBuilder::new()
                .id_salt(("onboarding-next", next))
                .max_rect(r),
        );
        if !enabled {
            cell.disable();
        }
        ctl::button(&mut cell, r, next, None, ButtonStyle::Primary).clicked()
    }

    fn welcome_step(&mut self, ui: &mut Ui) {
        let (mark, _) = ui.allocate_exact_size(vec2(ui.available_width(), 96.0), Sense::hover());
        ui.put(
            Rect::from_min_size(mark.min, Vec2::splat(96.0)),
            Image::new(&self.brand).fit_to_exact_size(Vec2::splat(96.0)),
        );
        ui.add_space(28.0);
        ui.label(RichText::new("Te damos la bienvenida a Megalodon").size(34.0));
        ui.add_space(12.0);
        ui.label(
            RichText::new(
                "Tus películas y series, con carteles, fondos y logotipos que se completan solos, perfiles para cada persona y Seguir viendo donde lo dejaste.",
            )
            .size(17.0)
            .color(ctl::muted()),
        );
        ui.add_space(8.0);
        ui.label(
            RichText::new("Tres pasos y listo. Todo se puede cambiar después en Ajustes.")
                .size(14.0)
                .color(ctl::muted()),
        );
        if self.step_actions(ui, false, "Empezar", true) {
            let name = self.current_user().name;
            self.onboarding_name = if name == "Mi perfil" {
                String::new()
            } else {
                name
            };
            self.onboarding = Some(1);
        }
    }

    fn profile_step(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("¿Quién va a ver?").size(30.0));
        ui.add_space(8.0);
        ui.label(
            RichText::new("Tu nombre para el perfil. Su color se asigna solo; podrás poner tu foto cuando conectes tu biblioteca.")
                .size(15.0)
                .color(ctl::muted()),
        );
        ui.add_space(28.0);
        let user = self.current_user();
        let auto = self.auto_avatar(&user.id);
        let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 96.0), Sense::hover());
        let name = if self.onboarding_name.trim().is_empty() {
            "?".to_owned()
        } else {
            self.onboarding_name.clone()
        };
        paint_avatar(
            ui,
            Some(&auto),
            None,
            &name,
            Rect::from_min_size(row.min, Vec2::splat(96.0)),
            1.0,
        );
        let field = Rect::from_min_size(
            row.min + vec2(120.0, 26.0),
            vec2((row.width() - 120.0).max(160.0), 48.0),
        );
        let response = ui.put(
            field,
            TextEdit::singleline(&mut self.onboarding_name)
                .hint_text("Tu nombre")
                .font(FontId::proportional(20.0))
                .margin(vec2(14.0, 12.0)),
        );
        if self.onboarding_name.is_empty() && !response.has_focus() {
            response.request_focus();
        }
        let ready = !self.onboarding_name.trim().is_empty();
        let enter = ready && ui.input(|i| i.key_pressed(Key::Enter));
        if self.step_actions(ui, true, "Continuar", ready) || enter {
            let name: String = self.onboarding_name.trim().chars().take(40).collect();
            let profile = self.preferences.local_profile.clone();
            self.preferences.local_profiles.insert(profile, name);
            let _ = self.save_preferences();
            self.load_local_portrait(ui.ctx());
            self.onboarding = Some(2);
        }
    }

    fn theme_step(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Elige cómo quieres verlo").size(30.0));
        ui.add_space(8.0);
        ui.label(
            RichText::new(
                "El tema cambia el fondo y los acentos; carteles y fondos conservan sus colores.",
            )
            .size(15.0)
            .color(ctl::muted()),
        );
        ui.add_space(24.0);
        let current = ctl::theme_id();
        if let Some(id) = theme_cards(ui, current) {
            self.apply_theme(ui.ctx(), id);
        }
        if self.step_actions(ui, true, "Continuar", true) {
            self.onboarding = Some(3);
        }
    }

    fn library_step(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Conecta tu biblioteca").size(30.0));
        ui.add_space(8.0);
        ui.label(
            RichText::new(
                "Megalodon lee tus vídeos donde ya estén. No mueve ni modifica tus archivos.",
            )
            .size(15.0)
            .color(ctl::muted()),
        );
        ui.add_space(24.0);
        let options = [
            (
                0,
                Icon::Folder,
                "Carpeta de este equipo",
                "Un disco o carpeta con tus películas y series",
            ),
            (
                1,
                Icon::Cloud,
                "Google Drive",
                "Inicia sesión en el navegador; acceso de solo lectura",
            ),
            (
                3,
                Icon::Cloud,
                "NAS o WebDAV",
                "Nextcloud, Synology u otro servidor de archivos",
            ),
            (
                2,
                Icon::Server,
                "Servidor Jellyfin",
                "Si ya tienes uno, usa su biblioteca",
            ),
        ];
        for (kind, icon, title, detail) in options {
            let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 72.0), Sense::hover());
            let tile = row.shrink2(vec2(0.0, 4.0));
            let response = ui
                .interact(tile, Id::new(("onboarding-source", kind)), Sense::click())
                .on_hover_cursor(CursorIcon::PointingHand);
            response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, title));
            let hover = ctl::motion(
                ui.ctx(),
                response.id.with("hover"),
                response.hovered() || response.has_focus(),
                0.14,
            );
            ui.painter().rect_filled(
                tile,
                12,
                Color32::from_white_alpha((10.0 + 10.0 * hover) as u8),
            );
            ctl::icon(
                ui.painter(),
                Rect::from_center_size(tile.left_center() + vec2(30.0, 0.0), Vec2::splat(24.0)),
                icon,
                ctl::focus(),
            );
            ui.painter().text(
                tile.left_top() + vec2(62.0, 13.0),
                Align2::LEFT_TOP,
                title,
                FontId::proportional(16.0),
                ctl::text_color(),
            );
            ui.painter().text(
                tile.left_top() + vec2(62.0, 37.0),
                Align2::LEFT_TOP,
                detail,
                FontId::proportional(13.0),
                ctl::muted(),
            );
            ctl::icon(
                ui.painter(),
                Rect::from_center_size(tile.right_center() - vec2(26.0, 0.0), Vec2::splat(18.0)),
                Icon::Forward,
                ctl::muted(),
            );
            if response.has_focus() {
                ui.painter().rect_stroke(
                    tile,
                    12,
                    Stroke::new(2.0_f32, ctl::focus()),
                    StrokeKind::Inside,
                );
            }
            if response.clicked() {
                self.open_source_editor();
                self.source_editor.kind = kind;
            }
        }
        ui.add_space(12.0);
        let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 48.0), Sense::hover());
        if ctl::button(
            ui,
            Rect::from_min_size(row.min, vec2(120.0, 48.0)),
            "Atrás",
            Some(Icon::Back),
            ButtonStyle::Quiet,
        )
        .clicked()
        {
            self.onboarding = Some(2);
        }
        if ctl::button(
            ui,
            Rect::from_min_size(row.right_top() - vec2(180.0, 0.0), vec2(180.0, 48.0)),
            "Lo haré después",
            None,
            ButtonStyle::Quiet,
        )
        .clicked()
        {
            self.finish_onboarding();
        }
    }
}
