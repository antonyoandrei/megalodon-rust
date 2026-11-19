//! Saved source credentials and one-time migration to app-owned profiles.
use super::{
    data,
    jellyfin::{Config, User, valid_id},
};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Serialize, Deserialize)]
struct SavedSession {
    server: String,
    user: String,
    name: String,
    image_tag: Option<String>,
    protected_token: Vec<u8>,
    #[serde(default)]
    legacy_admin: bool,
    #[serde(default)]
    administrator: bool,
}

fn read(dir: &Path) -> Vec<SavedSession> {
    std::fs::read(dir.join("profile-sessions.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}
fn same_server(a: &str, b: &str) -> bool {
    a.trim_end_matches('/') == b.trim_end_matches('/')
}

pub fn remember(dir: &Path, config: &Config, user: &User) -> Result<(), String> {
    if !valid_id(&user.id)
        || user.policy.is_disabled
        || config.user_id.as_ref().is_some_and(|id| id != &user.id)
    {
        return Err("La sesión no corresponde a este perfil.".into());
    }
    let mut sessions = read(dir);
    let legacy_admin = (config.user_id.is_none() && user.policy.is_administrator)
        || sessions.iter().any(|s| {
            s.legacy_admin
                && same_server(&s.server, &config.base_url)
                && s.user == user.id
                && user.policy.is_administrator
                && data::protect(&s.protected_token, true)
                    .is_ok_and(|bytes| bytes == config.token.as_bytes())
        });
    sessions.retain(|s| !(same_server(&s.server, &config.base_url) && s.user == user.id));
    sessions.push(SavedSession {
        server: config.base_url.clone(),
        user: user.id.clone(),
        name: user.name.clone(),
        image_tag: user.primary_image_tag.clone(),
        protected_token: data::protect(config.token.as_bytes(), false)?,
        // The original launcher supplied an API key, pinned to its existing administrator.
        // It is never reused to authenticate a different profile.
        legacy_admin,
        administrator: user.policy.is_administrator,
    });
    std::fs::create_dir_all(dir).map_err(|_| "No se pudo guardar la sesión.")?;
    let temporary = dir.join("profile-sessions.tmp");
    std::fs::write(
        &temporary,
        serde_json::to_vec(&sessions).map_err(|_| "Sesión no válida.")?,
    )
    .map_err(|_| "No se pudo guardar la sesión.")?;
    std::fs::rename(temporary, dir.join("profile-sessions.json"))
        .map_err(|_| "No se pudo guardar la sesión.".into())
}

// Kept separate from profile sessions. Household switching uses it only to authorize
// Jellyfin Quick Connect; the selected profile always receives its own user token.
pub fn management_token(dir: &Path, server: &str) -> Option<String> {
    read(dir).into_iter().find_map(|session| {
        if same_server(&session.server, server) && (session.legacy_admin || session.administrator) {
            data::protect(&session.protected_token, true)
                .ok()
                .and_then(|bytes| String::from_utf8(bytes).ok())
        } else {
            None
        }
    })
}

pub fn forget_server(dir: &Path, server: &str) -> Result<(), String> {
    let mut sessions = read(dir);
    sessions.retain(|s| !same_server(&s.server, server));
    let temporary = dir.join("profile-sessions.tmp");
    std::fs::write(&temporary, serde_json::to_vec(&sessions).unwrap())
        .and_then(|_| std::fs::rename(temporary, dir.join("profile-sessions.json")))
        .map_err(|_| "No se pudieron quitar las sesiones de esta conexión.".into())
}

// Import names and chosen images, never passwords or server privileges into a profile.
pub fn migrate_local_profiles(preferences: &mut data::Preferences) {
    if preferences.native_profiles {
        return;
    }
    let active = data::load_connection().and_then(|c| c.user_id);
    for saved in read(&data::data_dir()) {
        let id = format!("imported-{}", saved.user);
        preferences
            .local_profiles
            .entry(id.clone())
            .or_insert(saved.name);
        let legacy = format!("{}|{}", saved.server.trim_end_matches('/'), saved.user);
        if let Some(avatar) = preferences
            .avatars
            .get(&legacy)
            .cloned()
            .or_else(|| super::avatars::stored(&saved.user))
        {
            preferences
                .avatars
                .entry(format!("local|{id}"))
                .or_insert(avatar);
        }
        if let Some(settings) = preferences.profile_settings.get(&legacy).cloned() {
            preferences
                .profile_settings
                .entry(format!("local|{id}"))
                .or_insert(settings);
        }
        let prefix = format!("{legacy}|");
        let destination = format!("{}|{id}|", saved.server.trim_end_matches('/'));
        let watches: Vec<_> = preferences
            .history
            .iter()
            .filter_map(|(key, w)| {
                key.strip_prefix(&prefix)
                    .map(|suffix| (format!("{destination}{suffix}"), w.clone()))
            })
            .collect();
        for (key, w) in watches {
            preferences.history.entry(key).or_insert(w);
        }
        let favorites: Vec<_> = preferences
            .favorites
            .iter()
            .filter_map(|(key, f)| {
                key.strip_prefix(&prefix)
                    .map(|suffix| (format!("{destination}{suffix}"), *f))
            })
            .collect();
        for (key, f) in favorites {
            preferences.favorites.entry(key).or_insert(f);
        }
        if active.as_deref() == Some(&saved.user) && preferences.local_profile == "local" {
            preferences.local_profile = id;
        }
    }
    preferences.native_profiles = true;
    let _ = data::save_json("preferences.json", preferences);
}
