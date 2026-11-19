use super::{Command, Device, Kind, Status, lan_ip};
use reqwest::Url;
use std::{
    collections::HashSet,
    net::{IpAddr, Ipv4Addr},
    time::Duration,
};

fn local_url(value: &str, ip: Ipv4Addr) -> Result<Url, String> {
    let url = Url::parse(value).map_err(|_| "El televisor devolvió una dirección no válida.")?;
    if !lan_ip(ip)
        || url.scheme() != "http"
        || url.host_str() != Some(ip.to_string().as_str())
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err("El televisor devolvió una dirección ajena a su red.".into());
    }
    Ok(url)
}
fn location(packet: &str, ip: Ipv4Addr) -> Option<Url> {
    if !packet.starts_with("HTTP/1.1 200") {
        return None;
    }
    packet
        .lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(key, _)| key.eq_ignore_ascii_case("location"))
        .and_then(|(_, v)| local_url(v.trim(), ip).ok())
}
fn text<'a>(doc: &'a roxmltree::Document<'a>, name: &str) -> Option<&'a str> {
    doc.descendants()
        .find(|n| n.has_tag_name(name))
        .and_then(|n| n.text())
}
fn parse_device(xml: &str, location: &Url, ip: Ipv4Addr) -> Result<Device, String> {
    let doc =
        roxmltree::Document::parse(xml).map_err(|_| "Descripción del televisor no válida.")?;
    let service = doc
        .descendants()
        .filter(|n| n.has_tag_name("service"))
        .find(|node| {
            node.children().any(|n| {
                n.has_tag_name("serviceType")
                    && n.text()
                        .is_some_and(|s| s.starts_with("urn:schemas-upnp-org:service:AVTransport:"))
            })
        })
        .ok_or("El dispositivo no admite vídeo DLNA.")?;
    let kind = service
        .children()
        .find(|n| n.has_tag_name("serviceType"))
        .and_then(|n| n.text())
        .unwrap();
    if !kind
        .rsplit(':')
        .next()
        .is_some_and(|v| v.parse::<u32>().is_ok())
    {
        return Err("Servicio DLNA no válido.".into());
    }
    let path = service
        .children()
        .find(|n| n.has_tag_name("controlURL"))
        .and_then(|n| n.text())
        .ok_or("Falta el control del televisor.")?;
    let base = text(&doc, "URLBase")
        .filter(|s| !s.trim().is_empty())
        .map(|s| local_url(s.trim(), ip))
        .transpose()?
        .unwrap_or_else(|| location.clone());
    let control = local_url(
        base.join(path)
            .map_err(|_| "Control de vídeo no válido.")?
            .as_str(),
        ip,
    )?;
    Ok(Device {
        name: text(&doc, "friendlyName")
            .unwrap_or("Televisor")
            .chars()
            .take(100)
            .collect(),
        ip,
        kind: Kind::Dlna {
            control: control.into(),
            service: kind.into(),
        },
    })
}
pub async fn discover() -> Result<Vec<Device>, String> {
    let mut jobs = tokio::task::JoinSet::new();
    let mut ips = HashSet::new();
    for interface in if_addrs::get_if_addrs().map_err(|_| "No se pudo consultar la red local.")? {
        if let IpAddr::V4(ip) = interface.ip()
            && lan_ip(ip)
            && ips.insert(ip)
            && ips.len() <= 16
        {
            jobs.spawn(async move {
                let socket=tokio::net::UdpSocket::bind((ip,0)).await.ok()?;
                let _=socket.set_multicast_ttl_v4(2);
                let query=b"M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: 2\r\nST: urn:schemas-upnp-org:device:MediaRenderer:1\r\n\r\n";
                socket.send_to(query,(Ipv4Addr::new(239,255,255,250),1900)).await.ok()?;
                let until=tokio::time::Instant::now()+Duration::from_secs(3);
                let mut found=Vec::new();
                let mut bytes=[0u8;8192];
                while found.len()<32 {
                    let Ok(Ok((size,sender)))=tokio::time::timeout_at(until,socket.recv_from(&mut bytes)).await else {break};
                    if let IpAddr::V4(peer)=sender.ip() && let Some(url)=location(&String::from_utf8_lossy(&bytes[..size]),peer) {
                        found.push((peer,url));
                    }
                }
                Some(found)
            });
        }
    }
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(3))
        .build()
        .map_err(|_| "No se pudo buscar televisores.")?;
    let mut descriptions = tokio::task::JoinSet::new();
    let mut known = HashSet::new();
    while let Some(Ok(Some(found))) = jobs.join_next().await {
        for (ip, url) in found {
            if known.len() >= 32 || !known.insert(url.to_string()) {
                continue;
            }
            let client = client.clone();
            descriptions.spawn(async move {
                let mut response = client
                    .get(url.clone())
                    .send()
                    .await
                    .ok()?
                    .error_for_status()
                    .ok()?;
                let mut bytes = Vec::new();
                while let Some(chunk) = response.chunk().await.ok()? {
                    if bytes.len() + chunk.len() > 262_144 {
                        return None;
                    }
                    bytes.extend_from_slice(&chunk);
                }
                parse_device(std::str::from_utf8(&bytes).ok()?, &url, ip).ok()
            });
        }
    }
    let mut devices = Vec::new();
    while let Some(result) = descriptions.join_next().await {
        if let Ok(Some(device)) = result {
            devices.push(device);
        }
    }
    Ok(devices)
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn soap(service: &str, action: &str, args: &[(&str, String)]) -> String {
    let body = args
        .iter()
        .map(|(name, value)| format!("<{name}>{}</{name}>", escape(value)))
        .collect::<String>();
    format!(
        "<?xml version=\"1.0\"?><s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\"><s:Body><u:{action} xmlns:u=\"{service}\">{body}</u:{action}></s:Body></s:Envelope>"
    )
}
pub struct Connection {
    client: reqwest::blocking::Client,
    control: String,
    service: String,
    url: String,
}
impl Connection {
    pub fn open(
        control: &str,
        service: &str,
        url: &str,
        mime: &str,
        title: &str,
        start: f64,
    ) -> Result<Self, String> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| "No se pudo conectar con el televisor.")?;
        let mut connection = Self {
            client,
            control: control.into(),
            service: service.into(),
            url: url.into(),
        };
        let metadata = format!(
            "<DIDL-Lite xmlns=\"urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:upnp=\"urn:schemas-upnp-org:metadata-1-0/upnp/\"><item id=\"0\" parentID=\"-1\" restricted=\"1\"><dc:title>{}</dc:title><upnp:class>object.item.videoItem</upnp:class><res protocolInfo=\"http-get:*:{}:*\">{}</res></item></DIDL-Lite>",
            escape(title),
            escape(mime),
            escape(url)
        );
        connection.call(
            "SetAVTransportURI",
            vec![("CurrentURI", url.into()), ("CurrentURIMetaData", metadata)],
        )?;
        // SetAVTransportURI must succeed before Play. On failure, do not stop another owner's media.
        let mut loaded = || -> Result<(), String> {
            connection.call("Play", vec![("Speed", "1".into())])?;
            if start >= 1.0 {
                connection.command(Command::Seek(start))?;
            }
            Ok(())
        };
        if let Err(error) = loaded() {
            let _ = connection.command(Command::Stop);
            return Err(error);
        }
        Ok(connection)
    }
    fn call(&self, action: &str, mut args: Vec<(&str, String)>) -> Result<String, String> {
        use std::io::Read;
        args.insert(0, ("InstanceID", "0".into()));
        let response = self
            .client
            .post(&self.control)
            .header("Content-Type", "text/xml; charset=\"utf-8\"")
            .header("SOAPAction", format!("\"{}#{action}\"", self.service))
            .body(soap(&self.service, action, &args))
            .send()
            .map_err(|_| "El televisor no responde. Comprueba su conexión.")?;
        let ok = response.status().is_success();
        let mut body = String::new();
        response
            .take(262_145)
            .read_to_string(&mut body)
            .map_err(|_| "Respuesta del televisor no válida.")?;
        if body.len() > 262_144 {
            return Err("Respuesta del televisor demasiado grande.".into());
        }
        if !ok {
            return Err(match action {
                "Seek" => "Este televisor no admite saltos en este formato.",
                "SetAVTransportURI" => "El televisor no admite este vídeo.",
                _ => "El televisor rechazó el control de reproducción.",
            }
            .into());
        }
        Ok(body)
    }
    fn position(&self) -> Result<(String, f64, f64), String> {
        let body = self.call("GetPositionInfo", vec![])?;
        let doc =
            roxmltree::Document::parse(&body).map_err(|_| "Posición del televisor no válida.")?;
        Ok((
            text(&doc, "TrackURI").unwrap_or("").into(),
            seconds(text(&doc, "RelTime").unwrap_or("0")),
            seconds(text(&doc, "TrackDuration").unwrap_or("0")),
        ))
    }
    pub fn command(&mut self, command: Command) -> Result<(), String> {
        let (url, _, _) = self.position()?;
        if url != self.url {
            return Err("El televisor está reproduciendo otro contenido.".into());
        }
        match command {
            Command::Pause(true) => self.call("Pause", vec![]),
            Command::Pause(false) => self.call("Play", vec![("Speed", "1".into())]),
            Command::Stop => self.call("Stop", vec![]),
            Command::Seek(value) => {
                let n = value.max(0.0) as u64;
                self.call(
                    "Seek",
                    vec![
                        ("Unit", "REL_TIME".into()),
                        (
                            "Target",
                            format!("{:02}:{:02}:{:02}", n / 3600, n / 60 % 60, n % 60),
                        ),
                    ],
                )
            }
        }
        .map(|_| ())
    }
    pub fn status(&mut self) -> Result<Status, String> {
        let (url, position, duration) = self.position()?;
        if !url.is_empty() && url != self.url {
            return Err("La reproducción del televisor cambió desde otro dispositivo.".into());
        }
        let body = self.call("GetTransportInfo", vec![])?;
        let doc =
            roxmltree::Document::parse(&body).map_err(|_| "Estado del televisor no válido.")?;
        let state = text(&doc, "CurrentTransportState").unwrap_or("");
        Ok(Status {
            position,
            duration,
            paused: state == "PAUSED_PLAYBACK",
            ended: matches!(state, "STOPPED" | "NO_MEDIA_PRESENT"),
        })
    }
}
fn seconds(text: &str) -> f64 {
    let parts: Vec<_> = text.split(':').collect();
    if parts.len() != 3 {
        return 0.0;
    }
    parts
        .iter()
        .try_fold(0.0, |sum, p| p.parse::<f64>().ok().map(|v| sum * 60.0 + v))
        .filter(|v| v.is_finite() && *v >= 0.0)
        .unwrap_or(0.0)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn soap_loads_before_playing_and_never_stops_another_owners_video() {
        use axum::{Router, extract::State, http::HeaderMap, routing::post};
        use std::sync::{Arc, Mutex};
        #[derive(Default)]
        struct Receiver {
            uri: String,
            actions: Vec<String>,
            title: String,
            target: String,
        }
        async fn control(
            State(state): State<Arc<Mutex<Receiver>>>,
            headers: HeaderMap,
            body: String,
        ) -> String {
            let action = headers["soapaction"]
                .to_str()
                .unwrap()
                .trim_matches('"')
                .rsplit('#')
                .next()
                .unwrap();
            let doc = roxmltree::Document::parse(&body).unwrap();
            let mut state = state.lock().unwrap();
            state.actions.push(action.into());
            let result = match action {
                "SetAVTransportURI" => {
                    state.uri = text(&doc, "CurrentURI").unwrap().into();
                    let metadata =
                        roxmltree::Document::parse(text(&doc, "CurrentURIMetaData").unwrap())
                            .unwrap();
                    state.title = text(&metadata, "title").unwrap().into();
                    String::new()
                }
                "Seek" => {
                    state.target = text(&doc, "Target").unwrap().into();
                    String::new()
                }
                "GetPositionInfo" => format!(
                    "<TrackURI>{}</TrackURI><RelTime>00:01:12</RelTime><TrackDuration>01:30:00</TrackDuration>",
                    escape(&state.uri)
                ),
                "GetTransportInfo" => {
                    "<CurrentTransportState>PAUSED_PLAYBACK</CurrentTransportState>".into()
                }
                _ => String::new(),
            };
            format!("<Response>{result}</Response>")
        }
        let state = Arc::new(Mutex::new(Receiver::default()));
        let router = Router::new()
            .route("/control", post(control))
            .with_state(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/control", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        tokio::task::spawn_blocking(move || {
            let mut receiver = Connection::open(
                &url,
                "urn:schemas-upnp-org:service:AVTransport:1",
                "http://127.0.0.1/media?x=1&y=2",
                "video/mp4",
                "A & B",
                72.0,
            )
            .unwrap();
            receiver.command(Command::Pause(true)).unwrap();
            let status = receiver.status().unwrap();
            assert_eq!(status.position, 72.0);
            assert_eq!(status.duration, 5400.0);
            assert!(status.paused);
            {
                let mut state = state.lock().unwrap();
                assert_eq!(
                    &state.actions[..4],
                    &["SetAVTransportURI", "Play", "GetPositionInfo", "Seek"]
                );
                assert_eq!(state.title, "A & B");
                assert_eq!(state.target, "00:01:12");
                state.uri = "http://another-owner/video".into();
            }
            assert!(receiver.command(Command::Stop).is_err());
            assert!(
                !state
                    .lock()
                    .unwrap()
                    .actions
                    .iter()
                    .any(|action| action == "Stop")
            );
        })
        .await
        .unwrap();
        server.abort();
    }
    #[test]
    fn receiver_discovery_rejects_cross_host_urls_and_escapes_metadata() {
        let ip = Ipv4Addr::new(192, 168, 1, 20);
        let base = local_url("http://192.168.1.20:1234/device.xml", ip).unwrap();
        assert!(local_url("http://example.com/control", ip).is_err());
        assert!(local_url("http://user:pass@192.168.1.20/control", ip).is_err());
        let xml = "<root><device><friendlyName>LG salón</friendlyName><serviceList><service><serviceType>urn:schemas-upnp-org:service:AVTransport:1</serviceType><controlURL>/control</controlURL></service></serviceList></device></root>";
        assert_eq!(parse_device(xml, &base, ip).unwrap().name, "LG salón");
        assert!(
            parse_device(
                &xml.replace("/control", "http://192.168.1.21/control"),
                &base,
                ip
            )
            .is_err()
        );
        let body = soap(
            "urn:schemas-upnp-org:service:AVTransport:1",
            "SetAVTransportURI",
            &[("CurrentURI", "http://host/a?x=1&y=2".into())],
        );
        let doc = roxmltree::Document::parse(&body).unwrap();
        assert_eq!(text(&doc, "CurrentURI"), Some("http://host/a?x=1&y=2"));
        assert_eq!(seconds("01:02:03.5"), 3723.5);
    }
}
