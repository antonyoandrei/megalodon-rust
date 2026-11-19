//! Megalodon's controls share geometry, motion, focus and accessible names.
use super::*;

/// One way of seeing Megalodon: canvas, surfaces, text, focus and the accent
/// that fills primary buttons, the active tab, progress and switches.
/// Artwork keeps its own colours in every theme.
#[derive(Clone, Copy)]
pub struct Palette {
    pub cinema: Color32,
    pub screen: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub focus: Color32,
    pub menu: Color32,
    pub primary: Color32,
    pub on_primary: Color32,
}
const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}
/// (id, name, description, palette). The first is the default.
pub const THEMES: [(&str, &str, &str, Palette); 5] = [
    (
        "cine",
        "Cine",
        "Negro de sala y botones blancos",
        Palette {
            cinema: rgb(8, 9, 11),
            screen: rgb(22, 24, 28),
            text: rgb(245, 245, 247),
            muted: rgb(163, 167, 176),
            focus: rgb(134, 201, 250),
            menu: rgb(28, 30, 35),
            primary: rgb(245, 245, 247),
            on_primary: rgb(8, 9, 11),
        },
    ),
    (
        "abisal",
        "Abisal",
        "Azul marino profundo y acento cian",
        Palette {
            cinema: rgb(4, 16, 31),
            screen: rgb(12, 31, 56),
            text: rgb(234, 244, 255),
            muted: rgb(143, 176, 214),
            focus: rgb(92, 200, 255),
            menu: rgb(16, 39, 66),
            primary: rgb(92, 200, 255),
            on_primary: rgb(4, 16, 31),
        },
    ),
    (
        "ambar",
        "Ámbar",
        "Marrón cálido de proyector y acento dorado",
        Palette {
            cinema: rgb(18, 12, 7),
            screen: rgb(42, 29, 18),
            text: rgb(255, 243, 228),
            muted: rgb(199, 169, 138),
            focus: rgb(255, 180, 84),
            menu: rgb(47, 33, 22),
            primary: rgb(255, 180, 84),
            on_primary: rgb(26, 15, 6),
        },
    ),
    (
        "nebulosa",
        "Nebulosa",
        "Violeta nocturno y acento lila",
        Palette {
            cinema: rgb(13, 8, 22),
            screen: rgb(29, 19, 48),
            text: rgb(245, 239, 255),
            muted: rgb(180, 163, 214),
            focus: rgb(179, 136, 255),
            menu: rgb(35, 23, 58),
            primary: rgb(179, 136, 255),
            on_primary: rgb(18, 8, 31),
        },
    ),
    (
        "oled",
        "Negro puro",
        "Negro absoluto para OLED, todo en blanco",
        Palette {
            cinema: rgb(0, 0, 0),
            screen: rgb(15, 15, 16),
            text: rgb(255, 255, 255),
            muted: rgb(154, 154, 162),
            focus: rgb(255, 255, 255),
            menu: rgb(20, 20, 22),
            primary: rgb(255, 255, 255),
            on_primary: rgb(0, 0, 0),
        },
    ),
];
static THEME: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
pub fn set_theme(id: &str) {
    let index = THEMES.iter().position(|t| t.0 == id).unwrap_or(0);
    THEME.store(index, std::sync::atomic::Ordering::Relaxed);
}
pub fn theme_id() -> &'static str {
    THEMES[THEME
        .load(std::sync::atomic::Ordering::Relaxed)
        .min(THEMES.len() - 1)]
    .0
}
pub fn palette() -> Palette {
    THEMES[THEME
        .load(std::sync::atomic::Ordering::Relaxed)
        .min(THEMES.len() - 1)]
    .3
}
pub fn cinema() -> Color32 {
    palette().cinema
}
pub fn screen() -> Color32 {
    palette().screen
}
pub fn text_color() -> Color32 {
    palette().text
}
pub fn muted() -> Color32 {
    palette().muted
}
pub fn focus() -> Color32 {
    palette().focus
}

#[derive(Clone, Copy)]
pub enum Icon {
    Play,
    Pause,
    Back,
    Forward,
    Down,
    Up,
    Close,
    Check,
    Search,
    Settings,
    Plus,
    Volume,
    Muted,
    Fullscreen,
    ExitFullscreen,
    BackTen,
    ForwardTen,
    Next,
    Picture,
    Subtitles,
    Cast,
    Heart,
    Folder,
    Cloud,
    Server,
}

pub fn reduced(ctx: &Context) -> bool {
    ctx.data(|d| {
        d.get_temp::<bool>(Id::new("reduced-motion"))
            .unwrap_or(false)
    })
}
pub fn motion(ctx: &Context, id: Id, on: bool, duration: f32) -> f32 {
    if reduced(ctx) {
        if on { 1.0 } else { 0.0 }
    } else {
        ctx.animate_bool_with_time(id, on, duration)
    }
}

// OS chords such as Alt+Space must never trigger transport commands.
pub fn shortcut(ctx: &Context, key: Key) -> bool {
    ctx.input_mut(|input| {
        let mut pressed = false;
        input.events.retain(|event| {
            if matches!(event, Event::Key { key: actual, pressed: true, modifiers, .. } if *actual == key && *modifiers == Modifiers::NONE) {
                pressed = true;
                false
            } else { true }
        });
        pressed
    })
}

pub fn icon(p: &Painter, rect: Rect, kind: Icon, color: Color32) {
    let c = rect.center();
    let s = rect.width().min(rect.height()) / 24.0;
    let pt = |x: f32, y: f32| c + vec2(x, y) * s;
    let stroke = Stroke::new(1.8 * s, color);
    let line = |points: &[(f32, f32)]| {
        p.add(Shape::line(
            points.iter().map(|&(x, y)| pt(x, y)).collect(),
            stroke,
        ))
    };
    match kind {
        Icon::Folder => {
            line(&[
                (-9.0, -5.0),
                (-9.0, -8.0),
                (-3.0, -8.0),
                (0.0, -5.0),
                (9.0, -5.0),
                (9.0, 8.0),
                (-9.0, 8.0),
                (-9.0, -5.0),
                (9.0, -5.0),
            ]);
        }
        Icon::Server => {
            for y in [-5.0, 5.0] {
                p.rect_stroke(
                    Rect::from_center_size(pt(0.0, y), vec2(18.0, 7.0) * s),
                    2,
                    stroke,
                    StrokeKind::Inside,
                );
                p.circle_filled(pt(5.0, y), s, color);
            }
        }
        Icon::Cloud => {
            let points = [
                [(-6.0, 7.0), (-13.0, 7.0), (-13.0, -3.0), (-6.0, -3.0)],
                [(-6.0, -3.0), (-5.0, -12.0), (7.0, -12.0), (8.0, -2.0)],
                [(8.0, -2.0), (14.0, -1.0), (13.0, 7.0), (7.0, 7.0)],
            ];
            for curve in points {
                p.add(egui::epaint::CubicBezierShape::from_points_stroke(
                    curve.map(|(x, y)| pt(x, y)),
                    false,
                    Color32::TRANSPARENT,
                    stroke,
                ));
            }
            line(&[(-6.0, 7.0), (7.0, 7.0)]);
        }
        Icon::Heart => {
            for points in [
                [(0.0, 8.5), (-3.0, 6.0), (-9.0, 1.0), (-9.0, -3.5)],
                [(-9.0, -3.5), (-9.0, -9.0), (-2.8, -10.0), (0.0, -5.5)],
                [(0.0, -5.5), (2.8, -10.0), (9.0, -9.0), (9.0, -3.5)],
                [(9.0, -3.5), (9.0, 1.0), (3.0, 6.0), (0.0, 8.5)],
            ] {
                p.add(egui::epaint::CubicBezierShape::from_points_stroke(
                    points.map(|(x, y)| pt(x, y)),
                    false,
                    Color32::TRANSPARENT,
                    stroke,
                ));
            }
        }
        Icon::Cast => {
            line(&[
                (-9.0, -1.0),
                (-9.0, -7.0),
                (9.0, -7.0),
                (9.0, 6.0),
                (1.0, 6.0),
            ]);
            p.circle_filled(pt(-8.0, 7.0), 1.4 * s, color);
            for radius in [5.0, 9.0] {
                p.add(Shape::line(
                    (0..=16)
                        .map(|i| {
                            let a = -(i as f32) / 16.0 * std::f32::consts::FRAC_PI_2;
                            pt(-8.0 + radius * a.cos(), 7.0 + radius * a.sin())
                        })
                        .collect(),
                    stroke,
                ));
            }
        }
        Icon::Play => {
            p.add(Shape::convex_polygon(
                vec![pt(-5.0, -8.0), pt(8.0, 0.0), pt(-5.0, 8.0)],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Pause => {
            for x in [-5.0, 3.0] {
                p.rect_filled(Rect::from_min_max(pt(x, -8.0), pt(x + 3.0, 8.0)), 1, color);
            }
        }
        Icon::Back => {
            line(&[(3.0, -6.0), (-3.0, 0.0), (3.0, 6.0)]);
        }
        Icon::Forward => {
            line(&[(-3.0, -6.0), (3.0, 0.0), (-3.0, 6.0)]);
        }
        Icon::Down => {
            line(&[(-5.0, -2.0), (0.0, 3.0), (5.0, -2.0)]);
        }
        Icon::Up => {
            line(&[(-5.0, 2.0), (0.0, -3.0), (5.0, 2.0)]);
        }
        Icon::Close => {
            line(&[(-5.0, -5.0), (5.0, 5.0)]);
            line(&[(5.0, -5.0), (-5.0, 5.0)]);
        }
        Icon::Check => {
            line(&[(-6.0, 0.0), (-2.0, 4.0), (6.0, -5.0)]);
        }
        Icon::Search => {
            p.circle_stroke(pt(-2.0, -2.0), 6.0 * s, stroke);
            line(&[(3.0, 3.0), (8.0, 8.0)]);
        }
        Icon::Plus => {
            line(&[(-7.0, 0.0), (7.0, 0.0)]);
            line(&[(0.0, -7.0), (0.0, 7.0)]);
        }
        Icon::Settings => {
            let points = (0..48)
                .map(|i| {
                    let angle = i as f32 / 48.0 * std::f32::consts::TAU;
                    let radius = if matches!(i % 6, 0 | 1 | 4 | 5) {
                        7.0
                    } else {
                        9.5
                    };
                    pt(angle.cos() * radius, angle.sin() * radius)
                })
                .collect();
            p.add(Shape::closed_line(points, stroke));
            p.circle_stroke(c, 3.0 * s, stroke);
        }
        Icon::Volume | Icon::Muted => {
            p.add(Shape::closed_line(
                vec![
                    pt(-9.0, -3.0),
                    pt(-5.0, -3.0),
                    pt(0.0, -7.0),
                    pt(0.0, 7.0),
                    pt(-5.0, 3.0),
                    pt(-9.0, 3.0),
                ],
                stroke,
            ));
            if matches!(kind, Icon::Muted) {
                line(&[(4.0, -3.0), (10.0, 3.0)]);
                line(&[(10.0, -3.0), (4.0, 3.0)]);
            } else {
                for radius in [5.0, 9.0] {
                    p.add(Shape::line(
                        (0..=16)
                            .map(|i| {
                                let a = -0.85 + i as f32 / 16.0 * 1.7;
                                pt(radius * a.cos(), radius * a.sin())
                            })
                            .collect(),
                        stroke,
                    ));
                }
            }
        }
        Icon::Fullscreen | Icon::ExitFullscreen => {
            for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let (outer, inner) = if matches!(kind, Icon::Fullscreen) {
                    (8.0, 3.0)
                } else {
                    (3.0, 8.0)
                };
                line(&[
                    (inner * x, outer * y),
                    (outer * x, outer * y),
                    (outer * x, inner * y),
                ]);
            }
        }
        Icon::BackTen | Icon::ForwardTen => {
            let sign = if matches!(kind, Icon::BackTen) {
                -1.0
            } else {
                1.0
            };
            p.add(Shape::line(
                (0..=40)
                    .map(|i| {
                        let a = -std::f32::consts::FRAC_PI_2
                            + 0.65
                            + i as f32 / 40.0 * (std::f32::consts::TAU - 0.65);
                        pt(sign * 9.5 * a.cos(), 9.5 * a.sin())
                    })
                    .collect(),
                stroke,
            ));
            line(&[(sign * -3.0, -12.0), (0.0, -9.5), (sign * -3.0, -6.5)]);
            // Draw the number as geometry: its centre cannot drift with font metrics.
            let digit = Stroke::new(1.35 * s, color);
            p.add(Shape::line(
                vec![pt(-5.0, -1.8), pt(-3.5, -3.4), pt(-3.5, 3.4)],
                digit,
            ));
            p.line_segment([pt(-5.2, 3.4), pt(-1.8, 3.4)], digit);
            p.rect_stroke(
                Rect::from_min_max(pt(0.2, -3.4), pt(5.2, 3.4)),
                2.0 * s,
                digit,
                StrokeKind::Middle,
            );
        }
        Icon::Next => {
            p.add(Shape::convex_polygon(
                vec![pt(-6.0, -7.0), pt(4.0, 0.0), pt(-6.0, 7.0)],
                color,
                Stroke::NONE,
            ));
            line(&[(7.0, -7.0), (7.0, 7.0)]);
        }
        Icon::Picture => {
            p.rect_stroke(rect.shrink(3.0 * s), 3, stroke, StrokeKind::Inside);
            // Mountains stay inside the frame (inner edge at ±7).
            line(&[(-6.0, 5.0), (-2.0, 0.5), (1.0, 3.5), (3.5, 1.0), (6.0, 4.0)]);
            p.circle_filled(pt(3.0, -3.5), 1.4 * s, color);
        }
        Icon::Subtitles => {
            p.rect_stroke(
                Rect::from_min_max(pt(-10.0, -7.0), pt(10.0, 7.0)),
                3,
                stroke,
                StrokeKind::Inside,
            );
            for y in [-1.0, 3.0] {
                line(&[(-6.0, y), (-1.0, y)]);
                line(&[(2.0, y), (6.0, y)]);
            }
        }
    }
}

#[derive(Clone, Copy)]
pub enum ButtonStyle {
    Primary,
    Soft,
    Quiet,
    Floating,
}

pub fn button(
    ui: &mut Ui,
    rect: Rect,
    label: &str,
    glyph: Option<Icon>,
    style: ButtonStyle,
) -> Response {
    let response = ui
        .interact(rect, ui.next_auto_id(), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    ui.skip_ahead_auto_ids(1);
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), label));
    let over = motion(
        ui.ctx(),
        response.id.with("hover"),
        response.hovered() || response.has_focus(),
        0.10,
    );
    let now = ui.input(|i| i.time);
    let pulse_id = response.id.with("last-click");
    if response.clicked() {
        ui.data_mut(|d| d.insert_temp(pulse_id, now));
    }
    let clicked = ui
        .data(|d| d.get_temp::<f64>(pulse_id))
        .is_some_and(|at| now - at < 0.075);
    if clicked {
        ui.ctx().request_repaint();
    }
    let press = motion(
        ui.ctx(),
        response.id.with("press"),
        response.is_pointer_button_down_on() || clicked,
        0.075,
    );
    let r = rect.shrink2(
        rect.size()
            * (if reduced(ui.ctx()) {
                0.0
            } else {
                0.025 * press
            }),
    );
    let primary = matches!(style, ButtonStyle::Primary);
    let fill = match style {
        ButtonStyle::Primary => palette().primary.gamma_multiply(1.0 - 0.10 * press),
        ButtonStyle::Soft => Color32::from_white_alpha((22.0 + 20.0 * over + 12.0 * press) as u8),
        ButtonStyle::Quiet => Color32::from_white_alpha((25.0 * over + 14.0 * press) as u8),
        ButtonStyle::Floating => {
            Color32::from_rgba_unmultiplied(18, 20, 24, (170.0 + 30.0 * over + 24.0 * press) as u8)
        }
    };
    let color = if primary {
        palette().on_primary
    } else {
        text_color()
    }
    .gamma_multiply(if ui.is_enabled() { 1.0 } else { 0.35 });
    ui.painter().rect_filled(
        r,
        if label.is_empty() {
            r.height() / 2.0
        } else {
            12.0
        },
        fill,
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.expand(3.0),
            if label.is_empty() {
                rect.height() / 2.0 + 3.0
            } else {
                14.0
            },
            Stroke::new(2.0_f32, focus()),
            StrokeKind::Outside,
        );
    }
    let font = FontId::proportional(15.0);
    let galley = ui.painter().layout_no_wrap(label.into(), font, color);
    let group = galley.size().x
        + if glyph.is_some() && !label.is_empty() {
            28.0
        } else {
            0.0
        };
    let start = r.center().x - group / 2.0;
    if let Some(glyph) = glyph {
        let center = if label.is_empty() {
            r.center()
        } else {
            pos2(start + 10.0, r.center().y)
        };
        icon(
            ui.painter(),
            Rect::from_center_size(
                center,
                Vec2::splat(
                    if matches!(style, ButtonStyle::Floating) && rect.width() >= 60.0 {
                        if matches!(glyph, Icon::BackTen | Icon::ForwardTen) {
                            42.0
                        } else {
                            34.0
                        }
                    } else if matches!(glyph, Icon::BackTen | Icon::ForwardTen) {
                        27.0
                    } else {
                        22.0
                    },
                ),
            ),
            glyph,
            color,
        );
    }
    if !label.is_empty() {
        ui.painter().galley(
            pos2(
                start + if glyph.is_some() { 28.0 } else { 0.0 },
                r.center().y - galley.size().y / 2.0,
            ),
            galley,
            color,
        );
    }
    response
}

pub fn icon_button(ui: &mut Ui, rect: Rect, glyph: Icon, label: &str, primary: bool) -> Response {
    let response = button(
        ui,
        rect,
        "",
        Some(glyph),
        if primary {
            ButtonStyle::Primary
        } else {
            ButtonStyle::Quiet
        },
    );
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), label));
    response
}

pub fn floating_button(ui: &mut Ui, rect: Rect, glyph: Icon, label: &str) -> Response {
    let response = button(ui, rect, "", Some(glyph), ButtonStyle::Floating);
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), label));
    response
}

pub fn menu_frame() -> Frame {
    Frame::new()
        .fill(palette().menu.gamma_multiply(0.98))
        .corner_radius(16)
        .inner_margin(10)
        .shadow(Shadow {
            offset: [0, 14],
            blur: 36,
            spread: 0,
            color: Color32::from_black_alpha(100),
        })
}
pub fn menu_row(ui: &mut Ui, label: &str, detail: &str, selected: bool) -> Response {
    let (rect, _) = ui.allocate_exact_size(
        vec2(
            ui.available_width(),
            if detail.is_empty() { 44.0 } else { 64.0 },
        ),
        Sense::hover(),
    );
    let response = ui
        .interact(rect, ui.next_auto_id(), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    ui.skip_ahead_auto_ids(1);
    response
        .widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, true, selected, label));
    let hover = motion(
        ui.ctx(),
        response.id.with("hover"),
        response.hovered() || response.has_focus(),
        0.12,
    );
    ui.painter().rect_filled(
        rect,
        10,
        Color32::from_white_alpha((if selected { 18.0 } else { 0.0 } + hover * 14.0) as u8),
    );
    let mut job = egui::text::LayoutJob::simple(
        label.into(),
        FontId::proportional(15.0),
        text_color(),
        (rect.width() - 58.0).max(40.0),
    );
    job.wrap.max_rows = 1;
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    ui.painter().galley(
        pos2(
            rect.left() + 14.0,
            rect.center().y - galley.size().y / 2.0 - if detail.is_empty() { 0.0 } else { 10.0 },
        ),
        galley,
        text_color(),
    );
    if !detail.is_empty() {
        let mut job = egui::text::LayoutJob::simple(
            detail.into(),
            FontId::proportional(12.0),
            muted(),
            (rect.width() - 62.0).max(40.0),
        );
        job.wrap.max_rows = 1;
        let line = ui.fonts_mut(|f| f.layout_job(job));
        ui.painter().galley(
            pos2(rect.left() + 14.0, rect.center().y + 4.0),
            line,
            muted(),
        );
    }
    if selected {
        icon(
            ui.painter(),
            Rect::from_center_size(rect.right_center() - vec2(23.0, 0.0), vec2(20.0, 20.0)),
            Icon::Check,
            text_color(),
        );
    }
    if response.has_focus() {
        ui.painter()
            .rect_stroke(rect, 10, Stroke::new(1.5_f32, focus()), StrokeKind::Inside);
    }
    response
}

pub fn picker(
    ui: &mut Ui,
    id: &str,
    label: &str,
    width: f32,
    options: &[(String, bool)],
) -> Option<usize> {
    let mut selected = None;
    ui.push_id(id, |ui| {
        let (r, _) = ui.allocate_exact_size(vec2(width, 44.0), Sense::hover());
        let response = button(ui, r, "", None, ButtonStyle::Soft);
        response.widget_info(|| WidgetInfo::labeled(WidgetType::ComboBox, true, label));
        let mut job = egui::text::LayoutJob::simple(
            label.into(),
            FontId::proportional(15.0),
            text_color(),
            r.width() - 48.0,
        );
        job.wrap.max_rows = 1;
        let galley = ui.fonts_mut(|f| f.layout_job(job));
        ui.painter().galley(
            pos2(r.left() + 14.0, r.center().y - galley.size().y / 2.0),
            galley,
            text_color(),
        );
        icon(
            ui.painter(),
            Rect::from_center_size(r.right_center() - vec2(17.0, 0.0), vec2(16.0, 16.0)),
            Icon::Down,
            muted(),
        );
        Popup::menu(&response)
            .width(width.max(240.0))
            .frame(menu_frame())
            .show(|ui| {
                ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                    if let Some(index) = menu_choices(ui, id, options) {
                        selected = Some(index);
                        response.request_focus();
                        ui.close();
                    }
                });
            });
    });
    selected
}

pub fn menu_choices(ui: &mut Ui, name: &str, options: &[(String, bool)]) -> Option<usize> {
    menu_options(
        ui,
        name,
        &options
            .iter()
            .map(|(label, on)| (label.clone(), String::new(), *on))
            .collect::<Vec<_>>(),
    )
}

pub fn menu_options(ui: &mut Ui, name: &str, options: &[(String, String, bool)]) -> Option<usize> {
    if options.is_empty() {
        return None;
    }
    let id = ui.id().with(("menu-focus", name));
    let frame = ui.ctx().cumulative_frame_nr();
    let (last, mut focus) = ui.data(|d| d.get_temp::<(u64, usize)>(id)).unwrap_or((
        u64::MAX,
        options.iter().position(|(_, _, on)| *on).unwrap_or(0),
    ));
    let opened = last == u64::MAX || last.saturating_add(1) < frame;
    if opened {
        focus = options.iter().position(|(_, _, on)| *on).unwrap_or(0);
    }
    let down = ui.is_enabled() && shortcut(ui.ctx(), Key::ArrowDown);
    let up = ui.is_enabled() && shortcut(ui.ctx(), Key::ArrowUp);
    if up || down {
        ui.memory_mut(|m| m.move_focus(FocusDirection::None));
    }
    if down {
        focus = (focus + 1) % options.len();
    }
    if up {
        focus = (focus + options.len() - 1) % options.len();
    }
    focus = focus.min(options.len() - 1);
    let activate = ui.is_enabled()
        && !opened
        && ui.input_mut(|i| {
            i.consume_key(Modifiers::NONE, Key::Enter) || i.consume_key(Modifiers::NONE, Key::Space)
        });
    let mut selected = if activate { Some(focus) } else { None };
    for (index, (label, detail, on)) in options.iter().enumerate() {
        let response = menu_row(ui, label, detail, *on).on_hover_text(label);
        if ui.is_enabled()
            && (up
                || down
                || (opened && !ui.input(|i| i.pointer.any_down() || i.pointer.any_released())))
            && index == focus
        {
            response.request_focus();
            if up || down {
                response.scroll_to_me(Some(Align::Center));
            }
        }
        if !(opened || up || down) && response.has_focus() {
            focus = index;
        }
        if response.clicked() {
            selected = Some(index);
        }
    }
    ui.data_mut(|d| d.insert_temp(id, (frame, focus)));
    selected
}

pub fn toggle(ui: &mut Ui, value: &mut bool, label: &str) -> Response {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 48.0), Sense::hover());
    let mut response = ui
        .interact(r, ui.next_auto_id(), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    ui.skip_ahead_auto_ids(1);
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }
    response.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, true, *value, label));
    paint_switch(
        ui,
        Rect::from_center_size(r.right_center() - vec2(24.0, 0.0), vec2(44.0, 26.0)),
        &response,
        *value,
    );
    ui.painter().text(
        r.left_center(),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(16.0),
        text_color(),
    );
    response
}

/// A standalone switch for rows that draw their own label.
pub fn switch(ui: &mut Ui, rect: Rect, value: &mut bool, label: &str) -> Response {
    let mut response = ui
        .interact(rect.expand(6.0), ui.next_auto_id(), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    ui.skip_ahead_auto_ids(1);
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }
    let enabled = ui.is_enabled();
    response.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, enabled, *value, label));
    paint_switch(ui, rect, &response, *value);
    response
}

fn paint_switch(ui: &Ui, track: Rect, response: &Response, value: bool) {
    let t = motion(ui.ctx(), response.id.with("switch"), value, 0.16);
    let dim = if ui.is_enabled() { 1.0 } else { 0.45 };
    ui.painter().rect_filled(
        track,
        13,
        Color32::from_gray(55)
            .lerp_to_gamma(palette().primary, t)
            .gamma_multiply(dim),
    );
    ui.painter().circle_filled(
        pos2(track.left() + 13.0 + 18.0 * t, track.center().y),
        10.0,
        if value {
            palette().on_primary
        } else {
            text_color().gamma_multiply(dim)
        },
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            track.expand(3.0),
            16,
            Stroke::new(2.0_f32, focus()),
            StrokeKind::Outside,
        );
    }
}

/// Centred dialog over a dimmed library. Returns true when the reader asks to
/// close it (close button, Escape or a click outside).
pub fn dialog(
    ctx: &Context,
    id: &str,
    title: &str,
    width: f32,
    body: impl FnOnce(&mut Ui),
) -> bool {
    let mut close = false;
    let viewport = ctx.content_rect();
    let modal = egui::Modal::new(Id::new(("dialog", id)))
        .backdrop_color(Color32::from_black_alpha(150))
        .frame(menu_frame().inner_margin(Margin::symmetric(28, 24)))
        .show(ctx, |ui| {
            let width = width.min(viewport.width() - 48.0);
            ui.set_width(width);
            let (head, _) = ui.allocate_exact_size(vec2(width, 40.0), Sense::hover());
            ui.painter().text(
                head.left_center(),
                Align2::LEFT_CENTER,
                title,
                FontId::proportional(22.0),
                text_color(),
            );
            if icon_button(
                ui,
                Rect::from_min_size(head.right_top() - vec2(40.0, 0.0), Vec2::splat(40.0)),
                Icon::Close,
                "Cerrar",
                false,
            )
            .clicked()
            {
                close = true;
            }
            ui.add_space(16.0);
            // The modal's area would otherwise cap the body at its first-frame
            // height; let it grow to the window before scrolling.
            let room = (viewport.height() - 180.0).max(240.0);
            ScrollArea::vertical()
                .id_salt(("dialog-scroll", id))
                .max_height(room)
                .min_scrolled_height(room)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.set_width(width);
                    body(ui);
                });
        });
    close || modal.should_close()
}

pub fn loading(ui: &Ui, center: Pos2, radius: f32) {
    let time = ui.input(|i| i.time) as f32;
    let start = if reduced(ui.ctx()) { 0.0 } else { time * 3.8 };
    ui.painter().circle_stroke(
        center,
        radius,
        Stroke::new(2.0_f32, Color32::from_white_alpha(35)),
    );
    ui.painter().add(Shape::line(
        (0..=24)
            .map(|i| {
                let a = start + i as f32 / 24.0 * 1.8;
                center + vec2(a.cos(), a.sin()) * radius
            })
            .collect(),
        Stroke::new(2.0_f32, text_color()),
    ));
    if !reduced(ui.ctx()) {
        ui.ctx().request_repaint_after(Duration::from_millis(32));
    }
}

pub fn slider_range(rect: Rect) -> egui::emath::Rangef {
    // Match egui's circular handle input range, including its touch target.
    rect.x_range().shrink(rect.height() / 2.5)
}

pub fn slider(
    ui: &mut Ui,
    rect: Rect,
    value: &mut f64,
    max: f64,
    label: &str,
    buffered: f64,
) -> Response {
    let response = ui
        .scope(|ui| {
            ui.set_opacity(0.0);
            ui.spacing_mut().slider_width = rect.width();
            ui.spacing_mut().interact_size.y = rect.height();
            ui.spacing_mut().slider_rail_height = 4.0;
            ui.place(
                rect,
                Slider::new(value, 0.0..=max.max(1.0))
                    .show_value(false)
                    .smart_aim(false)
                    .handle_shape(egui::style::HandleShape::Circle),
            )
        })
        .inner
        .on_hover_cursor(CursorIcon::PointingHand);
    response.widget_info(|| WidgetInfo::slider(true, *value, label));
    let hover = motion(
        ui.ctx(),
        response.id.with("scrub"),
        response.hovered() || response.dragged() || response.has_focus(),
        0.13,
    );
    let positions = slider_range(response.rect);
    let rail = Rect::from_center_size(
        response.rect.center(),
        vec2(positions.span(), 3.0 + 2.0 * hover),
    );
    ui.painter()
        .rect_filled(rail, 3, Color32::from_white_alpha(45));
    let fraction = (*value / max.max(1.0)).clamp(0.0, 1.0) as f32;
    let buffered = (buffered / max.max(1.0)).clamp(0.0, 1.0) as f32;
    ui.painter().rect_filled(
        Rect::from_min_max(
            rail.min,
            pos2(rail.left() + rail.width() * buffered, rail.bottom()),
        ),
        3,
        Color32::from_white_alpha(75),
    );
    let x = rail.left() + rail.width() * fraction;
    ui.painter().rect_filled(
        Rect::from_min_max(rail.min, pos2(x, rail.bottom())),
        3,
        text_color(),
    );
    ui.painter()
        .circle_filled(pos2(x, rail.center().y), 3.0 + 3.0 * hover, text_color());
    if response.has_focus() {
        ui.painter().circle_stroke(
            pos2(x, rail.center().y),
            10.0,
            Stroke::new(1.5_f32, focus()),
        );
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn custom_slider_input_matches_its_painted_track() {
        let ctx = Context::default();
        let mut value = 0.0;
        let mut actual = Rect::NOTHING;
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(640.0, 480.0));
        let draw = |ctx: &Context, value: &mut f64, actual: &mut Rect| {
            CentralPanel::default().show(ctx, |ui| {
                *actual = slider(
                    ui,
                    Rect::from_min_size(pos2(20.0, 20.0), vec2(440.0, 32.0)),
                    value,
                    100.0,
                    "Position",
                    0.0,
                )
                .rect;
            });
        };
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            },
            |ctx| draw(ctx, &mut value, &mut actual),
        );
        let range = slider_range(actual);
        let pointer = pos2(range.min + range.span() * 0.25, actual.center().y);
        let input = RawInput {
            screen_rect: Some(screen),
            events: vec![
                Event::PointerMoved(pointer),
                Event::PointerButton {
                    pos: pointer,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| draw(ctx, &mut value, &mut actual));
        assert!(
            (value - 25.0).abs() < 0.01,
            "The thumb and hover time must agree with the seek position: {value}"
        );
    }
    #[test]
    fn transport_shortcuts_do_not_consume_windows_chords() {
        let ctx = Context::default();
        for modifiers in [Modifiers::ALT, Modifiers::NONE] {
            let mut input = RawInput::default();
            input.events.push(Event::Key {
                key: Key::Space,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            });
            let _ = ctx.run(input, |ctx| {
                assert_eq!(shortcut(ctx, Key::Space), modifiers == Modifiers::NONE);
                assert_eq!(
                    ctx.input(|i| i.events.len()),
                    usize::from(modifiers == Modifiers::ALT)
                );
            });
        }
    }
    #[test]
    fn menu_keyboard_moves_selection_and_activates_the_focused_row() {
        let ctx = Context::default();
        let options = vec![
            ("Audio".into(), true),
            ("Subtítulos".into(), false),
            ("Volumen".into(), false),
        ];
        let mut choice = None;
        for (frame, key) in [None, Some(Key::ArrowDown), Some(Key::Enter)]
            .into_iter()
            .enumerate()
        {
            let mut input = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(640.0, 480.0))),
                time: Some(frame as f64 / 60.0),
                ..Default::default()
            };
            if let Some(key) = key {
                input.events.push(Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                });
            }
            let _ = ctx.run(input, |ctx| {
                CentralPanel::default().show(ctx, |ui| {
                    if let Some(index) = menu_choices(ui, "test", &options) {
                        choice = Some(index);
                    }
                });
            });
        }
        assert_eq!(
            choice,
            Some(1),
            "Enter must select the row reached with Down, not the original selection"
        );
    }
}
