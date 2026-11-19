//! Saved connections contain credentials protected by Windows; removing one never removes media.
use super::{data, jellyfin::Config};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Serialize, Deserialize)]
pub struct Source {
    pub id: String,
    pub name: String,
    pub kind: Kind,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Kind {
    Jellyfin {
        server: String,
        user: Option<String>,
        credential: Vec<u8>,
    },
    Folder {
        path: PathBuf,
    },
    Drive {
        executable: PathBuf,
        config: PathBuf,
        remote: String,
    },
    WebDav {
        url: String,
        user: String,
        credential: Vec<u8>,
    },
}
impl Source {
    pub fn description(&self) -> String {
        match &self.kind {
            Kind::Jellyfin { server, .. } => server.clone(),
            Kind::Folder { path } => path.display().to_string(),
            Kind::Drive { remote, .. } => format!("Google Drive · {remote}"),
            Kind::WebDav { url, .. } => url.clone(),
        }
    }
    pub fn config(&self) -> Result<Option<Config>, String> {
        let Kind::Jellyfin {
            server,
            user,
            credential,
        } = &self.kind
        else {
            return Ok(None);
        };
        Ok(Some(Config {
            base_url: server.clone(),
            user_id: user.clone(),
            token: String::from_utf8(data::protect(credential, true)?)
                .map_err(|_| "Sesión no válida")?,
            port: 3210,
        }))
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Store {
    pub active: Option<String>,
    pub entries: Vec<Source>,
}
impl Store {
    pub fn load() -> Result<Self, String> {
        let path = data::data_dir().join("connections.json");
        if path.exists() {
            return serde_json::from_slice(
                &std::fs::read(path).map_err(|_| "No se pudieron leer las conexiones")?,
            )
            .map_err(|_| {
                "No se pudieron abrir las conexiones guardadas; se conservan sin cambios.".into()
            });
        }
        let mut store = Self::default();
        if let Some(config) = data::load_connection()
            .or_else(|| Config::load().ok())
            .filter(|c| !c.token.is_empty())
        {
            store.remember(&config, "Jellyfin")?;
        } else {
            store.save()?;
        }
        Ok(store)
    }
    pub fn current(&self) -> Option<&Source> {
        self.entries
            .iter()
            .find(|s| Some(&s.id) == self.active.as_ref())
    }
    pub fn save(&self) -> Result<(), String> {
        data::save_json("connections.json", self)
    }
    pub fn remember(&mut self, config: &Config, name: &str) -> Result<(), String> {
        let server = config.base_url.trim_end_matches('/').to_owned();
        let id = format!("server-{}", stable_id(&server));
        let previous_name = self
            .entries
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.name.clone());
        let source = Source {
            id: id.clone(),
            name: previous_name.unwrap_or_else(|| name.into()),
            kind: Kind::Jellyfin {
                server,
                user: config.user_id.clone(),
                credential: data::protect(config.token.as_bytes(), false)?,
            },
        };
        let mut next = self.clone();
        next.entries.retain(|s| s.id != id);
        next.entries.push(source);
        next.active = Some(id);
        next.save()?;
        *self = next;
        Ok(())
    }
    pub fn add(&mut self, source: Source) -> Result<(), String> {
        if self.entries.iter().any(|s| s.id == source.id) {
            return Err("Esta conexión ya está añadida.".into());
        }
        let mut next = self.clone();
        next.entries.push(source);
        next.save()?;
        *self = next;
        Ok(())
    }
    pub fn remove(&mut self, id: &str) -> Result<(), String> {
        let mut next = self.clone();
        next.entries.retain(|s| s.id != id);
        if next.active.as_deref() == Some(id) {
            next.active = None;
        }
        next.save()?;
        *self = next;
        Ok(())
    }
}
pub fn stable_id(value: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}
pub fn folder(path: &Path, name: &str) -> Result<Source, String> {
    let path = path
        .canonicalize()
        .map_err(|_| "La carpeta no está disponible.")?;
    if !path.is_dir() {
        return Err("Selecciona una carpeta, no un archivo.".into());
    }
    let name = if name.trim().is_empty() {
        path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    } else {
        name.trim().into()
    };
    Ok(Source {
        id: format!(
            "folder-{}",
            stable_id(&path.to_string_lossy().to_lowercase())
        ),
        name,
        kind: Kind::Folder { path },
    })
}

#[derive(Default)]
pub struct Editor {
    pub open: bool,
    /// Drive pane shows the existing-rclone form instead of browser sign-in.
    pub existing: bool,
    pub kind: usize,
    pub name: String,
    pub path: String,
    pub remote: String,
    pub config: String,
    pub endpoint: String,
    pub username: String,
    pub password: String,
    pub error: Option<String>,
    pub busy: bool,
    pub request: u64,
    pub remove: Option<String>,
    pub cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encrypted_connections_keep_independent_servers() {
        let a = Source {
            id: "a".into(),
            name: "Casa".into(),
            kind: Kind::Jellyfin {
                server: "https://one.example".into(),
                user: Some("u1".into()),
                credential: data::protect(b"private-test-session", false).unwrap(),
            },
        };
        let b = Source {
            id: "b".into(),
            name: "Otra".into(),
            kind: Kind::Folder {
                path: PathBuf::from("D:/Movies"),
            },
        };
        let store = Store {
            active: Some("a".into()),
            entries: vec![a, b],
        };
        let bytes = serde_json::to_vec(&store).unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("private-test-session"));
        let restored: Store = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            restored
                .current()
                .unwrap()
                .config()
                .unwrap()
                .unwrap()
                .user_id
                .as_deref(),
            Some("u1")
        );
        assert!(restored.entries[1].config().unwrap().is_none());
        assert_ne!(stable_id("folder-a|movie"), stable_id("folder-b|movie"));
    }
}
