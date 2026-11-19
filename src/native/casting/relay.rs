use super::{Library, Status, data};
#[cfg(test)]
#[path = "relay_tests.rs"]
mod integration_tests;
use axum::{
    Router,
    body::Body,
    extract::{ConnectInfo, Path, State},
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use reqwest::Url;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

struct Shared {
    ticket: String,
    peer: Option<Ipv4Addr>,
    client: reqwest::Client,
    resources: Mutex<Vec<Url>>,
    base: Url,
    prefix: String,
    public: String,
    title: String,
    start: f64,
    duration: f64,
    progress: Mutex<Option<(Status, Instant)>>,
    cancel: tokio::sync::watch::Receiver<bool>,
}
pub struct Relay {
    state: Arc<Shared>,
    cancel: tokio::sync::watch::Sender<bool>,
    pub mime: String,
    pub source: String,
    pub session: String,
    pub transcoding: bool,
    library: Arc<Library>,
    closed: bool,
    runtime: tokio::runtime::Handle,
}
impl Relay {
    pub async fn start(
        library: Arc<Library>,
        playback: &data::Playback,
        user: &str,
        local: Ipv4Addr,
        peer: Option<Ipv4Addr>,
        start: f64,
    ) -> Result<Self, String> {
        let info=library.client.post(format!("{}/Items/{}/PlaybackInfo",library.config.base_url,playback.item.id)).json(&json!({
            "UserId":user,"StartTimeTicks":0,"IsPlayback":true,"EnableDirectPlay":true,"EnableDirectStream":true,"EnableTranscoding":true,"AutoOpenLiveStream":true,
            "DeviceProfile":{"Name":"Megalodon Cast","MaxStreamingBitrate":40_000_000,
                "DirectPlayProfiles":[{"Type":"Video","Container":"mp4,m4v","VideoCodec":"h264","AudioCodec":"aac,mp3"}],
                "TranscodingProfiles":[{"Type":"Video","Container":"ts","Protocol":"hls","VideoCodec":"h264","AudioCodec":"aac","Context":"Streaming","MaxAudioChannels":"2","MinSegments":2,"BreakOnNonKeyFrames":false}],
                "CodecProfiles":[{"Type":"Video","Codec":"h264","Conditions":[{"Condition":"LessThanEqual","Property":"VideoLevel","Value":"41","IsRequired":true},{"Condition":"LessThanEqual","Property":"Width","Value":"1920","IsRequired":true},{"Condition":"LessThanEqual","Property":"Height","Value":"1080","IsRequired":true}]}],
                "SubtitleProfiles":[{"Format":"vtt","Method":"External"}]
            }
        })).send().await.map_err(|_|"Jellyfin no pudo preparar el vídeo para la TV.")?.error_for_status().map_err(|_|"Jellyfin no permite transmitir este vídeo.")?.json::<Value>().await.map_err(|_|"Respuesta de reproducción no válida.")?;
        let source = info["MediaSources"]
            .as_array()
            .and_then(|s| s.first())
            .ok_or("No hay un archivo disponible.")?;
        let source_id = source["Id"]
            .as_str()
            .ok_or("Fuente de vídeo no válida.")?
            .to_owned();
        let session = info["PlaySessionId"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or("Jellyfin no devolvió una sesión de reproducción.")?
            .to_owned();
        let base = Url::parse(&format!("{}/", library.config.base_url))
            .map_err(|_| "Dirección de Jellyfin no válida.")?;
        let prefix = format!("{}videos/{}/", base.path(), playback.item.id).to_lowercase();
        let direct = source["SupportsDirectPlay"].as_bool().unwrap_or(false);
        let (url, mime) = if direct {
            let mut url = base
                .join(&format!("Videos/{}/stream.mp4", playback.item.id))
                .map_err(|_| "URL de vídeo no válida.")?;
            url.query_pairs_mut()
                .append_pair("Static", "true")
                .append_pair("MediaSourceId", &source_id)
                .append_pair("PlaySessionId", &session);
            (url, "video/mp4")
        } else {
            let path=source["TranscodingUrl"].as_str().ok_or("El servidor no ofrece un formato compatible con este receptor. Comprueba que el perfil permita transcodificar.")?;
            (
                upstream(&base, &base, &prefix, path)?,
                "application/vnd.apple.mpegurl",
            )
        };
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Authorization",
            crate::jellyfin::authorization(&library.config.token)
                .parse()
                .map_err(|_| "Sesión no válida.")?,
        );
        // Stream indefinitely, but never follow a redirect carrying the session header.
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .connect_timeout(Duration::from_secs(8))
            .read_timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| "No se pudo iniciar el envío de vídeo.")?;
        let listener = tokio::net::TcpListener::bind((local, 0)).await.map_err(
            |_| "No se pudo abrir la conexión local. Comprueba el firewall de la red privada.",
        )?;
        let address = listener
            .local_addr()
            .map_err(|_| "No se pudo abrir la conexión local.")?;
        let mut random = [0; 24];
        getrandom::fill(&mut random).map_err(|_| "No se pudo proteger la sesión de vídeo.")?;
        let ticket = random
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let public = format!("http://{address}/{ticket}");
        let (cancel, rx) = tokio::sync::watch::channel(false);
        let state = Arc::new(Shared {
            ticket,
            peer,
            client,
            resources: Mutex::new(vec![url]),
            base,
            prefix,
            public,
            title: playback.item.name.clone(),
            start,
            duration: playback.item.run_time_ticks.unwrap_or(0) as f64 / 10_000_000.0,
            progress: Mutex::new(None),
            cancel: rx.clone(),
        });
        let router = Router::new()
            .route("/{ticket}/media/{index}", get(media))
            .route("/{ticket}/watch", get(watch))
            .route("/{ticket}/client/{asset}", get(asset))
            .route("/{ticket}/progress", post(progress))
            .layer(axum::extract::DefaultBodyLimit::max(2048))
            .with_state(state.clone());
        tokio::spawn(async move {
            let mut rx = rx;
            let _ = axum::serve(
                listener,
                router.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .with_graceful_shutdown(async move {
                let _ = rx.changed().await;
            })
            .await;
        });
        Ok(Self {
            state,
            cancel,
            mime: mime.into(),
            source: source_id,
            session,
            transcoding: !direct,
            library,
            closed: false,
            runtime: tokio::runtime::Handle::current(),
        })
    }
    pub fn media_url(&self) -> String {
        format!("{}/media/0", self.state.public)
    }
    pub fn watch_url(&self) -> String {
        format!("{}/watch", self.state.public)
    }
    pub fn progress(&self) -> Option<(Status, Instant)> {
        self.state.progress.lock().unwrap().clone()
    }
    pub fn close(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        let _ = self.cancel.send(true);
        if self.transcoding {
            let library = self.library.clone();
            let session = self.session.clone();
            self.runtime.spawn(async move {
                let _ = library
                    .client
                    .delete(format!(
                        "{}/Videos/ActiveEncodings",
                        library.config.base_url
                    ))
                    .query(&[
                        ("DeviceId", "megalodon-rust-desktop"),
                        ("PlaySessionId", session.as_str()),
                    ])
                    .send()
                    .await;
            });
        }
    }
}
impl Drop for Relay {
    fn drop(&mut self) {
        self.close();
    }
}
fn permitted(state: &Shared, ticket: &str, addr: SocketAddr) -> bool {
    if *state.cancel.borrow() || ticket != state.ticket {
        return false;
    }
    match addr.ip() {
        std::net::IpAddr::V4(ip) => state
            .peer
            .map_or_else(|| super::lan_ip(ip), |peer| peer == ip),
        _ => false,
    }
}
fn upstream(base: &Url, current: &Url, prefix: &str, value: &str) -> Result<Url, String> {
    let mut url = current
        .join(value)
        .map_err(|_| "Referencia de vídeo no válida.")?;
    if url.origin() != base.origin()
        || !url.path().to_lowercase().starts_with(prefix)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err("La lista de vídeo contiene una dirección no permitida.".into());
    }
    let pairs: Vec<_> = url
        .query_pairs()
        .filter(|(key, _)| {
            !matches!(
                key.to_ascii_lowercase().as_str(),
                "api_key" | "apikey" | "access_token" | "token"
            )
        })
        .map(|(a, b)| (a.into_owned(), b.into_owned()))
        .collect();
    url.set_query(None);
    if !pairs.is_empty() {
        url.query_pairs_mut().extend_pairs(pairs);
    }
    Ok(url)
}
fn mapped(state: &Shared, current: &Url, value: &str) -> Result<String, String> {
    let url = upstream(&state.base, current, &state.prefix, value)?;
    let mut urls = state.resources.lock().unwrap();
    let index = if let Some(index) = urls.iter().position(|u| u == &url) {
        index
    } else {
        if urls.len() >= 16_384 {
            return Err("La lista de vídeo es demasiado larga.".into());
        }
        urls.push(url);
        urls.len() - 1
    };
    Ok(format!("{}/media/{index}", state.public))
}
fn manifest(state: &Shared, current: &Url, input: &str) -> Result<String, String> {
    if !input.trim_start().starts_with("#EXTM3U") {
        return Err("Lista HLS no válida.".into());
    }
    let mut result = String::new();
    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() {
            result.push('\n');
            continue;
        }
        if !line.starts_with('#') {
            result.push_str(&mapped(state, current, line)?);
        } else if let Some((before, after)) = line.split_once("URI=\"") {
            let (value, rest) = after.split_once('"').ok_or("URI HLS no válida.")?;
            result.push_str(before);
            result.push_str("URI=\"");
            result.push_str(&mapped(state, current, value)?);
            result.push('"');
            result.push_str(rest);
        } else {
            result.push_str(line);
        }
        result.push('\n');
    }
    Ok(result)
}
fn headers(response: &mut Response) {
    for (name, value) in [
        ("access-control-allow-origin", "*"),
        (
            "access-control-expose-headers",
            "Content-Range, Accept-Ranges, Content-Length",
        ),
        ("cache-control", "no-store"),
        ("referrer-policy", "no-referrer"),
        ("transfermode.dlna.org", "Streaming"),
        ("x-content-type-options", "nosniff"),
    ] {
        response.headers_mut().insert(name, value.parse().unwrap());
    }
}
async fn media(
    State(state): State<Arc<Shared>>,
    Path((ticket, index)): Path<(String, usize)>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    method: Method,
    headers_in: HeaderMap,
) -> Response {
    if !permitted(&state, &ticket, peer) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Some(url) = state.resources.lock().unwrap().get(index).cloned() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let mut request = state.client.request(method.clone(), url.clone());
    if let Some(range) = headers_in.get("range") {
        request = request.header("range", range);
    }
    let mut cancel = state.cancel.clone();
    let response = tokio::select! {result=request.send()=>result,_=cancel.changed()=>return StatusCode::GONE.into_response()};
    let Ok(mut response) = response else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    if !response.status().is_success() {
        return StatusCode::BAD_GATEWAY.into_response();
    }
    let is_hls = url.path().ends_with(".m3u8")
        || response
            .headers()
            .get("content-type")
            .and_then(|s| s.to_str().ok())
            .is_some_and(|s| s.contains("mpegurl"));
    if is_hls && method != Method::HEAD {
        let mut bytes = Vec::new();
        loop {
            let chunk = tokio::select! {chunk=response.chunk()=>chunk,_=cancel.changed()=>return StatusCode::GONE.into_response()};
            match chunk {
                Ok(Some(chunk)) if bytes.len() + chunk.len() <= 2_000_000 => {
                    bytes.extend_from_slice(&chunk)
                }
                Ok(None) => break,
                _ => return StatusCode::BAD_GATEWAY.into_response(),
            }
        }
        let Ok(output) = std::str::from_utf8(&bytes)
            .map_err(|_| String::new())
            .and_then(|s| manifest(&state, &url, s))
        else {
            return StatusCode::BAD_GATEWAY.into_response();
        };
        let mut response =
            ([("content-type", "application/vnd.apple.mpegurl")], output).into_response();
        headers(&mut response);
        return response;
    }
    let mut builder = Response::builder().status(response.status());
    for name in [
        "content-type",
        "content-length",
        "content-range",
        "accept-ranges",
    ] {
        if let Some(value) = response.headers().get(name) {
            builder = builder.header(name, value);
        }
    }
    let stream = async_stream::stream! {
        loop {
            let chunk=tokio::select! {chunk=response.chunk()=>chunk,_=cancel.changed()=>break};
            match chunk {Ok(Some(bytes))=>yield Ok::<_,reqwest::Error>(bytes),Ok(None)=>break,Err(e)=>{yield Err(e);break;}}
        }
    };
    let mut response = builder
        .body(if method == Method::HEAD {
            Body::empty()
        } else {
            Body::from_stream(stream)
        })
        .unwrap();
    headers(&mut response);
    response
}
async fn watch(
    State(state): State<Arc<Shared>>,
    Path(ticket): Path<String>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
) -> Response {
    if !permitted(&state, &ticket, peer) || state.peer.is_some() {
        return StatusCode::NOT_FOUND.into_response();
    }
    let document = maud::html! {
        (maud::DOCTYPE) html lang="es" {head {meta charset="utf-8";meta name="viewport" content="width=device-width,initial-scale=1";title {(&state.title)}
            style {"html{color-scheme:dark;background:#08090b;color:#f5f5f7;font:17px system-ui}body{max-width:960px;margin:0 auto;padding:24px}video{width:100%;max-height:72vh;background:#000;border-radius:12px}h1{font-size:24px;font-weight:600}p{color:#a3a7b0;line-height:1.5}button{font:inherit;color:#08090b;background:#f5f5f7;border:0;border-radius:14px;padding:14px 22px;cursor:pointer}button:active{transform:scale(.97)}"}
        }body data-airplay="true" {h1 {(&state.title)}video id="handoff-video" controls playsinline preload="metadata" x-webkit-airplay="allow" data-start=(state.start) src=(format!("{}/media/0",state.public)) {} p {"Abre AirPlay desde los controles del vídeo en Safari y elige tu televisor."}p id="handoff-status" role="status" {} script type="module" { (maud::PreEscaped("import init from './client/megalodon_ui.js';init();")) }}}
    };
    let mut response = axum::response::Html(document.into_string()).into_response();
    headers(&mut response);
    response.headers_mut().insert("content-security-policy","default-src 'none'; script-src 'self' 'wasm-unsafe-eval' 'unsafe-inline'; style-src 'unsafe-inline'; media-src 'self'; connect-src 'self'; base-uri 'none'; frame-ancestors 'none'".parse().unwrap());
    response
}
async fn asset(
    State(state): State<Arc<Shared>>,
    Path((ticket, asset)): Path<(String, String)>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
) -> Response {
    if !permitted(&state, &ticket, peer) || state.peer.is_some() {
        return StatusCode::NOT_FOUND.into_response();
    }
    let (kind, bytes): (&str, &[u8]) = match asset.as_str() {
        "megalodon_ui.js" => (
            "text/javascript",
            include_bytes!("../../../static/client/megalodon_ui.js"),
        ),
        "megalodon_ui_bg.wasm" => (
            "application/wasm",
            include_bytes!("../../../static/client/megalodon_ui_bg.wasm"),
        ),
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    let mut response = ([("content-type", kind)], bytes).into_response();
    headers(&mut response);
    response
}
#[derive(Deserialize)]
struct Progress {
    position: f64,
    paused: bool,
    ended: bool,
}
async fn progress(
    State(state): State<Arc<Shared>>,
    Path(ticket): Path<String>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    axum::Json(progress): axum::Json<Progress>,
) -> StatusCode {
    if !permitted(&state, &ticket, peer) || state.peer.is_some() {
        return StatusCode::NOT_FOUND;
    }
    if !progress.position.is_finite()
        || progress.position < 0.0
        || (state.duration > 0.0 && progress.position > state.duration + 10.0)
    {
        return StatusCode::BAD_REQUEST;
    }
    *state.progress.lock().unwrap() = Some((
        Status {
            position: progress.position,
            duration: state.duration,
            paused: progress.paused,
            ended: progress.ended,
        },
        Instant::now(),
    ));
    StatusCode::NO_CONTENT
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hls_urls_are_scoped_to_the_title_and_never_export_credentials() {
        let base = Url::parse("http://127.0.0.1:8096/jellyfin/").unwrap();
        let current = base.join("Videos/movie/master.m3u8").unwrap();
        let prefix = "/jellyfin/videos/movie/";
        let valid = upstream(
            &base,
            &current,
            prefix,
            "main.m3u8?api_key=secret&MediaSourceId=source",
        )
        .unwrap();
        assert!(!valid.as_str().contains("secret"));
        assert!(valid.as_str().contains("MediaSourceId=source"));
        assert!(upstream(&base, &current, prefix, "http://evil.test/video").is_err());
        assert!(upstream(&base, &current, prefix, "../../Users").is_err());
        assert!(upstream(&base, &current, prefix, "../other/stream").is_err());
    }
}
