//! Avatar choices are generic: a built-in colour (`color-N`) or an image from the
//! person's own library (`library:<path>`). No third-party artwork is bundled.
use std::path::PathBuf;

/// Accepts only the generic forms; anything else (including retired bundled
/// presets) falls back to the initial.
pub fn normalize(id: &str) -> Option<String> {
    let id = id.trim();
    let color = id
        .strip_prefix("color-")
        .and_then(|n| n.parse::<u8>().ok())
        .is_some_and(|n| n < 8);
    let library = id.strip_prefix("library:").is_some_and(|path| {
        !path.is_empty()
            && !path.contains(['\\', '\0'])
            && !path
                .split('/')
                .any(|s| s.is_empty() || s == "." || s == "..")
    });
    (color || library).then(|| id.to_owned())
}

/// A choice saved by an earlier Megalodon, when it is still a valid generic id.
pub fn stored(user_id: &str) -> Option<String> {
    let mut paths = vec![PathBuf::from(".local/profile-avatars.json")];
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        paths.push(PathBuf::from(local).join("Megalodon/app/profile-avatars.json"));
    }
    if let Some(dir) = std::env::var_os("MEGALODON_DATA_DIR") {
        paths.insert(0, PathBuf::from(dir).join("profile-avatars.json"));
    }
    paths.into_iter().find_map(|path| {
        let data: serde_json::Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
        normalize(data["profiles"][user_id]["avatarId"].as_str()?)
    })
}

/// No bundled images remain; the web prototype shows its neutral fallback.
#[cfg_attr(feature = "desktop", allow(dead_code))]
pub fn bytes(_id: &str) -> Option<Vec<u8>> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_generic_avatar_ids_are_accepted() {
        assert_eq!(normalize("color-3").as_deref(), Some("color-3"));
        assert!(normalize("color-9").is_none());
        assert_eq!(
            normalize("library:posters/avatars/Anime/Goku.png").as_deref(),
            Some("library:posters/avatars/Anime/Goku.png")
        );
        assert!(normalize("library:../secrets.png").is_none());
        assert!(normalize("library:a\\b.png").is_none());
        assert!(normalize("pokemon-gengar").is_none());
        assert!(bytes("color-1").is_none());
    }
}
