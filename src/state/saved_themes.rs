//! The themes the user can switch between: their own, and the ones installed
//! from a file or the theme library. Kept in `saved_themes.json`, apart from
//! the config, so a damaged entry or a newer app never costs anything else.
//!
//! The file is read as untrusted, one entry at a time. An entry this app
//! cannot read, or whose values could break out of the style they end up in,
//! is kept exactly as written and left out of the list. An entry nobody
//! changed is written back as it was read, and fields or top-level keys a
//! newer Mechvibes added survive a save. Only a missing file counts as an
//! empty list: one that cannot be read, or that a newer Mechvibes wrote, is
//! never written over this session.

use crate::state::config::{ preserve_corrupt_config, LogoCustomization, BackgroundCustomization, Preserved };
use crate::state::paths;
use crate::utils::data;
use crate::utils::deep_link::is_theme_id;
use crate::utils::notice::{ self, NoticeAction, NoticeKind };
use crate::utils::soundpack_validator::is_portable_name;
use crate::utils::theme_share::{ clean_name, safe_color, safe_theme_css, Look, LookColors, SharedColors };
use chrono::{ DateTime, Utc };
use serde::{ Deserialize, Serialize };
use serde_json::{ Map, Value };
use std::path::{ Path, PathBuf };
use std::sync::atomic::{ AtomicU64, Ordering };
use std::sync::{ Mutex, MutexGuard, OnceLock };

/// The layout of `saved_themes.json` this app writes. A file of a later
/// layout is shown but never written.
const FORMAT: u64 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeKind {
    /// Made by the user. Edits change it in place.
    Mine,
    /// Installed from a `.mvtheme` file. Edits make a copy.
    Imported,
    /// Installed from the theme library. Edits make a copy, and installing it
    /// again updates it in place.
    Library,
}

/// One saved theme. A `None` logo or background is a part the theme has off.
///
/// No container-level `#[serde(default)]`: an entry without an id, a name, a
/// kind or colors is kept as written and listed as unreadable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedTheme {
    pub id: String,
    pub name: String,
    pub kind: ThemeKind,
    /// The theme library's id (`th-` and 32 hex), for a `Library` theme.
    pub library_id: Option<String>,
    pub library_checksum: Option<String>,
    /// Tells the same file imported twice apart from another one.
    pub fingerprint: Option<String>,
    pub colors: SharedColors,
    pub logo: Option<LogoCustomization>,
    pub background: Option<BackgroundCustomization>,
    #[serde(default)]
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub updated_at: DateTime<Utc>,
    /// Fields written by a newer Mechvibes, kept so a save never drops them.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl SavedTheme {
    /// What putting this theme on shows.
    pub fn look(&self) -> Look {
        Look {
            colors: LookColors::Known(self.colors.clone()),
            logo: self.logo.clone(),
            background: self.background.clone(),
        }
    }
}

/// Why the list cannot be saved this session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadOnly {
    /// The file is there but could not be read (locked, no permission), or a
    /// damaged one could not be moved aside.
    CouldNotRead,
    /// A newer Mechvibes wrote the file in a layout this app does not know.
    MadeByNewerApp,
}

impl ReadOnly {
    /// What the user is told, once, when the list is loaded.
    pub fn message(self) -> &'static str {
        match self {
            ReadOnly::CouldNotRead =>
                "Could not open your saved themes, so theme changes will not be saved for now. Restart Mechvibes to try again.",
            ReadOnly::MadeByNewerApp =>
                "Your saved themes come from a newer Mechvibes. Update Mechvibes to change them.",
        }
    }
}

/// The saved themes list, in the order the file has them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SavedThemes {
    slots: Vec<Slot>,
    /// Top-level keys this app does not know, written back as they were.
    extra: Map<String, Value>,
    /// At least one entry could not be read and is left out of the list.
    pub had_unreadable: bool,
    /// The file was damaged and was moved to this path this session.
    pub rescued: Option<PathBuf>,
    /// Set when nothing may be written this session.
    pub read_only: Option<ReadOnly>,
}

#[derive(Debug, Clone, PartialEq)]
enum Slot {
    /// `raw` is the entry as the file has it, written back until the user
    /// changes this theme.
    Readable {
        theme: SavedTheme,
        raw: Option<Value>,
    },
    /// Always written back verbatim.
    Unreadable(Value),
}

impl SavedThemes {
    /// The themes that could be read, in file order.
    pub fn themes(&self) -> impl Iterator<Item = &SavedTheme> {
        self.slots.iter().filter_map(|slot| match slot {
            Slot::Readable { theme, .. } => Some(theme),
            Slot::Unreadable(_) => None,
        })
    }

    /// Add a theme at the end of the list.
    pub fn insert(&mut self, theme: SavedTheme) {
        self.slots.push(Slot::Readable { theme, raw: None });
    }

    /// Change the theme with this id. Returns false when there is none.
    pub fn update(&mut self, id: &str, change: impl FnOnce(&mut SavedTheme)) -> bool {
        for slot in &mut self.slots {
            if let Slot::Readable { theme, raw } = slot {
                if theme.id == id {
                    change(theme);
                    // From now on the theme in memory is what gets written.
                    *raw = None;
                    return true;
                }
            }
        }
        false
    }

    /// Take the theme with this id off the list. Returns false when there is
    /// none. An unreadable entry is never removed.
    pub fn remove(&mut self, id: &str) -> bool {
        let position = self.slots
            .iter()
            .position(|slot| matches!(slot, Slot::Readable { theme, .. } if theme.id == id));
        match position {
            Some(index) => {
                self.slots.remove(index);
                true
            }
            None => false,
        }
    }

    /// Read the list at `path`. Only a missing file is an empty list that may
    /// be saved. A file that is not a list is moved aside (see
    /// `preserve_corrupt_config`); any other failure leaves the file alone and
    /// makes the list read-only.
    pub(crate) fn load_from(path: &Path) -> Self {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Self::default();
            }
            Err(e) => {
                crate::always_eprint!("❌ Could not read '{}', leaving it untouched: {}", path.display(), e);
                return Self { read_only: Some(ReadOnly::CouldNotRead), ..Self::default() };
            }
        };

        let document = match serde_json::from_slice::<Value>(&bytes) {
            Ok(Value::Object(document)) => Some(document),
            _ => None,
        };
        // A list without a layout number is read as this layout. One with a
        // number this app does not know is still shown, so the themes can be
        // put on, but never written. It is never taken for a damaged file
        // either, whatever shape the newer Mechvibes gave it.
        let made_by_newer_app = document
            .as_ref()
            .and_then(|document| document.get("format"))
            .is_some_and(|format| !format.as_u64().is_some_and(|format| format <= FORMAT));
        let document = document.filter(
            |document| made_by_newer_app || document.get("themes").is_some_and(Value::is_array)
        );
        let Some(mut document) = document else {
            crate::always_eprint!("❌ saved_themes.json is damaged");
            return match preserve_corrupt_config(path) {
                // Gone between the read and the rename: nothing to lose.
                Preserved::Nothing => Self::default(),
                Preserved::MovedTo(copy) => Self { rescued: Some(copy), ..Self::default() },
                Preserved::Failed(e) => {
                    crate::always_eprint!("   Could not set the damaged file aside, leaving it untouched: {}", e);
                    Self { read_only: Some(ReadOnly::CouldNotRead), ..Self::default() }
                }
            };
        };

        document.remove("format");
        let read_only = made_by_newer_app.then_some(ReadOnly::MadeByNewerApp);
        let entries = match document.remove("themes") {
            Some(Value::Array(entries)) => entries,
            _ => Vec::new(),
        };

        let mut had_unreadable = false;
        let slots = entries
            .into_iter()
            .map(|raw| {
                let theme = serde_json
                    ::from_value::<SavedTheme>(raw.clone())
                    .ok()
                    .and_then(checked);
                match theme {
                    Some(theme) => Slot::Readable { theme, raw: Some(raw) },
                    None => {
                        had_unreadable = true;
                        Slot::Unreadable(raw)
                    }
                }
            })
            .collect();
        if had_unreadable {
            crate::always_eprint!("⚠️  Some saved themes could not be read and were left as they are");
        }

        Self { slots, extra: document, had_unreadable, rescued: None, read_only }
    }

    /// Run `mutate` and write the result to `path`. Returns whether anything
    /// was written. Refuses without calling `mutate` while read-only, and puts
    /// the list back as it was when the result cannot be saved.
    fn apply_at(&mut self, path: &Path, mutate: impl FnOnce(&mut SavedThemes)) -> Result<bool, String> {
        if let Some(read_only) = self.read_only {
            return Err(read_only.message().to_string());
        }
        let before = self.clone();
        mutate(self);
        if *self == before {
            return Ok(false);
        }
        match self.check_changed().and_then(|()| self.save_at(path)) {
            Ok(()) => Ok(true),
            Err(e) => {
                *self = before;
                Err(e)
            }
        }
    }

    /// Put every changed theme through the checks a theme read from the file
    /// gets, so what is saved is what the next start reads back. Theme CSS is
    /// cleaned here; colors that clean to nothing are never saved, and never
    /// swapped for other colors either.
    fn check_changed(&mut self) -> Result<(), String> {
        for slot in &mut self.slots {
            if let Slot::Readable { theme, raw: None } = slot {
                let no_colors_left = matches!(
                    &theme.colors,
                    SharedColors::Custom { css } if safe_theme_css(css).is_empty()
                );
                if no_colors_left {
                    crate::always_eprint!("❌ Not saving theme '{}': its colors clean to nothing", theme.name);
                    return Err("Its colors have nothing this app can use.".to_string());
                }
                let Some(clean) = checked(theme.clone()) else {
                    crate::always_eprint!("❌ Not saving theme '{}': it has a value this app cannot use", theme.name);
                    return Err("It has a color or image this app cannot use.".to_string());
                };
                *theme = clean;
            }
        }
        Ok(())
    }

    /// Write the list to `path` in one step, so a failed write leaves the old
    /// file rather than half of a new one.
    fn save_at(&self, path: &Path) -> Result<(), String> {
        let mut themes = Vec::with_capacity(self.slots.len());
        for slot in &self.slots {
            themes.push(match slot {
                Slot::Readable { raw: Some(raw), .. } | Slot::Unreadable(raw) => raw.clone(),
                Slot::Readable { theme, raw: None } =>
                    serde_json::to_value(theme).map_err(|e| format!("Could not write the theme: {}", e))?,
            });
        }
        let mut document = self.extra.clone();
        document.insert("format".to_string(), Value::from(FORMAT));
        document.insert("themes".to_string(), Value::Array(themes));
        data::save_json_to_file_atomically(&Value::Object(document), path)
    }
}

/// `theme` with its values checked the way a theme file's are, or `None`
/// when one could break out of the style it is put in: six safe colors,
/// colors CSS that keeps something after cleaning, and images that name
/// nothing but an image. A theme installed from outside may only point at
/// this app's own images; the user's own may also use a web address or a
/// full path, as the config does. A library id that is not one is dropped,
/// which makes the theme an imported one.
pub fn checked(mut theme: SavedTheme) -> Option<SavedTheme> {
    if theme.id.trim().is_empty() {
        return None;
    }
    theme.name = clean_name(&theme.name)?;
    if !theme.library_id.as_deref().is_some_and(is_theme_id) {
        theme.library_id = None;
        if theme.kind == ThemeKind::Library {
            theme.kind = ThemeKind::Imported;
        }
    }

    let look = checked_look(theme.look(), theme.kind != ThemeKind::Mine)?;
    let LookColors::Known(colors) = look.colors else {
        return None;
    };
    theme.colors = colors;
    theme.logo = look.logo;
    theme.background = look.background;
    Some(theme)
}

/// `look` with its values checked the way a saved theme's are (see
/// `checked`), or `None` when one could break out of its style or the colors
/// are missing. `app_images_only` allows nothing but this app's own images.
pub fn checked_look(mut look: Look, app_images_only: bool) -> Option<Look> {
    let LookColors::Known(colors) = &mut look.colors else {
        return None;
    };
    if let SharedColors::Custom { css } = colors {
        *css = safe_theme_css(css);
        if css.is_empty() {
            return None;
        }
    }

    if let Some(logo) = &mut look.logo {
        let defaults = LogoCustomization::default();
        for (color, default) in [
            (&mut logo.border_color, defaults.border_color),
            (&mut logo.text_color, defaults.text_color),
            (&mut logo.shadow_color, defaults.shadow_color),
            (&mut logo.background_color, defaults.background_color),
            (&mut logo.muted_background, defaults.muted_background),
        ] {
            *color = safe_color(color, default).ok()?;
        }
        for image in [&logo.background_image, &logo.muted_background_image] {
            if !image_fits(image.as_deref(), app_images_only) {
                return None;
            }
        }
    }
    if let Some(background) = &mut look.background {
        background.background_color = safe_color(
            &background.background_color,
            BackgroundCustomization::default().background_color
        ).ok()?;
        if !image_fits(background.background_image.as_deref(), app_images_only) {
            return None;
        }
    }
    Some(look)
}

/// Whether an image setting can go into a `url()` as it is: no quote,
/// bracket, semicolon, space or control character, and a place an image can
/// be. An empty setting names no image and is fine.
fn image_fits(image: Option<&str>, app_images_only: bool) -> bool {
    let Some(image) = image.filter(|image| !image.is_empty()) else {
        return true;
    };
    if image.chars().any(|c| c.is_whitespace() || c.is_control() || matches!(c, '\'' | ')' | ';')) {
        return false;
    }
    if let Some(file_name) = image.strip_prefix("/custom-images/") {
        return is_app_image_name(file_name);
    }
    !app_images_only && (image.starts_with("https://") || Path::new(image).is_absolute())
}

/// A name this app gives an image it keeps: letters, digits, `-` and `_`,
/// with one dot before the extension.
fn is_app_image_name(name: &str) -> bool {
    let plain = |part: &str| !part.is_empty() && part.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_');
    name.split_once('.').is_some_and(|(stem, extension)| plain(stem) && plain(extension)) && is_portable_name(name)
}

// ===== The list the app uses =====

/// The list, loaded on first use. A change goes through `apply`, which writes
/// it, so the file and this never drift apart.
static AUTHORITY: OnceLock<Mutex<SavedThemes>> = OnceLock::new();

/// Bumped on every write that changed something, so the window can tell when
/// to show the list again, as `config_writer::generation` does for the config.
static GENERATION: AtomicU64 = AtomicU64::new(0);

fn authority() -> MutexGuard<'static, SavedThemes> {
    AUTHORITY.get_or_init(|| Mutex::new(load()))
        .lock()
        // A mutation that panicked leaves a list that is still whole.
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Read the list from disk and tell the user, once, when something is wrong
/// with it. The notices wait in the queue until the window is up.
fn load() -> SavedThemes {
    let path = paths::data::saved_themes_json();
    let store = SavedThemes::load_from(&path);
    if let Some(copy) = &store.rescued {
        crate::always_eprint!("   The damaged saved_themes.json was kept at: {}", copy.display());
        notice::report(
            NoticeKind::Error,
            format!("Your saved themes were damaged. A copy was kept at {}.", copy.display()),
            Some(NoticeAction::ShowFile(copy.clone()))
        );
    }
    if let Some(read_only) = store.read_only {
        notice::report(NoticeKind::Error, read_only.message(), None);
    }
    store
}

/// A snapshot of the list.
pub fn current() -> SavedThemes {
    authority().clone()
}

/// Change the list and save it. Returns whether anything was written.
///
/// `Err` while the list is read-only, without calling `mutate`, and when the
/// change cannot be saved, in which case the list stays as it was and the
/// user is told. `mutate` runs under the list's lock, so it must not call
/// `current` or `apply`.
pub fn apply(mutate: impl FnOnce(&mut SavedThemes)) -> Result<bool, String> {
    let result = {
        let mut store = authority();
        // Already said once at load; saying it on every edit would only nag.
        if let Some(read_only) = store.read_only {
            return Err(read_only.message().to_string());
        }
        store.apply_at(&paths::data::saved_themes_json(), mutate)
    };
    match &result {
        Ok(true) => {
            GENERATION.fetch_add(1, Ordering::Release);
        }
        Ok(false) => {}
        Err(e) => {
            crate::always_eprint!("❌ Failed to save saved_themes.json: {}", e);
            notice::report(NoticeKind::Error, format!("Could not save your theme. {}", e), None);
        }
    }
    result
}

/// How many times the list has changed since the process started.
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Acquire)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::libs::theme::BuiltInTheme;
    use serde_json::json;

    /// An empty folder of its own under the system temp folder.
    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mechvibes-saved-themes-{}-{}", name, uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A readable entry as the file holds it.
    fn entry(id: &str, name: &str, kind: &str) -> Value {
        json!({
            "id": id,
            "name": name,
            "kind": kind,
            "library_id": null,
            "library_checksum": null,
            "fingerprint": null,
            "colors": { "built_in": "Dark" },
            "logo": null,
            "background": null,
            "created_at": "2026-11-05T10:00:00Z",
            "updated_at": "2026-11-05T10:12:00Z"
        })
    }

    fn with_logo(mut entry: Value, edit: impl FnOnce(&mut Value)) -> Value {
        let mut logo = serde_json::to_value(LogoCustomization::default()).unwrap();
        edit(&mut logo);
        entry["logo"] = logo;
        entry
    }

    fn with_background_image(mut entry: Value, image: &str) -> Value {
        entry["background"] = json!({
            "background_color": "var(--color-base-100)",
            "background_image": image,
            "use_image": true
        });
        entry
    }

    fn write_list(path: &Path, themes: Vec<Value>) {
        std::fs::write(path, serde_json::to_vec_pretty(&json!({ "format": 1, "themes": themes })).unwrap()).unwrap();
    }

    fn read_file(path: &Path) -> Value {
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    }

    fn mine(id: &str, colors: SharedColors) -> SavedTheme {
        SavedTheme {
            id: id.to_string(),
            name: "Night drive".to_string(),
            kind: ThemeKind::Mine,
            library_id: None,
            library_checksum: None,
            fingerprint: None,
            colors,
            logo: None,
            background: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            extra: Map::new(),
        }
    }

    fn ids(store: &SavedThemes) -> Vec<&str> {
        store.themes().map(|theme| theme.id.as_str()).collect()
    }

    #[test]
    fn a_missing_file_is_an_empty_list() {
        let dir = temp_dir("missing");
        let path = dir.join("saved_themes.json");
        let mut store = SavedThemes::load_from(&path);
        assert_eq!(store.themes().count(), 0);
        assert_eq!(store.read_only, None, "a first run can save");
        assert!(!path.exists(), "reading writes nothing");

        let written = store.apply_at(&path, |store| store.insert(mine("a", SharedColors::BuiltIn(BuiltInTheme::Dark))));
        assert_eq!(written, Ok(true));
        assert_eq!(ids(&SavedThemes::load_from(&path)), ["a"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_store_that_cannot_be_read_is_never_overwritten() {
        let dir = temp_dir("unreadable");
        // A folder where the file should be cannot be read as one.
        let path = dir.join("saved_themes.json");
        std::fs::create_dir_all(&path).unwrap();

        let mut store = SavedThemes::load_from(&path);
        assert_eq!(store.read_only, Some(ReadOnly::CouldNotRead));
        assert_eq!(store.themes().count(), 0);

        let mut called = false;
        let written = store.apply_at(&path, |store| {
            called = true;
            store.insert(mine("a", SharedColors::BuiltIn(BuiltInTheme::Dark)));
        });
        assert!(written.is_err());
        assert!(!called, "the change is never even made");
        assert!(path.is_dir());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "nothing was moved or written");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_newer_store_format_is_never_written() {
        let dir = temp_dir("newer");
        let path = dir.join("saved_themes.json");
        let bytes = serde_json::to_vec(&json!({ "format": 2, "themes": [entry("a", "Night", "mine")] })).unwrap();
        std::fs::write(&path, &bytes).unwrap();

        let mut store = SavedThemes::load_from(&path);
        assert_eq!(ids(&store), ["a"], "its themes can still be shown and put on");
        assert_eq!(store.read_only, Some(ReadOnly::MadeByNewerApp));

        let written = store.apply_at(&path, |store| {
            store.update("a", |theme| theme.name = "Renamed".to_string());
        });
        assert!(written.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes, "the file is untouched");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_newer_store_in_another_shape_is_never_set_aside() {
        for (name, document) in [
            ("themes-object", json!({ "format": 2, "themes": { "items": [entry("a", "Night", "mine")] } })),
            ("renamed-key", json!({ "format": 2, "saved": [entry("a", "Night", "mine")] })),
        ] {
            let dir = temp_dir(name);
            let path = dir.join("saved_themes.json");
            let bytes = serde_json::to_vec(&document).unwrap();
            std::fs::write(&path, &bytes).unwrap();

            let mut store = SavedThemes::load_from(&path);
            assert_eq!(store.read_only, Some(ReadOnly::MadeByNewerApp), "{}", name);
            assert_eq!(store.rescued, None, "{}: not taken for a damaged file", name);
            assert_eq!(store.themes().count(), 0);
            assert!(store.apply_at(&path, |store| store.insert(mine("b", SharedColors::BuiltIn(BuiltInTheme::Dark)))).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), bytes, "{}: the file is untouched", name);
            assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "{}: nothing was moved or written", name);
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn a_damaged_entry_is_kept_as_written() {
        let dir = temp_dir("damaged-entry");
        let path = dir.join("saved_themes.json");
        let damaged = json!({ "id": "b", "name": 5, "kind": "mine" });
        write_list(&path, vec![entry("a", "First", "mine"), damaged.clone(), entry("c", "Third", "mine")]);

        let mut store = SavedThemes::load_from(&path);
        assert_eq!(ids(&store), ["a", "c"]);
        assert!(store.had_unreadable);

        let written = store.apply_at(&path, |store| {
            store.update("a", |theme| theme.name = "Renamed".to_string());
        });
        assert_eq!(written, Ok(true));
        let file = read_file(&path);
        assert_eq!(file["themes"][1], damaged, "the damaged entry is kept whole, in its place");
        assert_eq!(file["themes"][0]["name"], "Renamed");
        assert_eq!(ids(&SavedThemes::load_from(&path)), ["a", "c"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unknown_field_survives_a_save() {
        let dir = temp_dir("unknown-field");
        let path = dir.join("saved_themes.json");
        let mut first = entry("a", "First", "mine");
        first["sparkle"] = json!({ "speed": 3 });
        std::fs::write(
            &path,
            serde_json::to_vec(&json!({ "format": 1, "themes": [first], "shelves": ["top"] })).unwrap()
        ).unwrap();

        let mut store = SavedThemes::load_from(&path);
        assert_eq!(store.themes().next().unwrap().extra["sparkle"], json!({ "speed": 3 }));
        store
            .apply_at(&path, |store| {
                store.update("a", |theme| theme.name = "Renamed".to_string());
            })
            .unwrap();

        let file = read_file(&path);
        assert_eq!(file["themes"][0]["name"], "Renamed", "the entry was written from memory");
        assert_eq!(file["themes"][0]["sparkle"], json!({ "speed": 3 }));
        assert_eq!(file["shelves"], json!(["top"]));
        assert_eq!(file["format"], 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unknown_built_in_is_kept_verbatim() {
        let dir = temp_dir("unknown-built-in");
        let path = dir.join("saved_themes.json");
        let mut neon = entry("a", "Neon", "mine");
        neon["colors"] = json!({ "built_in": "Neon" });
        write_list(&path, vec![neon.clone()]);

        let mut store = SavedThemes::load_from(&path);
        assert_eq!(store.themes().count(), 0);
        assert!(store.had_unreadable);

        store.apply_at(&path, |store| store.insert(mine("b", SharedColors::BuiltIn(BuiltInTheme::Dark)))).unwrap();
        let file = read_file(&path);
        assert_eq!(file["themes"][0], neon);
        assert_eq!(file["themes"][1]["id"], "b");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The file is written again with sorted keys and indenting, so what is
    /// kept is the entry's JSON value: every field and value as it was read.
    #[test]
    fn an_unedited_entry_is_written_back_as_it_was() {
        let dir = temp_dir("unedited");
        let path = dir.join("saved_themes.json");
        let mut raw_css = entry("a", "  Raw   colors ", "imported");
        raw_css["colors"] = json!({ "custom": { "css": "--color-primary: red; } body { display: none }" } });
        write_list(&path, vec![raw_css.clone(), entry("b", "Other", "mine")]);

        let mut store = SavedThemes::load_from(&path);
        let first = store.themes().next().unwrap();
        assert_eq!(first.name, "Raw colors", "tidied in memory");
        assert_eq!(first.colors, SharedColors::Custom { css: "--color-primary: red;".to_string() });

        store
            .apply_at(&path, |store| {
                store.update("b", |theme| theme.name = "Renamed".to_string());
            })
            .unwrap();
        let file = read_file(&path);
        assert_eq!(file["themes"][0], raw_css, "the entry nobody changed is written as it was read");
        assert_eq!(file["themes"][1]["name"], "Renamed");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_bad_library_id_is_ignored_but_kept_in_the_file() {
        let dir = temp_dir("bad-library-id");
        let path = dir.join("saved_themes.json");
        let mut library = entry("a", "From the library", "library");
        library["library_id"] = json!("th-not-an-id");
        library["library_checksum"] = json!("abc");
        let mut good = entry("b", "Real", "library");
        good["library_id"] = json!(format!("th-{}", "0123456789abcdef".repeat(2)));
        write_list(&path, vec![library.clone(), good]);

        let mut store = SavedThemes::load_from(&path);
        let themes: Vec<&SavedTheme> = store.themes().collect();
        assert_eq!(themes[0].kind, ThemeKind::Imported);
        assert_eq!(themes[0].library_id, None);
        assert_eq!(themes[1].kind, ThemeKind::Library, "a real id is kept");

        store.apply_at(&path, |store| store.insert(mine("c", SharedColors::BuiltIn(BuiltInTheme::Dark)))).unwrap();
        let file = read_file(&path);
        assert_eq!(file["themes"][0]["kind"], "library");
        assert_eq!(file["themes"][0]["library_id"], "th-not-an-id");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_hand_edited_logo_color_cannot_break_out_of_its_style() {
        let dir = temp_dir("break-out");
        let path = dir.join("saved_themes.json");
        let colour = with_logo(entry("a", "Colour", "mine"), |logo| {
            logo["border_color"] = json!("red; position:fixed");
        });
        let list = with_logo(entry("b", "List", "mine"), |logo| {
            logo["shadow_color"] = json!("red, 0 0 0 100vmax black");
        });
        let image = with_background_image(
            entry("c", "Image", "mine"),
            "https://example.com/a.png),url(https://example.com/b.png"
        );
        let logo_image = with_logo(entry("d", "Logo image", "mine"), |logo| {
            logo["muted_background_image"] = json!("/custom-images/a.png');background:red;'");
        });
        let background = {
            let mut background = entry("e", "Background colour", "mine");
            background["background"] = json!({
                "background_color": "red; background-image: url(x)",
                "background_image": null,
                "use_image": false
            });
            background
        };
        let fine = with_logo(entry("f", "Fine", "mine"), |logo| {
            logo["border_color"] = json!("oklch(55% 0.3 240)");
            logo["background_image"] = json!("/custom-images/img-3f2ae1.webp");
        });
        write_list(&path, vec![colour, list, image, logo_image, background, fine]);

        let store = SavedThemes::load_from(&path);
        assert_eq!(ids(&store), ["f"]);
        assert!(store.had_unreadable);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_installed_theme_only_points_at_app_images() {
        let dir = temp_dir("installed-images");
        let path = dir.join("saved_themes.json");
        let full_path = if cfg!(windows) { "C:\\Users\\me\\wall.png" } else { "/home/me/wall.png" };
        write_list(
            &path,
            vec![
                with_background_image(entry("web-installed", "Web", "imported"), "https://example.com/wall.jpg"),
                with_background_image(entry("path-installed", "Path", "imported"), full_path),
                with_background_image(entry("escape", "Escape", "imported"), "/custom-images/../config.json"),
                with_background_image(entry("app", "App image", "imported"), "/custom-images/img-3f2ae1.webp"),
                with_background_image(entry("web-mine", "Web", "mine"), "https://example.com/wall.jpg"),
                with_background_image(entry("path-mine", "Path", "mine"), full_path),
                with_background_image(entry("relative-mine", "Relative", "mine"), "wall.png")
            ]
        );

        let store = SavedThemes::load_from(&path);
        assert_eq!(ids(&store), ["app", "web-mine", "path-mine"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_that_is_not_json_is_moved_aside() {
        for (name, damaged) in [
            ("not-json", b"{\"themes\": [\xff\xfe".to_vec()),
            ("not-a-list", b"{\"format\": 1, \"themes\": {}}".to_vec()),
            ("bare-array", b"[]".to_vec()),
        ] {
            let dir = temp_dir(name);
            let path = dir.join("saved_themes.json");
            std::fs::write(&path, &damaged).unwrap();

            let store = SavedThemes::load_from(&path);
            let copy = dir.join("saved_themes.json.corrupt");
            assert_eq!(store.rescued, Some(copy.clone()), "{}", name);
            assert_eq!(store.themes().count(), 0);
            assert_eq!(store.read_only, None, "a new list can be saved");
            assert_eq!(std::fs::read(&copy).unwrap(), damaged, "the old bytes are kept whole");
            assert!(!path.exists());
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn colors_are_cleaned_when_saved() {
        let dir = temp_dir("clean-on-save");
        let path = dir.join("saved_themes.json");
        let mut store = SavedThemes::load_from(&path);
        let css = "--color-primary: red;\n--color-secondary: url(x);\n} body { display: none; }".to_string();
        store.apply_at(&path, |store| store.insert(mine("a", SharedColors::Custom { css }))).unwrap();

        let file = read_file(&path);
        assert_eq!(file["themes"][0]["colors"], json!({ "custom": { "css": "--color-primary: red;" } }));
        assert_eq!(
            store.themes().next().unwrap().colors,
            SharedColors::Custom { css: "--color-primary: red;".to_string() },
            "memory holds what the file holds"
        );

        // Colors that clean to nothing are refused, never saved as something else.
        let before = std::fs::read(&path).unwrap();
        let nothing = SharedColors::Custom { css: "} body { display: none; }".to_string() };
        assert!(store.apply_at(&path, |store| store.insert(mine("b", nothing))).is_err());
        assert_eq!(ids(&store), ["a"]);
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failed_save_leaves_the_list_as_it_was() {
        let dir = temp_dir("failed-save");
        let path = dir.join("saved_themes.json");
        write_list(&path, vec![entry("a", "First", "mine")]);
        let mut store = SavedThemes::load_from(&path);
        let before = store.clone();

        // A folder in the way makes the final rename fail.
        let blocked = dir.join("blocked.json");
        std::fs::create_dir_all(&blocked).unwrap();
        let written = store.apply_at(&blocked, |store| {
            store.update("a", |theme| theme.name = "Renamed".to_string());
            store.insert(mine("b", SharedColors::BuiltIn(BuiltInTheme::Dark)));
        });
        assert!(written.is_err());
        assert_eq!(store, before);
        assert!(blocked.is_dir());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
