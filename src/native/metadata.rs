//! Optional, read-only enrichment. Source IDs are authoritative; ambiguous names stay untouched.
use super::{data, jellyfin::Item, sources};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    cell::Cell,
    collections::HashMap,
    io::Read,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

/// Project credentials compiled in by whoever builds Megalodon (build.rs reads
/// MEGALODON_TMDB_TOKEN / MEGALODON_FANART_KEY or .local/providers.env). They
/// never live in the repository; people using the app need no key of their own.
const BUILT_IN_TMDB: Option<&str> = option_env!("MEGALODON_TMDB_TOKEN");
const BUILT_IN_FANART: Option<&str> = option_env!("MEGALODON_FANART_KEY");
fn built_in_key(provider: Provider) -> Option<&'static str> {
    match provider {
        Provider::Tmdb => BUILT_IN_TMDB,
        Provider::Fanart => BUILT_IN_FANART,
        _ => None,
    }
    .map(str::trim)
    .filter(|k| !k.is_empty())
}

/// Artwork and fact sources. The user orders them; the first one in the list
/// with a suitable image wins, so the list is the image policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Provider {
    Cinemeta,
    Tmdb,
    Fanart,
    Tvmaze,
    Anilist,
    Kitsu,
}
impl Provider {
    /// Default order: free sources first, optional keyed ones last.
    pub const ALL: [Provider; 6] = [
        Provider::Cinemeta,
        Provider::Tvmaze,
        Provider::Anilist,
        Provider::Kitsu,
        Provider::Tmdb,
        Provider::Fanart,
    ];
    /// With a project TMDB key, Spanish titles, synopses and logos lead;
    /// otherwise the free sources do and keyed ones wait at the end.
    pub fn default_order() -> Vec<Provider> {
        let mut order = Provider::ALL.to_vec();
        if Provider::Tmdb.built_in() {
            order.retain(|p| *p != Provider::Tmdb);
            order.insert(0, Provider::Tmdb);
        }
        order
    }
    pub fn name(self) -> &'static str {
        match self {
            Provider::Cinemeta => "Cinemeta",
            Provider::Tmdb => "TMDB",
            Provider::Fanart => "fanart.tv",
            Provider::Tvmaze => "TVmaze",
            Provider::Anilist => "AniList",
            Provider::Kitsu => "Kitsu",
        }
    }
    pub fn summary(self) -> &'static str {
        match self {
            Provider::Cinemeta => "Películas y series · fondos hasta 4K, carteles y logotipos",
            Provider::Tmdb => "Películas y series · fichas, fondos sin texto y logotipos",
            Provider::Fanart => "Fondos sin texto y logotipos de alta calidad",
            Provider::Tvmaze => "Series · carteles y fondos",
            Provider::Anilist => "Anime · banners panorámicos para el destacado",
            Provider::Kitsu => "Anime · banners amplios",
        }
    }
    pub fn needs_key(self) -> bool {
        matches!(self, Provider::Tmdb | Provider::Fanart)
    }
    /// A project key ships with this build; a personal one is optional.
    pub fn built_in(self) -> bool {
        built_in_key(self).is_some()
    }
    pub fn key_url(self) -> Option<&'static str> {
        match self {
            Provider::Tmdb => Some("https://www.themoviedb.org/settings/api"),
            Provider::Fanart => Some("https://fanart.tv/get-an-api-key/"),
            _ => None,
        }
    }
    pub fn credits_url(self) -> &'static str {
        match self {
            Provider::Cinemeta => "https://www.stremio.com",
            Provider::Tmdb => "https://www.themoviedb.org",
            Provider::Fanart => "https://fanart.tv",
            Provider::Tvmaze => "https://www.tvmaze.com/api",
            Provider::Anilist => "https://anilist.co",
            Provider::Kitsu => "https://kitsu.app",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderEntry {
    pub provider: Provider,
    pub enabled: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub tmdb: Vec<u8>,
    pub fanart: Vec<u8>,
    pub replace_images: bool,
    /// Legacy switch from 0.3.2; seeds the TVmaze entry once.
    pub tvmaze: bool,
    pub providers: Vec<ProviderEntry>,
    /// Image folder inside the library; empty finds posters/images/artwork… automatically.
    pub art_folder: String,
    /// Language for titles, synopses and artwork with text. English by default
    /// (the person's decision); kept as data so it can be changed without a migration.
    pub language: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            tmdb: vec![],
            fanart: vec![],
            replace_images: false,
            tvmaze: true,
            providers: vec![],
            art_folder: String::new(),
            language: "en".into(),
        }
    }
}
impl Settings {
    pub fn load() -> Self {
        std::fs::read(data::data_dir().join("metadata-settings.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<Settings>(&b).ok())
            .unwrap_or_default()
            .normalized()
    }
    /// Every provider appears exactly once, in the stored order, new ones last.
    pub fn normalized(mut self) -> Self {
        let mut seen = vec![];
        self.providers.retain(|entry| {
            let fresh = !seen.contains(&entry.provider);
            seen.push(entry.provider);
            fresh
        });
        for provider in Provider::default_order() {
            if !seen.contains(&provider) {
                self.providers.push(ProviderEntry {
                    provider,
                    enabled: match provider {
                        Provider::Tmdb => !self.tmdb.is_empty() || provider.built_in(),
                        Provider::Fanart => !self.fanart.is_empty() || provider.built_in(),
                        Provider::Tvmaze => self.tvmaze,
                        Provider::Cinemeta | Provider::Anilist | Provider::Kitsu => true,
                    },
                });
            }
        }
        self.tvmaze = self.enabled(Provider::Tvmaze);
        if !matches!(self.language.as_str(), "en" | "es") {
            self.language = "en".into();
        }
        self
    }
    fn tmdb_language(&self) -> &'static str {
        if self.language == "es" {
            "es-ES"
        } else {
            "en-US"
        }
    }
    pub fn has_key(&self, provider: Provider) -> bool {
        match provider {
            Provider::Tmdb => !self.tmdb.is_empty() || provider.built_in(),
            Provider::Fanart => !self.fanart.is_empty() || provider.built_in(),
            _ => true,
        }
    }
    pub fn enabled(&self, provider: Provider) -> bool {
        self.providers
            .iter()
            .any(|e| e.provider == provider && e.enabled)
    }
    /// Enabled and usable now (keyed providers need their authorization).
    pub fn active(&self, provider: Provider) -> bool {
        self.enabled(provider) && self.has_key(provider)
    }
    /// Higher is preferred: the first entry in the list ranks highest.
    fn rank(&self, provider: Provider) -> u8 {
        let index = self
            .providers
            .iter()
            .position(|e| e.provider == provider)
            .unwrap_or(Provider::ALL.len());
        (Provider::ALL.len() - index.min(Provider::ALL.len())) as u8
    }
    pub fn configured(&self) -> bool {
        Provider::ALL.into_iter().any(|p| self.active(p))
    }
    pub fn save(&self) -> Result<(), String> {
        data::save_json("metadata-settings.json", self)
    }
}
pub struct State {
    pub settings: Settings,
    pub tmdb_input: String,
    pub fanart_input: String,
    pub busy: bool,
    /// Settings changed since the last enrichment run.
    pub dirty: bool,
    pub status: String,
    pub patches: HashMap<String, Patch>,
    /// Saved artwork overrides and the picker currently open (item id, tab).
    pub choices: Choices,
    pub picker: Option<String>,
    pub picker_kind: usize,
    /// Images saved from the app this session: (item, kind) → local copy shown
    /// until the library is listed again.
    pub uploads: HashMap<(String, String), std::path::PathBuf>,
    pub uploading: Option<(String, String)>,
    /// Video frames for Seguir viendo: `item#slot` → true while extracting, false if it failed.
    pub frames: HashMap<String, bool>,
    /// Set while drawing the Seguir viendo row, whose cards reveal a frame on hover.
    pub preview_frames: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            settings: Settings::load(),
            tmdb_input: String::new(),
            fanart_input: String::new(),
            busy: false,
            dirty: false,
            status: String::new(),
            patches: HashMap::new(),
            choices: load_choices(),
            picker: None,
            picker_kind: 0,
            uploads: HashMap::new(),
            uploading: None,
            frames: HashMap::new(),
            preview_frames: false,
        }
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Patch {
    pub id: String,
    pub name: Option<String>,
    pub overview: Option<String>,
    pub year: Option<u32>,
    pub runtime: Option<u64>,
    pub genres: Vec<String>,
    pub tmdb: Option<u64>,
    pub tvmaze: Option<u64>,
    /// Images in use: the automatic choice with the person's overrides applied.
    pub images: HashMap<String, String>,
    /// The automatic choice, kept so «Automático» can be restored.
    #[serde(default)]
    pub auto_images: HashMap<String, String>,
    /// Ranked alternatives per kind for the picker; the first is the recommendation.
    #[serde(default)]
    pub alternatives: HashMap<String, Vec<Alternative>>,
    /// Series only: TMDB still per episode, keyed `S{season}E{episode}`.
    #[serde(default)]
    pub episode_stills: HashMap<String, String>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Alternative {
    pub url: String,
    pub source: String,
    pub lang: String,
    pub width: u64,
    pub height: u64,
}
/// The person's artwork overrides: item id → kind → URL, or "local" for the
/// library's own image. Absent means automatic.
pub type Choices = HashMap<String, HashMap<String, String>>;
pub const LOCAL_CHOICE: &str = "local";
pub fn load_choices() -> Choices {
    std::fs::read(data::data_dir().join("artwork-choices.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}
pub fn save_choices(choices: &Choices) -> Result<(), String> {
    data::save_json("artwork-choices.json", choices)
}
impl Patch {
    /// Applies saved overrides on top of the automatic choice. Only URLs that
    /// were offered as alternatives (or the library's own image) are honoured.
    pub fn with_choices(mut self, choices: &Choices) -> Self {
        self.images = self.auto_images.clone();
        for (kind, choice) in choices.get(&self.id).into_iter().flatten() {
            if choice == LOCAL_CHOICE {
                self.images.remove(kind);
            } else if image_url_safe(choice) {
                // A saved choice survives new searches and re-rankings: it stays
                // in use and in the picker even if a provider no longer lists it.
                self.images.insert(kind.clone(), choice.clone());
                let list = self.alternatives.entry(kind.clone()).or_default();
                if !list.iter().any(|a| &a.url == choice) {
                    list.insert(
                        0,
                        Alternative {
                            url: choice.clone(),
                            source: source_name(choice).into(),
                            ..Default::default()
                        },
                    );
                }
            }
        }
        self
    }
    pub fn apply(&self, item: &mut Item) {
        if let Some(name) = &self.name {
            item.name = name.clone();
        }
        // Provider synopses only fill a gap; the library's own text is kept.
        if let Some(plot) = &self.overview
            && item.overview.as_deref().is_none_or(|o| o.trim().is_empty())
        {
            item.overview = Some(plot.clone());
        }
        item.production_year = self.year.or(item.production_year);
        item.run_time_ticks = item.run_time_ticks.or(self.runtime);
        if item.genres.is_empty() {
            item.genres = self.genres.clone();
        }
        if let Some(id) = self.tmdb {
            item.provider_ids.insert("Tmdb".into(), id.to_string());
        }
        if let Some(id) = self.tvmaze {
            item.provider_ids.insert("Tvmaze".into(), id.to_string());
        }
        for kind in self.images.keys() {
            if kind == "Backdrop" {
                if item.backdrop_image_tags.is_empty() {
                    item.backdrop_image_tags.push("metadata".into());
                }
            } else {
                item.image_tags
                    .entry(kind.clone())
                    .or_insert("metadata".into());
            }
        }
    }
}

#[derive(Clone)]
struct Candidate {
    url: String,
    lang: String,
    width: u64,
    height: u64,
    votes: u64,
    rating: f64,
    /// Position of the source in the user's list (higher wins).
    rank: u8,
}
fn image_url_safe(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|u| {
        u.scheme() == "https"
            && u.username().is_empty()
            && u.password().is_none()
            && u.query().is_none()
            && matches!(
                u.host_str(),
                Some(
                    "image.tmdb.org"
                        | "assets.fanart.tv"
                        | "static.tvmaze.com"
                        | "s4.anilist.co"
                        | "media.kitsu.app"
                        | "media.kitsu.io"
                        | "images.metahub.space"
                        | "live.metahub.space"
                )
            )
    })
}
/// Ranking: proper shape, then the user's provider order, then language,
/// resolution and community confidence.
fn quality(image: &Candidate, kind: &str, preferred: &str) -> Option<(u8, u8, u8, u64, u64)> {
    if !image_url_safe(&image.url) || image.height == 0 {
        return None;
    }
    let ratio = image.width as f64 / image.height as f64;
    // Panoramic banners (anime sources) can lead the hero, but a true 16:9
    // backdrop from any enabled provider is always preferred to them.
    let banner = kind == "Backdrop"
        && image.width >= 1600
        && image.height >= 340
        && (2.0..=5.0).contains(&ratio);
    let suitable = banner
        || match kind {
            "Primary" => image.width >= 500 && image.height >= 700 && (0.55..=0.8).contains(&ratio),
            "Backdrop" => {
                image.width >= 1280 && image.height >= 720 && (1.65..=1.95).contains(&ratio)
            }
            "Logo" => {
                image.width >= 400
                    && image.height >= 80
                    && (1.2..=8.0).contains(&ratio)
                    && (image.url.to_lowercase().ends_with(".png")
                        || image.url.contains("metahub.space/logo/"))
            }
            _ => false,
        };
    if !suitable {
        return None;
    }
    let neutral = matches!(image.lang.as_str(), "" | "00" | "null");
    let language = if kind == "Backdrop" && neutral {
        4
    } else if image.lang == preferred {
        3
    } else if neutral {
        2
    } else if matches!(image.lang.as_str(), "en" | "es") {
        1
    } else {
        return None;
    };
    // Resolution is capped at useful screen size; one unreviewed giant never beats a useful rated image solely by pixels.
    // Finer steps for panoramic art so the sharpest (up to 4K) wins among
    // otherwise equal choices; posters and logos stop gaining past screen size.
    let resolution = if kind == "Backdrop" {
        image.width.min(3840) / 160
    } else {
        image.width.min(1600) / 200
    };
    let confidence = ((image.rating.clamp(0.0, 10.0) * image.votes as f64 + 6.0 * 5.0)
        / (image.votes + 5) as f64
        * 100.0) as u64;
    Some((
        u8::from(!banner),
        image.rank,
        language,
        resolution,
        confidence,
    ))
}
fn source_name(url: &str) -> &'static str {
    match reqwest::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_owned))
        .as_deref()
    {
        Some("image.tmdb.org") => "TMDB",
        Some("assets.fanart.tv") => "fanart.tv",
        Some("static.tvmaze.com") => "TVmaze",
        Some("s4.anilist.co") => "AniList",
        Some("media.kitsu.app" | "media.kitsu.io") => "Kitsu",
        _ => "Cinemeta",
    }
}
/// Suitable images, best first, without duplicates; at most 18 per kind.
fn ranked(images: Vec<Candidate>, kind: &str, preferred: &str) -> Vec<Alternative> {
    let mut scored: Vec<_> = images
        .into_iter()
        .filter_map(|i| quality(&i, kind, preferred).map(|q| (q, i)))
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.url.cmp(&b.1.url)));
    let mut seen = std::collections::HashSet::new();
    scored
        .into_iter()
        .filter(|(_, i)| seen.insert(i.url.clone()))
        .take(18)
        .map(|(_, i)| Alternative {
            source: source_name(&i.url).into(),
            url: i.url,
            lang: i.lang,
            width: i.width,
            height: i.height,
        })
        .collect()
}
#[cfg(test)]
fn best(images: Vec<Candidate>, kind: &str) -> Option<String> {
    ranked(images, kind, "es").into_iter().next().map(|a| a.url)
}
fn number(v: &Value) -> u64 {
    v.as_u64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        .unwrap_or(0)
}
fn provider(item: &Item, key: &str) -> Option<String> {
    item.provider_ids
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .map(|(_, v)| v.clone())
}
fn normalized(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}
fn clean_name(item: &Item) -> String {
    let mut name = item.name.replace(['.', '_'], " ");
    for marker in [
        " [", " (", " 2160p", " 1080p", " 720p", " WEB-DL", " BluRay",
    ] {
        if let Some(i) = name.find(marker) {
            name.truncate(i);
        }
    }
    name.trim().to_owned()
}
fn search_match<'a>(item: &Item, results: &'a [Value]) -> Option<&'a Value> {
    let title = normalized(&clean_name(item));
    let year = item.production_year;
    let mut matches = results.iter().filter(|r| {
        let names = ["title", "original_title", "name", "original_name"];
        let same = names
            .iter()
            .filter_map(|k| r[*k].as_str())
            .any(|n| normalized(n) == title);
        let result_year = r[if item.kind == "Series" {
            "first_air_date"
        } else {
            "release_date"
        }]
        .as_str()
        .and_then(|s| s.get(..4))
        .and_then(|s| s.parse::<u32>().ok());
        same && year.is_none_or(|y| result_year == Some(y))
    });
    let result = matches.next()?;
    if matches.next().is_some() {
        None
    } else {
        Some(result)
    }
}
fn tvmaze_match<'a>(item: &Item, results: &'a [Value]) -> Option<&'a Value> {
    let tvdb = provider(item, "Tvdb");
    let imdb = provider(item, "Imdb");
    let mut matches = results.iter().map(|r| &r["show"]).filter(|show| {
        if let Some(tvdb) = &tvdb {
            return number(&show["externals"]["thetvdb"]).to_string() == *tvdb;
        }
        if let Some(imdb) = &imdb {
            return show["externals"]["imdb"].as_str() == Some(imdb.as_str());
        }
        let year = show["premiered"]
            .as_str()
            .and_then(|s| s.get(..4))
            .and_then(|s| s.parse::<u32>().ok());
        let title = normalized(&clean_name(item));
        let same_name = show["name"]
            .as_str()
            .is_some_and(|n| normalized(n) == title)
            || show["_akas"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|aka| aka["name"].as_str().is_some_and(|n| normalized(n) == title));
        same_name && item.production_year.is_none_or(|y| Some(y) == year)
    });
    let found = matches.next()?;
    matches.next().is_none().then_some(found)
}

struct Api {
    client: reqwest::blocking::Client,
    tmdb: String,
    fanart: String,
    /// AniList allows about 30 requests a minute; uncached calls are spaced.
    anilist_next: Cell<Instant>,
    /// Optional anime sources stop for this run after a failure; others continue.
    anime_unavailable: Cell<bool>,
    cinemeta_unavailable: Cell<bool>,
}
impl Api {
    fn json(
        &self,
        provider: &str,
        path: &str,
        query: &[(&str, String)],
        cancel: &AtomicBool,
    ) -> Result<Value, String> {
        self.request(provider, path, query, None, cancel)
    }
    fn request(
        &self,
        provider: &str,
        path: &str,
        query: &[(&str, String)],
        body: Option<&Value>,
        cancel: &AtomicBool,
    ) -> Result<Value, String> {
        if cancel.load(Ordering::Relaxed) {
            return Err("Actualización cancelada.".into());
        }
        let cache = data::data_dir()
            .join("metadata-api-cache")
            .join(sources::stable_id(&format!(
                "{provider}|{path}|{query:?}{}",
                body.map(|b| format!("|{b}")).unwrap_or_default()
            )));
        let cached = std::fs::read(&cache)
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok());
        if std::fs::metadata(&cache)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|a| a < Duration::from_secs(7 * 86400))
            && let Some(value) = cached.clone()
        {
            return Ok(value);
        }
        let base = match provider {
            "TMDB" => "https://api.themoviedb.org/3",
            "TVmaze" => "https://api.tvmaze.com",
            "AniList" => "https://graphql.anilist.co",
            "Kitsu" => "https://kitsu.io/api/edge",
            "Cinemeta" => "https://v3-cinemeta.strem.io",
            _ => "https://webservice.fanart.tv/v3.2",
        };
        for attempt in 0..2 {
            if provider == "AniList" {
                while Instant::now() < self.anilist_next.get() {
                    if cancel.load(Ordering::Relaxed) {
                        return Err("Actualización cancelada.".into());
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                self.anilist_next
                    .set(Instant::now() + Duration::from_millis(2100));
            }
            let request = match body {
                Some(body) => self.client.post(format!("{base}{path}")).json(body),
                None => self.client.get(format!("{base}{path}")).query(query),
            };
            let request = if provider == "TMDB" {
                request.bearer_auth(&self.tmdb)
            } else if provider == "fanart.tv" {
                request.header("client-key", &self.fanart)
            } else {
                request
            };
            let response = match request.send() {
                Ok(r) => r,
                Err(_) => {
                    return cached.ok_or_else(|| {
                        format!("{provider} no responde. Puedes volver a intentarlo.")
                    });
                }
            };
            if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                let wait = response
                    .headers()
                    .get("Retry-After")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(2);
                if attempt == 0 && wait <= 5 {
                    for _ in 0..wait * 10 {
                        if cancel.load(Ordering::Relaxed) {
                            return Err("Actualización cancelada.".into());
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    continue;
                }
                return Err(format!(
                    "{provider} ha limitado las consultas. Inténtalo más tarde."
                ));
            }
            if response.status() == reqwest::StatusCode::NOT_FOUND {
                return Ok(Value::Null);
            }
            if matches!(response.status().as_u16(), 401 | 403) {
                return Err(format!("Revisa la autorización de {provider} en Ajustes."));
            }
            if !response.status().is_success() {
                return cached.ok_or_else(|| format!("{provider} no está disponible."));
            }
            let mut bytes = vec![];
            response
                .take(4_194_305)
                .read_to_end(&mut bytes)
                .map_err(|_| format!("Respuesta de {provider} incompleta."))?;
            if bytes.len() > 4_194_304 {
                return Err(format!("Respuesta de {provider} demasiado grande."));
            }
            let value: Value = serde_json::from_slice(&bytes)
                .map_err(|_| format!("Respuesta de {provider} no válida."))?;
            if let Some(parent) = cache.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(cache, bytes);
            return Ok(value);
        }
        Err(format!("{provider} no está disponible."))
    }
    fn enrich(
        &self,
        item: &Item,
        seasons: &[u32],
        settings: &Settings,
        cancel: &AtomicBool,
    ) -> Result<Option<Patch>, String> {
        if !matches!(item.kind.as_str(), "Movie" | "Series") {
            return Ok(None);
        }
        let tv = item.kind == "Series";
        let namespace = if tv { "tv" } else { "movie" };
        let mut tmdb = provider(item, "Tmdb").and_then(|s| s.parse::<u64>().ok());
        let mut tvdb = provider(item, "Tvdb").and_then(|s| s.parse::<u64>().ok());
        let mut details = Value::Null;
        if settings.active(Provider::Tmdb) && !self.tmdb.is_empty() {
            if tmdb.is_none()
                && let Some((id, source)) = provider(item, "Imdb")
                    .map(|s| (s, "imdb_id"))
                    .or_else(|| tvdb.map(|id| (id.to_string(), "tvdb_id")))
                && id.chars().all(|c| c.is_ascii_alphanumeric())
            {
                let found = self.json(
                    "TMDB",
                    &format!("/find/{id}"),
                    &[("external_source", source.into())],
                    cancel,
                )?;
                let results = found[if tv { "tv_results" } else { "movie_results" }].as_array();
                tmdb = results
                    .filter(|r| r.len() == 1)
                    .and_then(|r| r[0]["id"].as_u64());
            }
            // Titles may be in English or in the library's own language (often
            // Spanish here): search in the preferred language, then in Spanish.
            for language in [settings.tmdb_language(), "es-ES"] {
                if tmdb.is_some() {
                    break;
                }
                let mut query = vec![
                    ("query", clean_name(item)),
                    ("language", language.into()),
                    ("include_adult", "false".into()),
                ];
                if let Some(year) = item.production_year {
                    query.push((
                        if tv { "first_air_date_year" } else { "year" },
                        year.to_string(),
                    ));
                }
                let found = self.json("TMDB", &format!("/search/{namespace}"), &query, cancel)?;
                tmdb = found["results"]
                    .as_array()
                    .and_then(|r| search_match(item, r))
                    .and_then(|v| v["id"].as_u64());
            }
            if let Some(id) = tmdb {
                details = self.json(
                    "TMDB",
                    &format!("/{namespace}/{id}"),
                    &[
                        ("language", settings.tmdb_language().into()),
                        ("append_to_response", "images,external_ids".into()),
                        ("include_image_language", "en,es,null".into()),
                    ],
                    cancel,
                )?;
                if number(&details["id"]) != id {
                    return Ok(None);
                }
                tvdb = details["external_ids"]["tvdb_id"].as_u64().or(tvdb);
            }
        }
        let fanart_id = if tv { tvdb } else { tmdb };
        let fanart = if settings.active(Provider::Fanart)
            && !self.fanart.is_empty()
            && let Some(id) = fanart_id
        {
            self.json(
                "fanart.tv",
                &format!("/{}/{id}", if tv { "tv" } else { "movies" }),
                &[],
                cancel,
            )?
        } else {
            Value::Null
        };
        let mut tvmaze_id = None;
        let mut maze_images = Value::Null;
        if tv && settings.active(Provider::Tvmaze) {
            let mut results = self.json(
                "TVmaze",
                "/search/shows",
                &[("q", clean_name(item))],
                cancel,
            )?;
            // Localized/original titles are accepted only when explicitly listed by
            // the provider, with the known release year. Never trust fuzzy search order.
            if results
                .as_array()
                .and_then(|r| tvmaze_match(item, r))
                .is_none()
                && provider(item, "Tvdb").is_none()
                && provider(item, "Imdb").is_none()
                && let Some(year) = item.production_year
                && let Some(shows) = results.as_array_mut()
            {
                let candidates: Vec<usize> = shows
                    .iter()
                    .enumerate()
                    .filter_map(|(index, result)| {
                        let release = result["show"]["premiered"]
                            .as_str()
                            .and_then(|s| s.get(..4))
                            .and_then(|s| s.parse::<u32>().ok());
                        (release == Some(year)).then_some(index)
                    })
                    .collect();
                if candidates.len() <= 3 {
                    for index in candidates {
                        if let Some(id) = shows[index]["show"]["id"].as_u64() {
                            shows[index]["show"]["_akas"] =
                                self.json("TVmaze", &format!("/shows/{id}/akas"), &[], cancel)?;
                        }
                    }
                }
            }
            tvmaze_id = results
                .as_array()
                .and_then(|r| tvmaze_match(item, r))
                .and_then(|s| s["id"].as_u64());
            if let Some(id) = tvmaze_id {
                maze_images = self.json("TVmaze", &format!("/shows/{id}/images"), &[], cancel)?;
            }
        }
        let wanted = |kind: &str| {
            settings.replace_images
                || if kind == "Backdrop" {
                    item.backdrop_image_tags.is_empty()
                } else {
                    !item.image_tags.contains_key(kind)
                }
        };
        let mut candidates: HashMap<&str, Vec<Candidate>> = HashMap::new();
        for (kind, tmdb_key, fanart_key, maze_type) in [
            (
                "Primary",
                "posters",
                if tv { "tvposter" } else { "movieposter" },
                "poster",
            ),
            (
                "Backdrop",
                "backdrops",
                if tv {
                    "showbackground"
                } else {
                    "moviebackground"
                },
                "background",
            ),
            (
                "Logo",
                "logos",
                if tv { "hdtvlogo" } else { "hdmovielogo" },
                "typography",
            ),
        ] {
            let list = candidates.entry(kind).or_default();
            for image in maze_images
                .as_array()
                .into_iter()
                .flatten()
                .filter(|i| i["type"].as_str() == Some(maze_type))
            {
                let original = &image["resolutions"]["original"];
                list.push(Candidate {
                    url: original["url"].as_str().unwrap_or("").into(),
                    lang: "".into(),
                    width: number(&original["width"]),
                    height: number(&original["height"]),
                    votes: 0,
                    rating: 6.0,
                    rank: settings.rank(Provider::Tvmaze),
                });
            }
            for image in details["images"][tmdb_key].as_array().into_iter().flatten() {
                let Some(path) = image["file_path"]
                    .as_str()
                    .filter(|p| p.starts_with('/') && !p.contains("..") && !p.contains('?'))
                else {
                    continue;
                };
                list.push(Candidate {
                    url: format!("https://image.tmdb.org/t/p/original{path}"),
                    lang: image["iso_639_1"].as_str().unwrap_or("").into(),
                    width: number(&image["width"]),
                    height: number(&image["height"]),
                    votes: number(&image["vote_count"]),
                    rating: image["vote_average"].as_f64().unwrap_or(6.0),
                    rank: settings.rank(Provider::Tmdb),
                });
            }
            for image in fanart[fanart_key].as_array().into_iter().flatten() {
                list.push(Candidate {
                    url: image["url"].as_str().unwrap_or("").into(),
                    lang: image["lang"].as_str().unwrap_or("").into(),
                    width: number(&image["width"]),
                    height: number(&image["height"]),
                    votes: number(&image["likes"]),
                    rating: 8.0,
                    rank: settings.rank(Provider::Fanart),
                });
            }
        }
        // Anime sources are consulted only for images still missing a proper
        // shape, which keeps the request budget for titles that need them.
        let lacking = |kind: &str, list: &HashMap<&str, Vec<Candidate>>| {
            !list
                .get(kind)
                .into_iter()
                .flatten()
                .any(|c| quality(c, kind, &settings.language).is_some_and(|q| q.0 == 1))
        };
        let missing =
            |kind: &str, list: &HashMap<&str, Vec<Candidate>>| wanted(kind) && lacking(kind, list);
        // Cinemeta is cheap and keyless: it also fills the picker for titles
        // whose library images are already present.
        if settings.active(Provider::Cinemeta)
            && !self.cinemeta_unavailable.get()
            && ["Primary", "Backdrop", "Logo"]
                .into_iter()
                .any(|kind| lacking(kind, &candidates))
        {
            match self.cinemeta(item, tv, settings, cancel) {
                Ok(found) => {
                    for (kind, candidate) in found {
                        candidates.entry(kind).or_default().push(candidate);
                    }
                }
                Err(e) if cancel.load(Ordering::Relaxed) => return Err(e),
                Err(_) => self.cinemeta_unavailable.set(true),
            }
        }
        if (settings.active(Provider::Anilist) || settings.active(Provider::Kitsu))
            && !self.anime_unavailable.get()
            && (missing("Backdrop", &candidates) || missing("Primary", &candidates))
        {
            match self.anime(item, tv, settings, cancel) {
                Ok(found) => {
                    for (kind, candidate) in found {
                        candidates.entry(kind).or_default().push(candidate);
                    }
                }
                Err(e) if cancel.load(Ordering::Relaxed) => return Err(e),
                Err(_) => self.anime_unavailable.set(true),
            }
        }
        if details.is_null() && candidates.values().all(|c| c.is_empty()) {
            return Ok(None);
        }
        let text = |key: &str| {
            details[key]
                .as_str()
                .filter(|s| !s.trim().is_empty())
                .map(str::to_owned)
        };
        let mut patch = Patch {
            id: item.id.clone(),
            name: text(if tv { "name" } else { "title" }),
            overview: text("overview"),
            year: details[if tv { "first_air_date" } else { "release_date" }]
                .as_str()
                .and_then(|s| s.get(..4))
                .and_then(|s| s.parse().ok()),
            runtime: details["runtime"]
                .as_u64()
                .map(|m| m.saturating_mul(600_000_000)),
            tmdb,
            tvmaze: tvmaze_id,
            genres: details["genres"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| v["name"].as_str().map(str::to_owned))
                .collect(),
            ..Default::default()
        };
        for (kind, list) in candidates {
            let ranked = ranked(list, kind, &settings.language);
            if wanted(kind)
                && let Some(first) = ranked.first()
            {
                patch.auto_images.insert(kind.into(), first.url.clone());
            }
            if !ranked.is_empty() {
                patch.alternatives.insert(kind.into(), ranked);
            }
        }
        patch.images = patch.auto_images.clone();
        // Each episode's own picture, one request per season the library has.
        if tv
            && settings.active(Provider::Tmdb)
            && !self.tmdb.is_empty()
            && let Some(id) = tmdb
        {
            for season in seasons {
                let data = self.json(
                    "TMDB",
                    &format!("/tv/{id}/season/{season}"),
                    &[("language", settings.tmdb_language().into())],
                    cancel,
                )?;
                for episode in data["episodes"].as_array().into_iter().flatten() {
                    let Some(path) = episode["still_path"]
                        .as_str()
                        .filter(|p| p.starts_with('/') && !p.contains("..") && !p.contains('?'))
                    else {
                        continue;
                    };
                    let number = number(&episode["episode_number"]);
                    patch.episode_stills.insert(
                        format!("S{season}E{number}"),
                        format!("https://image.tmdb.org/t/p/original{path}"),
                    );
                }
            }
        }
        if patch.name.is_none()
            && patch.overview.is_none()
            && patch.genres.is_empty()
            && patch.alternatives.is_empty()
            && patch.episode_stills.is_empty()
        {
            return Ok(None);
        }
        Ok(Some(patch))
    }
    /// Cinemeta (Stremio's public catalogue, no account) identifies the title by
    /// IMDb id or an exact name and year; Metahub serves its artwork, up to 4K.
    /// Only images are taken: its English synopsis never replaces local text.
    fn cinemeta(
        &self,
        item: &Item,
        tv: bool,
        settings: &Settings,
        cancel: &AtomicBool,
    ) -> Result<Vec<(&'static str, Candidate)>, String> {
        let kind = if tv { "series" } else { "movie" };
        let imdb = provider(item, "Imdb").filter(|id| {
            id.starts_with("tt") && id.len() <= 12 && id[2..].chars().all(|c| c.is_ascii_digit())
        });
        let id = match imdb {
            Some(id) => id,
            None => {
                let found = self.json(
                    "Cinemeta",
                    &format!(
                        "/catalog/{kind}/top/search={}.json",
                        path_escape(&clean_name(item))
                    ),
                    &[],
                    cancel,
                )?;
                let Some(id) = found["metas"]
                    .as_array()
                    .and_then(|m| cinemeta_match(item, m))
                    .and_then(|m| m["id"].as_str())
                    .filter(|id| {
                        id.starts_with("tt") && id[2..].chars().all(|c| c.is_ascii_digit())
                    })
                else {
                    return Ok(vec![]);
                };
                id.to_owned()
            }
        };
        let meta = self.json("Cinemeta", &format!("/meta/{kind}/{id}.json"), &[], cancel)?;
        let meta = &meta["meta"];
        let rank = settings.rank(Provider::Cinemeta);
        let candidate = |url: String, width, height| Candidate {
            url,
            lang: "".into(),
            width,
            height,
            votes: 0,
            rating: 6.0,
            rank,
        };
        let mut images = vec![(
            "Primary",
            candidate(
                format!("https://images.metahub.space/poster/large/{id}/img"),
                780,
                1170,
            ),
        )];
        // Metahub's large backgrounds are at least 1920×1080; logos are PNG.
        if meta["background"].as_str().is_some_and(|b| !b.is_empty()) {
            images.push((
                "Backdrop",
                candidate(
                    format!("https://images.metahub.space/background/large/{id}/img"),
                    1920,
                    1080,
                ),
            ));
        }
        if meta["logo"].as_str().is_some_and(|l| !l.is_empty()) {
            images.push((
                "Logo",
                candidate(
                    format!("https://images.metahub.space/logo/large/{id}/img"),
                    800,
                    250,
                ),
            ));
        }
        if meta["poster"].as_str().is_none_or(str::is_empty) {
            images.remove(0);
        }
        Ok(images)
    }
    /// AniList identifies the title (exact name or listed synonym, same year);
    /// Kitsu is reached only through that verified MyAnimeList identity.
    fn anime(
        &self,
        item: &Item,
        tv: bool,
        settings: &Settings,
        cancel: &AtomicBool,
    ) -> Result<Vec<(&'static str, Candidate)>, String> {
        let query = "query($search:String,$formats:[MediaFormat]){Page(perPage:10){media(search:$search,type:ANIME,format_in:$formats){id idMal startDate{year} title{romaji english native} synonyms bannerImage}}}";
        let formats: &[&str] = if tv {
            &["TV", "TV_SHORT", "ONA", "OVA"]
        } else {
            &["MOVIE"]
        };
        let body = serde_json::json!({
            "query": query,
            "variables": {"search": clean_name(item), "formats": formats},
        });
        let found = self.request("AniList", "", &[], Some(&body), cancel)?;
        let Some(media) = found["data"]["Page"]["media"]
            .as_array()
            .and_then(|m| anilist_match(item, m))
        else {
            return Ok(vec![]);
        };
        let mut images = vec![];
        if settings.active(Provider::Anilist)
            && let Some(url) = media["bannerImage"].as_str()
        {
            // AniList publishes banners at 1900×400 without listing dimensions.
            images.push((
                "Backdrop",
                Candidate {
                    url: url.into(),
                    lang: "".into(),
                    width: 1900,
                    height: 400,
                    votes: 0,
                    rating: 6.0,
                    rank: settings.rank(Provider::Anilist),
                },
            ));
        }
        if settings.active(Provider::Kitsu)
            && let Some(mal) = media["idMal"].as_u64()
        {
            let mapping = self.json(
                "Kitsu",
                "/mappings",
                &[
                    ("filter[externalSite]", "myanimelist/anime".to_owned()),
                    ("filter[externalId]", mal.to_string()),
                    ("include", "item".to_owned()),
                ],
                cancel,
            )?;
            let anime: Vec<_> = mapping["included"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|i| i["type"].as_str() == Some("anime"))
                .collect();
            if let [anime] = anime.as_slice() {
                let attributes = &anime["attributes"];
                for (kind, key) in [("Backdrop", "coverImage"), ("Primary", "posterImage")] {
                    let image = &attributes[key];
                    let size = &image["meta"]["dimensions"]["large"];
                    let Some(url) = image["large"].as_str() else {
                        continue;
                    };
                    images.push((
                        kind,
                        Candidate {
                            // Kitsu appends a cache-busting query; the plain path is the same file.
                            url: url.split('?').next().unwrap_or("").into(),
                            lang: "".into(),
                            width: number(&size["width"]),
                            height: number(&size["height"]),
                            votes: 0,
                            rating: 6.0,
                            rank: settings.rank(Provider::Kitsu),
                        },
                    ));
                }
            }
        }
        Ok(images)
    }
}
fn path_escape(value: &str) -> String {
    value
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
fn cinemeta_match<'a>(item: &Item, metas: &'a [Value]) -> Option<&'a Value> {
    let title = normalized(&clean_name(item));
    let mut matches = metas.iter().filter(|m| {
        let same = m["name"].as_str().is_some_and(|n| normalized(n) == title);
        let year = m["releaseInfo"]
            .as_str()
            .or(m["year"].as_str())
            .and_then(|s| s.get(..4))
            .and_then(|s| s.parse::<u32>().ok());
        same && item.production_year.is_none_or(|y| Some(y) == year)
    });
    let found = matches.next()?;
    matches.next().is_none().then_some(found)
}
fn anilist_match<'a>(item: &Item, media: &'a [Value]) -> Option<&'a Value> {
    let title = normalized(&clean_name(item));
    let mut matches = media.iter().filter(|m| {
        let names = ["romaji", "english", "native"]
            .into_iter()
            .filter_map(|k| m["title"][k].as_str())
            .chain(
                m["synonyms"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str),
            );
        let same = names.into_iter().any(|n| normalized(n) == title);
        let year = m["startDate"]["year"].as_u64().map(|y| y as u32);
        same && item.production_year.is_none_or(|y| Some(y) == year)
    });
    let found = matches.next()?;
    matches.next().is_none().then_some(found)
}
pub fn run(
    items: Vec<Item>,
    seasons: HashMap<String, Vec<u32>>,
    settings: Settings,
    cancel: &AtomicBool,
    mut received: impl FnMut(Patch),
) -> Result<usize, String> {
    let secret = |b: &[u8], provider: Provider| -> Result<String, String> {
        if b.is_empty() {
            Ok(built_in_key(provider).unwrap_or_default().to_owned())
        } else {
            String::from_utf8(data::protect(b, true)?).map_err(|_| "Autorización no válida.".into())
        }
    };
    let api = Api {
        client: reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(12))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("Megalodon/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|_| "No se puede iniciar la búsqueda de fichas.")?,
        tmdb: secret(&settings.tmdb, Provider::Tmdb)?,
        fanart: secret(&settings.fanart, Provider::Fanart)?,
        anilist_next: Cell::new(Instant::now()),
        anime_unavailable: Cell::new(false),
        cinemeta_unavailable: Cell::new(false),
    };
    let mut count = 0;
    for item in items {
        if cancel.load(Ordering::Relaxed) {
            return Err("Actualización cancelada.".into());
        }
        let item_seasons = seasons.get(&item.id).map_or(&[][..], Vec::as_slice);
        if let Some(patch) = api.enrich(&item, item_seasons, &settings, cancel)? {
            received(patch);
            count += 1;
        }
    }
    Ok(count)
}

pub fn allowed_image(url: &str) -> bool {
    image_url_safe(url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn tvmaze_matches_exact_identity_and_never_a_similarly_named_series() {
        let item = Item {
            name: "Dragon Ball".into(),
            kind: "Series".into(),
            production_year: Some(1986),
            ..Default::default()
        };
        let results = vec![
            serde_json::json!({"show":{"id":1,"name":"Dragon Ball Z","premiered":"1989-04-26"}}),
            serde_json::json!({"show":{"id":2,"name":"Dragon Ball","premiered":"1986-02-26","externals":{"thetvdb":76666}}}),
        ];
        assert_eq!(tvmaze_match(&item, &results).unwrap()["id"], 2);
        let wrong_year = Item {
            production_year: Some(2024),
            ..item.clone()
        };
        assert!(tvmaze_match(&wrong_year, &results).is_none());
        let mut by_id = wrong_year;
        by_id.provider_ids.insert("Tvdb".into(), "76666".into());
        assert_eq!(tvmaze_match(&by_id, &results).unwrap()["id"], 2);
        by_id.provider_ids.insert("Tvdb".into(), "999".into());
        assert!(tvmaze_match(&by_id, &results).is_none());
    }
    #[test]
    fn tvmaze_accepts_verified_aliases_only_in_the_same_year() {
        let item = Item {
            name: "Kamisama Hajimemashita".into(),
            kind: "Series".into(),
            production_year: Some(2012),
            ..Default::default()
        };
        let show = json!({"show":{"id":5545,"name":"Kamisama Kiss","premiered":"2012-10-02",
            "_akas":[{"name":"Kamisama Hajimemashita"}]}});
        assert_eq!(
            tvmaze_match(&item, std::slice::from_ref(&show)).unwrap()["id"],
            5545
        );
        assert!(tvmaze_match(&item, &[show.clone(), show.clone()]).is_none());
        let remake = Item {
            production_year: Some(2024),
            ..item
        };
        assert!(tvmaze_match(&remake, &[show]).is_none());
    }
    #[test]
    fn matching_rejects_remakes_ambiguity_and_fuzzy_titles() {
        let item = Item {
            name: "The Thing (1982)".into(),
            kind: "Movie".into(),
            production_year: Some(1982),
            ..Default::default()
        };
        let choices = vec![
            json!({"id":1,"title":"The Thing","release_date":"2011-10-12"}),
            json!({"id":2,"title":"The Thing","release_date":"1982-06-25"}),
        ];
        assert_eq!(search_match(&item, &choices).unwrap()["id"], 2);
        assert!(search_match(&item, &[choices[1].clone(), choices[1].clone()]).is_none());
        assert!(
            search_match(
                &item,
                &[json!({"id":3,"title":"The Thing Returns","release_date":"1982-06-25"})]
            )
            .is_none()
        );
    }
    #[test]
    fn artwork_selection_checks_role_language_dimensions_and_destination() {
        let image = |lang: &str, w, h, url: &str| Candidate {
            lang: lang.into(),
            width: w,
            height: h,
            url: url.into(),
            votes: 20,
            rating: 8.0,
            rank: 1,
        };
        let es = image(
            "es",
            1000,
            1500,
            "https://image.tmdb.org/t/p/original/es.jpg",
        );
        let en = image(
            "en",
            2000,
            3000,
            "https://image.tmdb.org/t/p/original/en.jpg",
        );
        assert!(best(vec![en, es], "Primary").unwrap().ends_with("/es.jpg"));
        assert!(
            quality(
                &image("", 300, 168, "https://assets.fanart.tv/small.jpg"),
                "Backdrop",
                "es"
            )
            .is_none()
        );
        assert!(
            quality(
                &image("es", 2000, 3000, "https://assets.fanart.tv/poster.jpg"),
                "Backdrop",
                "es"
            )
            .is_none()
        );
        assert!(
            quality(
                &image("es", 800, 310, "https://assets.fanart.tv/logo.jpg"),
                "Logo",
                "es"
            )
            .is_none()
        );
        assert!(!allowed_image(
            "https://image.tmdb.org.evil.invalid/image.png"
        ));
        assert!(!allowed_image("http://127.0.0.1:8096/secret"));
    }
    #[test]
    fn provider_list_orders_artwork_and_true_backdrops_beat_banners() {
        let candidate = |url: &str, w, h, rank| Candidate {
            url: url.into(),
            lang: "".into(),
            width: w,
            height: h,
            votes: 0,
            rating: 6.0,
            rank,
        };
        let banner = candidate("https://s4.anilist.co/file/banner.jpg", 1900, 400, 5);
        let backdrop = candidate("https://static.tvmaze.com/b.jpg", 1920, 1080, 1);
        assert!(
            best(vec![banner.clone(), backdrop.clone()], "Backdrop")
                .unwrap()
                .contains("tvmaze")
        );
        assert!(
            best(vec![banner.clone()], "Backdrop")
                .unwrap()
                .contains("anilist")
        );
        assert!(quality(&banner, "Primary", "es").is_none());
        let first = candidate("https://image.tmdb.org/t/p/original/a.jpg", 1920, 1080, 5);
        let second = candidate("https://assets.fanart.tv/b.jpg", 3840, 2160, 4);
        assert!(
            best(vec![second, first], "Backdrop")
                .unwrap()
                .contains("tmdb")
        );
    }
    #[test]
    fn provider_settings_keep_every_source_once_and_migrate_tvmaze() {
        let legacy: Settings =
            serde_json::from_str(r#"{"tvmaze":false,"replace_images":true}"#).unwrap();
        let settings = legacy.normalized();
        assert_eq!(settings.providers.len(), Provider::ALL.len());
        assert!(!settings.enabled(Provider::Tvmaze));
        assert!(settings.active(Provider::Anilist));
        assert_eq!(settings.active(Provider::Tmdb), Provider::Tmdb.built_in());
        let mut reordered = settings.clone();
        assert_eq!(settings.providers[0].provider, Provider::default_order()[0]);
        assert!(settings.active(Provider::Cinemeta));
        let tmdb = settings
            .providers
            .iter()
            .position(|e| e.provider == Provider::Tmdb)
            .unwrap();
        reordered.providers.swap(0, tmdb);
        reordered.providers.push(reordered.providers[0]);
        let reordered = reordered.normalized();
        assert_eq!(reordered.providers.len(), Provider::ALL.len());
        assert_eq!(reordered.providers[0].provider, Provider::Tmdb);
        assert!(reordered.rank(Provider::Tmdb) > reordered.rank(Provider::Anilist));
    }
    #[test]
    fn language_preference_and_saved_choices_shape_the_artwork() {
        let image = |lang: &str, url: &str| Candidate {
            url: url.into(),
            lang: lang.into(),
            width: 1000,
            height: 1500,
            votes: 10,
            rating: 7.0,
            rank: 5,
        };
        let en = image("en", "https://image.tmdb.org/t/p/original/en.jpg");
        let es = image("es", "https://image.tmdb.org/t/p/original/es.jpg");
        let list = ranked(vec![es.clone(), en.clone(), en.clone()], "Primary", "en");
        assert_eq!(list.len(), 2, "duplicates are removed");
        assert!(list[0].url.ends_with("/en.jpg"));
        assert_eq!(list[0].source, "TMDB");
        assert!(
            ranked(vec![es, en], "Primary", "es")[0]
                .url
                .ends_with("/es.jpg")
        );
        let mut patch = Patch {
            id: "a".into(),
            auto_images: HashMap::from([("Primary".into(), list[0].url.clone())]),
            alternatives: HashMap::from([("Primary".into(), list.clone())]),
            ..Default::default()
        };
        patch = patch.with_choices(&Choices::new());
        assert_eq!(patch.images["Primary"], list[0].url);
        let mut choices = Choices::new();
        choices.insert(
            "a".into(),
            HashMap::from([("Primary".into(), list[1].url.clone())]),
        );
        assert_eq!(
            patch.clone().with_choices(&choices).images["Primary"],
            list[1].url
        );
        choices.insert(
            "a".into(),
            HashMap::from([("Primary".into(), "https://evil.invalid/x.jpg".into())]),
        );
        assert_eq!(
            patch.clone().with_choices(&choices).images["Primary"],
            list[0].url
        );
        choices.insert(
            "a".into(),
            HashMap::from([("Primary".into(), LOCAL_CHOICE.into())]),
        );
        assert!(!patch.with_choices(&choices).images.contains_key("Primary"));
    }
    #[test]
    fn sharpest_backdrop_wins_and_saved_choices_survive() {
        let image = |w, h, url: &str| Candidate {
            url: url.into(),
            lang: "".into(),
            width: w,
            height: h,
            votes: 50,
            rating: 6.0,
            rank: 5,
        };
        let list = ranked(
            vec![
                image(3681, 2070, "https://image.tmdb.org/t/p/original/a.jpg"),
                image(3840, 2160, "https://image.tmdb.org/t/p/original/b.jpg"),
                image(1920, 1080, "https://image.tmdb.org/t/p/original/c.jpg"),
            ],
            "Backdrop",
            "en",
        );
        assert!(list[0].url.ends_with("/b.jpg") && list[2].url.ends_with("/c.jpg"));
        let patch = Patch {
            id: "a".into(),
            alternatives: HashMap::from([("Backdrop".into(), list.clone())]),
            ..Default::default()
        };
        let mut choices = Choices::new();
        // A choice no longer offered by the provider is kept and listed first.
        let kept = "https://image.tmdb.org/t/p/original/old.jpg".to_owned();
        choices.insert(
            "a".into(),
            HashMap::from([("Backdrop".into(), kept.clone())]),
        );
        let chosen = patch.with_choices(&choices);
        assert_eq!(chosen.images["Backdrop"], kept);
        assert_eq!(chosen.alternatives["Backdrop"][0].url, kept);
    }
    #[test]
    fn cinemeta_matches_exact_title_and_year_and_escapes_queries() {
        let item = Item {
            name: "Dragon Ball Z - Battle of Gods".into(),
            kind: "Movie".into(),
            production_year: Some(2013),
            ..Default::default()
        };
        let metas = vec![
            json!({"id":"tt2263944","name":"Dragon Ball Z: Battle of Gods","releaseInfo":"2013"}),
            json!({"id":"tt3819668","name":"Dragon Ball Z: Resurrection 'F'","releaseInfo":"2015"}),
        ];
        assert_eq!(cinemeta_match(&item, &metas).unwrap()["id"], "tt2263944");
        assert!(cinemeta_match(&item, &[metas[0].clone(), metas[0].clone()]).is_none());
        assert_eq!(path_escape("Dead Zone/1?"), "Dead%20Zone%2F1%3F");
        assert!(allowed_image(
            "https://images.metahub.space/background/large/tt2263944/img"
        ));
        let logo = Candidate {
            url: "https://images.metahub.space/logo/large/tt2263944/img".into(),
            lang: "".into(),
            width: 800,
            height: 250,
            votes: 0,
            rating: 6.0,
            rank: 6,
        };
        assert!(quality(&logo, "Logo", "es").is_some());
    }
    #[test]
    fn anilist_requires_exact_title_or_synonym_and_year() {
        let item = Item {
            name: "Dragon Ball Z - Dead Zone".into(),
            kind: "Movie".into(),
            production_year: Some(1989),
            ..Default::default()
        };
        let media = vec![
            json!({"id":894,"startDate":{"year":1989},"title":{"romaji":"Dragon Ball Z: Ora no Gohan wo Kaese!!","english":"Dragon Ball Z: Dead Zone"},"synonyms":[]}),
            json!({"id":1,"startDate":{"year":1990},"title":{"english":"Dragon Ball Z: The World's Strongest"},"synonyms":["Dead Zone 2"]}),
        ];
        assert_eq!(anilist_match(&item, &media).unwrap()["id"], 894);
        let other_year = Item {
            production_year: Some(2020),
            ..item
        };
        assert!(anilist_match(&other_year, &media).is_none());
    }
}
