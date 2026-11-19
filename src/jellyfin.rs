use reqwest::{Client, Url, header};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, RwLock};
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

#[derive(Clone)]
pub struct Config {
    pub base_url: String,
    pub token: String,
    pub user_id: Option<String>,
    pub port: u16,
}

impl Config {
    pub fn load() -> Result<Self, String> {
        let mut values = HashMap::new();
        if let Ok(entries) = dotenvy::from_path_iter("../megalodon-front/.env.local") {
            values.extend(entries.flatten());
        }
        // The launcher may have renewed the key after the original .env was written.
        // Read it in memory; never copy credentials into this project or the browser.
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let path = PathBuf::from(local).join("Megalodon/secrets.json");
            if let Ok(bytes) = std::fs::read(path)
                && let Ok(secrets) = serde_json::from_slice::<HashMap<String, String>>(&bytes)
                && let Some(key) = secrets.get("JELLYFIN_ADMIN_API_KEY")
            {
                values.insert("JELLYFIN_ADMIN_API_KEY".into(), key.clone());
            }
        }
        for path in [".env", ".env.local"] {
            if let Ok(entries) = dotenvy::from_path_iter(path) {
                values.extend(entries.flatten());
            }
        }
        values.extend(std::env::vars());
        let base_url = values
            .get("JELLYFIN_BASE_URL")
            .map(String::as_str)
            .unwrap_or("http://127.0.0.1:8096")
            .trim_end_matches('/')
            .to_owned();
        let parsed =
            Url::parse(&base_url).map_err(|_| "JELLYFIN_BASE_URL no es una URL válida.")?;
        if !matches!(parsed.scheme(), "http" | "https")
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
        {
            return Err(
                "Usa una dirección HTTP o HTTPS de Jellyfin sin credenciales ni parámetros.".into(),
            );
        }
        let token = values
            .get("JELLYFIN_API_KEY")
            .filter(|v| !v.trim().is_empty())
            .or_else(|| values.get("JELLYFIN_ADMIN_API_KEY"))
            .cloned()
            .unwrap_or_default();
        let user_id = values
            .get("JELLYFIN_USER_ID")
            .filter(|v| valid_id(v))
            .cloned();
        let port = values
            .get("MEGALODON_PORT")
            .map(|v| v.parse::<u16>())
            .transpose()
            .map_err(|_| "MEGALODON_PORT debe ser un puerto válido.")?
            .unwrap_or(3210);
        if port == 0 {
            return Err("El puerto debe ser mayor que cero.".into());
        }
        Ok(Self {
            base_url,
            token,
            user_id,
            port,
        })
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Item {
    pub id: String,
    pub name: String,
    #[serde(rename = "Type")]
    pub kind: String,
    pub overview: Option<String>,
    pub production_year: Option<u32>,
    pub run_time_ticks: Option<u64>,
    pub community_rating: Option<f64>,
    pub official_rating: Option<String>,
    pub genres: Vec<String>,
    pub image_tags: HashMap<String, String>,
    pub backdrop_image_tags: Vec<String>,
    pub date_created: Option<String>,
    pub premiere_date: Option<String>,
    pub index_number: Option<u32>,
    pub parent_index_number: Option<u32>,
    pub child_count: Option<u32>,
    pub recursive_item_count: Option<u32>,
    pub series_name: Option<String>,
    pub series_id: Option<String>,
    pub people: Vec<Person>,
    pub studios: Vec<Studio>,
    pub taglines: Vec<String>,
    pub user_data: Option<UserData>,
    pub provider_ids: HashMap<String, String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Person {
    pub id: String,
    pub name: String,
    pub role: Option<String>,
    #[serde(rename = "Type")]
    pub kind: String,
    pub primary_image_tag: Option<String>,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Studio {
    pub name: String,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct UserData {
    pub playback_position_ticks: u64,
    pub played: bool,
    pub is_favorite: bool,
}
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct User {
    pub id: String,
    pub name: String,
    pub primary_image_tag: Option<String>,
    pub has_password: bool,
    pub configuration: serde_json::Value,
    #[serde(skip)]
    pub avatar_id: Option<String>,
    pub policy: Policy,
}
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Policy {
    pub is_administrator: bool,
    pub is_disabled: bool,
}
#[derive(Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Page {
    pub items: Vec<Item>,
    pub total_record_count: usize,
}

impl Item {
    pub fn display_genres(&self) -> Vec<&str> {
        let mut genres = Vec::new();
        for genre in &self.genres {
            let label = genre_label(genre);
            if !genres.contains(&label) {
                genres.push(label);
            }
        }
        genres
    }
    pub fn runtime(&self) -> String {
        let minutes = self.run_time_ticks.unwrap_or(0) / 600_000_000;
        match minutes {
            0 => String::new(),
            1..60 => format!("{minutes} min"),
            _ => format!("{} h {:02} min", minutes / 60, minutes % 60),
        }
    }
    pub fn year(&self) -> String {
        self.production_year
            .map(|v| v.to_string())
            .unwrap_or_default()
    }
    pub fn label(&self) -> &str {
        if self.kind == "Series" {
            "Serie"
        } else if self.kind == "Episode" {
            "Episodio"
        } else {
            "Película"
        }
    }
    pub fn image(&self, kind: &str, width: u32) -> String {
        let tag = if kind == "Backdrop" {
            self.backdrop_image_tags.first()
        } else {
            self.image_tags.get(kind)
        };
        let mut url = format!("/image/{}/{kind}?width={width}", self.id);
        if let Some(tag) = tag {
            url.push_str(&format!("&tag={}", encode(tag)));
        }
        url
    }
    pub fn art_kind(&self) -> &str {
        if !self.backdrop_image_tags.is_empty() {
            "Backdrop"
        } else if self.image_tags.contains_key("Thumb") {
            "Thumb"
        } else {
            "Primary"
        }
    }
}

#[derive(Clone)]
pub struct Catalog {
    pub items: Vec<Item>,
    pub user: User,
    pub server_name: String,
    pub loaded: Instant,
}
impl Catalog {
    pub fn movies(&self) -> usize {
        self.items.iter().filter(|i| i.kind == "Movie").count()
    }
    pub fn series(&self) -> usize {
        self.items.iter().filter(|i| i.kind == "Series").count()
    }
    pub fn genres(&self) -> BTreeMap<String, usize> {
        let mut genres = BTreeMap::new();
        for item in &self.items {
            for genre in item.display_genres() {
                *genres.entry(genre.to_owned()).or_default() += 1;
            }
        }
        genres
    }
}

pub struct Library {
    pub config: Config,
    pub client: Client,
    cache: RwLock<Option<Arc<Catalog>>>,
    refresh: Mutex<()>,
}

pub const DEVICE_ID: &str = if cfg!(feature = "desktop") {
    "megalodon-rust-desktop"
} else {
    "megalodon-rust-local"
};
pub fn authorization(token: &str) -> String {
    format!(
        "MediaBrowser Client=\"Megalodon\", Device=\"Windows\", DeviceId=\"{DEVICE_ID}\", Version=\"{}\", Token=\"{token}\"",
        env!("CARGO_PKG_VERSION")
    )
}

impl Library {
    pub fn new(config: Config) -> Result<Self, String> {
        let mut headers = header::HeaderMap::new();
        let mut token = header::HeaderValue::from_str(&authorization(&config.token))
            .map_err(|_| "La clave de Jellyfin no es válida.")?;
        token.set_sensitive(true);
        headers.insert(header::AUTHORIZATION, token);
        let client = Client::builder()
            .default_headers(headers)
            .connect_timeout(Duration::from_secs(4))
            .timeout(Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| "No se pudo preparar la conexión.")?;
        Ok(Self {
            config,
            client,
            cache: RwLock::new(None),
            refresh: Mutex::new(()),
        })
    }

    pub async fn json<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, String)],
    ) -> Result<T, String> {
        if self.config.token.is_empty() {
            return Err("Añade JELLYFIN_API_KEY a .env.local o configura el launcher de Megalodon para conectar tu biblioteca.".into());
        }
        let response = self
            .client
            .get(format!("{}{path}", self.config.base_url))
            .query(params)
            .send()
            .await
            .map_err(|_| {
                "No podemos conectar con Jellyfin. Abre el servidor y vuelve a intentarlo."
                    .to_string()
            })?;
        match response.status().as_u16() {
            401 | 403 => {
                return Err(
                    "Jellyfin ha rechazado el acceso. Revisa la clave API configurada.".into(),
                );
            }
            404 => return Err("Este título ya no está disponible en la biblioteca.".into()),
            200..=299 => {}
            _ => {
                return Err(
                    "Jellyfin no ha podido completar la consulta. Vuelve a intentarlo.".into(),
                );
            }
        }
        response
            .json()
            .await
            .map_err(|_| "La respuesta de Jellyfin no tiene el formato esperado.".into())
    }

    pub async fn catalog(&self, force: bool) -> Result<(Arc<Catalog>, Option<String>), String> {
        if !force
            && let Some(data) = self
                .cache
                .read()
                .await
                .as_ref()
                .filter(|c| c.loaded.elapsed() < Duration::from_secs(300))
        {
            return Ok((data.clone(), None));
        }
        let _guard = self.refresh.lock().await;
        if !force
            && let Some(data) = self
                .cache
                .read()
                .await
                .as_ref()
                .filter(|c| c.loaded.elapsed() < Duration::from_secs(300))
        {
            return Ok((data.clone(), None));
        }
        match self.load_catalog().await {
            Ok(data) => {
                let data = Arc::new(data);
                *self.cache.write().await = Some(data.clone());
                Ok((data, None))
            }
            Err(error) => match self.cache.read().await.as_ref() {
                Some(data) => Ok((
                    data.clone(),
                    Some(format!("{error} Mostramos el último catálogo cargado.")),
                )),
                None => Err(error),
            },
        }
    }

    async fn load_catalog(&self) -> Result<Catalog, String> {
        let users: Vec<User> = if let Some(id) = &self.config.user_id {
            vec![self.json(&format!("/Users/{id}"), &[]).await?]
        } else {
            self.json("/Users", &[]).await?
        };
        let mut user = if let Some(id) = &self.config.user_id {
            users.iter().find(|u| &u.id == id && !u.policy.is_disabled)
        } else {
            users
                .iter()
                .find(|u| u.policy.is_administrator && !u.policy.is_disabled)
                .or_else(|| users.iter().find(|u| !u.policy.is_disabled))
        }
        .cloned()
        .ok_or("No se encontró un perfil activo en Jellyfin.")?;
        user.avatar_id = crate::avatars::stored(&user.id).or_else(|| {
            user.configuration["CustomPrefs"]["megalodon.avatarId"]
                .as_str()
                .and_then(crate::avatars::normalize)
        });
        let items = self
            .all_items(&user.id, "Movie,Series", None, "SortName")
            .await?;
        let info: serde_json::Value = self
            .json("/System/Info/Public", &[])
            .await
            .unwrap_or_default();
        let server_name = info["ServerName"].as_str().unwrap_or("Jellyfin").to_owned();
        Ok(Catalog {
            items,
            user,
            server_name,
            loaded: Instant::now(),
        })
    }

    // Always follow TotalRecordCount. No silent 100/500/10,000-title cutoff.
    pub async fn all_items(
        &self,
        user: &str,
        kinds: &str,
        parent: Option<&str>,
        sort: &str,
    ) -> Result<Vec<Item>, String> {
        let mut items = Vec::new();
        let mut seen = HashSet::new();
        let mut offset = 0;
        loop {
            let mut params = vec![
                ("UserId", user.to_owned()), ("Recursive", "true".into()), ("IncludeItemTypes", kinds.into()),
                ("StartIndex", offset.to_string()), ("Limit", "500".into()), ("SortBy", sort.into()), ("SortOrder", "Ascending".into()),
                ("Fields", "Overview,ProviderIds,Genres,DateCreated,Taglines,ChildCount,RecursiveItemCount,SeriesId,SeriesName,ParentIndexNumber,IndexNumber,UserData".into()),
                ("EnableUserData", "true".into()), ("ImageTypeLimit", "1".into()),
            ];
            if let Some(parent) = parent {
                params.push(("ParentId", parent.into()));
            }
            let page: Page = self.json("/Items", &params).await?;
            let count = page.items.len();
            if count == 0 && offset < page.total_record_count {
                return Err(
                    "El catálogo cambió mientras se cargaba. Actualiza para volver a intentarlo."
                        .into(),
                );
            }
            let before = items.len();
            for item in page.items {
                if seen.insert(item.id.clone()) {
                    items.push(item);
                }
            }
            offset += count;
            if count == 0 || offset >= page.total_record_count {
                break;
            }
            if items.len() == before {
                return Err(
                    "Jellyfin ha repetido una página del catálogo. Vuelve a intentarlo.".into(),
                );
            }
        }
        Ok(items)
    }

    pub async fn detail(&self, user: &str, id: &str) -> Result<Item, String> {
        self.json(
            &format!("/Users/{user}/Items/{id}"),
            &[(
                "Fields",
                "Overview,People,Studios,Genres,Taglines,ChildCount,RecursiveItemCount,UserData"
                    .into(),
            )],
        )
        .await
    }
}

pub fn genre_label(genre: &str) -> &str {
    match genre {
        "Action" => "Acción",
        "Adventure" => "Aventura",
        "Animation" => "Animación",
        "Action & Adventure" => "Acción y aventura",
        "Sci-Fi & Fantasy" => "Ciencia ficción y fantasía",
        "Science Fiction" => "Ciencia ficción",
        "Fantasy" => "Fantasía",
        "Comedy" => "Comedia",
        "Family" => "Familia",
        "Music" => "Música",
        "Documentary" => "Documental",
        "Horror" => "Terror",
        "Thriller" => "Suspense",
        "Mystery" => "Misterio",
        _ => genre,
    }
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
pub fn encode(value: &str) -> String {
    url_encode(value)
}
fn url_encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
pub fn search_key(value: &str) -> String {
    value
        .nfd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn search_handles_spanish_and_ids_cannot_escape_paths() {
        assert_eq!(
            search_key("Cómo entrenar a tu dragón"),
            "como entrenar a tu dragon"
        );
        assert!(!valid_id("../Users"));
        assert!(!valid_id("id?api_key=secret"));
        assert!(valid_id("abc-123"));
        assert_eq!(
            encode("Ciencia ficción & drama"),
            "Ciencia%20ficci%C3%B3n%20%26%20drama"
        );
    }
    #[test]
    fn sparse_metadata_is_supported() {
        let item: Item =
            serde_json::from_str(r#"{"Id":"a","Name":"Película","Type":"Movie","Overview":null}"#)
                .unwrap();
        assert_eq!(item.runtime(), "");
        assert_eq!(item.art_kind(), "Primary");
    }

    #[tokio::test]
    async fn complete_pagination_cache_and_server_failures() {
        use axum::{
            Json, Router,
            extract::{Query, State},
            response::IntoResponse,
            routing::get,
        };
        use std::sync::{
            Mutex as StdMutex,
            atomic::{AtomicU8, Ordering},
        };
        #[derive(Default)]
        struct Mock {
            mode: AtomicU8,
            offsets: StdMutex<Vec<usize>>,
        }
        async fn items(
            State(mock): State<Arc<Mock>>,
            Query(query): Query<HashMap<String, String>>,
        ) -> axum::response::Response {
            let offset: usize = query["StartIndex"].parse().unwrap();
            assert_eq!(query["Limit"], "500");
            mock.offsets.lock().unwrap().push(offset);
            let mode = mock.mode.load(Ordering::Relaxed);
            if mode == 1 {
                return axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response();
            }
            let start = if mode == 2 { 0 } else { offset };
            let end = if mode == 3 && start > 0 {
                start
            } else {
                (start + 500).min(1203)
            };
            let rows: Vec<_> = (start..end).map(|i| serde_json::json!({"Id":format!("id{i}"),"Name":format!("Película {i}"),"Type":"Movie"})).collect();
            Json(serde_json::json!({"Items":rows,"TotalRecordCount":1203})).into_response()
        }
        let mock = Arc::new(Mock::default());
        let router = Router::new()
            .route("/Items", get(items))
            .route("/Users", get(|| async { Json(serde_json::json!([{"Id":"u1","Name":"Test","Policy":{"IsAdministrator":true}}])) }))
            .route("/System/Info/Public", get(|| async { Json(serde_json::json!({"ServerName":"Test server"})) }))
            .with_state(mock.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let library = Library::new(Config {
            base_url: format!("http://{address}"),
            token: "test-token".into(),
            user_id: None,
            port: 3210,
        })
        .unwrap();
        let (catalog, warning) = library.catalog(false).await.unwrap();
        assert!(warning.is_none());
        assert_eq!(catalog.items.len(), 1203);
        assert_eq!(catalog.items.last().unwrap().id, "id1202");
        assert_eq!(*mock.offsets.lock().unwrap(), vec![0, 500, 1000]);
        assert!(Arc::ptr_eq(
            &catalog,
            &library.catalog(false).await.unwrap().0
        ));
        assert_eq!(mock.offsets.lock().unwrap().len(), 3);
        mock.mode.store(1, Ordering::Relaxed);
        let (cached, warning) = library.catalog(true).await.unwrap();
        assert!(warning.unwrap().contains("último catálogo"));
        assert!(Arc::ptr_eq(&catalog, &cached));
        mock.mode.store(2, Ordering::Relaxed);
        assert!(
            library
                .all_items("u1", "Movie", None, "SortName")
                .await
                .unwrap_err()
                .contains("repetido")
        );
        mock.mode.store(3, Ordering::Relaxed);
        assert!(
            library
                .all_items("u1", "Movie", None, "SortName")
                .await
                .unwrap_err()
                .contains("cambió")
        );
        server.abort();
    }
}
