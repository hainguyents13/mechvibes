use crate::state::paths;
use crate::utils::{ data, path, soundpack };
use serde::{ Deserialize, Serialize };
use std::collections::HashMap;
use std::path::Path;

// ===== SOUNDPACK TYPES =====

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum SoundpackType {
    Keyboard,
    Mouse,
}

// Default function for config_version field
fn default_config_version() -> u32 {
    2
}

// Default function for soundpack_type field
fn default_soundpack_type() -> SoundpackType {
    SoundpackType::Keyboard
}

// Default function for options field
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SoundpackOptions {
    #[serde(default = "default_recommended_volume")]
    pub recommended_volume: f32,
    #[serde(default = "default_random_pitch")]
    pub random_pitch: bool,
}

fn default_recommended_volume() -> f32 {
    1.0
}

fn default_random_pitch() -> bool {
    false
}

impl Default for SoundpackOptions {
    fn default() -> Self {
        Self {
            recommended_volume: 1.0,
            random_pitch: false,
        }
    }
}

// Key definition structure for V2 format
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KeyDefinition {
    pub timing: Vec<[f32; 2]>, // Array of [start_ms, end_ms] pairs
    #[serde(default)]
    pub audio_file: Option<String>, // For "multi" definition method
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SoundPack {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub config_version: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub audio_file: Option<String>, // "single": the file every key plays from; "multi": for keys that name none
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub created_at: Option<String>, // ISO-8601 string
    pub definition_method: String, // "single" or "multi"
    #[serde(default)]
    pub options: SoundpackOptions,
    #[serde(default = "default_soundpack_type")]
    pub soundpack_type: SoundpackType, // Type of soundpack (Keyboard or Mouse) - for internal use
    #[serde(default = "default_config_version")]
    pub config_version_num: u32, // Internal config version number
    pub definitions: HashMap<String, KeyDefinition>,
}

impl SoundPack {}

impl SoundpackType {}

// ===== SOUNDPACK METADATA =====

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SoundpackMetadata {
    pub id: String, // Original ID from soundpack config (should not be modified)
    pub name: String,
    pub author: Option<String>,
    pub description: Option<String>,
    pub version: String,
    pub tags: Vec<String>,
    pub icon: Option<String>,
    #[serde(default = "default_soundpack_type")]
    pub soundpack_type: SoundpackType, // Type of soundpack (Keyboard or Mouse)
    #[serde(default)]
    pub folder_path: String, // Relative path from soundpacks directory (e.g., "keyboard/Super Paper Mario Talk")
    pub last_modified: u64,
    pub last_accessed: u64,
    // Validation fields
    pub config_version: Option<u32>,
    pub is_valid_v2: bool,
    pub validation_status: String,
    pub can_be_converted: bool,
    // Error tracking
    #[serde(default)]
    pub last_error: Option<String>,
}

// ===== SOUNDPACK CACHE =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundpackCache {
    pub soundpacks: HashMap<String, SoundpackMetadata>,
    pub last_scan: u64,
    pub cache_version: u32, // Add version to force regeneration when format changes
    #[serde(default)]
    pub count: SoundpackCount, // Count of soundpacks by type
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SoundpackCount {
    pub keyboard: usize,
    pub mouse: usize,
}

impl SoundpackCache {
    fn cache_file() -> String {
        paths::data::soundpack_cache_json().to_string_lossy().to_string()
    }

    pub fn load() -> Self {
        let cache_file = Self::cache_file();
        // Load metadata cache using data utilities
        let mut cache = match
            data::load_json_from_file::<SoundpackCache>(std::path::Path::new(&cache_file))
        {
            Ok(cache) => {
                crate::always_print!(
                    "📦 Loaded soundpack metadata cache with {} entries",
                    cache.soundpacks.len()
                );
                cache
            }
            Err(e) => {
                crate::always_eprint!("⚠️  Failed to load cache file: {}", e);
                Self::new()
            }
        };

        // Auto-refresh if cache is empty or missing
        if cache.soundpacks.is_empty() {
            crate::always_print!("🔄 Cache is empty, refreshing from soundpack directories...");
            cache.refresh_from_directory();
            cache.save();
        }

        cache
    }
    pub fn new() -> Self {
        Self {
            soundpacks: HashMap::new(),
            last_scan: 0,
            cache_version: 4, // Current version with error tracking support
            count: SoundpackCount::default(),
        }
    }

    pub fn save(&self) {
        let cache_file = Self::cache_file();

        // Ensure parent directory exists
        if let Some(parent) = Path::new(&cache_file).parent() {
            if let Err(e) = path::ensure_directory_exists(parent) {
                crate::always_eprint!("⚠️  Failed to create cache directory: {}", e);
                return;
            }
        }

        match data::save_json_to_file(self, std::path::Path::new(&cache_file)) {
            Ok(_) =>
                crate::always_print!(
                    "💾 Saved soundpack metadata cache with {} entries",
                    self.soundpacks.len()
                ),
            Err(e) => crate::always_eprint!("⚠️  Failed to save metadata cache: {}", e),
        }
    }

    // Add or update soundpack metadata
    pub fn add_soundpack(&mut self, metadata: SoundpackMetadata) {
        self.soundpacks.insert(metadata.id.clone(), metadata);
    } // Refresh cache by scanning soundpacks directory
    pub fn refresh_from_directory(&mut self) {
        crate::always_print!("📂 Scanning soundpacks directories...");

        self.soundpacks.clear(); // Clear all existing entries

        // Scan built-in soundpacks (app root)
        let builtin_soundpacks_dir = paths::soundpacks::get_builtin_soundpacks_dir()
            .to_string_lossy()
            .to_string();
        crate::always_print!("📂 Scanning built-in soundpacks in: {}", builtin_soundpacks_dir);
        self.scan_soundpack_type(&builtin_soundpacks_dir, "keyboard", false);
        self.scan_soundpack_type(&builtin_soundpacks_dir, "mouse", true);

        // Scan custom soundpacks (system app data)
        let custom_soundpacks_dir = paths::soundpacks::get_custom_soundpacks_dir()
            .to_string_lossy()
            .to_string();
        crate::always_print!("📂 Scanning custom soundpacks in: {}", custom_soundpacks_dir);
        self.scan_soundpack_type(&custom_soundpacks_dir, "keyboard", false);
        self.scan_soundpack_type(&custom_soundpacks_dir, "mouse", true);

        // Update count based on loaded soundpacks
        self.update_count();

        self.last_scan = std::time::SystemTime
            ::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        crate::always_print!("📦 Loaded {} soundpacks metadata", self.soundpacks.len());
    }

    // Update count based on current soundpacks in cache
    pub fn update_count(&mut self) {
        let mut keyboard_count = 0;
        let mut mouse_count = 0;

        for metadata in self.soundpacks.values() {
            match metadata.soundpack_type {
                SoundpackType::Keyboard => {
                    keyboard_count += 1;
                }
                SoundpackType::Mouse => {
                    mouse_count += 1;
                }
            }
        }
        self.count.keyboard = keyboard_count;
        self.count.mouse = mouse_count;

        crate::always_print!("📊 Updated count: {} keyboard, {} mouse soundpacks", keyboard_count, mouse_count);
    }

    fn scan_soundpack_type(&mut self, soundpacks_dir: &str, soundpack_type: &str, is_mouse: bool) {
        let type_dir = std::path::Path::new(soundpacks_dir).join(soundpack_type);
        crate::always_print!(
            "📂 [CACHE DEBUG] Scanning {} soundpacks in: {}",
            soundpack_type,
            type_dir.display()
        );
        crate::always_print!("   Directory exists: {}", type_dir.exists());

        if type_dir.exists() {
            crate::always_print!("✅ Directory found, reading entries...");

            if let Ok(entries) = std::fs::read_dir(&type_dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    if let Some(soundpack_name) = entry.file_name().to_str() {
                        let full_soundpack_id = format!("{}/{}", soundpack_type, soundpack_name);
                        crate::always_print!("🔍 [CACHE DEBUG] Processing soundpack: {}", full_soundpack_id);

                        match soundpack::load_soundpack_metadata(&full_soundpack_id) {
                            Ok(metadata) => {
                                crate::always_print!("✅ [CACHE DEBUG] Successfully loaded metadata for: {}", full_soundpack_id);
                                self.soundpacks.insert(full_soundpack_id, metadata);
                            }
                            Err(e) => {
                                crate::always_print!(
                                    "❌ [CACHE DEBUG] Failed to load {} metadata for {}: {}",
                                    soundpack_type,
                                    soundpack_name,
                                    e
                                );
                                self.insert_error_metadata(
                                    &full_soundpack_id,
                                    soundpack_name,
                                    e,
                                    is_mouse
                                );
                            }
                        }
                    }
                }
            } else {
                crate::always_print!("❌ [CACHE DEBUG] Failed to read directory: {}", type_dir.display());
            }
        } else {
            crate::always_print!("⚠️ [CACHE DEBUG] Directory does not exist: {}", type_dir.display());
            crate::always_print!("   Expected at: {}", type_dir.display());
            if let Some(parent) = type_dir.parent() {
                crate::always_print!("   Parent directory: {}", parent.display());
                crate::always_print!("   Parent exists: {}", parent.exists());
            }
        }
    }

    fn insert_error_metadata(
        &mut self,
        full_soundpack_id: &str,
        soundpack_name: &str,
        error: String,
        is_mouse: bool
    ) {
        let soundpack_type = if is_mouse { SoundpackType::Mouse } else { SoundpackType::Keyboard };
        let error_metadata = SoundpackMetadata {
            id: full_soundpack_id.to_string(),
            name: format!("Error: {}", soundpack_name),
            author: None,
            description: Some(format!("Failed to load: {}", error)),
            version: "unknown".to_string(),
            tags: vec!["error".to_string()],
            icon: None,
            soundpack_type,
            folder_path: full_soundpack_id.to_string(), // Use full_soundpack_id as folder path for error entries
            last_modified: 0,
            last_accessed: 0,
            config_version: None,
            is_valid_v2: false,
            validation_status: "error".to_string(),
            can_be_converted: false,
            last_error: Some(error),
        };
        self.soundpacks.insert(full_soundpack_id.to_string(), error_metadata);
    }
}
