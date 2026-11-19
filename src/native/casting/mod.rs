//! LAN receivers are discovered only on request. A relay exposes one temporary media session,
//! never the Jellyfin credential or a general-purpose proxy.
mod cast;
mod dlna;
mod relay;

use super::{data, jellyfin::Library};
pub use relay::Relay;
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct Device {
    pub name: String,
    pub ip: Ipv4Addr,
    pub kind: Kind,
}
#[derive(Clone)]
pub enum Kind {
    Cast(u16),
    Dlna { control: String, service: String },
}
impl Device {
    pub fn protocol(&self) -> &'static str {
        match self.kind {
            Kind::Cast(_) => "Chromecast",
            Kind::Dlna { .. } => "TV · DLNA",
        }
    }
    pub fn key(&self) -> String {
        format!("{}:{}", self.protocol(), self.ip)
    }
}
pub fn local_address(ip: Ipv4Addr) -> Result<Ipv4Addr, String> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))
        .map_err(|_| "No se pudo abrir la conexión de red.")?;
    socket
        .connect((ip, 9))
        .map_err(|_| "El receptor no está en una red accesible.")?;
    match socket
        .local_addr()
        .map_err(|_| "No se pudo identificar la red.")?
        .ip()
    {
        IpAddr::V4(ip) if ip.is_private() || ip.is_link_local() => Ok(ip),
        _ => Err("Conecta ambos dispositivos a la misma red local.".into()),
    }
}
pub fn lan_ip(ip: Ipv4Addr) -> bool {
    ip.is_private() || ip.is_link_local()
}

pub async fn discover() -> Result<Vec<Device>, String> {
    let (cast, dlna) = tokio::join!(discover_cast(), dlna::discover());
    if cast.is_err() && dlna.is_err() {
        return Err(
            "No se pudo buscar en la red local. Comprueba la conexión y el firewall.".into(),
        );
    }
    let mut devices = cast.unwrap_or_default();
    devices.extend(dlna.unwrap_or_default());
    devices.sort_by_key(|d| d.name.to_lowercase());
    devices.dedup_by_key(|d| d.key());
    Ok(devices)
}
async fn discover_cast() -> Result<Vec<Device>, String> {
    use mdns_sd::{ServiceDaemon, ServiceEvent};
    let daemon = ServiceDaemon::new().map_err(|_| "No se pudo buscar Chromecast.")?;
    let events = daemon
        .browse("_googlecast._tcp.local.")
        .map_err(|_| "No se pudo buscar Chromecast.")?;
    let mut devices = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(4);
    while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, events.recv_async()).await {
        if let ServiceEvent::ServiceResolved(info) = event {
            // Bit 0 is video output; Chromecast Audio is not a video target.
            let video = info
                .get_property_val_str("ca")
                .and_then(|s| s.parse::<u32>().ok())
                .is_none_or(|v| v & 1 != 0);
            if video && let Some(ip) = info.get_addresses_v4().into_iter().find(|ip| lan_ip(*ip)) {
                devices.push(Device {
                    name: info
                        .get_property_val_str("fn")
                        .unwrap_or("Chromecast")
                        .to_owned(),
                    ip,
                    kind: Kind::Cast(info.get_port()),
                });
            }
        }
    }
    let _ = daemon.stop_browse("_googlecast._tcp.local.");
    let _ = daemon.shutdown();
    Ok(devices)
}

#[derive(Clone, Copy, Debug)]
pub enum Command {
    Pause(bool),
    Seek(f64),
    Stop,
}
#[derive(Clone, Default)]
pub struct Status {
    pub position: f64,
    pub duration: f64,
    pub paused: bool,
    pub ended: bool,
}
pub enum Event {
    Connected,
    Status(Status),
    Failed(String),
    Ended,
}

pub struct Remote {
    pub device: Device,
    pub relay: Relay,
    pub status: Status,
    pub connected: bool,
    pub playback: data::Playback,
    pub events: mpsc::Receiver<Event>,
    commands: mpsc::Sender<Command>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Remote {
    pub fn command(&self, command: Command) {
        let _ = self.commands.send(command);
    }
    pub fn shutdown(&mut self) {
        self.command(Command::Stop);
        self.relay.close();
    }
    pub fn finish(&mut self) {
        self.shutdown();
        // Every socket operation has a deadline. Do not freeze the UI waiting for a TV.
        if self.worker.as_ref().is_some_and(|w| w.is_finished())
            && let Some(worker) = self.worker.take()
        {
            let _ = worker.join();
        }
    }
}
impl Drop for Remote {
    fn drop(&mut self) {
        self.finish();
    }
}

pub async fn start(
    library: Arc<Library>,
    playback: data::Playback,
    user: String,
    device: Device,
    start: f64,
) -> Result<Remote, String> {
    let local = local_address(device.ip)?;
    let relay = Relay::start(library, &playback, &user, local, Some(device.ip), start).await?;
    let (commands, rx) = mpsc::channel();
    let (tx, events) = mpsc::channel();
    let address = SocketAddr::from((
        device.ip,
        match device.kind {
            Kind::Cast(port) => port,
            _ => 0,
        },
    ));
    let selected = device.clone();
    let url = relay.media_url();
    let mime = relay.mime.clone();
    let title = playback.item.name.clone();
    let duration = playback.item.run_time_ticks.unwrap_or(0) as f64 / 10_000_000.0;
    let worker = std::thread::Builder::new()
        .name("megalodon-cast".into())
        .spawn(move || {
            let run = || -> Result<(), String> {
                let mut backend = match &selected.kind {
                    Kind::Cast(_) => Backend::Cast(Box::new(cast::Connection::open(
                        address, &url, &mime, &title, start, duration,
                    )?)),
                    Kind::Dlna { control, service } => Backend::Dlna(dlna::Connection::open(
                        control, service, &url, &mime, &title, start,
                    )?),
                };
                // Cancelled while the receiver was connecting: stop only this session.
                if matches!(
                    rx.try_recv(),
                    Ok(Command::Stop) | Err(mpsc::TryRecvError::Disconnected)
                ) {
                    let _ = backend.command(Command::Stop);
                    return Ok(());
                }
                let _ = tx.send(Event::Connected);
                let mut last_poll = Instant::now() - Duration::from_secs(3);
                loop {
                    match rx.recv_timeout(Duration::from_millis(150)) {
                        Ok(command) => {
                            backend.command(command)?;
                            if matches!(command, Command::Stop) {
                                return Ok(());
                            }
                            last_poll = Instant::now() - Duration::from_secs(3);
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => {
                            let _ = backend.command(Command::Stop);
                            return Ok(());
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                    }
                    if last_poll.elapsed() >= Duration::from_secs(2) {
                        let status = backend.status()?;
                        let ended = status.ended;
                        let _ = tx.send(Event::Status(status));
                        if ended {
                            let _ = tx.send(Event::Ended);
                            return Ok(());
                        }
                        last_poll = Instant::now();
                    }
                }
            };
            if let Err(error) = run() {
                let _ = tx.send(Event::Failed(error));
            }
        })
        .map_err(|_| "No se pudo iniciar la transmisión.")?;
    let mut remote_playback = playback;
    remote_playback.start = start;
    remote_playback.session_id.clone_from(&relay.session);
    remote_playback.source_id.clone_from(&relay.source);
    Ok(Remote {
        device,
        status: Status {
            position: start,
            duration,
            ..Default::default()
        },
        connected: false,
        playback: remote_playback,
        relay,
        events,
        commands,
        worker: Some(worker),
    })
}
enum Backend {
    Cast(Box<cast::Connection>),
    Dlna(dlna::Connection),
}
impl Backend {
    fn command(&mut self, command: Command) -> Result<(), String> {
        match self {
            Self::Cast(c) => c.command(command),
            Self::Dlna(c) => c.command(command),
        }
    }
    fn status(&mut self) -> Result<Status, String> {
        match self {
            Self::Cast(c) => c.status(),
            Self::Dlna(c) => c.status(),
        }
    }
}
