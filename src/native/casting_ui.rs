use super::*;
use controls::{self as ctl, ButtonStyle, Icon, muted, text_color};

#[derive(Default)]
pub(super) struct Casting {
    pub open: bool,
    pub searching: bool,
    pub connecting: bool,
    pub request: u64,
    pub devices: Vec<casting::Device>,
    pub error: Option<String>,
    pub remote: Option<casting::Remote>,
    pub handoff: Option<casting::Relay>,
    handoff_started: bool,
    last_report: Option<Instant>,
    last_handoff: Option<Instant>,
    seek: f64,
    dragging: bool,
    burst: Option<playback_ui::SeekBurst>,
}
impl Megalodon {
    pub(super) fn open_cast(&mut self, ctx: &Context) {
        self.playback_menu_open = false;
        self.casting.open = true;
        self.casting.error = None;
        if self.casting.remote.is_none() && self.casting.handoff.is_none() {
            self.find_receivers(ctx);
        }
    }
    fn find_receivers(&mut self, ctx: &Context) {
        self.casting.request = self.casting.request.wrapping_add(1);
        self.casting.searching = true;
        let request = self.casting.request;
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        self.rt.spawn(async move {
            let result = casting::discover().await;
            let _ = tx.send(Message::Receivers(request, result));
            ctx.request_repaint();
        });
    }
    fn connect_receiver(&mut self, ctx: &Context, device: Option<casting::Device>) {
        let (Some(library), Some(playback), Some(catalog), Some(player)) = (
            self.library.clone(),
            self.playback.clone(),
            self.catalog.clone(),
            self.player.clone(),
        ) else {
            return;
        };
        self.casting.request = self.casting.request.wrapping_add(1);
        let request = self.casting.request;
        self.casting.searching = false;
        self.casting.connecting = true;
        self.casting.error = None;
        let position = player.lock().unwrap().number("time-pos");
        let user = catalog.user.id.clone();
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        self.rt.spawn(async move {
            let message = if let Some(device) = device {
                Message::Remote(
                    request,
                    casting::start(library, playback, user, device, position)
                        .await
                        .map(Box::new),
                )
            } else {
                let local = if_addrs::get_if_addrs().ok().and_then(|interfaces| {
                    interfaces.into_iter().find_map(|i| match i.ip() {
                        std::net::IpAddr::V4(ip) if casting::lan_ip(ip) => Some(ip),
                        _ => None,
                    })
                });
                let result = match local {
                    Some(ip) => {
                        casting::Relay::start(library, &playback, &user, ip, None, position).await
                    }
                    None => Err(
                        "Conecta el equipo a una red local para usar AirPlay desde Safari.".into(),
                    ),
                };
                Message::Handoff(request, result)
            };
            let _ = tx.send(message);
            ctx.request_repaint();
        });
    }
    pub(super) fn cast_menu(&mut self, ctx: &Context) {
        let modal=egui::Modal::new(Id::new("cast-menu")).frame(ctl::menu_frame().inner_margin(24)).show(ctx,|ui| {
            ui.set_width(400.0_f32.min(ctx.content_rect().width()-80.0));
            ui.horizontal(|ui| {
                ui.label(RichText::new("Ver en otro dispositivo").size(22.0));
                ui.with_layout(Layout::right_to_left(Align::Center),|ui| {
                    let (r,_)=ui.allocate_exact_size(Vec2::splat(40.0),Sense::hover());
                    if ctl::icon_button(ui,r,Icon::Close,"Cerrar dispositivos",false).clicked(){self.casting.open=false;}
                });
            });
            ui.add_space(16.0);
            if self.casting.connecting || self.casting.remote.as_ref().is_some_and(|r|!r.connected) {
                let (r,_)=ui.allocate_exact_size(vec2(ui.available_width(),96.0),Sense::hover());
                ctl::loading(ui,r.center()-vec2(0.0,14.0),12.0);
                ui.painter().text(r.center()+vec2(0.0,22.0),Align2::CENTER_CENTER,"Conectando con el receptor…",FontId::proportional(15.0),muted());
                let (r,_)=ui.allocate_exact_size(vec2(ui.available_width(),44.0),Sense::hover());
                if ctl::button(ui,r,"Cancelar conexión",None,ButtonStyle::Quiet).clicked(){self.end_cast(false);}
            } else if let Some(relay)=&self.casting.handoff {
                ui.label("Abre este enlace en Safari en tu iPhone, iPad o Mac. Usa AirPlay desde los controles del vídeo.");
                ui.add_space(16.0);
                let mut url=relay.watch_url();
                ui.add(TextEdit::singleline(&mut url).font(TextStyle::Monospace).desired_width(f32::INFINITY).margin(vec2(12.0,12.0)));
                ui.add_space(12.0);
                let (r,_)=ui.allocate_exact_size(vec2(ui.available_width(),44.0),Sense::hover());
                if ctl::button(ui,r,"Copiar enlace",None,ButtonStyle::Primary).clicked(){ctx.copy_text(relay.watch_url());}
                ui.add_space(12.0);
                ui.label(RichText::new("El enlace caduca al cerrar esta transmisión.").color(muted()));
                let (r,_)=ui.allocate_exact_size(vec2(ui.available_width(),44.0),Sense::hover());
                if ctl::button(ui,r,"Cerrar enlace",None,ButtonStyle::Quiet).clicked(){self.end_cast(false);}
            } else if let Some(remote)=&self.casting.remote {
                ui.label(RichText::new(&remote.device.name).size(20.0));
                ui.add_space(16.0);
                let (r,_)=ui.allocate_exact_size(vec2(ui.available_width(),44.0),Sense::hover());
                if ctl::button(ui,r,"Volver a este equipo",Some(Icon::Play),ButtonStyle::Primary).clicked(){self.end_cast(true);}
            } else {
                if self.casting.searching {
                    ui.horizontal(|ui| {let (r,_)=ui.allocate_exact_size(Vec2::splat(24.0),Sense::hover());ctl::loading(ui,r.center(),8.0);ui.label(RichText::new("Buscando en tu red…").color(muted()));});
                }
                let devices=self.casting.devices.clone();
                ScrollArea::vertical().max_height(260.0).show(ui,|ui| {
                    for device in devices {
                        let (r,_)=ui.allocate_exact_size(vec2(ui.available_width(),64.0),Sense::hover());
                        let response=ui.interact(r,Id::new(("receiver",device.key())),Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
                        response.widget_info(||WidgetInfo::labeled(WidgetType::Button,true,format!("Transmitir a {}",device.name)));
                        let hover=ctl::motion(ctx,response.id,response.hovered()||response.has_focus(),0.16);
                        ui.painter().rect_filled(r.shrink(2.0),12,Color32::from_white_alpha((16.0*hover) as u8));
                        ctl::icon(ui.painter(),Rect::from_center_size(pos2(r.left()+24.0,r.center().y),Vec2::splat(25.0)),Icon::Cast,text_color());
                        ui.painter().text(r.left_top()+vec2(52.0,19.0),Align2::LEFT_CENTER,&device.name,FontId::proportional(17.0),text_color());
                        ui.painter().text(r.left_top()+vec2(52.0,43.0),Align2::LEFT_CENTER,device.protocol(),FontId::proportional(13.0),muted());
                        if response.clicked(){self.connect_receiver(ctx,Some(device));}
                    }
                });
                if !self.casting.searching && self.casting.devices.is_empty() {ui.label(RichText::new("No hay receptores disponibles. Enciende tu TV y conéctala a la misma red.").color(muted()));}
                ui.add_space(16.0);
                let (r,_)=ui.allocate_exact_size(vec2(ui.available_width(),48.0),Sense::hover());
                if ctl::button(ui,r,"AirPlay desde Safari",Some(Icon::Cast),ButtonStyle::Soft).clicked(){self.connect_receiver(ctx,None);}
                if !self.casting.searching {let (r,_)=ui.allocate_exact_size(vec2(ui.available_width(),44.0),Sense::hover());if ctl::button(ui,r,"Buscar de nuevo",None,ButtonStyle::Quiet).clicked(){self.find_receivers(ctx);}}
            }
            if let Some(error)=&self.casting.error {ui.add_space(12.0);ui.label(RichText::new(error).color(ctl::focus()));}
        });
        if modal.should_close() {
            self.casting.open = false;
        }
    }
    pub(super) fn poll_cast(&mut self, ctx: &Context) {
        let events: Vec<_> = self
            .casting
            .remote
            .as_ref()
            .map_or_else(Vec::new, |r| r.events.try_iter().collect());
        for event in events {
            match event {
                casting::Event::Connected => {
                    self.sync_progress("/Stopped");
                    if let Some(player) = &self.player {
                        let _ = player.lock().unwrap().command(&["set", "pause", "yes"]);
                    }
                    if let Some(remote) = &mut self.casting.remote {
                        remote.connected = true;
                    }
                    self.casting.open = false;
                    self.cast_report("");
                }
                casting::Event::Status(status) => {
                    if let Some(remote) = &mut self.casting.remote {
                        let duration = if status.duration > 0.0 {
                            status.duration
                        } else {
                            remote.status.duration
                        };
                        remote.status = casting::Status { duration, ..status };
                    }
                }
                casting::Event::Failed(error) => {
                    self.end_cast(false);
                    self.casting.error = Some(error);
                    self.casting.open = true;
                }
                casting::Event::Ended => {
                    self.stop();
                }
            }
        }
        if let Some((status, seen)) = self.casting.handoff.as_ref().and_then(|r| r.progress()) {
            if !self.casting.handoff_started {
                self.sync_progress("/Stopped");
                if let Some(player) = &self.player {
                    let _ = player.lock().unwrap().command(&["set", "pause", "yes"]);
                }
                self.casting.handoff_started = true;
                self.casting.open = false;
                self.cast_report("");
            }
            self.casting.last_handoff = Some(seen);
            if status.ended {
                self.stop();
            }
        }
        if (self.casting.remote.as_ref().is_some_and(|r| r.connected)
            || (self.casting.handoff_started
                && self
                    .casting
                    .last_handoff
                    .is_some_and(|seen| seen.elapsed() < Duration::from_secs(20))))
            && self
                .casting
                .last_report
                .is_none_or(|last| last.elapsed() >= Duration::from_secs(10))
        {
            self.cast_report("/Progress");
        }
        if self.casting.remote.is_some() || self.casting.handoff.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
    fn cast_report(&mut self, _endpoint: &str) {
        self.casting.last_report = Some(Instant::now());
        if self.no_report {
            return;
        }
        let Some(mut playback) = self.playback.clone() else {
            return;
        };
        let status = if let Some(remote) = &self.casting.remote {
            if !remote.connected {
                return;
            }
            playback = remote.playback.clone();
            remote.status.clone()
        } else if let Some(relay) = &self.casting.handoff {
            let Some((status, _)) = relay.progress() else {
                return;
            };
            status
        } else {
            return;
        };
        self.record_watch(
            playback.item,
            status.position,
            status.duration,
            status.ended,
        );
        let _ = self.save_preferences();
    }
    pub(super) fn end_cast(&mut self, resume: bool) {
        let position = self
            .casting
            .remote
            .as_ref()
            .filter(|r| r.connected)
            .map(|r| r.status.position)
            .or_else(|| {
                self.casting
                    .handoff
                    .as_ref()
                    .and_then(|r| r.progress().map(|(s, _)| s.position))
            });
        self.cast_report("/Stopped");
        self.casting.request = self.casting.request.wrapping_add(1);
        self.casting.connecting = false;
        self.casting.searching = false;
        self.casting.handoff_started = false;
        self.casting.last_report = None;
        self.casting.last_handoff = None;
        self.casting.burst = None;
        let _enter = self.rt.enter();
        self.casting.remote.take();
        self.casting.handoff.take();
        drop(_enter);
        if let (Some(position), Some(player)) = (position, &self.player) {
            let p = player.lock().unwrap();
            let _ = p.command(&["seek", &position.to_string(), "absolute+exact"]);
            if resume {
                let _ = p.command(&["set", "pause", "no"]);
            }
        }
        if resume {
            self.casting.open = false;
            self.controls_seen = Instant::now();
            self.sync_progress("");
        }
    }
    pub(super) fn remote_ui(&mut self, ctx: &Context) {
        let (name, status, is_handoff) = if let Some(remote) = &self.casting.remote {
            (remote.device.name.clone(), remote.status.clone(), false)
        } else if let Some((status, _)) = self.casting.handoff.as_ref().and_then(|r| r.progress()) {
            ("AirPlay desde Safari".into(), status, true)
        } else {
            return;
        };
        if ctl::shortcut(ctx, Key::Escape) {
            self.casting.open = true;
        }
        if !self.casting.open && !is_handoff {
            if ctl::shortcut(ctx, Key::Space)
                && let Some(remote) = &self.casting.remote
            {
                remote.command(casting::Command::Pause(!status.paused));
            }
            for (key, seconds) in [(Key::ArrowLeft, -10), (Key::ArrowRight, 10)] {
                if ctl::shortcut(ctx, key) {
                    self.remote_skip(&status, seconds);
                }
            }
        }
        if let Some(burst) = &mut self.casting.burst {
            if !burst.dispatched && burst.updated.elapsed() >= Duration::from_millis(180) {
                if let Some(remote) = &self.casting.remote {
                    remote.command(casting::Command::Seek(burst.target));
                }
                burst.dispatched = true;
            }
            if burst.updated.elapsed() > Duration::from_millis(1100) {
                self.casting.burst = None;
            }
        }
        if !self.casting.dragging {
            self.casting.seek = self
                .casting
                .burst
                .as_ref()
                .map_or(status.position, |b| b.target);
        }
        CentralPanel::default()
            .frame(Frame::NONE.fill(ctl::cinema()))
            .show(ctx, |ui| {
                let r = ui.max_rect();
                if ctl::icon_button(
                    ui,
                    Rect::from_min_size(r.min + vec2(24.0, 20.0), Vec2::splat(44.0)),
                    Icon::Back,
                    "Cerrar reproducción",
                    false,
                )
                .clicked()
                {
                    self.stop();
                    return;
                }
                let center = r.center() - vec2(0.0, 60.0);
                ctl::icon(
                    ui.painter(),
                    Rect::from_center_size(center - vec2(0.0, 100.0), Vec2::splat(56.0)),
                    Icon::Cast,
                    ctl::focus(),
                );
                ui.painter().text(
                    center - vec2(0.0, 32.0),
                    Align2::CENTER_CENTER,
                    &name,
                    FontId::proportional(26.0),
                    text_color(),
                );
                let title = self.playback.as_ref().map_or("", |p| &p.item.name);
                let title = crate::ui::paragraph(
                    ui,
                    title,
                    (r.width() - 80.0).min(640.0),
                    18.0,
                    muted(),
                    2,
                );
                ui.painter()
                    .galley(center + vec2(-title.size().x / 2.0, 8.0), title, muted());
                if !is_handoff {
                    for (offset, icon, seconds, label) in [
                        (-94.0, Icon::BackTen, -10, "Retroceder 10 segundos"),
                        (94.0, Icon::ForwardTen, 10, "Avanzar 10 segundos"),
                    ] {
                        if ctl::floating_button(
                            ui,
                            Rect::from_center_size(center + vec2(offset, 110.0), Vec2::splat(60.0)),
                            icon,
                            label,
                        )
                        .clicked()
                        {
                            self.remote_skip(&status, seconds);
                        }
                    }
                    if ctl::floating_button(
                        ui,
                        Rect::from_center_size(center + vec2(0.0, 110.0), Vec2::splat(74.0)),
                        if status.paused {
                            Icon::Play
                        } else {
                            Icon::Pause
                        },
                        "Reproducir o pausar en TV",
                    )
                    .clicked()
                        && let Some(remote) = &self.casting.remote
                    {
                        remote.command(casting::Command::Pause(!status.paused));
                    }
                    let timeline = Rect::from_center_size(
                        center + vec2(0.0, 180.0),
                        vec2((r.width() - 80.0).min(540.0), 32.0),
                    );
                    let seek = ctl::slider(
                        ui,
                        timeline,
                        &mut self.casting.seek,
                        status.duration,
                        "Posición en TV",
                        0.0,
                    );
                    self.casting.dragging = seek.dragged();
                    if seek.drag_stopped() || (seek.changed() && !seek.dragged()) {
                        self.casting.burst = None;
                        if let Some(remote) = &self.casting.remote {
                            remote.command(casting::Command::Seek(self.casting.seek));
                        }
                    }
                    ui.painter().text(
                        center + vec2(0.0, 212.0),
                        Align2::CENTER_CENTER,
                        format!(
                            "{} / {}",
                            crate::ui::time_label(self.casting.seek),
                            crate::ui::time_label(status.duration)
                        ),
                        FontId::proportional(14.0),
                        muted(),
                    );
                } else {
                    ui.painter().text(
                        center + vec2(0.0, 90.0),
                        Align2::CENTER_CENTER,
                        "Controla la reproducción desde Safari o tu TV.",
                        FontId::proportional(15.0),
                        muted(),
                    );
                }
                if ctl::button(
                    ui,
                    Rect::from_center_size(
                        pos2(r.center().x, r.bottom() - 52.0),
                        vec2(232.0, 46.0),
                    ),
                    "Volver a este equipo",
                    Some(Icon::Play),
                    ButtonStyle::Soft,
                )
                .clicked()
                {
                    self.end_cast(true);
                }
            });
    }
    fn remote_skip(&mut self, status: &casting::Status, seconds: i32) {
        self.casting.burst = Some(playback_ui::SeekBurst::push(
            self.casting.burst.take(),
            status.position,
            status.duration,
            seconds,
            Instant::now(),
        ));
    }
}
