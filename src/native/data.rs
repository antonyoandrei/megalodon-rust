use crate::jellyfin::{Config, Item, Library, valid_id};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{path::PathBuf, time::Duration};

pub fn data_dir() -> PathBuf {
    std::env::var_os("MEGALODON_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default())
                .join("MegalodonRust")
        })
}

#[derive(Serialize, Deserialize)]
struct Connection {
    server: String,
    user: Option<String>,
    protected_token: Vec<u8>,
}

pub(super) fn protect(bytes: &[u8], decrypt: bool) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::{Foundation::LocalFree, Security::Cryptography::*};
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    // DPAPI binds the credential to this Windows account. Output belongs to LocalAlloc.
    let ok = unsafe {
        if decrypt {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
    };
    if ok == 0 {
        return Err("Windows no ha podido proteger las credenciales de esta cuenta.".into());
    }
    let result =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe {
        LocalFree(output.pbData.cast());
    }
    Ok(result)
}

pub fn load_connection() -> Option<Config> {
    let saved: Connection =
        serde_json::from_slice(&std::fs::read(data_dir().join("connection.json")).ok()?).ok()?;
    Some(Config {
        base_url: saved.server,
        user_id: saved.user,
        token: String::from_utf8(protect(&saved.protected_token, true).ok()?).ok()?,
        port: 3210,
    })
}

pub fn save_connection(config: &Config) -> Result<(), String> {
    let saved = Connection {
        server: config.base_url.clone(),
        user: config.user_id.clone(),
        protected_token: protect(config.token.as_bytes(), false)?,
    };
    save_json("connection.json", &saved)
}

pub fn save_json(name: &str, value: &impl Serialize) -> Result<(), String> {
    let dir = data_dir();
    std::fs::create_dir_all(&dir).map_err(|_| "No se puede crear la carpeta de configuración.")?;
    let bytes =
        serde_json::to_vec_pretty(value).map_err(|_| "No se pudo guardar la configuración.")?;
    let path = dir.join(name);
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, bytes).map_err(|_| "No se pudo escribir la configuración.")?;
    std::fs::rename(temporary, path).map_err(|_| "No se pudo reemplazar la configuración.".into())
}

pub fn server_url(input: &str) -> Result<String, String> {
    let url = reqwest::Url::parse(input.trim())
        .map_err(|_| "Escribe la dirección completa: https://servidor o http://servidor:8096.")?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("La dirección debe ser HTTP o HTTPS, sin credenciales ni parámetros.".into());
    }
    Ok(url.as_str().trim_end_matches('/').to_owned())
}

pub async fn authenticate(
    server: String,
    username: String,
    password: String,
) -> Result<Config, String> {
    let base = server_url(&server)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "No se pudo crear la conexión.")?;
    let response = client.post(format!("{base}/Users/AuthenticateByName"))
        .header("Authorization", concat!("MediaBrowser Client=\"Megalodon\", Device=\"Windows\", DeviceId=\"megalodon-rust-desktop\", Version=\"", env!("CARGO_PKG_VERSION"), "\""))
        .json(&json!({"Username":username,"Pw":password})).send().await.map_err(|_| "No podemos conectar con ese servidor.")?;
    if !response.status().is_success() {
        return Err(
            "No se ha podido iniciar sesión. Comprueba el servidor, el usuario y la contraseña."
                .into(),
        );
    }
    let body: Value = response
        .json()
        .await
        .map_err(|_| "Respuesta de inicio de sesión no válida.")?;
    let token = body["AccessToken"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("El servidor no devolvió una sesión.")?;
    let user = body["User"]["Id"]
        .as_str()
        .filter(|s| valid_id(s))
        .ok_or("El servidor no devolvió un perfil válido.")?;
    let config = Config {
        base_url: base,
        token: token.to_owned(),
        user_id: Some(user.to_owned()),
        port: 3210,
    };
    Ok(config)
}

#[derive(Clone)]
pub struct Playback {
    pub item: Item,
    pub url: String,
    pub headers: String,
    pub source_id: String,
    pub session_id: String,
    pub start: f64,
    pub subtitles: Vec<(String, String, String)>,
}

pub async fn prepare(
    library: &Library,
    user: &str,
    id: &str,
    start: f64,
) -> Result<Playback, String> {
    if !valid_id(id) {
        return Err("El título no es válido.".into());
    }
    let item = library.detail(user, id).await?;
    if !matches!(item.kind.as_str(), "Movie" | "Episode") {
        return Err("Selecciona una película o un episodio.".into());
    }
    let response = library.client.post(format!("{}/Items/{id}/PlaybackInfo", library.config.base_url))
        .json(&json!({"UserId":user,"StartTimeTicks":(start*10_000_000.0) as u64,"IsPlayback":true,"EnableDirectPlay":true,"EnableDirectStream":true,"EnableTranscoding":false,"AutoOpenLiveStream":true}))
        .send().await.map_err(|_| "Jellyfin no ha respondido al preparar la reproducción.")?;
    if !response.status().is_success() {
        return Err(
            "Jellyfin no permite reproducir este archivo o el archivo no está disponible.".into(),
        );
    }
    let info: Value = response
        .json()
        .await
        .map_err(|_| "Información de reproducción no válida.")?;
    let source = info["MediaSources"]
        .as_array()
        .and_then(|s| s.first())
        .ok_or("No hay un archivo disponible para este título.")?;
    let source_id = source["Id"]
        .as_str()
        .ok_or("La fuente de vídeo no tiene identificador.")?
        .to_owned();
    let mut url = reqwest::Url::parse(&format!("{}/Videos/{id}/stream", library.config.base_url))
        .map_err(|_| "Dirección de vídeo no válida.")?;
    url.query_pairs_mut()
        .append_pair("Static", "true")
        .append_pair("MediaSourceId", &source_id);
    let subtitles = source["MediaStreams"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|s| s["Type"] == "Subtitle" && s["IsExternal"] == true)
        .filter_map(|s| {
            let index = s["Index"].as_u64()?;
            let format = match s["Codec"].as_str().unwrap_or("srt") {
                "ass" | "ssa" => "ass",
                "webvtt" => "vtt",
                _ => "srt",
            };
            Some((
                format!(
                    "{}/Videos/{id}/{}/Subtitles/{index}/Stream.{format}",
                    library.config.base_url,
                    crate::jellyfin::encode(&source_id)
                ),
                s["DisplayTitle"]
                    .as_str()
                    .unwrap_or("Subtítulos")
                    .to_owned(),
                s["Language"].as_str().unwrap_or("").to_owned(),
            ))
        })
        .collect();
    Ok(Playback {
        item,
        url: url.into(),
        // A single header without commas avoids mpv's comma-separated option parser.
        headers: format!(
            "Authorization: MediaBrowser Token=\"{}\"",
            library.config.token
        ),
        source_id,
        session_id: info["PlaySessionId"].as_str().unwrap_or("").to_owned(),
        start,
        subtitles,
    })
}

#[derive(Default, Serialize, Deserialize)]
pub struct Preferences {
    #[serde(default)]
    pub native_profiles: bool,
    #[serde(default)]
    pub imported_history_sources: std::collections::HashSet<String>,
    #[serde(default)]
    pub profile_settings: std::collections::HashMap<String, ProfileSettings>,
    #[serde(default)]
    pub profile_defaults: Option<ProfileSettings>,
    #[serde(default = "default_auto_next")]
    pub automatic_updates: bool,
    #[serde(default)]
    pub update_repository: String,
    #[serde(default)]
    pub history: std::collections::HashMap<String, super::history::Watch>,
    #[serde(default)]
    pub favorites: std::collections::HashMap<String, bool>,
    #[serde(default = "default_local_profile")]
    pub local_profile: String,
    #[serde(default = "default_local_profiles")]
    pub local_profiles: std::collections::BTreeMap<String, String>,
    #[serde(default = "default_auto_next")]
    pub auto_next: bool,
    #[serde(default)]
    pub reduced_motion: bool,
    #[serde(default = "default_volume")]
    pub volume: f64,
    #[serde(default)]
    pub positions: std::collections::HashMap<String, f64>,
    #[serde(default)]
    pub avatars: std::collections::HashMap<String, String>,
    /// Appearance theme id (controls::THEMES); empty means the default.
    #[serde(default)]
    pub theme: String,
    /// The first-run welcome has been completed or skipped.
    #[serde(default)]
    pub onboarded: bool,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct ProfileSettings {
    pub auto_next: bool,
    pub reduced_motion: bool,
    pub volume: f64,
}
impl Preferences {
    pub fn current_settings(&self) -> ProfileSettings {
        ProfileSettings {
            auto_next: self.auto_next,
            reduced_motion: self.reduced_motion,
            volume: self.volume,
        }
    }
    pub fn activate_settings(&mut self, key: &str) {
        // Snapshot the pre-profile defaults once, so a newly created profile cannot
        // inherit the last person's volume or accessibility choices by accident.
        let legacy = self.current_settings();
        let defaults = self.profile_defaults.get_or_insert(legacy);
        let settings = self
            .profile_settings
            .entry(key.into())
            .or_insert_with(|| defaults.clone());
        self.auto_next = settings.auto_next;
        self.reduced_motion = settings.reduced_motion;
        self.volume = settings.volume.clamp(0.0, 100.0);
    }
    pub fn remember_settings(&mut self, key: &str) {
        self.profile_settings
            .insert(key.into(), self.current_settings());
    }
}
fn default_local_profile() -> String {
    "local".into()
}
fn default_local_profiles() -> std::collections::BTreeMap<String, String> {
    [("local".into(), "Mi perfil".into())].into()
}
fn default_auto_next() -> bool {
    true
}
fn default_volume() -> f64 {
    75.0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profile_settings_migrate_once_and_do_not_follow_the_last_person() {
        let mut prefs: Preferences = serde_json::from_value(json!({
            "volume": 63.0, "reduced_motion": true, "auto_next": false,
            "avatars": {"folder-a|local": "pokemon-gengar"}
        }))
        .unwrap();
        prefs.activate_settings("local|local");
        prefs.volume = 18.0;
        prefs.reduced_motion = false;
        prefs.remember_settings("local|local");
        prefs.activate_settings("local|guest");
        assert_eq!(prefs.volume, 63.0);
        assert!(prefs.reduced_motion);
        assert!(!prefs.auto_next);
        prefs.volume = 90.0;
        prefs.remember_settings("local|guest");
        let mut restored: Preferences =
            serde_json::from_slice(&serde_json::to_vec(&prefs).unwrap()).unwrap();
        restored.activate_settings("local|local");
        assert_eq!(restored.volume, 18.0);
        assert!(!restored.reduced_motion);
        assert_eq!(restored.avatars["folder-a|local"], "pokemon-gengar");
        restored.activate_settings("https://server|local");
        assert_eq!(restored.volume, 63.0); // A server account is a separate identity.
    }
    #[test]
    fn avatar_preferences_preserve_existing_playback_settings() {
        let mut preferences: Preferences = serde_json::from_value(json!({
            "volume": 63.0, "reduced_motion": true, "positions": {"episode": 428.0}
        }))
        .unwrap();
        assert!(preferences.auto_next);
        assert!(preferences.avatars.is_empty());
        preferences
            .avatars
            .insert("server-a|user".into(), "pokemon-gengar".into());
        preferences
            .avatars
            .insert("server-b|user".into(), "pokemon-pikachu".into());
        let restored: Preferences =
            serde_json::from_slice(&serde_json::to_vec(&preferences).unwrap()).unwrap();
        assert_eq!(restored.positions["episode"], 428.0);
        assert_eq!(restored.volume, 63.0);
        assert!(restored.reduced_motion);
        assert_ne!(
            restored.avatars["server-a|user"],
            restored.avatars["server-b|user"]
        );
    }
    #[tokio::test]
    async fn playback_uses_local_position_and_keeps_source_credentials_private() {
        use axum::{
            Json, Router,
            extract::State,
            http::{HeaderMap, StatusCode},
            routing::{get, post},
        };
        use std::sync::{Arc, Mutex};
        let reports = Arc::new(Mutex::new(Vec::<Value>::new()));
        async fn record(
            State(reports): State<Arc<Mutex<Vec<Value>>>>,
            headers: HeaderMap,
            Json(body): Json<Value>,
        ) -> StatusCode {
            assert!(
                headers["authorization"]
                    .to_str()
                    .unwrap()
                    .contains("test-session")
            );
            reports.lock().unwrap().push(body);
            StatusCode::NO_CONTENT
        }
        let app=Router::new()
            .route("/Users/u1/Items/movie1",get(||async{Json(json!({"Id":"movie1","Type":"Movie","Name":"Test","UserData":{"PlaybackPositionTicks":420000000}}))}))
            .route("/Items/movie1/PlaybackInfo",post(|Json(body):Json<Value>|async move {
                assert_eq!(body["EnableTranscoding"],false);
                assert_eq!(body["UserId"],"u1");
                Json(json!({"PlaySessionId":"session1","MediaSources":[{"Id":"source1","MediaStreams":[{"Type":"Subtitle","Index":2,"IsExternal":true,"Codec":"srt","Language":"spa","DisplayTitle":"Español"}]}]}))
            }))
            .route("/Sessions/Playing/Progress",post(record))
            .with_state(reports.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let library = Library::new(Config {
            base_url: format!("http://{address}"),
            token: "test-session".into(),
            user_id: Some("u1".into()),
            port: 3210,
        })
        .unwrap();
        let resumed = prepare(&library, "u1", "movie1", 84.5).await.unwrap();
        assert_eq!(resumed.start, 84.5);
        assert_eq!(resumed.subtitles.len(), 1);
        assert!(resumed.headers.contains("test-session"));
        assert!(!resumed.url.contains("test-session"));
        assert!(!resumed.subtitles[0].0.contains("test-session"));
        assert_eq!(
            prepare(&library, "u1", "movie1", 0.0).await.unwrap().start,
            0.0
        );
        assert!(
            reports.lock().unwrap().is_empty(),
            "Local progress must not alter the source account"
        );
        task.abort();
    }
    #[test]
    fn connection_validation_and_windows_credential_roundtrip() {
        assert_eq!(
            server_url(" https://example.org/jellyfin/ ").unwrap(),
            "https://example.org/jellyfin"
        );
        for invalid in [
            "file:///a",
            "http://user:secret@example.org",
            "http://example.org?key=secret",
        ] {
            assert!(server_url(invalid).is_err());
        }
        let protected = protect(b"test-session-not-real", false).unwrap();
        assert_ne!(protected, b"test-session-not-real");
        assert_eq!(protect(&protected, true).unwrap(), b"test-session-not-real");
    }
}
