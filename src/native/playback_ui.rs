use super::controls::{self as ctl, ButtonStyle, Icon, muted, text_color};
use super::ui::{gradient, paragraph, text, time_label};
use super::*;

const MENU: &str = "playback-options";

pub(super) struct SeekBurst {
    pub(super) target: f64,
    delta: i32,
    pub(super) updated: Instant,
    pub(super) dispatched: bool,
}
impl SeekBurst {
    pub(super) fn push(
        previous: Option<Self>,
        position: f64,
        duration: f64,
        seconds: i32,
        now: Instant,
    ) -> Self {
        let previous =
            previous.filter(|b| now.duration_since(b.updated) < Duration::from_millis(900));
        let base = previous.as_ref().map_or(position, |b| b.target);
        let target = (base + seconds as f64).clamp(0.0, duration.max(0.0));
        let delta = previous.map_or(0, |b| b.delta) + (target - base).round() as i32;
        Self {
            target,
            delta,
            updated: now,
            dispatched: false,
        }
    }
}

impl Megalodon {
    pub(super) fn preparing_ui(&mut self, ctx: &Context) {
        let cancel = ctl::shortcut(ctx, Key::Escape);
        CentralPanel::default()
            .frame(Frame::NONE.fill(Color32::BLACK))
            .show(ctx, |ui| {
                let r = ui.max_rect();
                if ctl::button(
                    ui,
                    Rect::from_min_size(r.min + vec2(28.0, 24.0), vec2(112.0, 44.0)),
                    "Cancelar",
                    Some(Icon::Back),
                    ButtonStyle::Quiet,
                )
                .clicked()
                    || cancel
                {
                    self.stop();
                }
                if self.playback_started.elapsed() > Duration::from_millis(120) {
                    ctl::loading(ui, r.center() - vec2(0.0, 26.0), 16.0);
                    let title = paragraph(
                        ui,
                        &self.preparing_title,
                        r.width().min(560.0) - 60.0,
                        22.0,
                        text_color(),
                        2,
                    );
                    ui.painter().galley(
                        r.center() + vec2(-title.size().x / 2.0, 18.0),
                        title,
                        text_color(),
                    );
                    ui.painter().text(
                        r.center() + vec2(0.0, 80.0),
                        Align2::CENTER_CENTER,
                        "Conectando con el vídeo…",
                        FontId::proportional(14.0),
                        muted(),
                    );
                }
            });
        ctx.request_repaint_after(Duration::from_millis(32));
    }

    fn skip_seconds(&mut self, seconds: i32) {
        let Some(player) = &self.player else {
            return;
        };
        let (position, duration) = {
            let p = player.lock().unwrap();
            (p.number("time-pos"), p.number("duration"))
        };
        let burst = SeekBurst::push(
            self.skip_burst.take(),
            position,
            duration,
            seconds,
            Instant::now(),
        );
        self.seek = burst.target;
        self.feedback = Some((format!("{:+} s", burst.delta), burst.updated));
        self.skip_burst = Some(burst);
        self.controls_seen = Instant::now();
    }

    pub(super) fn player_ui(&mut self, ctx: &Context) {
        let Some(player) = self.player.clone() else {
            return;
        };
        let just_loaded = player.lock().unwrap().poll();
        if just_loaded {
            if let Some(playback) = &self.playback {
                for (url, title, lang) in &playback.subtitles {
                    let _ = player
                        .lock()
                        .unwrap()
                        .command(&["sub-add", url, "auto", title, lang]);
                }
            }
            self.sync_progress("");
        }
        let mouse = ctx.input(|i| i.pointer.hover_pos());
        if mouse != self.last_mouse || ctx.input(|i| !i.events.is_empty()) {
            self.controls_seen = Instant::now();
            self.last_mouse = mouse;
        }
        if ctx.input(|i| i.key_pressed(Key::Tab)) {
            self.controls_keyboard = true;
        }
        if ctx.input(|i| i.pointer.delta().length_sq() > 0.0 || i.pointer.any_pressed()) {
            self.controls_keyboard = false;
        }
        let menu_open = self.playback_menu_open;
        if !self.casting.open && ctl::shortcut(ctx, Key::Escape) {
            if menu_open {
                self.playback_menu_open = false;
            } else if self.fullscreen {
                self.fullscreen = false;
                ctx.send_viewport_cmd(ViewportCommand::Fullscreen(false));
            } else {
                self.stop();
                return;
            }
        }
        if ctl::shortcut(ctx, Key::F) {
            self.fullscreen = !self.fullscreen;
            ctx.send_viewport_cmd(ViewportCommand::Fullscreen(self.fullscreen));
        }
        if !menu_open && !self.casting.open {
            if ctl::shortcut(ctx, Key::Space) {
                let _ = player.lock().unwrap().command(&["cycle", "pause"]);
                self.feedback = None;
            }
            for (key, seconds) in [(Key::ArrowLeft, -10), (Key::ArrowRight, 10)] {
                if ctl::shortcut(ctx, key) {
                    self.skip_seconds(seconds);
                }
            }
            if ctl::shortcut(ctx, Key::M) {
                let _ = player.lock().unwrap().command(&["cycle", "mute"]);
            }
        }
        let (position, duration, paused, buffering, ready, ended, error, buffer_end) = {
            let p = player.lock().unwrap();
            (
                p.number("time-pos"),
                p.number("duration"),
                p.flag("pause"),
                p.flag("paused-for-cache"),
                p.loaded,
                p.ended,
                p.error.clone(),
                p.number("demuxer-cache-time"),
            )
        };
        if let Some(error) = error {
            self.error = Some(error);
            self.stop();
            return;
        }
        if !ended {
            self.ended_at = None;
        }
        if ready && self.last_report.elapsed() > Duration::from_secs(10) {
            self.sync_progress("/Progress");
        }
        if let Some(burst) = &mut self.skip_burst {
            if !burst.dispatched && burst.updated.elapsed() >= Duration::from_millis(140) {
                if let Err(error) = player.lock().unwrap().command(&[
                    "seek",
                    &burst.target.to_string(),
                    "absolute+exact",
                ]) {
                    self.error = Some(error);
                }
                burst.dispatched = true;
            }
            if burst.updated.elapsed() > Duration::from_millis(900) {
                self.skip_burst = None;
            }
        }
        if !self.seek_dragging {
            self.seek = self.skip_burst.as_ref().map_or(position, |b| b.target);
        }
        let r = ctx.content_rect();
        let on_controls = mouse.is_some_and(|p| p.y > r.bottom() - 150.0 || p.y < r.top() + 90.0);
        let visible = self.controls_seen.elapsed().as_secs_f32() < 2.8
            || !ready
            || buffering
            || ended
            || menu_open
            || self.seek_dragging
            || on_controls
            || self.controls_keyboard;
        let alpha = ctl::motion(ctx, Id::new("player-controls"), visible, 0.22);
        // Move both plain and styled subtitles above the transport controls. ASS
        // dialogue can ignore OSC margins; sub-pos retains its fonts and colours.
        // Quantize to a hundredth of a percent so idle frames do not invalidate video.
        let subtitle_position =
            ((100.0 - 146.0 / r.height().max(1.0) * 100.0 * alpha).clamp(60.0, 100.0) * 100.0)
                .round() as i32;
        if ready && self.subtitle_position != subtitle_position {
            let _ = player
                .lock()
                .unwrap()
                .set("sub-pos", &(subtitle_position as f32 / 100.0).to_string());
            self.subtitle_position = subtitle_position;
        }
        CentralPanel::default()
            .frame(Frame::NONE.fill(Color32::BLACK))
            .show(ctx, |ui| {
                let r = ui.max_rect();
                if ready {
                    let player = player.clone();
                    ui.painter().add(egui::PaintCallback {
                        rect: r,
                        callback: Arc::new(eframe::egui_glow::CallbackFn::new(
                            move |info, painter| {
                                player
                                    .lock()
                                    .unwrap()
                                    .render(painter.gl(), info.screen_size_px);
                            },
                        )),
                    });
                }
                let surface = ui.interact(r, Id::new("video-surface"), Sense::click());
                if surface.clicked() && !menu_open && ready {
                    let _ = player.lock().unwrap().command(&["cycle", "pause"]);
                    self.feedback = None;
                }
                if surface.double_clicked() {
                    self.fullscreen = !self.fullscreen;
                    ctx.send_viewport_cmd(ViewportCommand::Fullscreen(self.fullscreen));
                }
                if (!ready || buffering)
                    && self.playback_started.elapsed() > Duration::from_millis(180)
                {
                    ctl::loading(ui, r.center(), 17.0);
                    ui.painter().text(
                        r.center() + vec2(0.0, 42.0),
                        Align2::CENTER_TOP,
                        if buffering {
                            "Cargando vídeo…"
                        } else {
                            "Abriendo vídeo…"
                        },
                        FontId::proportional(15.0),
                        text_color(),
                    );
                }
                if let Some((message, since)) = &self.feedback {
                    let age = since.elapsed().as_secs_f32();
                    if age < 0.8 {
                        let opacity = ((0.8 - age) / 0.25).clamp(0.0, 1.0);
                        let label = ui.painter().layout_no_wrap(
                            message.clone(),
                            FontId::proportional(17.0),
                            text_color(),
                        );
                        let feedback = Rect::from_center_size(
                            r.center() + vec2(0.0, 76.0),
                            label.size() + vec2(32.0, 20.0),
                        );
                        ui.painter().rect_filled(
                            feedback,
                            16,
                            Color32::from_black_alpha((180.0 * opacity) as u8),
                        );
                        ui.painter().galley(
                            feedback.center() - label.size() / 2.0,
                            label,
                            text_color().gamma_multiply(opacity),
                        );
                        ctx.request_repaint();
                    }
                }
                if alpha <= 0.0 {
                    ctx.set_cursor_icon(CursorIcon::None);
                    return;
                }
                ui.set_opacity(alpha);
                gradient(
                    ui.painter(),
                    Rect::from_min_size(r.min, vec2(r.width(), 124.0)),
                    Color32::from_black_alpha(195),
                    Color32::TRANSPARENT,
                    true,
                );
                gradient(
                    ui.painter(),
                    Rect::from_min_max(pos2(r.left(), r.bottom() - 184.0), r.max),
                    Color32::TRANSPARENT,
                    Color32::from_black_alpha(180),
                    true,
                );
                let gutter = if r.width() < 800.0 { 24.0 } else { 36.0 };
                let back = Rect::from_min_size(r.min + vec2(gutter - 8.0, 20.0), vec2(44.0, 44.0));
                if ctl::icon_button(ui, back, Icon::Back, "Volver · Esc", false).clicked() {
                    self.stop();
                    return;
                }
                if let Some(playback) = &self.playback {
                    let title = paragraph(
                        ui,
                        &playback.item.name,
                        r.width() - 2.0 * gutter - 116.0,
                        20.0,
                        text_color(),
                        1,
                    );
                    ui.painter()
                        .galley(r.min + vec2(gutter + 52.0, 26.0), title, text_color());
                    if let Some(episode) = playback.item.index_number {
                        text(
                            ui.painter(),
                            r.min + vec2(gutter + 52.0, 55.0),
                            format!(
                                "Temporada {} · Episodio {episode}",
                                playback.item.parent_index_number.unwrap_or(1)
                            ),
                            12.0,
                            muted(),
                        );
                    }
                }
                if let Some(next) = self.next_episode()
                    && ctl::icon_button(
                        ui,
                        Rect::from_min_size(
                            pos2(r.right() - gutter - 44.0, r.top() + 20.0),
                            vec2(44.0, 44.0),
                        ),
                        Icon::Next,
                        "Siguiente episodio",
                        false,
                    )
                    .clicked()
                {
                    self.play(ctx, &next.id, false);
                    return;
                }
                // Transport floats over the picture; the footer has no enclosing surface.
                let transport_alpha = ctl::motion(
                    ctx,
                    Id::new("transport-overlay"),
                    !self.playback_menu_open,
                    0.14,
                );
                ui.scope(|ui| {
                    ui.multiply_opacity(transport_alpha);
                    if self.playback_menu_open || !ready {
                        ui.disable();
                    }
                    for (offset, glyph, seconds, label) in [
                        (-108.0, Icon::BackTen, -10, "Retroceder 10 segundos"),
                        (108.0, Icon::ForwardTen, 10, "Avanzar 10 segundos"),
                    ] {
                        if ctl::floating_button(
                            ui,
                            Rect::from_center_size(
                                r.center() + vec2(offset, 0.0),
                                Vec2::splat(68.0),
                            ),
                            glyph,
                            label,
                        )
                        .clicked()
                        {
                            self.skip_seconds(seconds);
                        }
                    }
                    if ctl::floating_button(
                        ui,
                        Rect::from_center_size(r.center(), Vec2::splat(80.0)),
                        if paused { Icon::Play } else { Icon::Pause },
                        if paused {
                            "Reproducir · Espacio"
                        } else {
                            "Pausar · Espacio"
                        },
                    )
                    .clicked()
                    {
                        let _ = player.lock().unwrap().command(&["cycle", "pause"]);
                        self.feedback = None;
                    }
                });
                let track_width = (r.width() * 0.72)
                    .clamp(440.0, 1080.0)
                    .min(r.width() - 64.0);
                let track_left = r.center().x - track_width / 2.0;
                let timeline = Rect::from_min_size(
                    pos2(track_left - 12.0, r.bottom() - 74.0),
                    vec2(track_width + 24.0, 32.0),
                );
                let scrub = ctl::slider(
                    ui,
                    timeline,
                    &mut self.seek,
                    duration,
                    "Posición de reproducción",
                    buffer_end,
                );
                self.seek_dragging = scrub.dragged();
                if scrub.drag_started() {
                    self.skip_burst = None;
                }
                if scrub.drag_stopped() || (scrub.changed() && !scrub.dragged()) {
                    self.skip_burst = None;
                    if let Err(error) = player.lock().unwrap().command(&[
                        "seek",
                        &self.seek.to_string(),
                        "absolute+exact",
                    ]) {
                        self.error = Some(error);
                    }
                    self.seek_dragging = false;
                }
                if (scrub.hovered() || scrub.dragged())
                    && let Some(pointer) = scrub
                        .interact_pointer_pos()
                        .or_else(|| ctx.input(|i| i.pointer.hover_pos()))
                {
                    let track = ctl::slider_range(scrub.rect);
                    let value =
                        ((pointer.x - track.min) / track.span()).clamp(0.0, 1.0) as f64 * duration;
                    let preview = Rect::from_center_size(
                        pos2(
                            pointer
                                .x
                                .clamp(track_left + 39.0, track_left + track_width - 39.0),
                            r.bottom() - 94.0,
                        ),
                        vec2(78.0, 30.0),
                    );
                    ui.painter()
                        .rect_filled(preview, 10, Color32::from_black_alpha(225));
                    ui.painter().text(
                        preview.center(),
                        Align2::CENTER_CENTER,
                        time_label(value),
                        FontId::proportional(14.0),
                        text_color(),
                    );
                }
                let footer_y = r.bottom() - 108.0;
                ui.painter().text(
                    pos2(track_left, footer_y),
                    Align2::LEFT_CENTER,
                    format!("{}  /  {}", time_label(self.seek), time_label(duration)),
                    FontId::proportional(14.0),
                    text_color(),
                );
                let full_rect = Rect::from_center_size(
                    pos2(track_left + track_width - 22.0, footer_y),
                    Vec2::splat(44.0),
                );
                let options_rect = full_rect.translate(vec2(-56.0, 0.0));
                if self.library.is_some()
                    && self
                        .playback
                        .as_ref()
                        .is_some_and(|p| !p.headers.is_empty())
                    && ctl::floating_button(
                        ui,
                        options_rect.translate(vec2(-56.0, 0.0)),
                        Icon::Cast,
                        "Ver en otro dispositivo",
                    )
                    .clicked()
                {
                    self.open_cast(ctx);
                }
                let settings =
                    ctl::floating_button(ui, options_rect, Icon::Subtitles, "Audio y subtítulos");
                if settings.clicked() {
                    self.playback_menu_open = !self.playback_menu_open;
                    if self.playback_menu_open {
                        let p = player.lock().unwrap();
                        self.playback_tracks = [p.tracks("audio"), p.tracks("sub")];
                    }
                }
                if menu_open && !self.playback_menu_open {
                    settings.request_focus();
                }
                if ctl::floating_button(
                    ui,
                    full_rect,
                    if self.fullscreen {
                        Icon::ExitFullscreen
                    } else {
                        Icon::Fullscreen
                    },
                    if self.fullscreen {
                        "Salir de pantalla completa · F"
                    } else {
                        "Pantalla completa · F"
                    },
                )
                .clicked()
                {
                    self.fullscreen = !self.fullscreen;
                    ctx.send_viewport_cmd(ViewportCommand::Fullscreen(self.fullscreen));
                }
                // Stable bounds keep the tab strip and pointer target stationary.
                let audio_height = self.playback_tracks[0].len().max(1) as f32 * 68.0 + 56.0;
                let subtitle_height = (self.playback_tracks[1].len() + 1) as f32 * 68.0;
                let panel_height = (audio_height.max(subtitle_height) + 80.0)
                    .clamp(148.0, (r.height() - 144.0).clamp(204.0, 360.0));
                let panel_size = vec2(320.0_f32.min(r.width() - 48.0), panel_height);
                let panel_rect = Rect::from_min_size(
                    pos2(
                        (options_rect.right() - panel_size.x)
                            .clamp(r.left() + 24.0, r.right() - panel_size.x - 24.0),
                        options_rect.top() - panel_size.y - 10.0,
                    ),
                    panel_size,
                );
                if self.playback_menu_open
                    && ctx.input(|i| {
                        i.pointer.any_pressed()
                            && i.pointer.interact_pos().is_some_and(|p| {
                                !panel_rect.contains(p) && !options_rect.contains(p)
                            })
                    })
                {
                    self.playback_menu_open = false;
                    settings.request_focus();
                }
                let panel_alpha = ctl::motion(
                    ctx,
                    Id::new("playback-panel-opacity"),
                    self.playback_menu_open,
                    if self.playback_menu_open { 0.18 } else { 0.12 },
                );
                if panel_alpha > 0.0 {
                    Area::new(Id::new(MENU))
                        .order(Order::Foreground)
                        .fixed_pos(panel_rect.min)
                        .fade_in(false)
                        .interactable(self.playback_menu_open)
                        .show(ctx, |ui| {
                            ui.set_opacity(panel_alpha);
                            if !self.playback_menu_open {
                                ui.disable();
                            }
                            let (outer, _) = ui.allocate_exact_size(panel_size, Sense::hover());
                            let inner = outer.shrink(12.0);
                            ui.painter()
                                .add(ctl::menu_frame().inner_margin(12).paint(inner));
                            let mut content = ui.new_child(UiBuilder::new().max_rect(inner));
                            content.set_clip_rect(inner);
                            self.playback_menu(&mut content, &player);
                        });
                }
                if menu_open && !self.playback_menu_open {
                    settings.request_focus();
                }
            });
        if self.playback.is_none() {
            return;
        }
        if ended
            && self.auto_next
            && let Some(next) = self.next_episode()
        {
            let since = self.ended_at.get_or_insert_with(Instant::now).elapsed();
            Area::new(Id::new("up-next"))
                .anchor(Align2::RIGHT_BOTTOM, vec2(-32.0, -170.0))
                .order(Order::Foreground)
                .show(ctx, |ui| {
                    ctl::menu_frame().show(ui, |ui| {
                        ui.set_width(300.0);
                        ui.label(RichText::new(&next.name).size(18.0));
                        ui.label(
                            RichText::new(format!(
                                "Siguiente episodio en {} s",
                                5u64.saturating_sub(since.as_secs())
                            ))
                            .color(muted()),
                        );
                        let (r, _) = ui.allocate_exact_size(vec2(300.0, 44.0), Sense::hover());
                        if ctl::button(ui, r, "Cancelar", None, ButtonStyle::Quiet).clicked() {
                            self.auto_next = false;
                        }
                    });
                });
            if since >= Duration::from_secs(5) && self.auto_next {
                self.play(ctx, &next.id, false);
            }
        }
        ctx.request_repaint_after(Duration::from_millis(if paused { 100 } else { 33 }));
    }

    fn playback_menu(&mut self, ui: &mut Ui, player: &Arc<Mutex<player::Player>>) {
        ui.spacing_mut().item_spacing.y = 4.0;
        let (head, _) = ui.allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::hover());
        if ctl::icon_button(
            ui,
            Rect::from_min_size(pos2(head.right() - 40.0, head.top()), vec2(40.0, 44.0)),
            Icon::Close,
            "Cerrar opciones",
            false,
        )
        .clicked()
        {
            self.playback_menu_open = false;
        }
        let tabs = [("Audio", Icon::Volume), ("Subtítulos", Icon::Subtitles)];
        let tab_rect = Rect::from_min_max(head.min, pos2(head.right() - 44.0, head.bottom()));
        let tab_width = tab_rect.width() / 2.0;
        let mut next_tab = self.playback_menu_tab;
        if ui.is_enabled() {
            if ctl::shortcut(ui.ctx(), Key::ArrowRight) {
                next_tab = (next_tab + 1) % 2;
                ui.memory_mut(|m| m.move_focus(FocusDirection::None));
            }
            if ctl::shortcut(ui.ctx(), Key::ArrowLeft) {
                next_tab = (next_tab + 1) % 2;
                ui.memory_mut(|m| m.move_focus(FocusDirection::None));
            }
        }
        let highlight = if ctl::reduced(ui.ctx()) {
            self.playback_menu_tab as f32
        } else {
            ui.ctx().animate_value_with_time(
                Id::new("player-tab-highlight"),
                self.playback_menu_tab as f32,
                0.16,
            )
        };
        ui.painter().rect_filled(
            Rect::from_min_size(
                tab_rect.min + vec2(highlight * tab_width, 0.0),
                vec2(tab_width - 4.0, 44.0),
            ),
            12,
            Color32::from_white_alpha(22),
        );
        for (index, (label, glyph)) in tabs.into_iter().enumerate() {
            let r = Rect::from_min_size(
                tab_rect.min + vec2(index as f32 * tab_width, 0.0),
                vec2(tab_width - 4.0, 44.0),
            );
            let response = ctl::button(ui, r, "", None, ButtonStyle::Quiet);
            response.widget_info(|| {
                WidgetInfo::selected(
                    WidgetType::SelectableLabel,
                    true,
                    index == self.playback_menu_tab,
                    label,
                )
            });
            let color = if index == self.playback_menu_tab {
                text_color()
            } else {
                muted()
            };
            ctl::icon(
                ui.painter(),
                Rect::from_center_size(r.left_center() + vec2(17.0, 0.0), vec2(17.0, 17.0)),
                glyph,
                color,
            );
            text(ui.painter(), r.min + vec2(31.0, 12.0), label, 13.0, color);
            if response.clicked() {
                next_tab = index;
            }
        }
        self.playback_menu_tab = next_tab;
        ui.add_space(12.0);
        ui.push_id(("playback-tab-content", self.playback_menu_tab), |ui| {
            let audio = self.playback_menu_tab == 0;
            if audio {
                let (row, _) =
                    ui.allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::hover());
                let muted = player.lock().unwrap().flag("mute");
                if ctl::icon_button(
                    ui,
                    Rect::from_min_size(row.min, Vec2::splat(44.0)),
                    if muted { Icon::Muted } else { Icon::Volume },
                    if muted {
                        "Activar sonido · M"
                    } else {
                        "Silenciar · M"
                    },
                    false,
                )
                .clicked()
                {
                    let _ = player.lock().unwrap().command(&["cycle", "mute"]);
                }
                let mut volume = self.preferences.volume;
                if ctl::slider(
                    ui,
                    Rect::from_min_size(row.min + vec2(44.0, 6.0), vec2(row.width() - 92.0, 32.0)),
                    &mut volume,
                    100.0,
                    "Volumen",
                    0.0,
                )
                .changed()
                {
                    self.preferences.volume = volume;
                    let p = player.lock().unwrap();
                    let _ = p.set("volume", &volume.to_string());
                    if muted && volume > 0.0 {
                        let _ = p.set("mute", "no");
                    }
                }
                ui.painter().text(
                    row.right_center() - vec2(4.0, 0.0),
                    Align2::RIGHT_CENTER,
                    format!("{}%", volume.round() as u32),
                    FontId::proportional(12.0),
                    ctl::muted(),
                );
                ui.add_space(8.0);
            }
            ScrollArea::vertical()
                .id_salt(("playback-tracks", self.playback_menu_tab))
                .max_height(ui.available_height().max(1.0))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let kind = if self.playback_menu_tab == 0 {
                        "audio"
                    } else {
                        "sub"
                    };
                    let tracks = self.playback_tracks[self.playback_menu_tab].clone();
                    let mut options: Vec<_> = tracks.iter().map(track_description).collect();
                    if kind == "sub" {
                        options.insert(
                            0,
                            (
                                "Desactivados".into(),
                                "Sin subtítulos".into(),
                                !tracks.iter().any(|t| t.selected),
                            ),
                        );
                    }
                    if options.is_empty() {
                        ui.add_space(22.0);
                        ui.label(RichText::new("No hay pistas disponibles.").color(muted()));
                    }
                    if let Some(index) = ctl::menu_options(ui, kind, &options) {
                        let id = if kind == "sub" && index == 0 {
                            "no"
                        } else {
                            &tracks[index - usize::from(kind == "sub")].id
                        };
                        let p = player.lock().unwrap();
                        if let Err(error) = p.set(if kind == "audio" { "aid" } else { "sid" }, id) {
                            self.error = Some(error);
                        }
                        self.playback_tracks[self.playback_menu_tab] = p.tracks(kind);
                    }
                });
        });
    }
}

fn track_description(track: &player::Track) -> (String, String, bool) {
    let language = match track.language.to_ascii_lowercase().as_str() {
        "spa" | "es" => "Español",
        "eng" | "en" => "Inglés",
        "jpn" | "ja" => "Japonés",
        "fra" | "fre" | "fr" => "Francés",
        "deu" | "ger" | "de" => "Alemán",
        "ita" | "it" => "Italiano",
        "por" | "pt" => "Portugués",
        "cat" | "ca" => "Catalán",
        "kor" | "ko" => "Coreano",
        "zho" | "chi" | "zh" => "Chino",
        "und" | "" => "",
        _ => &track.language,
    };
    let untitled = track.title == format!("Pista {}", track.id);
    let title = if untitled && !language.is_empty() {
        language.to_owned()
    } else {
        track.title.clone()
    };
    let mut detail = Vec::new();
    if !language.is_empty() && !title.eq_ignore_ascii_case(language) {
        detail.push(language.to_owned());
    }
    if !track.codec.is_empty() {
        detail.push(track.codec.to_uppercase());
    }
    if track.channels > 0 {
        detail.push(match track.channels {
            1 => "Mono".into(),
            2 => "Estéreo".into(),
            n => format!("{n} canales"),
        });
    }
    if untitled && !language.is_empty() {
        detail.push(track.title.clone());
    }
    (title, detail.join(" · "), track.selected)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rapid_skips_accumulate_against_the_pending_target() {
        let now = Instant::now();
        let a = SeekBurst::push(None, 100.0, 500.0, 10, now);
        let b = SeekBurst::push(Some(a), 100.0, 500.0, 10, now + Duration::from_millis(80));
        let c = SeekBurst::push(Some(b), 100.0, 500.0, 10, now + Duration::from_millis(160));
        assert_eq!((c.target, c.delta), (130.0, 30));
        let d = SeekBurst::push(Some(c), 100.0, 500.0, -10, now + Duration::from_millis(240));
        assert_eq!((d.target, d.delta), (120.0, 20));
        let fresh = SeekBurst::push(Some(d), 210.0, 500.0, 10, now + Duration::from_secs(2));
        assert_eq!((fresh.target, fresh.delta), (220.0, 10));
        let end = SeekBurst::push(None, 495.0, 500.0, 10, now);
        assert_eq!((end.target, end.delta), (500.0, 5));
    }
}
