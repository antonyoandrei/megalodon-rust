use super::{
    jellyfin::{Item, UserData},
    *,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Watch {
    pub item: Item,
    pub position: f64,
    pub duration: f64,
    pub updated: u64,
    pub complete: bool,
}
impl Watch {
    pub fn new(mut item: Item, position: f64, duration: f64, ended: bool) -> Self {
        let position = if position.is_finite() {
            position.max(0.0)
        } else {
            0.0
        };
        let duration = if duration.is_finite() {
            duration.max(0.0)
        } else {
            0.0
        };
        let complete = ended || (duration > 0.0 && position / duration >= 0.95);
        if duration > 0.0 {
            item.run_time_ticks = Some((duration * 10_000_000.0) as u64);
        }
        let data = item.user_data.get_or_insert_with(UserData::default);
        data.played = complete;
        data.playback_position_ticks = if complete {
            0
        } else {
            (position * 10_000_000.0) as u64
        };
        Self {
            item,
            position: if complete { 0.0 } else { position },
            duration,
            complete,
            updated: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
}
pub fn resumable(item: &Item) -> bool {
    item.user_data.as_ref().is_some_and(|d| {
        !d.played
            && d.playback_position_ticks > 0
            && item.run_time_ticks.is_none_or(|total| {
                total == 0 || d.playback_position_ticks as f64 / (total as f64) < 0.95
            })
    })
}
fn same_title(a: &str, b: &str) -> bool {
    let normalize = |s: &str| {
        let s = s
            .rsplit_once(" (")
            .filter(|(_, year)| {
                year.len() == 5
                    && year.ends_with(')')
                    && year[..4].bytes().all(|c| c.is_ascii_digit())
            })
            .map_or(s, |(title, _)| title);
        s.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect::<String>()
    };
    !a.is_empty() && !b.is_empty() && normalize(a) == normalize(b)
}
fn same_work(a: &Item, b: &Item) -> bool {
    if a.kind != b.kind {
        return false;
    }
    if a.kind == "Episode" {
        return a.parent_index_number.is_some()
            && a.index_number.is_some()
            && a.parent_index_number == b.parent_index_number
            && a.index_number == b.index_number
            && same_title(
                a.series_name.as_deref().unwrap_or(""),
                b.series_name.as_deref().unwrap_or(""),
            );
    }
    let shared_id = a.provider_ids.iter().any(|(k, v)| {
        !v.is_empty()
            && b.provider_ids
                .iter()
                .any(|(bk, bv)| k.eq_ignore_ascii_case(bk) && v == bv)
    });
    shared_id
        || (same_title(&a.name, &b.name)
            && a.production_year.is_some()
            && a.production_year == b.production_year)
}
// One-time bridge from already-saved server history to the same files in a direct source.
// Exact, unique matches only. Existing direct-source progress is never overwritten.
pub fn import_direct_history(preferences: &mut data::Preferences, source: &str, items: &[Item]) {
    if preferences.imported_history_sources.contains(source) {
        return;
    }
    let old = preferences.history.clone();
    for profile in preferences.local_profiles.keys() {
        let suffix = format!("|{profile}");
        for (key, watch) in &old {
            let Some((scope, _)) = key.rsplit_once('|') else {
                continue;
            };
            if !scope.ends_with(&suffix)
                || !(scope.starts_with("http://") || scope.starts_with("https://"))
            {
                continue;
            }
            let mut candidates = items.iter().filter(|i| same_work(&watch.item, i));
            let Some(item) = candidates.next() else {
                continue;
            };
            if candidates.next().is_some() {
                continue;
            }
            let destination = format!("{source}|{profile}|{}", item.id);
            let mut imported =
                Watch::new(item.clone(), watch.position, watch.duration, watch.complete);
            imported.updated = watch.updated;
            preferences
                .history
                .entry(destination.clone())
                .or_insert(imported);
            if let Some(favorite) = preferences.favorites.get(key).copied() {
                preferences.favorites.entry(destination).or_insert(favorite);
            }
        }
    }
    preferences.imported_history_sources.insert(source.into());
}
fn apply_profile_state(item: &mut Item, preferences: &data::Preferences, scope: &str) {
    let key = format!("{scope}|{}", item.id);
    item.user_data = Some(UserData::default());
    if let Some(watch) = preferences.history.get(&key) {
        item.user_data = watch.item.user_data.clone();
        item.run_time_ticks = watch.item.run_time_ticks.or(item.run_time_ticks);
    }
    item.user_data
        .get_or_insert_with(UserData::default)
        .is_favorite = preferences.favorites.get(&key).copied().unwrap_or(false);
}
impl Megalodon {
    pub(super) fn record_watch(&mut self, item: Item, position: f64, duration: f64, ended: bool) {
        if self.no_report || item.id.is_empty() {
            return;
        }
        if self.catalog.is_none() {
            return;
        }
        let key = format!(
            "{}|{}",
            self.profile_key(&self.preferences.local_profile),
            item.id
        );
        let watch = Watch::new(item, position, duration, ended);
        self.preferences.positions.remove(&key);
        self.update_item(&watch.item);
        self.resume_items.retain(|i| i.id != watch.item.id);
        if resumable(&watch.item) {
            self.resume_items.insert(0, watch.item.clone());
        }
        self.preferences.history.insert(key, watch);
    }
    pub(super) fn update_item(&mut self, item: &Item) {
        if let Some(catalog) = &mut self.catalog {
            let catalog = Arc::make_mut(catalog);
            if let Some(old) = catalog.items.iter_mut().find(|i| i.id == item.id) {
                *old = item.clone();
            }
        }
        if self.detail.as_ref().is_some_and(|i| i.id == item.id) {
            self.detail = Some(item.clone());
        }
        if let Some(old) = self.episodes.iter_mut().find(|i| i.id == item.id) {
            *old = item.clone();
        }
        if let Some(old) = self.resume_items.iter_mut().find(|i| i.id == item.id) {
            *old = item.clone();
        }
    }
    pub(super) fn local_state(&self, item: &mut Item) {
        if let Some(patch) = self.metadata.patches.get(&item.id) {
            patch.apply(item);
        }
        for (id, kind) in self.metadata.uploads.keys() {
            if id == &item.id {
                if kind == "Backdrop" {
                    if !item.backdrop_image_tags.iter().any(|t| t == "local") {
                        item.backdrop_image_tags.insert(0, "local".into());
                    }
                } else {
                    item.image_tags.insert(kind.clone(), "local".into());
                }
            }
        }
        let scope = self.profile_key(&self.preferences.local_profile);
        apply_profile_state(item, &self.preferences, &scope);
    }
    pub(super) fn refresh_profile_state(&mut self) {
        if let Some(catalog) = self.catalog.take() {
            let mut catalog = (*catalog).clone();
            for item in &mut catalog.items {
                self.local_state(item);
            }
            self.catalog = Some(Arc::new(catalog));
        }
        let prefix = format!("{}|", self.profile_key(&self.preferences.local_profile));
        let mut watched: Vec<_> = self
            .preferences
            .history
            .iter()
            .filter(|(key, watch)| key.starts_with(&prefix) && resumable(&watch.item))
            .map(|(_, w)| w.clone())
            .collect();
        watched.sort_by_key(|w| std::cmp::Reverse(w.updated));
        self.resume_items = watched
            .into_iter()
            .filter_map(|mut w| {
                let exists = if let Some(files) = &self.files {
                    if let Some(current) = files.items.iter().find(|i| i.id == w.item.id) {
                        w.item = current.clone();
                    }
                    files.media.contains_key(&w.item.id)
                } else {
                    self.catalog.as_ref().is_some_and(|c| {
                        c.items
                            .iter()
                            .any(|i| i.id == w.item.id || Some(&i.id) == w.item.series_id.as_ref())
                    })
                };
                if !exists {
                    return None;
                }
                self.local_state(&mut w.item);
                Some(w.item)
            })
            .collect();
    }
    pub(super) fn toggle_favorite(&mut self, _ctx: &Context, mut item: Item) {
        let favorite = !item.user_data.as_ref().is_some_and(|d| d.is_favorite);
        let key = format!(
            "{}|{}",
            self.profile_key(&self.preferences.local_profile),
            item.id
        );
        let previous = self.preferences.favorites.insert(key.clone(), favorite);
        if let Err(e) = self.save_preferences() {
            if let Some(old) = previous {
                self.preferences.favorites.insert(key, old);
            } else {
                self.preferences.favorites.remove(&key);
            }
            self.error = Some(e);
            return;
        }
        item.user_data
            .get_or_insert_with(UserData::default)
            .is_favorite = favorite;
        self.update_item(&item);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn direct_migration_keeps_people_separate_and_rejects_ambiguous_episodes() {
        let mut p: data::Preferences = serde_json::from_str("{}").unwrap();
        p.local_profiles.insert("alice".into(), "Alice".into());
        p.local_profiles.insert("bob".into(), "Bob".into());
        let old = Item {
            id: "server-id".into(),
            kind: "Episode".into(),
            series_name: Some("Dragon Ball".into()),
            parent_index_number: Some(1),
            index_number: Some(2),
            ..Default::default()
        };
        let direct = Item {
            id: "file-id".into(),
            series_name: Some("Dragon Ball (1986)".into()),
            ..old.clone()
        };
        p.history.insert(
            "http://localhost|alice|server-id".into(),
            Watch::new(old, 42.0, 900.0, false),
        );
        import_direct_history(&mut p, "drive", std::slice::from_ref(&direct));
        assert_eq!(p.history["drive|alice|file-id"].position, 42.0);
        assert!(!p.history.contains_key("drive|bob|file-id"));
        let duplicate = Item {
            id: "duplicate".into(),
            ..direct.clone()
        };
        import_direct_history(&mut p, "another", &[direct, duplicate]);
        assert!(!p.history.keys().any(|k| k.starts_with("another|")));
    }
    #[test]
    fn profiles_and_sources_never_inherit_each_others_progress_or_server_state() {
        let mut preferences: data::Preferences = serde_json::from_str("{}").unwrap();
        let mut item = Item {
            id: "film".into(),
            user_data: Some(UserData {
                is_favorite: true,
                played: true,
                playback_position_ticks: 900_000_000,
            }),
            ..Default::default()
        };
        preferences.history.insert(
            "drive|alice|film".into(),
            Watch::new(item.clone(), 42.25, 180.0, false),
        );
        preferences
            .favorites
            .insert("drive|alice|film".into(), true);
        apply_profile_state(&mut item, &preferences, "drive|alice");
        assert!(item.user_data.as_ref().unwrap().is_favorite);
        assert_eq!(
            item.user_data.as_ref().unwrap().playback_position_ticks,
            422_500_000
        );
        for scope in ["drive|bob", "folder|alice", "jellyfin|alice"] {
            apply_profile_state(&mut item, &preferences, scope);
            let state = item.user_data.as_ref().unwrap();
            assert!(!state.is_favorite && !state.played);
            assert_eq!(state.playback_position_ticks, 0);
        }
        apply_profile_state(&mut item, &preferences, "drive|alice");
        assert_eq!(
            item.user_data.as_ref().unwrap().playback_position_ticks,
            422_500_000
        );
    }
    #[test]
    fn finished_titles_leave_resume_and_unknown_lengths_stay_resumable() {
        let item = Item {
            id: "test".into(),
            kind: "Episode".into(),
            ..Default::default()
        };
        assert!(resumable(&Watch::new(item.clone(), 42.25, 0.0, false).item));
        assert!(!resumable(
            &Watch::new(item.clone(), 96.0, 100.0, false).item
        ));
        assert!(!resumable(&Watch::new(item.clone(), 42.0, 0.0, true).item));
        assert_eq!(
            Watch::new(item, 42.25, 100.0, false)
                .item
                .user_data
                .unwrap()
                .playback_position_ticks,
            422_500_000
        );
    }
}
