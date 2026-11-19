use super::*;
use controls::{self as ctl, ButtonStyle, muted, text_color};
impl Megalodon {
    fn update_repository(&self) -> String {
        if self.preferences.update_repository.trim().is_empty() {
            option_env!("MEGALODON_UPDATE_REPO").unwrap_or("").into()
        } else {
            self.preferences.update_repository.trim().into()
        }
    }
    pub(super) fn check_updates(&mut self, ctx: &Context) {
        let repo = self.update_repository();
        if repo.is_empty() || self.updates.checking || self.updates.downloading {
            return;
        }
        self.updates.checking = true;
        self.updates.error = None;
        self.updates.request = self.updates.request.wrapping_add(1);
        let request = self.updates.request;
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        self.rt.spawn_blocking(move || {
            let result = updates::check(&repo);
            let _ = tx.send(Message::UpdateChecked(request, result));
            ctx.request_repaint();
        });
    }
    fn download_update(&mut self, ctx: &Context) {
        let Some(release) = self.updates.release.clone() else {
            return;
        };
        if self.updates.downloading {
            return;
        }
        self.updates.downloading = true;
        self.updates.error = None;
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        let request = self.updates.request;
        self.rt.spawn_blocking(move || {
            let mut last = Instant::now();
            let result = updates::download(
                &release,
                &data::data_dir().join("updates"),
                |done, total| {
                    // Receiver coalesces progress; bounded to avoid waking the renderer for every chunk.
                    if last.elapsed() >= Duration::from_millis(200) || done == total {
                        let _ = tx.send(Message::UpdateProgress(request, done, total));
                        ctx.request_repaint();
                        last = Instant::now();
                    }
                },
            );
            let _ = tx.send(Message::UpdateDownloaded(request, result));
            ctx.request_repaint();
        });
    }
    fn install_update(&mut self, ctx: &Context) {
        let Some(path) = &self.updates.ready else {
            return;
        };
        use std::os::windows::process::CommandExt;
        match std::process::Command::new(path)
            .args(["--update", "--wait-pid", &std::process::id().to_string()])
            .creation_flags(0x08000000)
            .spawn()
        {
            Ok(_) => ctx.send_viewport_cmd(ViewportCommand::Close),
            Err(_) => {
                self.updates.error =
                    Some("No se pudo abrir el instalador. Puedes volver a intentarlo.".into())
            }
        }
    }
    pub(super) fn updates_settings(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Actualizaciones").size(21.0));
        ui.add_space(8.0);
        if ctl::toggle(
            ui,
            &mut self.preferences.automatic_updates,
            "Avisarme de nuevas versiones",
        )
        .changed()
        {
            let _ = self.save_preferences();
        }
        let repo = self.update_repository();
        if repo.is_empty() {
            ui.label(
                RichText::new(
                    "Esta compilación local todavía no tiene un repositorio de versiones asociado.",
                )
                .size(13.0)
                .color(muted()),
            );
        } else {
            ui.label(
                RichText::new(format!("Megalodon {}", env!("CARGO_PKG_VERSION")))
                    .size(14.0)
                    .color(muted()),
            );
        }
        if self.updates.release.is_some() {
            self.update_action(ui);
        } else {
            let (r, _) = ui.allocate_exact_size(vec2(220.0, 44.0), Sense::hover());
            ui.add_enabled_ui(!repo.is_empty() && !self.updates.checking, |ui| {
                if ctl::button(
                    ui,
                    r,
                    if self.updates.checking {
                        "Buscando…"
                    } else {
                        "Buscar actualizaciones"
                    },
                    None,
                    ButtonStyle::Quiet,
                )
                .clicked()
                {
                    self.check_updates(ui.ctx());
                }
            });
            if self.updates.checked && self.updates.error.is_none() {
                ui.label(
                    RichText::new("Tienes la última versión disponible.")
                        .size(13.0)
                        .color(muted()),
                );
            }
        }
        ui.add_space(16.0);
        ui.label(RichText::new("Repositorio de versiones").size(15.0));
        ui.scope(|ui| {
            ui.add_enabled_ui(!self.updates.downloading && !self.updates.checking, |ui| {
                ui.label(
                    RichText::new(
                        "Se configura automáticamente en las versiones publicadas con el proyecto.",
                    )
                    .size(13.0)
                    .color(muted()),
                );
                ui.add_sized(
                    vec2(ui.available_width(), 44.0),
                    TextEdit::singleline(&mut self.preferences.update_repository)
                        .hint_text("usuario/repositorio")
                        .margin(vec2(12.0, 12.0)),
                );
                let (r, _) = ui.allocate_exact_size(vec2(180.0, 44.0), Sense::hover());
                if ctl::button(ui, r, "Guardar repositorio", None, ButtonStyle::Quiet).clicked() {
                    match updates::repository(&self.preferences.update_repository) {
                        Ok(repo) => {
                            self.preferences.update_repository = repo;
                            self.updates = Default::default();
                            let _ = self.save_preferences();
                            self.check_updates(ui.ctx());
                        }
                        Err(e) => self.updates.error = Some(e),
                    }
                }
            });
        });
        if let Some(error) = &self.updates.error {
            ui.label(RichText::new(error).color(ctl::focus()).size(13.0));
        }
        ui.add_space(28.0);
    }
    fn update_action(&mut self, ui: &mut Ui) {
        if self.updates.downloading {
            let (done, total) = self.updates.progress;
            ui.label(
                RichText::new(
                    if let Some(percent) = done.saturating_mul(100).checked_div(total) {
                        format!("Descargando · {percent} %")
                    } else {
                        "Descargando actualización…".into()
                    },
                )
                .size(14.0),
            );
            let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 3.0), Sense::hover());
            ui.painter()
                .rect_filled(r, 2, Color32::from_white_alpha(30));
            if total > 0 {
                ui.painter().rect_filled(
                    Rect::from_min_size(
                        r.min,
                        vec2(
                            r.width() * (done as f32 / total as f32).clamp(0.0, 1.0),
                            3.0,
                        ),
                    ),
                    2,
                    text_color(),
                );
            }
        } else {
            let ready = self.updates.ready.is_some();
            let (r, _) = ui.allocate_exact_size(vec2(240.0, 44.0), Sense::hover());
            if ctl::button(
                ui,
                r,
                if ready {
                    "Reiniciar y actualizar"
                } else {
                    "Descargar actualización"
                },
                None,
                ButtonStyle::Primary,
            )
            .clicked()
            {
                if ready {
                    self.install_update(ui.ctx());
                } else {
                    self.download_update(ui.ctx());
                }
            }
        }
    }
    pub(super) fn update_notice(&mut self, ctx: &Context) {
        let visible = self.playback.is_none()
            && !self.preparing
            && self.page != Page::Settings
            && !self.updates.dismissed
            && self.updates.release.is_some();
        let opacity = ctl::motion(ctx, Id::new("update-notice-opacity"), visible, 0.18);
        if opacity <= 0.001 || self.playback.is_some() {
            return;
        }
        let Some(release) = self.updates.release.clone() else {
            return;
        };
        Area::new(Id::new("update-notice"))
            .anchor(
                Align2::RIGHT_BOTTOM,
                vec2(-24.0, -24.0 + 6.0 * (1.0 - opacity)),
            )
            .order(Order::Foreground)
            .show(ctx, |ui| {
                ui.set_opacity(opacity);
                if !visible {
                    ui.disable();
                }
                ctl::menu_frame().inner_margin(20).show(ui, |ui| {
                    ui.set_width(344.0_f32.min(ctx.content_rect().width() - 88.0));
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("Megalodon {} disponible", release.version))
                                .size(18.0),
                        );
                        let (r, _) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::hover());
                        if ctl::icon_button(ui, r, ctl::Icon::Close, "Más tarde", false).clicked()
                        {
                            self.updates.dismissed = true;
                        }
                    });
                    ui.label(RichText::new(&release.name).color(muted()).size(13.0));
                    ui.add_space(12.0);
                    self.update_action(ui);
                    if let Some(error) = &self.updates.error {
                        ui.label(RichText::new(error).color(ctl::focus()).size(13.0));
                    }
                });
            });
    }
}
