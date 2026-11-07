//! "Added at" timestamps and New-badge state for the soundpack library.
//!
//! Both live in `AppConfig` rather than in `SoundpackMetadata`, because
//! `SoundpackCache::refresh_from_directory` clears its map and rebuilds it from
//! disk on every scan; anything kept beside the scanned metadata would not
//! survive. Config is also the right home for them on their own terms: they are
//! user-facing state, not facts about the files.

use std::collections::HashMap;

/// Sort orders offered by the soundpack lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundpackSort {
    /// Most recently added first.
    RecentlyAdded,
    /// A-Z by display name, case-insensitive.
    Name,
}

impl SoundpackSort {
    pub fn label(self) -> &'static str {
        match self {
            SoundpackSort::RecentlyAdded => "Recently added",
            SoundpackSort::Name => "Name",
        }
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime
        ::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Records `folder_path` as added now, unless it already has a timestamp.
///
/// Called on import. Returns whether the config changed, so callers can skip a
/// write when nothing moved.
pub fn mark_added(added_at: &mut HashMap<String, u64>, folder_path: &str) -> bool {
    if added_at.contains_key(folder_path) {
        return false;
    }
    added_at.insert(folder_path.to_string(), now_secs());
    true
}

/// Backfills timestamps for packs that predate this feature.
///
/// The first run after upgrading sees a library with no timestamps at all.
/// Stamping those with "now" would light up every pack as New, so they are
/// instead recorded as seen: only packs that arrive afterwards are new. Packs
/// installed later take the `mark_added` path above and do get a badge.
///
/// Returns whether anything changed.
pub fn backfill_existing(
    added_at: &mut HashMap<String, u64>,
    seen: &mut Vec<String>,
    known_folder_paths: &[String]
) -> bool {
    let stamp = now_secs();
    let mut changed = false;

    for folder_path in known_folder_paths {
        if added_at.contains_key(folder_path) {
            continue;
        }
        added_at.insert(folder_path.clone(), stamp);
        if !seen.iter().any(|id| id == folder_path) {
            seen.push(folder_path.clone());
        }
        changed = true;
    }

    changed
}

/// Clears the New badge for `folder_path`. Returns whether anything changed.
pub fn mark_seen(seen: &mut Vec<String>, folder_path: &str) -> bool {
    if seen.iter().any(|id| id == folder_path) {
        return false;
    }
    seen.push(folder_path.to_string());
    true
}

/// Whether `folder_path` should carry a New badge.
pub fn is_new(seen: &[String], folder_path: &str) -> bool {
    !seen.iter().any(|id| id == folder_path)
}

/// "keyboard" or "mouse", from a pack id such as "keyboard/cherrymx-red-abs".
fn kind_of(folder_path: &str) -> &str {
    folder_path.split('/').next().unwrap_or_default()
}

/// The ids of the packs that ship with the app, read once from the built-in
/// soundpacks folder.
pub fn builtin_pack_ids() -> &'static [String] {
    static IDS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    IDS.get_or_init(|| {
        let root = crate::state::paths::soundpacks::get_builtin_soundpacks_dir();
        let mut ids = Vec::new();
        for kind in ["keyboard", "mouse"] {
            let Ok(entries) = std::fs::read_dir(root.join(kind)) else {
                continue;
            };
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    ids.push(format!("{}/{}", kind, entry.file_name().to_string_lossy()));
                }
            }
        }
        ids
    })
}

/// Whether `folder_path` is starred. A list (keyboard or mouse) with no star
/// of its own counts its built-in packs as starred, so the pack list on Home
/// is never empty.
pub fn is_starred(stored: &[String], builtin: &[String], folder_path: &str) -> bool {
    let kind = kind_of(folder_path);
    let has_own_stars = stored.iter().any(|id| kind_of(id) == kind);
    if has_own_stars {
        stored.iter().any(|id| id == folder_path)
    } else {
        builtin.iter().any(|id| id == folder_path)
    }
}

/// Stars `folder_path`, or takes the star off when it has one. The first
/// change to a list with no star of its own starts from its built-in packs,
/// which were showing as starred.
pub fn toggle_star(stored: &mut Vec<String>, builtin: &[String], folder_path: &str) {
    let kind = kind_of(folder_path);
    if !stored.iter().any(|id| kind_of(id) == kind) {
        stored.extend(builtin.iter().filter(|id| kind_of(id) == kind).cloned());
    }
    match stored.iter().position(|id| id == folder_path) {
        Some(index) => {
            stored.remove(index);
        }
        None => stored.push(folder_path.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pack_is_stamped_once_and_keeps_its_first_timestamp() {
        let mut added = HashMap::new();

        assert!(mark_added(&mut added, "keyboard/one"));
        let first = added["keyboard/one"];

        // A second import of the same folder must not restamp it, otherwise
        // reinstalling a pack would move it to the top of "recently added".
        assert!(!mark_added(&mut added, "keyboard/one"));
        assert_eq!(added["keyboard/one"], first);
    }

    #[test]
    fn upgrading_an_existing_library_produces_no_badges() {
        let mut added = HashMap::new();
        let mut seen = Vec::new();
        let existing = vec!["keyboard/one".to_string(), "mouse/two".to_string()];

        assert!(backfill_existing(&mut added, &mut seen, &existing));

        // The whole point: nobody upgrading finds their library covered in
        // New badges.
        assert!(!is_new(&seen, "keyboard/one"));
        assert!(!is_new(&seen, "mouse/two"));

        // A pack imported after the backfill still gets one.
        assert!(mark_added(&mut added, "keyboard/three"));
        assert!(is_new(&seen, "keyboard/three"));
    }

    #[test]
    fn backfill_leaves_an_already_stamped_pack_alone() {
        let mut added = HashMap::new();
        let mut seen = Vec::new();
        mark_added(&mut added, "keyboard/new-arrival");
        let stamped = added["keyboard/new-arrival"];

        let changed = backfill_existing(
            &mut added,
            &mut seen,
            &["keyboard/new-arrival".to_string()]
        );

        // It was imported through the normal path, so it keeps both its
        // timestamp and its badge.
        assert!(!changed);
        assert_eq!(added["keyboard/new-arrival"], stamped);
        assert!(is_new(&seen, "keyboard/new-arrival"));
    }

    #[test]
    fn selecting_a_pack_clears_its_badge_and_only_its_badge() {
        let mut seen = Vec::new();

        assert!(mark_seen(&mut seen, "keyboard/one"));
        assert!(!is_new(&seen, "keyboard/one"));
        assert!(is_new(&seen, "keyboard/two"));

        // Idempotent: selecting the same pack again is not a config change.
        assert!(!mark_seen(&mut seen, "keyboard/one"));
    }

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|id| id.to_string()).collect()
    }

    #[test]
    fn with_no_stars_the_built_in_packs_are_starred() {
        let builtin = ids(&["keyboard/red", "mouse/chat"]);
        assert!(is_starred(&[], &builtin, "keyboard/red"));
        assert!(is_starred(&[], &builtin, "mouse/chat"));
        assert!(!is_starred(&[], &builtin, "keyboard/downloaded"));
    }

    #[test]
    fn each_list_falls_back_on_its_own() {
        // Starring a keyboard pack must not leave the mouse list empty.
        let builtin = ids(&["keyboard/red", "mouse/chat"]);
        let stored = ids(&["keyboard/downloaded"]);
        assert!(is_starred(&stored, &builtin, "keyboard/downloaded"));
        assert!(!is_starred(&stored, &builtin, "keyboard/red"), "keyboard has its own stars now");
        assert!(is_starred(&stored, &builtin, "mouse/chat"), "mouse still has none of its own");
    }

    #[test]
    fn the_first_star_keeps_the_built_in_ones_that_were_showing() {
        let builtin = ids(&["keyboard/red", "keyboard/blue", "mouse/chat"]);
        let mut stored = Vec::new();

        toggle_star(&mut stored, &builtin, "keyboard/downloaded");
        assert_eq!(stored, ids(&["keyboard/red", "keyboard/blue", "keyboard/downloaded"]));

        // Taking a star off a built-in pack only removes that one.
        let mut stored = Vec::new();
        toggle_star(&mut stored, &builtin, "keyboard/red");
        assert_eq!(stored, ids(&["keyboard/blue"]));
        assert!(!is_starred(&stored, &builtin, "keyboard/red"));
        assert!(is_starred(&stored, &builtin, "mouse/chat"), "the mouse list is untouched");
    }

    #[test]
    fn a_star_comes_off_again() {
        let builtin = ids(&["keyboard/red"]);
        let mut stored = ids(&["keyboard/one", "keyboard/two"]);
        toggle_star(&mut stored, &builtin, "keyboard/one");
        assert_eq!(stored, ids(&["keyboard/two"]));
        toggle_star(&mut stored, &builtin, "keyboard/one");
        assert!(is_starred(&stored, &builtin, "keyboard/one"));
    }
}
