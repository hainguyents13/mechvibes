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

/// Stars `folder_path`, or takes the star off when it has one.
pub fn toggle_pin(pinned: &mut Vec<String>, folder_path: &str) {
    match pinned.iter().position(|id| id == folder_path) {
        Some(index) => {
            pinned.remove(index);
        }
        None => pinned.push(folder_path.to_string()),
    }
}

/// Whether `folder_path` is starred.
pub fn is_pinned(pinned: &[String], folder_path: &str) -> bool {
    pinned.iter().any(|id| id == folder_path)
}

/// Where a pack goes in a pack picker, smallest first: starred packs before
/// the rest, then the most recently added, then by name.
pub fn picker_order(
    pinned: bool,
    added_at: u64,
    name: &str
) -> (std::cmp::Reverse<bool>, std::cmp::Reverse<u64>, String) {
    (std::cmp::Reverse(pinned), std::cmp::Reverse(added_at), name.to_lowercase())
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

    #[test]
    fn a_star_goes_on_and_comes_off_and_touches_no_other_pack() {
        let mut pinned = vec!["keyboard/one".to_string()];

        toggle_pin(&mut pinned, "keyboard/two");
        assert!(is_pinned(&pinned, "keyboard/one"));
        assert!(is_pinned(&pinned, "keyboard/two"));

        toggle_pin(&mut pinned, "keyboard/one");
        assert!(!is_pinned(&pinned, "keyboard/one"));
        assert_eq!(pinned, ["keyboard/two"]);
    }

    #[test]
    fn starred_packs_come_first_then_the_newest_then_by_name() {
        let mut packs = vec![
            ("Zeta", false, 300u64),
            ("Old favorite", true, 100),
            ("alpha", false, 200),
            ("Beta", false, 200),
            ("New favorite", true, 400),
        ];
        packs.sort_by_key(|(name, pinned, added)| picker_order(*pinned, *added, name));
        let names: Vec<_> = packs.iter().map(|(name, ..)| *name).collect();
        assert_eq!(names, ["New favorite", "Old favorite", "Zeta", "alpha", "Beta"]);
    }
}
