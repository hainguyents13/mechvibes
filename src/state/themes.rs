use crate::state::paths;
use crate::utils::{data, path};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

impl ThemesConfig {
    pub fn load() -> Self {
        let themes_path = paths::data::themes_json();

        // Ensure data directory exists
        if let Some(parent) = themes_path.parent() {
            if let Err(e) = path::ensure_directory_exists(parent) {
                crate::always_eprint!("Warning: Could not create themes data directory: {}", e);
            }
        }

        match data::load_json_from_file::<ThemesConfig>(&themes_path) {
            Ok(config) => {
                crate::always_print!(
                    "✅ Loaded themes configuration from {}",
                    themes_path.display()
                );
                config
            }
            Err(e) => {
                crate::always_eprint!("❌ Failed to load themes.json: {}", e);
                crate::always_print!("📝 Creating new themes configuration");
                let config = Self::default();
                if let Err(e) = config.save() {
                    crate::always_eprint!("❌ Failed to create initial themes.json: {}", e);
                }
                config
            }
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let themes_path = paths::data::themes_json();

        // Ensure the data directory exists
        if let Some(parent) = themes_path.parent() {
            path::ensure_directory_exists(parent)
                .map_err(|e| format!("Failed to create data directory: {}", e))?;
        }

        data::save_json_to_file(self, &themes_path)?;
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
            return Err("Theme name cannot be empty".to_string());
        }

        // Check for duplicate names
        for theme in self.custom_themes.values() {
            if theme.name.to_lowercase() == name.to_lowercase() {
                return Err("A theme with this name already exists".to_string());
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
            return Err("Theme name cannot be empty".to_string());
        }

        // Check for duplicate names (excluding current theme)
        for (theme_id, theme) in &self.custom_themes {
            if theme_id != id && theme.name.to_lowercase() == name.to_lowercase() {
                return Err("A theme with this name already exists".to_string());
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
}