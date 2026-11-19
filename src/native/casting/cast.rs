//! Cast V2 framing from rust-cast, with bounded reads, deadlines and heartbeat handling.
use super::{Command, Status};
use native_tls::{TlsConnector, TlsStream};
use protobuf::Message;
use rust_cast::cast::cast_channel::{
    CastMessage,
    cast_message::{PayloadType, ProtocolVersion},
};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    time::{Duration, Instant},
};

const RECEIVER: &str = "urn:x-cast:com.google.cast.receiver";
const MEDIA: &str = "urn:x-cast:com.google.cast.media";
const HEARTBEAT: &str = "urn:x-cast:com.google.cast.tp.heartbeat";
const CONNECTION: &str = "urn:x-cast:com.google.cast.tp.connection";
const SENDER: &str = "megalodon";
pub struct Connection {
    stream: TlsStream<TcpStream>,
    request: u32,
    transport: String,
    media_session: i64,
    url: String,
    duration: f64,
}
fn frame(namespace: &str, destination: &str, payload: &Value) -> Result<Vec<u8>, String> {
    let mut message = CastMessage::new();
    message.set_protocol_version(ProtocolVersion::CASTV2_1_0);
    message.set_source_id(SENDER.into());
    message.set_destination_id(destination.into());
    message.set_namespace(namespace.into());
    message.set_payload_type(PayloadType::STRING);
    message.set_payload_utf8(payload.to_string());
    let bytes = message
        .write_to_bytes()
        .map_err(|_| "No se pudo preparar el mensaje de Chromecast.")?;
    let mut frame = (bytes.len() as u32).to_be_bytes().to_vec();
    frame.extend(bytes);
    Ok(frame)
}
fn packet(stream: &mut impl Read) -> Result<CastMessage, String> {
    let mut size = [0; 4];
    stream
        .read_exact(&mut size)
        .map_err(|_| "Chromecast no responde.")?;
    let size = u32::from_be_bytes(size) as usize;
    if size == 0 || size > 1_048_576 {
        return Err("Mensaje de Chromecast no válido.".into());
    }
    let mut bytes = vec![0; size];
    stream
        .read_exact(&mut bytes)
        .map_err(|_| "Chromecast interrumpió la conexión.")?;
    CastMessage::parse_from_bytes(&bytes).map_err(|_| "Respuesta de Chromecast no válida.".into())
}
impl Connection {
    pub fn open(
        address: SocketAddr,
        url: &str,
        mime: &str,
        title: &str,
        start: f64,
        duration: f64,
    ) -> Result<Self, String> {
        let std::net::IpAddr::V4(ip) = address.ip() else {
            return Err("Receptor no válido.".into());
        };
        if !super::lan_ip(ip) {
            return Err("Chromecast debe estar en la red local.".into());
        }
        let socket = TcpStream::connect_timeout(&address, Duration::from_secs(4))
            .map_err(|_| "No se pudo conectar con Chromecast.")?;
        socket
            .set_read_timeout(Some(Duration::from_secs(4)))
            .map_err(|_| "No se pudo preparar la conexión.")?;
        socket
            .set_write_timeout(Some(Duration::from_secs(4)))
            .map_err(|_| "No se pudo preparar la conexión.")?;
        // Cast receivers use self-signed device certificates, not public Web PKI.
        // This connector is scoped to the selected LAN IP; no Jellyfin token crosses it.
        let tls = TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true)
            .build()
            .map_err(|_| "No se pudo iniciar Cast V2.")?;
        let stream = tls
            .connect(&ip.to_string(), socket)
            .map_err(|_| "Chromecast rechazó la conexión Cast V2.")?;
        let mut connection = Self {
            stream,
            request: 0,
            transport: String::new(),
            media_session: 0,
            url: url.into(),
            duration,
        };
        connection.send(
            CONNECTION,
            "receiver-0",
            &json!({"type":"CONNECT","origin":{}}),
        )?;
        let app = connection.request(
            RECEIVER,
            "receiver-0",
            json!({"type":"LAUNCH","appId":"CC1AD845"}),
        )?;
        let app = app["status"]["applications"]
            .as_array()
            .and_then(|apps| apps.iter().find(|a| a["appId"] == "CC1AD845"))
            .ok_or("Chromecast no pudo abrir el reproductor.")?;
        let transport = app["transportId"]
            .as_str()
            .ok_or("Chromecast no devolvió una sesión.")?
            .to_owned();
        let session = app["sessionId"]
            .as_str()
            .ok_or("Chromecast no devolvió una sesión.")?;
        connection.send(
            CONNECTION,
            &transport,
            &json!({"type":"CONNECT","origin":{}}),
        )?;
        connection.transport.clone_from(&transport);
        let loaded=connection.request(MEDIA,&transport,json!({"type":"LOAD","sessionId":session,"autoplay":true,"currentTime":start,"media":{"contentId":url,"contentType":mime,"streamType":"BUFFERED","duration":duration,"metadata":{"metadataType":0,"title":title}}}))?;
        connection.media_session = loaded["status"]
            .as_array()
            .and_then(|s| s.first())
            .and_then(|s| s["mediaSessionId"].as_i64())
            .ok_or("Chromecast no pudo cargar el vídeo.")?;
        Ok(connection)
    }
    fn send(&mut self, namespace: &str, destination: &str, body: &Value) -> Result<(), String> {
        self.stream
            .write_all(&frame(namespace, destination, body)?)
            .map_err(|_| "No se pudo enviar el control a Chromecast.".into())
    }
    fn request(
        &mut self,
        namespace: &str,
        destination: &str,
        mut body: Value,
    ) -> Result<Value, String> {
        self.request = self.request.wrapping_add(1);
        body["requestId"] = json!(self.request);
        self.send(namespace, destination, &body)?;
        let deadline = Instant::now() + Duration::from_secs(12);
        while Instant::now() < deadline {
            let message = packet(&mut self.stream)?;
            if message.payload_type() != PayloadType::STRING {
                continue;
            }
            let body: Value = serde_json::from_str(message.payload_utf8())
                .map_err(|_| "Respuesta de Chromecast no válida.")?;
            if message.namespace() == HEARTBEAT && body["type"] == "PING" {
                self.send(HEARTBEAT, message.source_id(), &json!({"type":"PONG"}))?;
            } else if message.namespace() == CONNECTION && body["type"] == "CLOSE" {
                return Err("Chromecast ha cerrado la sesión.".into());
            } else if message.namespace() == namespace
                && body["requestId"].as_u64() == Some(self.request as u64)
            {
                if matches!(
                    body["type"].as_str(),
                    Some(
                        "LOAD_FAILED"
                            | "LOAD_CANCELLED"
                            | "LAUNCH_ERROR"
                            | "INVALID_REQUEST"
                            | "INVALID_PLAYER_STATE"
                    )
                ) {
                    return Err("Chromecast rechazó el vídeo o el control solicitado.".into());
                }
                return Ok(body);
            }
        }
        Err("Chromecast tardó demasiado en responder.".into())
    }
    fn entry(&mut self) -> Result<Value, String> {
        let transport = self.transport.clone();
        let body = self.request(MEDIA, &transport, json!({"type":"GET_STATUS"}))?;
        let entry = body["status"]
            .as_array()
            .and_then(|s| {
                s.iter()
                    .find(|s| s["mediaSessionId"].as_i64() == Some(self.media_session))
            })
            .ok_or("La sesión de Chromecast ha terminado o cambió de contenido.")?;
        if entry["media"]["contentId"]
            .as_str()
            .is_some_and(|url| url != self.url)
        {
            return Err("Chromecast está reproduciendo otro contenido.".into());
        }
        Ok(entry.clone())
    }
    pub fn status(&mut self) -> Result<Status, String> {
        let entry = self.entry()?;
        let state = entry["playerState"].as_str().unwrap_or("");
        if state == "IDLE" && entry["idleReason"] == "ERROR" {
            return Err("Chromecast no pudo reproducir este formato.".into());
        }
        Ok(Status {
            position: entry["currentTime"].as_f64().unwrap_or(0.0),
            duration: entry["media"]["duration"].as_f64().unwrap_or(self.duration),
            paused: state == "PAUSED",
            ended: state == "IDLE",
        })
    }
    pub fn command(&mut self, command: Command) -> Result<(), String> {
        self.entry()?; // A different media session must never receive our controls.
        let mut payload = json!({"mediaSessionId":self.media_session});
        payload["type"] = json!(match command {
            Command::Pause(true) => "PAUSE",
            Command::Pause(false) => "PLAY",
            Command::Seek(_) => "SEEK",
            Command::Stop => "STOP",
        });
        if let Command::Seek(seconds) = command {
            payload["currentTime"] = json!(seconds.max(0.0));
        }
        let transport = self.transport.clone();
        self.request(MEDIA, &transport, payload).map(|_| ())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cast_frames_round_trip_and_oversized_messages_are_rejected() {
        let encoded = frame(
            MEDIA,
            "transport-1",
            &json!({"type":"SEEK","requestId":7,"mediaSessionId":42,"currentTime":30}),
        )
        .unwrap();
        let decoded = packet(&mut encoded.as_slice()).unwrap();
        assert_eq!(decoded.namespace(), MEDIA);
        assert_eq!(decoded.destination_id(), "transport-1");
        let payload: Value = serde_json::from_str(decoded.payload_utf8()).unwrap();
        assert_eq!(payload["mediaSessionId"], 42);
        assert_eq!(payload["currentTime"], 30);
        assert!(packet(&mut u32::MAX.to_be_bytes().as_slice()).is_err());
    }
}
