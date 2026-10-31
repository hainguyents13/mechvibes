use crate::state::config::{preserve_corrupt_config, Preserved};
use crate::state::paths;
use crate::utils::notice::{self, NoticeAction, NoticeKind};
use crate::utils::{data, path};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CustomThemeData {
    pub id: String,
    pub name: String,
    pub description: String,
    pub css: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub is_built_in: bool, // Indicates if this is a built-in theme
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThemesConfig {
    pub version: String,
    pub custom_themes: HashMap<String, CustomThemeData>,
    pub last_updated: DateTime<Utc>,
}

impl Default for ThemesConfig {
    fn default() -> Self {
        Self {
            version: "1.0.0".to_string(),
            custom_themes: HashMap::new(),
            last_updated: Utc::now(),
        }
    }
}

/// How the colors list came off the disk.
#[derive(Debug, Clone, PartialEq)]
pub enum ThemesLoad {
    /// There was no file yet. A new one is written.
    Fresh,
    Loaded,
    /// The file was damaged and now lives at this path. A new one is written.
    KeptAside(PathBuf),
    /// The file could not be read, or a damaged one could not be moved. It is
    /// left exactly as it is, and nothing is saved over it this session.
    ReadOnly(String),
}

/// Set when the list could not be read at startup, so every save refuses.
static READ_ONLY: AtomicBool = AtomicBool::new(false);
const READ_ONLY_ERROR: &str =
    "Could not read your colors list, so nothing in it was changed. Restart Mechvibes to try again.";

impl ThemesConfig {
    /// The colors list from disk. Never writes over a file it could not read:
    /// a damaged one is moved aside first, and one that cannot be read at all
    /// (locked, no permission) makes the list read-only for this session.
    pub fn load() -> Self {
        let themes_path = paths::data::themes_json();

        // Ensure data directory exists
        if let Some(parent) = themes_path.parent() {
            if let Err(e) = path::ensure_directory_exists(parent) {
                crate::always_eprint!("Warning: Could not create themes data directory: {}", e);
            }
        }

        let (config, outcome) = Self::load_from(&themes_path);
        match &outcome {
            ThemesLoad::Loaded => {
                crate::always_print!(
                    "✅ Loaded themes configuration from {}",
                    themes_path.display()
                );
            }
            ThemesLoad::Fresh | ThemesLoad::KeptAside(_) => {
                crate::always_print!("📝 Creating new themes configuration");
                if let Err(e) = config.save_at(&themes_path, false) {
                    crate::always_eprint!("❌ Failed to create initial themes.json: {}", e);
                }
            }
            ThemesLoad::ReadOnly(e) => {
                crate::always_eprint!("❌ Failed to load themes.json, leaving it untouched: {}", e);
                READ_ONLY.store(true, Ordering::Release);
            }
        }
        // Waits in the queue until the window is up to show it.
        match outcome {
            ThemesLoad::KeptAside(copy) => {
                crate::always_eprint!("   The damaged themes.json was kept at: {}", copy.display());
                notice::report(
                    NoticeKind::Error,
                    format!("Your colors list was damaged. A copy was kept at {}.", copy.display()),
                    Some(NoticeAction::ShowFile(copy))
                );
            }
            // Saved colors are missing from the screen this session, so say
            // why now rather than only at the next save.
            ThemesLoad::ReadOnly(_) => notice::report(NoticeKind::Error, READ_ONLY_ERROR, None),
            ThemesLoad::Fresh | ThemesLoad::Loaded => {}
        }
        config
    }

    /// Read the list at `path`. Only a missing file starts an empty list
    /// that may be saved; a damaged one is moved aside (see
    /// `preserve_corrupt_config`), and any other failure leaves the file alone.
    pub(crate) fn load_from(path: &Path) -> (Self, ThemesLoad) {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return (Self::default(), ThemesLoad::Fresh);
            }
            Err(e) => {
                return (Self::default(), ThemesLoad::ReadOnly(format!("could not read '{}': {}", path.display(), e)));
            }
        };
        match serde_json::from_slice(&bytes) {
            Ok(config) => (config, ThemesLoad::Loaded),
            Err(e) => {
                crate::always_eprint!("❌ themes.json is damaged: {}", e);
                let outcome = match preserve_corrupt_config(path) {
                    // Gone between the read and the rename: nothing to lose.
                    Preserved::Nothing => ThemesLoad::Fresh,
                    Preserved::MovedTo(copy) => ThemesLoad::KeptAside(copy),
                    Preserved::Failed(e) => ThemesLoad::ReadOnly(format!("could not set the damaged file aside: {}", e)),
                };
                (Self::default(), outcome)
            }
        }
    }

    pub fn save(&self) -> Result<(), String> {
        self.save_at(&paths::data::themes_json(), READ_ONLY.load(Ordering::Acquire))
    }

    fn save_at(&self, themes_path: &Path, read_only: bool) -> Result<(), String> {
        if read_only {
            return Err(READ_ONLY_ERROR.to_string());
        }

        // Ensure the data directory exists
        if let Some(parent) = themes_path.parent() {
            path::ensure_directory_exists(parent)
                .map_err(|e| format!("Failed to create data directory: {}", e))?;
        }

        // Atomic, so a failed write leaves the old list rather than half of one.
        data::save_json_to_file_atomically(self, themes_path)?;
        crate::always_print!("💾 Saved themes configuration to {}", themes_path.display());
        Ok(())
    }
    pub fn add_theme(
        &mut self,
        name: String,
        description: String,
        css: String,
    ) -> Result<String, String> {
        if name.trim().is_empty() {
            return Err("Give it a name first.".to_string());
        }

        // Check for duplicate names
        for theme in self.custom_themes.values() {
            if theme.name.to_lowercase() == name.to_lowercase() {
                return Err("You already have one with this name. Pick another name.".to_string());
            }
        }

        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();

        let theme_data = CustomThemeData {
            id: id.clone(),
            name,
            description,
            css,
            created_at: now,
            updated_at: now,
            is_built_in: false,
        };

        self.custom_themes.insert(id.clone(), theme_data);
        self.last_updated = now;

        Ok(id)
    }
    pub fn update_theme(
        &mut self,
        id: &str,
        name: String,
        description: String,
        css: String,
    ) -> Result<(), String> {
        if name.trim().is_empty() {
            return Err("Give it a name first.".to_string());
        }

        // Check for duplicate names (excluding current theme)
        for (theme_id, theme) in &self.custom_themes {
            if theme_id != id && theme.name.to_lowercase() == name.to_lowercase() {
                return Err("You already have one with this name. Pick another name.".to_string());
            }
        }

        if let Some(theme) = self.custom_themes.get_mut(id) {
            theme.name = name;
            theme.description = description;
            theme.css = css;
            theme.updated_at = Utc::now();
            self.last_updated = Utc::now();
            Ok(())
        } else {
            Err("Theme not found".to_string())
        }
    }

    pub fn delete_theme(&mut self, id: &str) -> Result<(), String> {
        if self.custom_themes.remove(id).is_some() {
            self.last_updated = Utc::now();
            Ok(())
        } else {
            Err("Theme not found".to_string())
        }
    }

    /// Add a theme someone shared and return its id. Importing the same theme
    /// again reuses the copy already here; a different theme with a name in
    /// use gets a number after it, as "Night drive (2)".
    pub fn import_theme(&mut self, name: &str, css: &str) -> Result<String, String> {
        let same_name = |theme: &CustomThemeData, name: &str| theme.name.to_lowercase() == name.to_lowercase();
        if let Some(existing) = self.custom_themes.values().find(|theme| same_name(theme, name) && theme.css == css) {
            return Ok(existing.id.clone());
        }
        let mut unique = name.to_string();
        let mut number = 2;
        while self.custom_themes.values().any(|theme| same_name(theme, &unique)) {
            unique = format!("{} ({})", name, number);
            number += 1;
        }
        self.add_theme(unique, String::new(), css.to_string())
    }

    pub fn get_theme_by_id(&self, id: &str) -> Option<&CustomThemeData> {
        self.custom_themes.get(id)
    }

    pub fn list_themes(&self) -> Vec<&CustomThemeData> {
        let mut themes: Vec<&CustomThemeData> = self.custom_themes.values().collect();
        themes.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        // Sort by most recently updated
        themes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn importing_reuses_the_same_theme_and_numbers_a_different_one() {
        let mut themes = ThemesConfig::default();
        let first = themes.import_theme("Night", "--a: red;").unwrap();
        assert_eq!(themes.import_theme("night", "--a: red;").unwrap(), first);
        let second = themes.import_theme("Night", "--a: blue;").unwrap();
        assert_ne!(second, first);
        assert_eq!(themes.get_theme_by_id(&second).unwrap().name, "Night (2)");
        let third = themes.import_theme("Night", "--a: green;").unwrap();
        assert_eq!(themes.get_theme_by_id(&third).unwrap().name, "Night (3)");
    }

    /// An empty folder of its own under the system temp folder.
    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mechvibes-themes-{}-{}", name, uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_missing_colors_list_starts_empty() {
        let dir = temp_dir("missing");
        let path = dir.join("themes.json");
        let (themes, outcome) = ThemesConfig::load_from(&path);
        assert_eq!(outcome, ThemesLoad::Fresh);
        assert!(themes.custom_themes.is_empty());
        assert!(!path.exists(), "reading writes nothing");

        let mut themes = themes;
        themes.import_theme("Night", "--color-primary: red;").unwrap();
        themes.save_at(&path, false).unwrap();
        assert_eq!(ThemesConfig::load_from(&path), (themes, ThemesLoad::Loaded));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_damaged_colors_list_is_moved_aside_not_overwritten() {
        let dir = temp_dir("damaged");
        let path = dir.join("themes.json");
        // Not JSON, and not even text.
        let damaged = b"{\"custom_themes\": {\xff\xfe".to_vec();
        std::fs::write(&path, &damaged).unwrap();

        let (themes, outcome) = ThemesConfig::load_from(&path);
        let copy = dir.join("themes.json.corrupt");
        assert_eq!(outcome, ThemesLoad::KeptAside(copy.clone()));
        assert!(themes.custom_themes.is_empty());
        assert_eq!(std::fs::read(&copy).unwrap(), damaged, "the old bytes are kept whole");
        assert!(!path.exists(), "the damaged file was moved, so a new one can be written");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_colors_list_that_cannot_be_read_is_never_overwritten() {
        let dir = temp_dir("unreadable");
        // A folder where the file should be cannot be read as one.
        let path = dir.join("themes.json");
        std::fs::create_dir_all(&path).unwrap();
        let (_, outcome) = ThemesConfig::load_from(&path);
        assert!(matches!(outcome, ThemesLoad::ReadOnly(_)), "{:?}", outcome);
        assert!(path.is_dir());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "nothing was moved or written");

        // While read-only, a save refuses and writes nothing.
        let file = dir.join("other.json");
        assert_eq!(ThemesConfig::default().save_at(&file, true), Err(READ_ONLY_ERROR.to_string()));
        assert!(!file.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}