use super::{
    data, drive,
    jellyfin::{Catalog, Item, User},
    sources::{self, Kind, Source},
};
use std::{
    collections::HashMap,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

pub struct Files {
    pub source: Source,
    pub items: Vec<Item>,
    pub media: HashMap<String, String>,
    pub pictures: HashMap<(String, String), String>,
    pub art_revision: String,
    pub stream: Option<drive::Stream>,
    /// The library's image folder (relative to its root), found or to be created.
    pub art_folder: String,
    /// Whether that folder already exists in the library.
    pub art_folder_exists: bool,
    /// Title folder inside the image folder matched to each item.
    pub art_dirs: HashMap<String, String>,
    /// The person's avatars: `<image folder>/avatars/<category>/…` or `avatars/…`.
    pub avatars: Vec<LibraryAvatar>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct LibraryAvatar {
    /// Stable choice id: `library:<relative path>`.
    pub id: String,
    pub label: String,
    pub category: String,
    /// Local path or library stream URL.
    pub location: String,
    /// True when `location` is a file on this computer (local library or a fresh upload).
    pub local: bool,
}
fn is_image(path: &str) -> bool {
    path.rsplit_once('.').is_some_and(|(_, ext)| {
        matches!(ext.to_lowercase().as_str(), "jpg" | "jpeg" | "png" | "webp")
    })
}
/// Avatars found under `<image folder>/avatars/` or `avatars/`. A first-level
/// folder is a category; images directly inside go to «General».
fn collect_avatars(
    paths: &std::collections::HashSet<String>,
    art_folder: &str,
) -> Vec<(String, String, String)> {
    let roots = [
        format!("{}/avatars/", art_folder.to_lowercase()),
        "avatars/".to_owned(),
    ];
    let mut found: Vec<(String, String, String)> = paths
        .iter()
        .filter(|p| is_image(p))
        .filter_map(|path| {
            let lower = path.to_lowercase();
            let root = roots.iter().find(|r| lower.starts_with(r.as_str()))?;
            let rest = &path[root.len()..];
            let (category, file) = match rest.split_once('/') {
                Some((category, file)) if !file.contains('/') => (category.to_owned(), file),
                None => ("General".to_owned(), rest),
                _ => return None,
            };
            let label = file
                .rsplit_once('.')
                .map_or(file, |(stem, _)| stem)
                .replace(['_', '-'], " ");
            Some((path.clone(), category, label))
        })
        .collect();
    found.sort_by(|a, b| {
        (a.1.to_lowercase(), a.2.to_lowercase()).cmp(&(b.1.to_lowercase(), b.2.to_lowercase()))
    });
    found
}
/// Square, centred, 512 px PNG: the shape every avatar is shown in.
pub fn prepare_avatar(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let image = image::load_from_memory(bytes)
        .map_err(|_| "Este archivo no es una imagen compatible (JPG, PNG o WebP).")?;
    let side = image.width().min(image.height());
    if side < 96 {
        return Err("La imagen es demasiado pequeña para un avatar.".into());
    }
    let square = image.crop_imm(
        (image.width() - side) / 2,
        (image.height() - side) / 2,
        side,
        side,
    );
    let square = if side > 512 {
        square.resize_exact(512, 512, image::imageops::FilterType::Lanczos3)
    } else {
        square
    };
    let mut out = std::io::Cursor::new(Vec::new());
    square
        .write_to(&mut out, image::ImageFormat::Png)
        .map_err(|_| "No se pudo preparar el avatar.")?;
    Ok(out.into_inner())
}
/// Folder names recognised automatically as a library's own image folder.
pub const ART_FOLDERS: [&str; 7] = [
    "posters",
    "images",
    "imagenes",
    "imágenes",
    "artwork",
    "covers",
    "portadas",
];
/// File names inside a title folder, per kind; the first one is used when saving.
pub fn art_names(kind: &str) -> &'static [&'static str] {
    match kind {
        "Primary" => &["poster", "cover", "folder"],
        "Backdrop" => &["background", "backdrop", "fanart"],
        "Logo" => &["logo", "clearlogo"],
        _ => &[],
    }
}
/// A Windows-, Drive- and WebDAV-safe folder name for a title.
pub fn title_folder_name(name: &str, year: Option<u32>) -> String {
    let clean: String = name
        .chars()
        .map(|c| {
            if "<>:\"/\\|?*".contains(c) || c.is_control() {
                ' '
            } else {
                c
            }
        })
        .collect();
    let clean = clean.split_whitespace().collect::<Vec<_>>().join(" ");
    let clean = clean.trim_matches(['.', ' ']).to_owned();
    let clean = if clean.is_empty() {
        "Sin título".to_owned()
    } else {
        clean
    };
    match year {
        Some(year) => format!("{clean} ({year})"),
        None => clean,
    }
}
fn title_key(name: &str) -> (String, Option<u32>) {
    let mut name = name.trim().to_owned();
    if let Some(start) = name.find(" [") {
        name.truncate(start);
    }
    let mut year = None;
    if let Some((title, suffix)) = name.rsplit_once(" (")
        && let Some(value) = suffix.strip_suffix(')').and_then(|s| s.parse::<u32>().ok())
        && (1888..=2100).contains(&value)
    {
        year = Some(value);
        name = title.to_owned();
    }
    (
        name.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect(),
        year,
    )
}
/// Picks the image folder: the configured name, else the first known one present.
fn find_art_folder(configured: &str, paths: &std::collections::HashSet<String>) -> (String, bool) {
    let configured = configured
        .trim()
        .trim_matches(['/', '\\'])
        .replace('\\', "/");
    let exists = |folder: &str| {
        let prefix = format!("{}/", folder.to_lowercase());
        paths.iter().any(|p| p.to_lowercase().starts_with(&prefix))
    };
    if !configured.is_empty() {
        return (configured.clone(), exists(&configured));
    }
    for name in ART_FOLDERS {
        if let Some(path) = paths.iter().find(|p| {
            p.split('/')
                .next()
                .is_some_and(|first| first.to_lowercase() == name)
                && p.contains('/')
        }) {
            return (path.split('/').next().unwrap_or(name).to_owned(), true);
        }
    }
    ("posters".into(), false)
}
impl Files {
    #[cfg(test)]
    pub fn load(source: Source, cancel: Arc<AtomicBool>) -> Result<Self, String> {
        Self::load_with_art(source, cancel, "")
    }
    pub fn load_with_art(
        source: Source,
        cancel: Arc<AtomicBool>,
        art_folder: &str,
    ) -> Result<Self, String> {
        let metadata = data::data_dir().join("metadata").join(&source.id);
        let (entries, root, stream) = match &source.kind {
            Kind::Folder { path } => (
                walk(path, &cancel)?
                    .into_iter()
                    .map(|relative| {
                        let mod_time = std::fs::metadata(path.join(&relative))
                            .ok()
                            .and_then(|m| m.modified().ok())
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map_or(String::new(), |t| format!("{:020}", t.as_secs()));
                        drive::Entry {
                            path: relative,
                            mod_time,
                        }
                    })
                    .collect::<Vec<_>>(),
                Some(path.clone()),
                None,
            ),
            Kind::Drive {
                executable,
                config,
                remote,
            } => {
                // Installed updates own their runtime; legacy connections can retain a dev path.
                let runtime = drive::executable().unwrap_or_else(|| executable.clone());
                let executable = runtime.as_path();
                let cached = cached_inventory(&source.id);
                let fresh = cached.is_none();
                let paths = match cached {
                    Some(entries) => entries,
                    None => drive::inventory(executable, config, remote, &cancel, &[])?,
                };
                if (fresh || !metadata.exists())
                    && paths
                        .iter()
                        .any(|e| e.path.to_lowercase().ends_with(".nfo"))
                {
                    drive::sync_metadata(executable, config, remote, &metadata, &cancel, &[])?;
                }
                if fresh {
                    save_inventory(&source.id, &paths);
                }
                let stream = drive::Stream::start(executable, config, remote)?;
                (paths, None, Some(stream))
            }
            Kind::WebDav {
                url,
                user,
                credential,
            } => {
                let exe = drive::prepare()?;
                let password = String::from_utf8(data::protect(credential, true)?)
                    .map_err(|_| "La credencial WebDAV no es válida.")?;
                let environment = drive::webdav_environment(&exe, url, user, &password)?;
                let config = Path::new("NUL");
                let cached = cached_inventory(&source.id);
                let fresh = cached.is_none();
                let paths = match cached {
                    Some(entries) => entries,
                    None => {
                        drive::inventory(&exe, config, drive::WEBDAV_REMOTE, &cancel, &environment)?
                    }
                };
                if (fresh || !metadata.exists())
                    && paths
                        .iter()
                        .any(|e| e.path.to_lowercase().ends_with(".nfo"))
                {
                    drive::sync_metadata(
                        &exe,
                        config,
                        drive::WEBDAV_REMOTE,
                        &metadata,
                        &cancel,
                        &environment,
                    )?;
                }
                if fresh {
                    save_inventory(&source.id, &paths);
                }
                let stream = drive::Stream::start_with_env(
                    &exe,
                    config,
                    drive::WEBDAV_REMOTE,
                    &environment,
                )?;
                (paths, None, Some(stream))
            }
            _ => return Err("Esta conexión no contiene archivos directos.".into()),
        };
        let mut artwork_versions: Vec<_> = entries
            .iter()
            .filter(|e| !is_video(Path::new(&e.path)))
            .map(|e| format!("{}|{}", e.path, e.mod_time))
            .collect();
        artwork_versions.sort();
        let art_revision = sources::stable_id(&artwork_versions.join("\n"));
        let mut files = Self {
            source,
            items: vec![],
            media: HashMap::new(),
            pictures: HashMap::new(),
            art_revision,
            stream,
            art_folder: String::new(),
            art_folder_exists: false,
            art_dirs: HashMap::new(),
            avatars: vec![],
        };
        let available: std::collections::HashSet<_> =
            entries.iter().map(|e| e.path.replace('\\', "/")).collect();
        let (folder, folder_exists) = find_art_folder(art_folder, &available);
        files.art_folder = folder;
        files.art_folder_exists = folder_exists;
        let art_prefix = format!("{}/", files.art_folder.to_lowercase());
        for (path, category, label) in collect_avatars(&available, &files.art_folder) {
            let (location, local) = match &root {
                Some(root) => (root.join(&path).to_string_lossy().into_owned(), true),
                None => (
                    files
                        .stream
                        .as_ref()
                        .and_then(|s| s.url(&path).ok())
                        .unwrap_or_default(),
                    false,
                ),
            };
            files.avatars.push(LibraryAvatar {
                id: format!("library:{path}"),
                label,
                category,
                location,
                local,
            });
        }
        // Names an item can be recognised by in the image folder: its title and
        // the folder that holds its video.
        let mut title_keys: HashMap<String, Vec<String>> = HashMap::new();
        let nfo_root = root.as_deref().unwrap_or(&metadata);
        for entry in entries.iter().filter(|e| {
            is_video(Path::new(&e.path)) && !e.path.to_lowercase().starts_with(&art_prefix)
        }) {
            let relative = &entry.path;
            if cancel.load(Ordering::Relaxed) {
                return Err("Conexión cancelada.".into());
            }
            let path = Path::new(relative);
            let stem = path.file_stem().unwrap_or_default().to_string_lossy();
            let parent = path.parent().unwrap_or(Path::new(""));
            let mut episode = episode_numbers(&stem);
            let name = stem.replace(['.', '_'], " ");
            let id = format!(
                "file-{}",
                sources::stable_id(&format!("{}|{}", files.source.id, relative))
            );
            let mut item = Item {
                id: id.clone(),
                date_created: Some(entry.mod_time.clone()),
                name,
                kind: if episode.is_some() {
                    "Episode"
                } else {
                    "Movie"
                }
                .into(),
                ..Default::default()
            };
            let nfo = path.with_extension("nfo");
            if available.contains(&nfo.to_string_lossy().replace('\\', "/")) {
                read_nfo(&nfo_root.join(&nfo), &mut item);
            } else if available.contains(
                &parent
                    .join("movie.nfo")
                    .to_string_lossy()
                    .replace('\\', "/"),
            ) {
                read_nfo(&nfo_root.join(parent).join("movie.nfo"), &mut item);
            }
            episode = item.parent_index_number.zip(item.index_number).or(episode);
            tidy_title(&mut item);
            if episode.is_some() {
                item.kind = "Episode".into();
            }
            if let Some((season, number)) = episode {
                item.parent_index_number.get_or_insert(season);
                item.index_number.get_or_insert(number);
                tidy_title(&mut item);
                let folder_name = parent
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase();
                let series_folder = if folder_name.starts_with("season")
                    || folder_name.starts_with("temporada")
                    || folder_name.starts_with("s0")
                    || folder_name == "specials"
                {
                    parent.parent().unwrap_or(parent)
                } else {
                    parent
                };
                let series_id = format!(
                    "series-{}",
                    sources::stable_id(&format!("{}|{}", files.source.id, series_folder.display()))
                );
                let series_name = series_folder
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| {
                        stem.split(['S', 's'])
                            .next()
                            .unwrap_or(&stem)
                            .trim_matches(['.', ' ', '-'])
                            .to_owned()
                    });
                if !files.items.iter().any(|i| i.id == series_id) {
                    let mut series = Item {
                        id: series_id.clone(),
                        name: series_name.clone(),
                        kind: "Series".into(),
                        date_created: item.date_created.clone(),
                        ..Default::default()
                    };
                    read_nfo(
                        &nfo_root.join(series_folder).join("tvshow.nfo"),
                        &mut series,
                    );
                    tidy_title(&mut series);
                    files.attach_art(&mut series, series_folder, "", &available, root.as_deref());
                    title_keys.insert(
                        series_id.clone(),
                        vec![
                            series.name.clone(),
                            series_folder
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into_owned(),
                        ],
                    );
                    files.items.push(series);
                }
                if let Some(series) = files.items.iter_mut().find(|i| i.id == series_id)
                    && item.date_created > series.date_created
                {
                    series.date_created = item.date_created.clone();
                }
                item.series_name = files
                    .items
                    .iter()
                    .find(|i| i.id == series_id)
                    .map(|i| i.name.clone());
                item.series_id = Some(series_id);
            }
            files.attach_art(&mut item, parent, &stem, &available, root.as_deref());
            if item.kind == "Movie" {
                title_keys.insert(
                    id.clone(),
                    vec![
                        item.name.clone(),
                        parent
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned(),
                    ],
                );
            }
            let location = if let Some(root) = &root {
                root.join(path).to_string_lossy().into_owned()
            } else {
                files.stream.as_ref().unwrap().url(relative)?
            };
            files.media.insert(id, location);
            files.items.push(item);
        }
        files.attach_library_art(&available, &title_keys, root.as_deref());
        files.items.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(files)
    }
    /// The image folder is the person's curated choice: it wins over images next
    /// to the video and over every provider. One folder per title.
    fn attach_library_art(
        &mut self,
        paths: &std::collections::HashSet<String>,
        title_keys: &HashMap<String, Vec<String>>,
        root: Option<&Path>,
    ) {
        if !self.art_folder_exists {
            return;
        }
        let prefix = format!("{}/", self.art_folder.to_lowercase());
        let mut dirs: HashMap<String, Vec<String>> = HashMap::new();
        for path in paths {
            if !path.to_lowercase().starts_with(&prefix) {
                continue;
            }
            let rest = &path[prefix.len()..];
            if let Some((dir, file)) = rest.split_once('/')
                && !file.contains('/')
            {
                dirs.entry(dir.to_owned())
                    .or_default()
                    .push(file.to_owned());
            }
        }
        let keyed: Vec<_> = dirs
            .keys()
            .filter(|d| !d.eq_ignore_ascii_case("avatars"))
            .map(|d| (d.clone(), title_key(d)))
            .collect();
        for item in &mut self.items {
            let Some(names) = title_keys.get(&item.id) else {
                continue;
            };
            let wanted: Vec<_> = names.iter().map(|n| title_key(n).0).collect();
            let year = item.production_year;
            let mut matches: Vec<_> = keyed
                .iter()
                .filter(|(_, (key, dir_year))| {
                    !key.is_empty()
                        && wanted.contains(key)
                        && (dir_year.is_none() || year.is_none() || *dir_year == year)
                })
                .collect();
            if matches.len() > 1 {
                matches.retain(|(_, (_, dir_year))| dir_year.is_some() && *dir_year == year);
            }
            let [(dir, _)] = matches.as_slice() else {
                continue;
            };
            self.art_dirs
                .insert(item.id.clone(), format!("{}/{dir}", self.art_folder));
            let files = &dirs[dir];
            for kind in ["Primary", "Backdrop", "Logo"] {
                let Some(file) = art_names(kind).iter().find_map(|name| {
                    files.iter().find(|f| {
                        f.rsplit_once('.').is_some_and(|(stem, ext)| {
                            stem.eq_ignore_ascii_case(name)
                                && matches!(
                                    ext.to_lowercase().as_str(),
                                    "jpg" | "jpeg" | "png" | "webp"
                                )
                        })
                    })
                }) else {
                    continue;
                };
                let path = format!("{}/{dir}/{file}", self.art_folder);
                let location = if let Some(root) = root {
                    root.join(&path).to_string_lossy().into_owned()
                } else {
                    self.stream
                        .as_ref()
                        .map_or(Ok(String::new()), |s| s.url(&path))
                        .unwrap_or_default()
                };
                self.pictures
                    .insert((item.id.clone(), kind.into()), location);
                if kind == "Backdrop" {
                    if !item.backdrop_image_tags.iter().any(|t| t == "local") {
                        item.backdrop_image_tags.insert(0, "local".into());
                    }
                } else {
                    item.image_tags.insert(kind.into(), "local".into());
                }
            }
        }
    }
    /// Writes an image into the library (local folder, Drive or WebDAV).
    pub fn save_art(
        &self,
        relative: &str,
        local: &Path,
        cancel: &AtomicBool,
    ) -> Result<(), String> {
        if relative
            .split(['/', '\\'])
            .any(|s| s.is_empty() || s == "." || s == "..")
            || Path::new(relative).is_absolute()
        {
            return Err("Ruta de imagen no válida.".into());
        }
        match &self.source.kind {
            Kind::Folder { path } => {
                let target = path.join(relative);
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|_| "No se pudo crear la carpeta de imágenes.")?;
                }
                std::fs::copy(local, &target)
                    .map(|_| ())
                    .map_err(|_| "No se pudo guardar la imagen en la carpeta.".into())
            }
            Kind::Drive {
                executable,
                config,
                remote,
            } => {
                let runtime = drive::executable().unwrap_or_else(|| executable.clone());
                drive::upload(&runtime, config, remote, relative, local, cancel, &[])
            }
            Kind::WebDav {
                url,
                user,
                credential,
            } => {
                let exe = drive::prepare()?;
                let password = String::from_utf8(data::protect(credential, true)?)
                    .map_err(|_| "La credencial WebDAV no es válida.")?;
                let environment = drive::webdav_environment(&exe, url, user, &password)?;
                drive::upload(
                    &exe,
                    Path::new("NUL"),
                    drive::WEBDAV_REMOTE,
                    relative,
                    local,
                    cancel,
                    &environment,
                )
            }
            Kind::Jellyfin { .. } => Err("Esta conexión no admite imágenes propias.".into()),
        }
    }
    /// Where a new avatar is saved: `<image folder>/avatars/<category>/<name>.png`,
    /// never replacing one that already exists.
    pub fn avatar_target(&self, category: &str, name: &str) -> String {
        let category = title_folder_name(category, None);
        let base = title_folder_name(name, None);
        let mut candidate = format!("{}/avatars/{category}/{base}.png", self.art_folder);
        let mut n = 2;
        while self
            .avatars
            .iter()
            .any(|a| a.id.eq_ignore_ascii_case(&format!("library:{candidate}")))
        {
            candidate = format!("{}/avatars/{category}/{base} {n}.png", self.art_folder);
            n += 1;
        }
        candidate
    }
    /// Where an image for this title is saved: its matched folder, or a new one
    /// named after the library's own title and year.
    pub fn art_target(&self, id: &str, kind: &str) -> Option<String> {
        let name = art_names(kind).first()?;
        let ext = if kind == "Logo" { "png" } else { "jpg" };
        let dir = match self.art_dirs.get(id) {
            Some(dir) => dir.clone(),
            None => {
                let item = self.items.iter().find(|i| i.id == id)?;
                format!(
                    "{}/{}",
                    self.art_folder,
                    title_folder_name(&item.name, item.production_year)
                )
            }
        };
        Some(format!("{dir}/{name}.{ext}"))
    }
    fn attach_art(
        &mut self,
        item: &mut Item,
        parent: &Path,
        stem: &str,
        paths: &std::collections::HashSet<String>,
        root: Option<&Path>,
    ) {
        for (kind, names) in [
            (
                "Primary",
                vec![
                    format!("{stem}-poster"),
                    format!("{stem}-thumb"),
                    "poster".into(),
                    "folder".into(),
                ],
            ),
            (
                "Backdrop",
                vec![format!("{stem}-fanart"), "fanart".into(), "backdrop".into()],
            ),
            ("Logo", vec!["logo".into(), "clearlogo".into()]),
        ] {
            let found = names
                .iter()
                .flat_map(|name| {
                    ["jpg", "png", "webp"].into_iter().map(move |ext| {
                        parent
                            .join(format!("{name}.{ext}"))
                            .to_string_lossy()
                            .replace('\\', "/")
                    })
                })
                .find(|p| paths.contains(p));
            if let Some(path) = found {
                let location = if let Some(root) = root {
                    root.join(&path).to_string_lossy().into_owned()
                } else {
                    self.stream.as_ref().unwrap().url(&path).unwrap_or_default()
                };
                self.pictures
                    .insert((item.id.clone(), kind.into()), location);
                if kind == "Backdrop" {
                    item.backdrop_image_tags.push("local".into());
                } else {
                    item.image_tags.insert(kind.into(), "local".into());
                }
            }
        }
    }
    pub fn catalog(&self, user: User) -> Catalog {
        Catalog {
            items: self
                .items
                .iter()
                .filter(|i| i.kind != "Episode")
                .cloned()
                .collect(),
            user,
            server_name: self.source.name.clone(),
            loaded: Instant::now(),
        }
    }
    pub fn episodes(&self, series: &str) -> Vec<Item> {
        let mut episodes: Vec<_> = self
            .items
            .iter()
            .filter(|i| i.series_id.as_deref() == Some(series))
            .cloned()
            .collect();
        episodes.sort_by_key(|i| (i.parent_index_number, i.index_number));
        episodes
    }
    pub fn playback(&self, item: Item, start: f64) -> Result<data::Playback, String> {
        let url = self
            .media
            .get(&item.id)
            .ok_or("Este título no contiene un vídeo disponible.")?
            .clone();
        Ok(data::Playback {
            item,
            url,
            headers: self
                .stream
                .as_ref()
                .map_or(String::new(), |s| s.header.clone()),
            source_id: String::new(),
            session_id: String::new(),
            start,
            subtitles: vec![],
        })
    }
}
/// Validates an image and stores it in a predictable format: PNG for logos
/// (transparency), high-quality JPEG for the rest, at most 3840 px wide.
pub fn prepare_art(bytes: &[u8], kind: &str) -> Result<Vec<u8>, String> {
    let image = image::load_from_memory(bytes)
        .map_err(|_| "Este archivo no es una imagen compatible (JPG, PNG o WebP).")?;
    let (min_w, min_h) = if kind == "Logo" {
        (200, 40)
    } else {
        (300, 160)
    };
    if image.width() < min_w || image.height() < min_h {
        return Err("La imagen es demasiado pequeña para verse bien.".into());
    }
    let image = if image.width() > 3840 {
        image.resize(3840, 3840 * 4, image::imageops::FilterType::Lanczos3)
    } else {
        image
    };
    let mut out = std::io::Cursor::new(Vec::new());
    if kind == "Logo" {
        image
            .write_to(&mut out, image::ImageFormat::Png)
            .map_err(|_| "No se pudo preparar la imagen.")?;
    } else {
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 92)
            .encode_image(&image.to_rgb8())
            .map_err(|_| "No se pudo preparar la imagen.")?;
    }
    Ok(out.into_inner())
}
pub fn is_video(path: &Path) -> bool {
    path.extension().is_some_and(|e| {
        matches!(
            e.to_string_lossy().to_ascii_lowercase().as_str(),
            "mkv" | "mp4" | "m4v" | "avi" | "mov" | "webm" | "ts" | "m2ts" | "wmv" | "mpg" | "mpeg"
        )
    })
}
fn inventory_path(source: &str) -> std::path::PathBuf {
    data::data_dir()
        .join("inventories")
        .join(sources::stable_id(source))
}
fn cached_inventory(source: &str) -> Option<Vec<drive::Entry>> {
    if cfg!(test) {
        return None;
    }
    let path = inventory_path(source);
    if std::fs::metadata(&path)
        .ok()?
        .modified()
        .ok()?
        .elapsed()
        .ok()?
        .as_secs()
        >= 3600
    {
        return None;
    }
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}
fn save_inventory(source: &str, entries: &[drive::Entry]) {
    if cfg!(test) {
        return;
    }
    let path = inventory_path(source);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(bytes) = serde_json::to_vec(entries) {
        let _ = std::fs::write(path, bytes);
    }
}
pub fn invalidate_inventory(source: &str) {
    let _ = std::fs::remove_file(inventory_path(source));
}
fn walk(root: &Path, cancel: &AtomicBool) -> Result<Vec<String>, String> {
    let root = root
        .canonicalize()
        .map_err(|_| "La carpeta ya no está disponible.")?;
    let mut pending = vec![root.clone()];
    let mut files = vec![];
    while let Some(dir) = pending.pop() {
        if cancel.load(Ordering::Relaxed) {
            return Err("Lectura cancelada.".into());
        }
        for entry in std::fs::read_dir(&dir)
            .map_err(|_| "No se puede leer una carpeta de esta biblioteca.")?
        {
            let entry = entry.map_err(|_| "No se puede leer un archivo.")?;
            let meta = entry
                .path()
                .symlink_metadata()
                .map_err(|_| "Archivo no disponible.")?;
            // Skip symlinks/junctions: a selected folder does not grant access to other roots or cycles.
            use std::os::windows::fs::MetadataExt;
            if meta.file_attributes() & 0x400 != 0 {
                continue;
            }
            if meta.is_dir() {
                pending.push(entry.path());
            } else if meta.is_file() {
                files.push(
                    entry
                        .path()
                        .strip_prefix(&root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
    }
    files.sort();
    Ok(files)
}
fn read_nfo(path: &Path, item: &mut Item) {
    if std::fs::metadata(path).map_or(true, |m| m.len() > 1_048_576) {
        return;
    }
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let Ok(xml) = roxmltree::Document::parse(&text) else {
        return;
    };
    let get = |name| {
        xml.root_element()
            .children()
            .find(|n| n.has_tag_name(name))
            .and_then(|n| n.text())
    };
    for node in xml
        .root_element()
        .children()
        .filter(|n| n.has_tag_name("uniqueid"))
    {
        if let (Some(kind), Some(value)) = (node.attribute("type"), node.text()) {
            item.provider_ids
                .insert(kind.to_owned(), value.trim().to_owned());
        }
    }
    for (tag, kind) in [("tmdbid", "Tmdb"), ("tvdbid", "Tvdb"), ("imdbid", "Imdb")] {
        if let Some(value) = get(tag) {
            item.provider_ids.insert(kind.into(), value.trim().into());
        }
    }
    if let Some(name) = get("title").filter(|s| !s.trim().is_empty()) {
        item.name = name.into();
    }
    item.overview = get("plot").map(str::to_owned);
    if let Some(date) = get("dateadded") {
        item.date_created = Some(date.replace(' ', "T"));
    }
    item.community_rating = get("rating").and_then(|s| s.parse().ok());
    item.production_year = get("year")
        .and_then(|s| s.parse().ok())
        .or(item.production_year);
    item.run_time_ticks = get("runtime")
        .and_then(|s| s.parse::<u64>().ok())
        .map(|minutes| minutes.saturating_mul(600_000_000));
    item.index_number = get("episode").and_then(|s| s.parse().ok());
    item.parent_index_number = get("season").and_then(|s| s.parse().ok());
    item.genres = xml
        .root_element()
        .children()
        .filter(|n| n.has_tag_name("genre"))
        .filter_map(|n| n.text().map(str::to_owned))
        .collect();
}
fn episode_numbers(name: &str) -> Option<(u32, u32)> {
    let bytes = name.as_bytes();
    for i in 0..bytes.len() {
        if !matches!(bytes[i], b'S' | b's') {
            continue;
        }
        let mut j = i + 1;
        while j < bytes.len() && bytes[j].is_ascii_digit() {
            j += 1;
        }
        if j == i + 1 || j >= bytes.len() || !matches!(bytes[j], b'E' | b'e') {
            continue;
        }
        let mut k = j + 1;
        while k < bytes.len() && bytes[k].is_ascii_digit() {
            k += 1;
        }
        if k > j + 1 {
            return Some((name[i + 1..j].parse().ok()?, name[j + 1..k].parse().ok()?));
        }
    }
    None
}

// Only explicit filename metadata is removed; meaningful title words remain untouched.
fn tidy_title(item: &mut Item) {
    if let Some((title, suffix)) = item.name.rsplit_once(" (")
        && let Some(year) = suffix.strip_suffix(')').and_then(|s| s.parse::<u32>().ok())
        && (1888..=2100).contains(&year)
    {
        item.production_year.get_or_insert(year);
        item.name = title.to_owned();
    }
    if let Some((season, episode)) = item.parent_index_number.zip(item.index_number) {
        let parts: Vec<_> = item.name.split(" - ").collect();
        if let Some(index) = parts
            .iter()
            .position(|part| episode_numbers(part) == Some((season, episode)))
            && index + 1 < parts.len()
        {
            item.name = parts[index + 1..].join(" - ");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filename_metadata_is_separated_without_guessing_titles() {
        let mut movie = Item {
            name: "Moonwalker (1988)".into(),
            ..Default::default()
        };
        tidy_title(&mut movie);
        assert_eq!(movie.name, "Moonwalker");
        assert_eq!(movie.production_year, Some(1988));
        let mut episode = Item {
            name: "Dragon Ball - S01E001 - Bulma y Son Goku".into(),
            parent_index_number: Some(1),
            index_number: Some(1),
            ..Default::default()
        };
        tidy_title(&mut episode);
        assert_eq!(episode.name, "Bulma y Son Goku");
        let mut title = Item {
            name: "2001: A Space Odyssey".into(),
            ..Default::default()
        };
        tidy_title(&mut title);
        assert_eq!(title.name, "2001: A Space Odyssey");
        assert_eq!(title.production_year, None);
    }
    #[test]
    fn episode_numbers_are_extracted_only_from_explicit_metadata() {
        assert_eq!(episode_numbers("Serie.S01E12.1080p"), Some((1, 12)));
        assert_eq!(episode_numbers("Película 2026"), None);
        assert_eq!(episode_numbers("Especial.s00e02"), Some((0, 2)));
        assert!(is_video(Path::new("movie.MKV")));
        assert!(!is_video(Path::new("poster.png")));
    }
    #[test]
    fn image_folder_matches_titles_and_wins_over_sidecar_art() {
        let root = std::env::temp_dir().join(format!("megalodon-art-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for dir in [
            "Movies/Dead Zone (1989)",
            "Posters/Dragon Ball Z - Dead Zone (1989)",
            "Posters/Other (2001)",
        ] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        std::fs::write(
            root.join("Movies/Dead Zone (1989)/Dragon Ball Z - Dead Zone (1989).mkv"),
            [],
        )
        .unwrap();
        std::fs::write(root.join("Movies/Dead Zone (1989)/poster.jpg"), []).unwrap();
        for file in [
            "poster.jpg",
            "Background.PNG",
            "logo.png",
            "thumb.webp",
            "notes.txt",
        ] {
            std::fs::write(
                root.join("Posters/Dragon Ball Z - Dead Zone (1989)")
                    .join(file),
                [],
            )
            .unwrap();
        }
        let library = Files::load(
            sources::folder(&root, "Test").unwrap(),
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
        assert_eq!(library.art_folder, "Posters");
        assert!(library.art_folder_exists);
        let movie = library.items.iter().find(|i| i.kind == "Movie").unwrap();
        assert_eq!(library.items.len(), 1, "images are never treated as titles");
        let poster = &library.pictures[&(movie.id.clone(), "Primary".to_owned())];
        assert!(
            poster
                .replace('\\', "/")
                .contains("Posters/Dragon Ball Z - Dead Zone (1989)/poster.jpg")
        );
        for kind in ["Backdrop", "Logo"] {
            assert!(
                library
                    .pictures
                    .contains_key(&(movie.id.clone(), kind.to_owned())),
                "{kind}"
            );
        }
        assert_eq!(
            library.art_target(&movie.id, "Logo").unwrap(),
            "Posters/Dragon Ball Z - Dead Zone (1989)/logo.png"
        );
        let upload = root.join("upload.png");
        std::fs::write(&upload, [1, 2, 3]).unwrap();
        let target = library.art_target(&movie.id, "Logo").unwrap();
        library
            .save_art(&target, &upload, &AtomicBool::new(false))
            .unwrap();
        assert_eq!(std::fs::read(root.join(&target)).unwrap(), [1, 2, 3]);
        assert!(
            library
                .save_art("../escape.png", &upload, &AtomicBool::new(false))
                .is_err()
        );
        std::fs::remove_dir_all(&root).unwrap();
    }
    #[test]
    fn avatars_are_read_by_category_and_never_become_titles() {
        let paths: std::collections::HashSet<String> = [
            "posters/avatars/Anime/Goku.png",
            "posters/avatars/Anime/Vegeta_SSJ.jpg",
            "posters/avatars/Familia/Abuela.webp",
            "posters/avatars/notes.txt",
            "avatars/Solo.png",
            "posters/avatars/Deep/Inner/x.png",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        let found = collect_avatars(&paths, "posters");
        let labels: Vec<_> = found.iter().map(|(_, c, l)| format!("{c}/{l}")).collect();
        assert_eq!(
            labels,
            [
                "Anime/Goku",
                "Anime/Vegeta SSJ",
                "Familia/Abuela",
                "General/Solo"
            ]
        );
        let mut square = std::io::Cursor::new(Vec::new());
        image::RgbaImage::new(900, 600)
            .write_to(&mut square, image::ImageFormat::Png)
            .unwrap();
        let avatar = image::load_from_memory(&prepare_avatar(square.get_ref()).unwrap()).unwrap();
        assert_eq!((avatar.width(), avatar.height()), (512, 512));
    }
    #[test]
    fn uploads_are_validated_and_normalised() {
        let mut png = std::io::Cursor::new(Vec::new());
        image::RgbaImage::new(800, 450)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let jpeg = prepare_art(png.get_ref(), "Backdrop").unwrap();
        assert_eq!(
            image::guess_format(&jpeg).unwrap(),
            image::ImageFormat::Jpeg
        );
        let logo = prepare_art(png.get_ref(), "Logo").unwrap();
        assert_eq!(image::guess_format(&logo).unwrap(), image::ImageFormat::Png);
        assert!(prepare_art(b"not an image", "Primary").is_err());
        let mut tiny = std::io::Cursor::new(Vec::new());
        image::RgbaImage::new(40, 40)
            .write_to(&mut tiny, image::ImageFormat::Png)
            .unwrap();
        assert!(prepare_art(tiny.get_ref(), "Primary").is_err());
    }
    #[test]
    fn new_title_folders_are_safe_and_named_by_title_and_year() {
        assert_eq!(
            title_folder_name("Spider-Man: Brand New Day?", Some(2026)),
            "Spider-Man Brand New Day (2026)"
        );
        assert_eq!(title_folder_name("  ..  ", None), "Sin título");
        let paths: std::collections::HashSet<String> = [
            "Images/A (2000)/poster.jpg".to_owned(),
            "Movies/a.mkv".to_owned(),
        ]
        .into();
        assert_eq!(find_art_folder("", &paths), ("Images".to_owned(), true));
        assert_eq!(
            find_art_folder("Mis portadas/", &paths),
            ("Mis portadas".to_owned(), false)
        );
        let none: std::collections::HashSet<String> = ["Movies/a.mkv".to_owned()].into();
        assert_eq!(find_art_folder("", &none), ("posters".to_owned(), false));
        assert_eq!(
            title_key("Dragon Ball Z - Dead Zone (1989)"),
            ("dragonballzdeadzone".into(), Some(1989))
        );
    }
    #[test]
    fn local_library_uses_real_nfo_and_works_without_a_server() {
        let root =
            std::env::temp_dir().join(format!("megalodon-folder-test-{}", std::process::id()));
        std::fs::create_dir_all(root.join("Show/Season 01")).unwrap();
        std::fs::write(root.join("Show/Season 01/Show.S01E02.mkv"), []).unwrap();
        std::fs::write(root.join("Show/Season 01/Show.S01E02.nfo"),"<episodedetails><title>Episode two</title><plot>Real plot</plot><uniqueid type=\"tmdb\">12345</uniqueid><dateadded>2026-11-18 12:00:00</dateadded><season>1</season><episode>2</episode></episodedetails>").unwrap();
        let library = Files::load(
            sources::folder(&root, "Test").unwrap(),
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
        let series = library.items.iter().find(|i| i.kind == "Series").unwrap();
        let episodes = library.episodes(&series.id);
        assert_eq!(episodes[0].name, "Episode two");
        assert_eq!(episodes[0].index_number, Some(2));
        assert_eq!(
            episodes[0].provider_ids.get("tmdb").map(String::as_str),
            Some("12345")
        );
        assert_eq!(
            episodes[0].date_created.as_deref(),
            Some("2026-11-18T12:00:00")
        );
        assert!(
            library
                .playback(episodes[0].clone(), 42.0)
                .unwrap()
                .headers
                .is_empty()
        );
        assert_eq!(library.catalog(User::default()).items.len(), 1);
        std::fs::remove_dir_all(root).unwrap();
    }
}
