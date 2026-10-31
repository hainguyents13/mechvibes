use crate::debug_print;
use crate::libs::theme::{ BuiltInTheme, Theme };
use crate::state::paths;
use crate::utils::{ data, path };
use crate::utils::auto_updater::AutoUpdateConfig;
use chrono::{ DateTime, Utc };
use serde::{ Deserialize, Serialize };
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MusicPlayerConfig {
    pub current_track_id: Option<String>,
    pub volume: f32, // 0.0 to 100.0
    pub is_muted: bool,
    pub auto_play: bool, // Auto-play music when app starts
    pub music_last_updated: u64, // timestamp for music cache
}

impl Default for MusicPlayerConfig {
    fn default() -> Self {
        Self {
            current_track_id: None,
            volume: 50.0,
            is_muted: false,
            auto_play: false,
            music_last_updated: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LogoCustomization {
    pub border_color: String,
    pub text_color: String,
    pub shadow_color: String,
    pub background_color: String,
    pub background_image: Option<String>, // Path to background image
    pub use_background_image: bool, // Whether to use image instead of color for background
    pub muted_background: String,
    pub muted_background_image: Option<String>, // Path to muted background image
    pub use_muted_background_image: bool, // Whether to use image instead of color for muted background
    pub dimmed_when_muted: bool,
}

impl Default for LogoCustomization {
    fn default() -> Self {
        Self {
            border_color: "var(--color-base-content)".to_string(),
            text_color: "var(--color-base-content)".to_string(),
            shadow_color: "var(--color-base-content)".to_string(),
            background_color: "var(--color-base-200)".to_string(),
            background_image: None,
            use_background_image: false,
            muted_background: "var(--color-base-300)".to_string(),
            muted_background_image: None,
            use_muted_background_image: false,
            dimmed_when_muted: false,
        }
    }
}

impl LogoCustomization {
    /// What the logo looks like while the user's own logo is off: the
    /// theme's main colour with a faint fill of it, faded when sound is off.
    /// Customize starts from this when the saved settings are still the
    /// defaults, so the first change does not turn the logo another colour.
    pub fn theme_look() -> Self {
        let main = "var(--color-primary)".to_string();
        let fill = "oklch(from var(--color-primary) l c h / 0.05)".to_string();
        Self {
            border_color: main.clone(),
            text_color: main.clone(),
            shadow_color: main,
            background_color: fill.clone(),
            muted_background: fill,
            dimmed_when_muted: true,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BackgroundCustomization {
    pub background_color: String,
    pub background_image: Option<String>, // Path to background image
    pub use_image: bool, // Whether to use image instead of color
}

impl Default for BackgroundCustomization {
    fn default() -> Self {
        Self {
            // A theme variable rather than an empty string: the layout
            // interpolates this straight into `background: {};`, so `""`
            // produced the invalid declaration `background: ;` and the app
            // silently kept the theme background - which made Reset, and
            // enabling the toggle before picking a colour, look like no-ops.
            background_color: "var(--color-base-100)".to_string(),
            background_image: None,
            use_image: false,
        }
    }
}

/// Every field defaults independently, so one bad or absent entry costs the
/// user that single setting instead of the whole document. Without this a
/// hand-edited `"volume": "loud"` fails the entire parse, and the load path
/// then writes defaults over the file - theme, customizations and device
/// choices included.
///
/// `deserialize_lenient` additionally absorbs a wrong-typed *value* (`null`,
/// `[]`, a string where a number belongs), which a bare `#[serde(default)]`
/// does not: `default` covers a missing key, not a present-but-invalid one.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AppConfig {
    // Metadata
    pub version: String,
    pub last_updated: DateTime<Utc>,
    pub commit: Option<String>,
    // Audio settings
    pub keyboard_soundpack: String,
    pub mouse_soundpack: String,
    pub volume: f32,
    pub mouse_volume: f32, // Separate volume for mouse sounds
    pub enable_volume_boost: bool, // Enable/disable volume boost to 200%
    pub enable_sound: bool,
    pub enable_keyboard_sound: bool, // Enable/disable keyboard sounds specifically
    pub enable_mouse_sound: bool, // Enable/disable mouse sounds specifically
    // Soundpack library state
    //
    // Both live here rather than in `SoundpackMetadata` because
    // `SoundpackCache::refresh_from_directory` clears the map and rebuilds it
    // from disk, which would wipe anything stored alongside the scanned
    // metadata. Keyed by folder_path, the same id the cache and config use.
    /// When a soundpack was first seen, as a Unix timestamp. Written on import
    /// and on the first scan that observes a pack, so it is a real "added"
    /// time rather than the folder's mtime, which changes whenever a file
    /// inside the pack is edited.
    #[serde(default)]
    pub soundpack_added_at: HashMap<String, u64>,
    /// Soundpacks whose "New" badge has been cleared by selecting them.
    #[serde(default)]
    pub soundpacks_seen: Vec<String>,
    // Device settings
    pub selected_audio_device: Option<String>, // Selected audio output device
    pub enabled_keyboards: Vec<String>, // Enabled physical keyboards (by device instance ID)
    pub enabled_mice: Vec<String>, // Enabled physical mice (by device instance ID)
    // UI settings
    pub theme: Theme,
    pub custom_css: String, // Legacy field for existing custom CSS
    pub logo_customization: LogoCustomization,
    pub enable_logo_customization: bool, // Enable/disable logo customization panel
    pub background_customization: BackgroundCustomization,
    pub enable_background_customization: bool, // Enable/disable background customization panel
    /// The saved theme the look on screen belongs to: a saved theme id, or
    /// "starter:<slug>". None means the Default theme or a look not saved yet.
    pub active_theme_id: Option<String>,
    // Music player settings
    pub music_player: MusicPlayerConfig, // Ambiance settings
    pub ambiance_active_sounds: HashMap<String, f32>, // sound_id -> volume (0.0 to 1.0)
    pub ambiance_global_volume: f32, // 0.0 to 1.0 - global multiplier
    pub ambiance_is_muted: bool,
    // Note: ambiance play state is not persistent - always starts paused
    // System settings
    pub auto_start: bool,
    pub start_minimized: bool, // Start minimized to tray when starting at login
    pub landscape_mode: bool, // Enable/disable landscape mode layout
    pub auto_update: AutoUpdateConfig, // Auto-update settings
    /// Send one anonymous launch event (OS + app version) per start.
    /// Opt-out, so an absent field must land on `true` - `bool::default()`
    /// would silently opt the user out on upgrade from a config predating it.
    #[serde(default = "default_true")]
    pub enable_telemetry: bool,
}

/// serde default for opt-out booleans, which have no `#[derive(Default)]`
/// equivalent since `bool::default()` is `false`.
fn default_true() -> bool {
    true
}

/// Parse a config document, discarding only the entries that cannot be read.
///
/// Each key is tried on its own against a default document; one that fails to
/// deserialize is dropped, and `#[serde(default)]` on the struct then fills
/// that field from `AppConfig::default()`. This matters over a per-field
/// `unwrap_or_default()`: the fallback for a damaged `volume` has to be the
/// config's 1.0, not `f32::default()`, which would silently leave the app
/// inaudible.
///
/// Only a document that is not a JSON object at all - truncated, or not JSON -
/// still fails here, which is what routes the caller to the preserve path.
pub fn parse_lenient(contents: &str) -> Result<AppConfig, String> {
    let value: serde_json::Value = serde_json
        ::from_str(contents)
        .map_err(|e| format!("not valid JSON: {}", e))?;

    let Some(object) = value.as_object() else {
        return Err("config is not a JSON object".to_string());
    };

    // Probe each entry against an otherwise-default document, so a key is
    // judged only on its own merits and cannot be failed by a bad neighbour.
    let mut accepted = serde_json::Map::new();
    for (key, entry) in object {
        // A null soundpack reads as "no pack" rather than as a damaged entry:
        // "" is the engine's supported no-sound state, so honouring the user's
        // null is closer to their intent than restoring the default pack.
        let entry = if
            entry.is_null() &&
            matches!(key.as_str(), "keyboard_soundpack" | "mouse_soundpack")
        {
            serde_json::Value::String(String::new())
        } else {
            entry.clone()
        };

        let mut probe = serde_json::Map::new();
        probe.insert(key.clone(), entry.clone());

        if serde_json::from_value::<AppConfig>(serde_json::Value::Object(probe)).is_ok() {
            accepted.insert(key.clone(), entry);
        } else {
            crate::always_eprint!("⚠️  Ignoring unusable config entry '{}', using its default", key);
        }
    }

    serde_json
        ::from_value(serde_json::Value::Object(accepted))
        .map_err(|e| format!("config could not be rebuilt: {}", e))
}

/// Outcome of trying to move an unparseable config out of the way.
pub(crate) enum Preserved {
    /// There was no file to preserve - a first run, or it was deleted.
    Nothing,
    /// The user's bytes now live at this path and are safe to overwrite from.
    MovedTo(std::path::PathBuf),
    /// The file is still in place and must not be written over.
    Failed(String),
}

/// Move an unparseable config aside so its bytes survive, returning where it
/// went.
///
/// Renaming rather than copying means the original path is free for a fresh
/// default file without a window where both are half-written. An existing
/// `.corrupt` from an earlier failure is never clobbered - the first failure
/// usually holds the settings worth recovering, so later ones get a numbered
/// suffix instead. Works on any file, and the colors list uses it too.
pub(crate) fn preserve_corrupt_config(config_path: &std::path::Path) -> Preserved {
    if !config_path.exists() {
        return Preserved::Nothing;
    }

    let base = {
        let mut name = config_path.as_os_str().to_os_string();
        name.push(".corrupt");
        std::path::PathBuf::from(name)
    };

    // First failure gets the plain `.corrupt` name; later ones are numbered so
    // no earlier rescue is overwritten.
    let mut target = base.clone();
    let mut attempt = 1;
    while target.exists() {
        let mut name = base.as_os_str().to_os_string();
        name.push(format!(".{}", attempt));
        target = std::path::PathBuf::from(name);
        attempt += 1;

        // Refuse to spin forever if something is generating these.
        if attempt > 100 {
            return Preserved::Failed(
                "too many saved copies of a damaged config already exist".to_string()
            );
        }
    }

    match std::fs::rename(config_path, &target) {
        Ok(()) => Preserved::MovedTo(target),
        Err(e) => Preserved::Failed(e.to_string()),
    }
}

impl AppConfig {
    /// Check if config data has changed (excluding metadata fields)
    pub fn data_equals(&self, other: &Self) -> bool {
        // Compare all fields except metadata (version, last_updated, commit)
        self.keyboard_soundpack == other.keyboard_soundpack
            && self.mouse_soundpack == other.mouse_soundpack
            && self.volume == other.volume
            && self.mouse_volume == other.mouse_volume
            && self.enable_volume_boost == other.enable_volume_boost
            && self.enable_sound == other.enable_sound
            && self.enable_keyboard_sound == other.enable_keyboard_sound
            && self.enable_mouse_sound == other.enable_mouse_sound
            && self.selected_audio_device == other.selected_audio_device
            && self.soundpack_added_at == other.soundpack_added_at
            && self.soundpacks_seen == other.soundpacks_seen
            && self.enabled_keyboards == other.enabled_keyboards
            && self.enabled_mice == other.enabled_mice
            && self.theme == other.theme
            && self.custom_css == other.custom_css
            && self.logo_customization == other.logo_customization
            && self.enable_logo_customization == other.enable_logo_customization
            && self.background_customization == other.background_customization
            && self.enable_background_customization == other.enable_background_customization
            && self.active_theme_id == other.active_theme_id
            && self.music_player == other.music_player
            && self.ambiance_active_sounds == other.ambiance_active_sounds
            && self.ambiance_global_volume == other.ambiance_global_volume
            && self.ambiance_is_muted == other.ambiance_is_muted
            && self.auto_start == other.auto_start
            && self.start_minimized == other.start_minimized
            && self.landscape_mode == other.landscape_mode
            && self.auto_update == other.auto_update
            && self.enable_telemetry == other.enable_telemetry
    }

    pub fn load() -> Self {
        let config_path = paths::data::config_json();

        // Ensure data directory exists
        if let Some(parent) = config_path.parent() {
            if let Err(_) = path::ensure_directory_exists(parent) {
                crate::always_eprint!("⚠️  Could not create data directory");
            }
        }

        debug_print!("📖 Loading config from: {}", config_path.display());

        // Read and parse separately: a read failure (first run) and a parse
        // failure (damaged document) need different handling, and only the
        // latter has bytes worth preserving.
        let parsed = std::fs
            ::read_to_string(&config_path)
            .map_err(|e| format!("could not read '{}': {}", config_path.display(), e))
            .and_then(|contents| parse_lenient(&contents));

        match parsed {
            Ok(mut config) => {
                let mut config_updated = false;

                // Migrate old soundpack IDs to new format
                if config.keyboard_soundpack == "oreo" {
                    crate::always_print!("🔄 Migrating keyboard soundpack: oreo -> keyboard/eg-oreo");
                    config.keyboard_soundpack = "keyboard/eg-oreo".to_string();
                    config_updated = true;
                }
                if config.mouse_soundpack == "test-mouse" {
                    crate::always_print!("🔄 Migrating mouse soundpack: test-mouse -> mouse/ping");
                    config.mouse_soundpack = "mouse/ping".to_string();
                    config_updated = true;
                }

                // Drop index-based audio device IDs, reverting to the system
                // default.
                //
                // `output_{index}` resolved by enumeration position, which is
                // not an identity: unplugging any device shifts everything
                // after it down a slot, so the saved index may already point at
                // a different device than the one the user picked. Resolving it
                // one last time would only launder that guess into a
                // permanent-looking name-based ID, so a selection that can no
                // longer be trusted is discarded instead. The user picks again,
                // and from then on the name-based ID stays put.
                //
                // Deliberately does not enumerate devices: that costs hundreds
                // of ms on a path that runs during `config_writer`'s
                // `get_or_init`. Nothing reachable from here may call
                // `config_writer::current()` either - that would re-enter the
                // initialising `OnceLock` and deadlock.
                if let Some(device_id) = config.selected_audio_device.clone() {
                    if crate::libs::device_manager::is_legacy_index_device_id(&device_id) {
                        crate::always_print!(
                            "🔄 Dropping index-based audio device {}: reverting to system default",
                            device_id
                        );
                        config.selected_audio_device = None;
                        config_updated = true;
                    }
                }

                // Sync auto_start with actual registry state
                let actual_auto_start = crate::utils::auto_startup::get_auto_startup_state();
                if config.auto_start != actual_auto_start {
                    crate::always_print!(
                        "🔄 Syncing auto_start config with registry: {} -> {}",
                        config.auto_start,
                        actual_auto_start
                    );
                    config.auto_start = actual_auto_start;
                    config_updated = true;
                }

                // Save if any migrations were applied
                if config_updated {
                    config.last_updated = chrono::Utc::now();
                    let _ = config.save();
                }

                config
            }
            Err(e) => {
                // Per-field tolerance above means reaching here needs the
                // document itself to be unreadable (truncated, not JSON, or
                // unreadable from disk) - not merely one bad setting.
                crate::always_eprint!("❌ Failed to load config file: {}", e);
                crate::always_eprint!("   Config path: {}", config_path.display());

                // Never overwrite the user's file in place: it is the only
                // copy of their theme, customizations and device choices, and
                // a hand-editing slip is recoverable only while those bytes
                // still exist. Move it aside first, and if it cannot be moved,
                // do not write defaults over it at all.
                match preserve_corrupt_config(&config_path) {
                    Preserved::Nothing => {
                        // No file to lose (first run, or it was deleted).
                        let default_config = Self::default();
                        let _ = default_config.save();
                        default_config
                    }
                    Preserved::MovedTo(backup) => {
                        crate::always_eprint!(
                            "   Your previous config was kept at: {}",
                            backup.display()
                        );
                        crate::always_eprint!("   Defaults are in use for this session.");
                        let default_config = Self::default();
                        let _ = default_config.save();
                        default_config
                    }
                    Preserved::Failed(err) => {
                        crate::always_eprint!("   Could not set the damaged config aside: {}", err);
                        crate::always_eprint!(
                            "   Leaving it untouched and running on defaults; \
                             settings changed now will not be saved over it."
                        );
                        Self::default()
                    }
                }
            }
        }
    }

    /// Write this struct over `config.json`, replacing every field.
    ///
    /// Deliberately **not** public. Reaching this from a subsystem means
    /// holding an `AppConfig` across time and writing it back, which reverts
    /// whatever another subsystem changed in between - the defect that shipped
    /// three times from three unrelated call sites. `config_writer::apply` is
    /// the only caller outside this module's own load path, and it always
    /// writes state it owns and has just mutated.
    pub(in crate::state) fn save(&self) -> Result<(), String> {
        let config_path = paths::data::config_json();
        debug_print!("💾 Saving config to: {}", config_path.display());
        debug_print!("   keyboard_soundpack: {}", self.keyboard_soundpack);
        debug_print!("   mouse_soundpack: {}", self.mouse_soundpack);
        data::save_json_to_file_atomically(self, &config_path)
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: crate::utils::constants::APP_VERSION.to_string(),
            last_updated: Utc::now(),
            commit: option_env!("GIT_HASH").map(|s| s.to_string()),
            keyboard_soundpack: "keyboard/eg-oreo".to_string(),
            mouse_soundpack: "mouse/ping".to_string(),
            volume: 1.0,
            mouse_volume: 1.0, // Default mouse volume to 100%
            enable_volume_boost: false, // Default volume boost disabled
            enable_sound: true,
            enable_keyboard_sound: true, // Default keyboard sounds enabled
            enable_mouse_sound: true, // Default mouse sounds enabled
            soundpack_added_at: HashMap::new(),
            soundpacks_seen: Vec::new(),
            selected_audio_device: None, // Default to system default audio device
            enabled_keyboards: Vec::new(), // Default to no keyboards enabled (all keyboards will work)
            enabled_mice: Vec::new(), // Default to no mice enabled (all mice will work)
            theme: Theme::BuiltIn(BuiltInTheme::System), // Default to System theme
            custom_css: String::new(),
            logo_customization: LogoCustomization::default(),
            enable_logo_customization: false, // Default logo customization disabled
            background_customization: BackgroundCustomization::default(),
            enable_background_customization: false, // Default background customization disabled
            active_theme_id: None,
            music_player: MusicPlayerConfig::default(),
            ambiance_active_sounds: HashMap::new(),
            ambiance_global_volume: 0.5, // Default global ambiance volume to 50%
            ambiance_is_muted: false,
            // Note: ambiance play state is not persistent - always starts paused
            auto_start: false,
            start_minimized: false, // Default to not starting minimized
            landscape_mode: false, // Default landscape mode disabled
            auto_update: AutoUpdateConfig::default(), // Default auto-update settings
            enable_telemetry: true, // Opt-out: on by default, disclosed in README and Settings
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_theme_look_matches_the_logo_with_customization_off() {
        use crate::components::logo::logo_style;

        let look = LogoCustomization::theme_look();
        for sound_on in [true, false] {
            let (theme_class, theme_style) = logo_style(None, sound_on, false);
            let (own_class, own_style) = logo_style(Some(&look), sound_on, false);
            // The theme's logo takes its border and text from classes.
            assert!(theme_class.contains("border-primary text-primary"));
            assert!(own_style.contains("border-color: var(--color-primary); color: var(--color-primary);"), "{}", own_style);
            let from_classes = ["border-color: revert-layer", "color: revert-layer"];
            for declaration in theme_style
                .split(';')
                .map(str::trim)
                .filter(|d| !d.is_empty() && !from_classes.contains(d))
            {
                assert!(own_style.contains(declaration), "{:?} missing from {:?}", declaration, own_style);
            }
            assert_eq!(theme_class.contains("opacity-50"), own_class.contains("opacity-50"), "sound on: {}", sound_on);
        }
    }

    /// Why `save()` is `pub(in crate::state)` and `config_writer::apply` is the
    /// only public way to write.
    ///
    /// `save()` rewrites the entire struct, so a writer that mutates a copy it
    /// captured earlier reverts every field someone else changed in between.
    /// This is the mute bug: the volume path held a config from before the mute
    /// click and wrote `enable_sound: true` back over it. The shape is
    /// reproduced here on plain structs because it can no longer be expressed
    /// against the real API - there is no public function that accepts an
    /// `AppConfig` to persist, so a caller has nothing to hold and write back.
    #[test]
    fn holding_a_struct_and_writing_it_back_is_what_reverts_a_concurrent_change() {
        let on_disk = AppConfig::default();

        // Volume path reads the config...
        let mut volume_writer_copy = on_disk.clone();

        // ...then the user mutes, and that write lands on disk first.
        let mut after_mute = on_disk.clone();
        after_mute.enable_sound = false;

        // Volume path now saves the copy it captured before the mute.
        volume_writer_copy.volume = 0.5;
        let persisted = volume_writer_copy;

        assert!(!after_mute.enable_sound, "mute was applied to disk");
        assert!(
            persisted.enable_sound,
            "stale save silently restores enable_sound - which is why no public \
             API accepts a config to write"
        );
    }

    /// Telemetry is opt-out, so a fresh install must have it on. If this ever
    /// flips, the README and the Settings caption both become lies.
    #[test]
    fn telemetry_defaults_to_on() {
        assert!(AppConfig::default().enable_telemetry);
    }

    /// A config written before `enable_telemetry` existed must still load. The
    /// load path resets *every* setting on a deserialize error, so a missing
    /// `#[serde(default)]` here would wipe the user's whole config on upgrade.
    #[test]
    fn a_config_without_the_telemetry_field_still_loads() {
        let mut value = serde_json::to_value(AppConfig::default()).expect("config serializes");
        value.as_object_mut().expect("config is a json object").remove("enable_telemetry");

        let restored: AppConfig = serde_json
            ::from_value(value)
            .expect("configs predating enable_telemetry must deserialize, not reset");

        assert!(restored.enable_telemetry, "an absent field must default to on, not to false");
    }

    /// `data_equals` drives whether a change is persisted at all. A field
    /// missing from it means the Settings toggle appears to work and is
    /// silently forgotten on restart.
    #[test]
    fn toggling_telemetry_counts_as_a_change_worth_saving() {
        let original = AppConfig::default();
        let mut toggled = original.clone();
        toggled.enable_telemetry = !original.enable_telemetry;

        assert!(!toggled.data_equals(&original), "the toggle must mark the config dirty");
    }

    /// Pointing at another saved theme must be saved, or the app forgets
    /// which theme is on screen as soon as it restarts.
    #[test]
    fn changing_the_active_theme_counts_as_a_change_worth_saving() {
        let original = AppConfig::default();
        let mut pointed = original.clone();
        pointed.active_theme_id = Some("starter:retro-beige".to_string());
        assert!(!pointed.data_equals(&original), "pointing at a theme must mark the config dirty");

        let mut moved = pointed.clone();
        moved.active_theme_id = Some("0b6f6a3e-4c1e-4d0b-9a55-2f0d0c1b7e11".to_string());
        assert!(!moved.data_equals(&pointed), "pointing at another theme must mark the config dirty");
    }

    /// A config from before saved themes has no pointer, which means the
    /// Default theme or a look not saved yet.
    #[test]
    fn an_old_config_loads_with_no_active_theme() {
        let mut value = serde_json::to_value(AppConfig::default()).expect("config serializes");
        let object = value.as_object_mut().expect("config is a json object");
        object.remove("active_theme_id");
        object.insert("enable_logo_customization".to_string(), serde_json::json!(true));

        let restored = parse_lenient(&value.to_string()).expect("an old config must load");

        assert_eq!(restored.active_theme_id, None);
        assert!(restored.enable_logo_customization, "the rest of the config must be kept");
    }

    /// The fix: re-read immediately before mutating, so the concurrent change
    /// is already present in the struct that gets written back.
    #[test]
    fn reloading_before_mutating_preserves_a_concurrent_change() {
        let mut on_disk = AppConfig::default();

        // User mutes first.
        on_disk.enable_sound = false;

        // Volume path re-reads *now* rather than reusing an older copy.
        let mut fresh = on_disk.clone();
        fresh.volume = 0.5;

        assert!(!fresh.enable_sound, "mute must survive an unrelated config write");
        assert_eq!(fresh.volume, 0.5, "the volume change must still apply");
    }

    /// Build a config document from the defaults, then apply edits, the way a
    /// user hand-editing `config.json` would.
    fn config_json_with(edits: &[(&str, serde_json::Value)]) -> String {
        let mut value = serde_json::to_value(AppConfig::default()).expect("config serializes");
        let object = value.as_object_mut().expect("config is a json object");
        for (key, entry) in edits {
            object.insert((*key).to_string(), entry.clone());
        }
        value.to_string()
    }

    /// A null where a string belongs used to fail the whole document, which
    /// sent the load path down the arm that wrote defaults over the file.
    /// It must now cost only that one field.
    #[test]
    fn a_null_soundpack_costs_only_that_field() {
        let document = config_json_with(
            &[
                ("keyboard_soundpack", serde_json::Value::Null),
                ("volume", serde_json::json!(0.42)),
                ("custom_css", serde_json::json!("body { color: red; }")),
            ]
        );

        assert!(
            serde_json::from_str::<AppConfig>(&document).is_err(),
            "a null in a non-optional field must really break a strict parse, \
             or this test is not exercising the bug"
        );

        let restored = parse_lenient(&document).expect("a null field must not fail the document");

        assert_eq!(restored.keyboard_soundpack, "", "null must read as the no-pack state");
        assert_eq!(restored.volume, 0.42, "every other setting must survive intact");
        assert_eq!(restored.custom_css, "body { color: red; }", "and so must the rest");
    }

    /// The other shapes a hand-edit produces: an array or a string where a
    /// number belongs, and an outright unknown key. None may cost more than
    /// the field it appears on - and a dropped field must fall back to the
    /// config's own default, not the type's.
    #[test]
    fn wrong_typed_and_unknown_fields_do_not_fail_the_document() {
        let document = config_json_with(
            &[
                ("mouse_soundpack", serde_json::json!([])),
                ("volume", serde_json::json!("loud")),
                ("landscape_mode", serde_json::json!(true)),
                ("a_key_from_a_future_build", serde_json::json!({ "x": 1 })),
            ]
        );

        let restored = parse_lenient(&document).expect(
            "wrong-typed fields must not fail the document"
        );

        assert_eq!(
            restored.mouse_soundpack,
            AppConfig::default().mouse_soundpack,
            "an array is damaged rather than intentional, so it falls back to the default pack"
        );
        assert_eq!(
            restored.volume,
            AppConfig::default().volume,
            "a damaged volume must fall back to the audible config default, not 0.0"
        );
        assert!(restored.landscape_mode, "a valid neighbouring setting must survive");
    }

    /// A missing field is the upgrade case: an older config lacking a newer
    /// key must load with that key defaulted, not reset everything.
    #[test]
    fn a_missing_field_defaults_without_losing_the_rest() {
        let mut value = serde_json::to_value(AppConfig::default()).expect("config serializes");
        let object = value.as_object_mut().expect("config is a json object");
        object.remove("landscape_mode");
        object.insert("custom_css".to_string(), serde_json::json!("body { color: red; }"));

        let restored = parse_lenient(&value.to_string()).expect(
            "a missing field must default rather than fail"
        );

        assert!(!restored.landscape_mode, "the absent field takes its default");
        assert_eq!(
            restored.custom_css,
            "body { color: red; }",
            "the user's other settings must be untouched"
        );
    }

    /// An empty array in place of the whole document, and other non-object
    /// shapes, cannot be salvaged field-by-field - they must be reported as a
    /// parse failure so the caller preserves the file rather than parsing it
    /// into silent defaults.
    #[test]
    fn a_non_object_document_is_a_parse_failure() {
        assert!(parse_lenient("[]").is_err(), "a bare array is not a config");
        assert!(parse_lenient("\"hello\"").is_err(), "a bare string is not a config");
        assert!(parse_lenient("{\"volume\": 0.5").is_err(), "a truncated document must fail");
    }

    /// The opt-out flag must land on `true` when its stored value is damaged,
    /// exactly as when the key is absent - a fallback to `bool::default()`
    /// would silently opt the user out.
    #[test]
    fn a_damaged_telemetry_flag_stays_opted_in() {
        let document = config_json_with(&[("enable_telemetry", serde_json::json!("yes"))]);
        let restored = parse_lenient(&document).expect("a damaged flag must not fail the document");

        assert!(restored.enable_telemetry, "the opt-out default must survive a damaged value");
    }

    /// The destructive case the fix targets: a document too damaged to parse
    /// at all. The user's bytes must end up in `.corrupt`, and the original
    /// path must not still hold them - it is replaced by a fresh default.
    #[test]
    fn a_truncated_config_is_preserved_rather_than_overwritten() {
        let dir = std::env::temp_dir().join(format!("mechvibes-corrupt-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let config_path = dir.join("config.json");

        let original = br#"{"volume": 0.7, "theme": "#.to_vec();
        std::fs::write(&config_path, &original).expect("write truncated config");

        assert!(
            serde_json::from_slice::<AppConfig>(&original).is_err(),
            "this fixture must really be unparseable, or the test proves nothing"
        );

        let preserved = preserve_corrupt_config(&config_path);
        let backup = match preserved {
            Preserved::MovedTo(path) => path,
            _ => panic!("an existing damaged config must be preserved"),
        };

        assert_eq!(
            std::fs::read(&backup).expect("backup readable"),
            original,
            "the user's original bytes must survive verbatim"
        );
        assert!(!config_path.exists(), "the damaged file is moved, not copied");

        std::fs::remove_dir_all(&dir).ok();
    }

    /// A second failure must not erase the first rescue, which is usually the
    /// one still holding the user's real settings.
    #[test]
    fn a_second_failure_does_not_clobber_the_first_rescue() {
        let dir = std::env::temp_dir().join(
            format!("mechvibes-corrupt-twice-{}", std::process::id())
        );
        std::fs::create_dir_all(&dir).expect("temp dir");
        let config_path = dir.join("config.json");

        std::fs::write(&config_path, b"first damaged config").expect("write");
        let first = match preserve_corrupt_config(&config_path) {
            Preserved::MovedTo(path) => path,
            _ => panic!("first preservation must succeed"),
        };

        std::fs::write(&config_path, b"second damaged config").expect("write");
        let second = match preserve_corrupt_config(&config_path) {
            Preserved::MovedTo(path) => path,
            _ => panic!("second preservation must succeed"),
        };

        assert_ne!(first, second, "the second rescue must take a distinct path");
        assert_eq!(
            std::fs::read(&first).expect("first backup readable"),
            b"first damaged config",
            "the earlier rescue must still hold its original bytes"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    /// Nothing on disk means nothing to rescue - writing defaults there is
    /// correct and must not be mistaken for a failure.
    #[test]
    fn a_missing_config_file_is_not_treated_as_a_rescue() {
        let dir = std::env::temp_dir().join(format!("mechvibes-absent-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");

        let preserved = preserve_corrupt_config(&dir.join("config.json"));
        assert!(
            matches!(preserved, Preserved::Nothing),
            "an absent config is a first run, not a rescue"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    /// Guards the "skip redundant writes" half of the fix: re-asserting a value
    /// that already matches must not count as a change, or mount effects will
    /// rewrite the config continuously.
    #[test]
    fn reasserting_an_identical_value_is_not_a_change() {
        let config = AppConfig::default();
        let mut same = config.clone();
        same.volume = config.volume;

        assert!(same.data_equals(&config), "no-op write must not be treated as a change");

        same.volume = config.volume + 0.25;
        assert!(!same.data_equals(&config), "a real change must still be detected");
    }

    /// `config_writer::apply` reverts a mutation that `data_equals` reports as
    /// unchanged, so a field missing from that comparison can never be
    /// persisted: it is written, judged equal, and rolled back. The soundpack
    /// library fields were added without it, which left both the New badge and
    /// sort-by-added-time reading an empty map forever.
    #[test]
    fn the_soundpack_library_fields_count_as_real_changes() {
        let base = AppConfig::default();

        let mut stamped = base.clone();
        stamped.soundpack_added_at.insert("keyboard/one".to_string(), 1_700_000_000);
        assert!(
            !base.data_equals(&stamped),
            "an added-at stamp must count as a change, or apply() rolls it back"
        );

        let mut seen = base.clone();
        seen.soundpacks_seen.push("keyboard/one".to_string());
        assert!(
            !base.data_equals(&seen),
            "clearing a New badge must count as a change, or apply() rolls it back"
        );
    }

    /// Every setting must count as a change in `data_equals`, or
    /// `config_writer::apply` judges a real edit equal and rolls it back. A
    /// hand-written list of fields cannot promise that, so this walks the
    /// serialized config instead: a field added later is covered without
    /// anyone remembering to add it here.
    #[test]
    fn every_config_field_counts_as_a_change() {
        use serde_json::{ json, Value };

        /// Fields that only describe the document, which `data_equals` skips
        /// on purpose.
        const METADATA: [&str; 3] = ["version", "last_updated", "commit"];

        let default = AppConfig::default();
        let document = serde_json::to_value(&default).expect("config serializes");
        let fields = document.as_object().expect("config is a json object");

        let mut auto_update = document["auto_update"].clone();
        auto_update["last_check"] = json!(1_700_000_000u64);

        // Values the generic edit below cannot make: a `null`, an empty list
        // or map has nothing to change, and a built-in theme's name cannot
        // just grow a letter.
        let overrides: HashMap<&str, Value> = HashMap::from([
            ("theme", json!({ "Custom": "my-colors" })),
            ("selected_audio_device", json!("output_name:0b3a976597860826")),
            ("soundpack_added_at", json!({ "keyboard/one": 1_700_000_000u64 })),
            ("soundpacks_seen", json!(["keyboard/one"])),
            ("enabled_keyboards", json!(["keyboard-1"])),
            ("enabled_mice", json!(["mouse-1"])),
            ("ambiance_active_sounds", json!({ "rain": 0.5 })),
            ("active_theme_id", json!("starter:retro-beige")),
            ("auto_update", auto_update),
        ]);

        /// Another valid value of the same shape: a flipped bool, a number
        /// plus one, a string with one more letter, or an object with its
        /// first such leaf changed. `None` when there is nothing to change.
        fn changed(value: &Value) -> Option<Value> {
            match value {
                Value::Bool(flag) => Some(Value::Bool(!flag)),
                Value::Number(number) =>
                    Some(
                        if let Some(n) = number.as_u64() {
                            json!(n + 1)
                        } else if let Some(n) = number.as_i64() {
                            json!(n + 1)
                        } else {
                            json!(number.as_f64()? + 1.0)
                        }
                    ),
                Value::String(text) => Some(Value::String(format!("{}x", text))),
                Value::Object(object) =>
                    object.iter().find_map(|(key, leaf)| {
                        let leaf = changed(leaf)?;
                        let mut object = object.clone();
                        object.insert(key.clone(), leaf);
                        Some(Value::Object(object))
                    }),
                Value::Null | Value::Array(_) => None,
            }
        }

        for key in overrides.keys() {
            assert!(fields.contains_key(*key), "the override for '{}' names no config field", key);
        }

        for (key, value) in fields {
            if METADATA.contains(&key.as_str()) {
                continue;
            }
            let new_value = overrides
                .get(key.as_str())
                .cloned()
                .or_else(|| changed(value))
                .unwrap_or_else(|| panic!("'{}' has nothing to change: add an override for it", key));
            assert_ne!(&new_value, value, "the override for '{}' must differ from the default", key);

            let mut edited = document.clone();
            edited[key.as_str()] = new_value.clone();
            let config: AppConfig = serde_json
                ::from_value(edited)
                .unwrap_or_else(|e| panic!("the changed '{}' is not a valid value ({}): add an override for it", key, e));

            let round_trip = serde_json::to_value(&config).expect("config serializes");
            assert_eq!(
                round_trip[key.as_str()],
                new_value,
                "the changed '{}' did not survive a round trip: add an override for it",
                key
            );
            assert!(
                !default.data_equals(&config),
                "changing '{}' is not a change to data_equals, so config_writer would roll it back",
                key
            );
        }
    }

    /// A device saved as `output_{index}` cannot be trusted: the index is a
    /// position in the enumeration, so unplugging anything ahead of it makes
    /// it point somewhere else. Config load drops it rather than resolving it,
    /// which would only launder the guess into a permanent-looking id.
    #[test]
    fn an_index_based_device_id_is_recognised_as_legacy_and_droppable() {
        use crate::libs::device_manager::is_legacy_index_device_id;

        assert!(is_legacy_index_device_id("output_2"));
        assert!(is_legacy_index_device_id("output_0"));

        // Name-based ids and the default sentinel survive: they are stable, so
        // there is nothing to drop.
        assert!(!is_legacy_index_device_id("output_name:0b3a976597860826"));
        assert!(!is_legacy_index_device_id("output_default"));
    }
}
