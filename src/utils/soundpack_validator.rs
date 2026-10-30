use serde_json::Value;
use std::fs::File;
use std::io::Read;
use std::path::{ Component, Path };
use uuid::Uuid;
use zip::ZipArchive;

// ===== Rules for what is inside a sound pack ZIP =====
//
// Shared by the validator and the installer, so the file a ZIP is checked by
// is the file it is installed with.

/// Largest `config.json` read from a ZIP. Far above any real config.
pub const MAX_CONFIG_BYTES: u64 = 1024 * 1024;

/// Device names Windows reserves in every folder, with or without an extension.
const WINDOWS_RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// True when `name` is one ordinary path component that every system a pack
/// installs on can store as it is.
///
/// No separators, no `.` or `..`, no drive prefix, so a value from the ZIP can
/// never point outside the folder it is joined onto. And none of what Windows
/// refuses or quietly changes: the characters `<>:"|?*` and control
/// characters, a trailing dot or space, and device names such as `CON` or
/// `NUL.txt`. A pack made on Linux must install on Windows too.
pub fn is_portable_name(name: &str) -> bool {
    let mut components = Path::new(name).components();
    if !matches!((components.next(), components.next()), (Some(Component::Normal(_)), None)) {
        return false;
    }
    if name.chars().any(|c| c.is_control() || matches!(c, '/' | '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*')) {
        return false;
    }
    if name.ends_with(['.', ' ']) {
        return false;
    }
    let stem = name.split('.').next().unwrap_or_default().trim_end();
    !WINDOWS_RESERVED.iter().any(|reserved| stem.eq_ignore_ascii_case(reserved))
}

/// Archive clutter that is never part of a pack: macOS resource forks and
/// folder settings files. Skipped wherever a pack is read.
pub fn is_archive_noise(entry_name: &str) -> bool {
    let normalised = entry_name.replace('\\', "/");
    let file = normalised.rsplit('/').next().unwrap_or_default();
    normalised.split('/').any(|part| part == "__MACOSX") ||
        file.starts_with("._") ||
        [".DS_Store", "Thumbs.db", "desktop.ini"].contains(&file)
}

/// The name an entry is installed under. Every file goes to the root of the
/// pack's folder, so only the last segment counts; ZIPs made on Windows can
/// separate folders with `\`, so both separators do. `Ok(None)` means a folder
/// entry or archive clutter.
pub fn entry_file_name(entry_name: &str) -> Result<Option<&str>, String> {
    if is_archive_noise(entry_name) {
        return Ok(None);
    }
    let name = entry_name.rsplit(['/', '\\']).next().unwrap_or_default();
    if name.is_empty() {
        return Ok(None);
    }
    if is_portable_name(name) {
        Ok(Some(name))
    } else {
        Err(format!("The ZIP contains a file with a name that cannot be used: '{}'", entry_name))
    }
}

/// Check every entry name in `archive` and return the index of its
/// `config.json`.
///
/// Files land side by side in one folder, so two entries that would end up
/// with the same name, ignoring case as Windows and macOS do, make a pack
/// whose content depends on the order they are written in. That is refused,
/// and so there is only ever one `config.json` to pick: the one checked here
/// is the one installed.
pub fn locate_pack_config<R: std::io::Read + std::io::Seek>(
    archive: &mut ZipArchive<R>
) -> Result<usize, String> {
    let mut seen = std::collections::HashSet::new();
    let mut config = None;
    for i in 0..archive.len() {
        let entry = archive.by_index(i).map_err(|e| format!("Failed to read archive entry: {}", e))?;
        let Some(name) = entry_file_name(entry.name())?.map(str::to_string) else {
            continue;
        };
        let key = name.to_lowercase();
        if !seen.insert(key.clone()) {
            return Err(
                format!("The ZIP has more than one file named '{}'. Every file in a sound pack needs a name of its own.", name)
            );
        }
        if key == "config.json" {
            config = Some(i);
        }
    }
    config.ok_or_else(|| "No config.json found in ZIP file".to_string())
}

/// Read the `config.json` at `index`, refusing one that unpacks past
/// `MAX_CONFIG_BYTES` before it is held in memory.
pub fn read_pack_config<R: std::io::Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    index: usize
) -> Result<String, String> {
    let entry = archive.by_index(index).map_err(|e| format!("Failed to read archive entry: {}", e))?;
    let mut content = String::new();
    entry
        .take(MAX_CONFIG_BYTES + 1)
        .read_to_string(&mut content)
        .map_err(|e| format!("Failed to read config.json: {}", e))?;
    if (content.len() as u64) > MAX_CONFIG_BYTES {
        return Err("The sound pack's config.json is too large.".to_string());
    }
    Ok(content)
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum SoundpackValidationStatus {
    Valid,
    InvalidVersion,
    InvalidStructure(String),
    MissingRequiredFields(Vec<String>),
    VersionOneNeedsConversion,
    /// The pack declares a config version this build does not understand.
    /// Refusing it with a clear message beats guessing at a layout that was
    /// designed after this build shipped.
    RequiresNewerAppVersion(u32),
}

/// Read `config_version` regardless of whether it was written as a JSON number
/// or as a string.
///
/// Both spellings exist in the wild: every pack this app ships stores `"2"` as
/// a string, because that is what the V1 converter writes, while hand-written
/// and third-party configs commonly use the bare number `2`. Accepting only one
/// form silently misclassifies half of the real corpus.
fn read_config_version(config: &Value) -> Option<u32> {
    let raw = config.get("config_version")?;

    if let Some(n) = raw.as_u64() {
        return u32::try_from(n).ok();
    }

    raw.as_str()?.trim().parse::<u32>().ok()
}

/// The key-definition map, under either name it has been given.
///
/// V2 packs store their key definitions under `definitions`; that is what the
/// converter emits and what the deserializer in `state::soundpack` requires.
/// `defs` is accepted purely as an alias so a config using the shorter spelling
/// is not rejected outright.
fn key_definitions(config: &Value) -> Option<&Value> {
    config.get("definitions").or_else(|| config.get("defs"))
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SoundpackValidationResult {
    pub status: SoundpackValidationStatus,
    pub config_version: Option<u32>,
    pub detected_version: Option<String>,
    pub is_valid_v2: bool,
    pub can_be_converted: bool,
    pub message: String,
}

/// Detect and validate soundpack configuration version and structure
pub fn validate_soundpack_config(config_path: &str) -> SoundpackValidationResult {
    // Try to read and parse the config file
    let content = match crate::utils::path::read_file_contents(config_path) {
        Ok(content) => content,
        Err(e) => {
            return SoundpackValidationResult {
                status: SoundpackValidationStatus::InvalidStructure(format!(
                    "Cannot read config file: {}",
                    e
                )),
                config_version: None,
                detected_version: None,
                is_valid_v2: false,
                can_be_converted: false,
                message: format!("Failed to read config file: {}", e),
            };
        }
    };

    let config: Value = match serde_json::from_str(&content) {
        Ok(config) => config,
        Err(e) => {
            return SoundpackValidationResult {
                status: SoundpackValidationStatus::InvalidStructure(format!("Invalid JSON: {}", e)),
                config_version: None,
                detected_version: None,
                is_valid_v2: false,
                can_be_converted: false,
                message: format!("Invalid JSON format: {}", e),
            };
        }
    };

    // Extract version information
    let config_version = read_config_version(&config);
    let package_version = config
        .get("version")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Check for V1 indicators
    let has_defines = config.get("defines").is_some();
    let has_sound_field = config.get("sound").is_some();
    let has_method_field =
        config.get("method").is_some() || config.get("key_define_type").is_some();

    // Check for V2 indicators
    let has_defs = key_definitions(&config).is_some();
    let _has_source_field = config.get("source").is_some();
    let has_author = config.get("author").is_some();

    // Determine version based on structure
    if let Some(version) = config_version.filter(|v| *v > 2) {
        // Newer than anything this build knows how to read. Say so plainly
        // instead of reporting the unfamiliar layout as missing fields.
        SoundpackValidationResult {
            status: SoundpackValidationStatus::RequiresNewerAppVersion(version),
            config_version: Some(version),
            detected_version: package_version,
            is_valid_v2: false,
            can_be_converted: false,
            message: "This soundpack requires a newer version of Mechvibes".to_string(),
        }
    } else if config_version == Some(2) {
        // Explicitly marked as V2, validate V2 structure
        validate_v2_structure(&config, config_version, package_version)
    } else if config_version == Some(1) || (has_defines && has_sound_field && !has_defs) {
        // Explicitly V1 or has V1 structure (defines + sound + no defs)
        SoundpackValidationResult {
            status: SoundpackValidationStatus::VersionOneNeedsConversion,
            config_version: Some(1),
            detected_version: package_version,
            is_valid_v2: false,
            can_be_converted: true,
            message: if has_method_field {
                "Version 1 soundpack with method field detected, needs conversion to V2 format"
                    .to_string()
            } else {
                "Version 1 soundpack detected, needs conversion to V2 format".to_string()
            },
        }
    } else if has_defs && has_author {
        // Looks like V2 but no explicit version
        validate_v2_structure(&config, None, package_version)
    } else {
        // Unknown or invalid structure
        let mut missing_fields = Vec::new();

        if !has_defs && !has_defines {
            missing_fields.push("definitions or defines".to_string());
        }

        if !config.get("name").is_some() {
            missing_fields.push("name".to_string());
        }
        SoundpackValidationResult {
            status: SoundpackValidationStatus::MissingRequiredFields(missing_fields.clone()),
            config_version: config_version,
            detected_version: package_version,
            is_valid_v2: false,
            can_be_converted: has_defines && has_sound_field, // Can convert if it looks like V1
            message: format!("Missing required fields: {}", missing_fields.join(", ")),
        }
    }
}

/// Validate V2 soundpack structure
fn validate_v2_structure(
    config: &Value,
    config_version: Option<u32>,
    package_version: Option<String>,
) -> SoundpackValidationResult {
    let mut missing_fields = Vec::new();
    let mut issues = Vec::new();

    // Check required V2 fields
    if !config.get("name").is_some() {
        missing_fields.push("name".to_string());
    }

    if !config.get("author").is_some() && !config.get("m_author").is_some() {
        missing_fields.push("author".to_string());
    }

    let definitions = key_definitions(config);

    if definitions.is_none() {
        missing_fields.push("definitions".to_string());
    }

    // In a "multi" pack a key that names its own file may leave out its
    // timing, and then plays the whole file.
    let is_multi = config.get("definition_method").and_then(Value::as_str) == Some("multi");

    // Validate the key-definition structure
    if let Some(defs) = definitions {
        if let Some(defs_obj) = defs.as_object() {
            for (key, value) in defs_obj {
                // A key maps either to `{ "timing": [[start, end], ...] }` (the
                // shape the converter writes and the loader deserializes) or
                // directly to the timing array itself.
                let timings = match value.get("timing") {
                    Some(timing) => timing,
                    None if is_multi && value.get("audio_file").is_some() => continue,
                    None => value,
                };

                let arr = match timings.as_array() {
                    Some(arr) => arr,
                    None => {
                        issues.push(format!(
                            "Invalid definitions entry for '{}': expected timing array",
                            key
                        ));
                        continue;
                    }
                };

                for (i, timing) in arr.iter().enumerate() {
                    if let Some(timing_arr) = timing.as_array() {
                        if timing_arr.len() != 2 {
                            issues.push(format!(
                                "Invalid timing array for '{}[{}]': expected [start, end]",
                                key, i
                            ));
                        }
                    } else {
                        issues.push(format!(
                            "Invalid timing entry for '{}[{}]': expected array",
                            key, i
                        ));
                    }
                }
            }
        } else {
            issues.push("definitions field should be an object".to_string());
        }
    }

    // Check mouse field
    if let Some(mouse) = config.get("mouse") {
        if !mouse.is_boolean() {
            issues.push("mouse field should be boolean".to_string());
        }
    }

    // Determine final status
    if !missing_fields.is_empty() {
        SoundpackValidationResult {
            status: SoundpackValidationStatus::MissingRequiredFields(missing_fields.clone()),
            config_version,
            detected_version: package_version,
            is_valid_v2: false,
            can_be_converted: false,
            message: format!("Missing required V2 fields: {}", missing_fields.join(", ")),
        }
    } else if !issues.is_empty() {
        SoundpackValidationResult {
            status: SoundpackValidationStatus::InvalidStructure(issues.join("; ")),
            config_version,
            detected_version: package_version,
            is_valid_v2: false,
            can_be_converted: false,
            message: format!("V2 structure issues: {}", issues.join("; ")),
        }
    } else {
        SoundpackValidationResult {
            status: SoundpackValidationStatus::Valid,
            config_version: config_version.or(Some(2)), // Default to 2 if not specified but valid
            detected_version: package_version,
            is_valid_v2: true,
            can_be_converted: false,
            message: "Valid V2 soundpack configuration".to_string(),
        }
    }
}

/// Validate ZIP file structure and basic requirements
pub async fn validate_zip_file(file_path: &str) -> Result<(), String> {
    // Check if file exists and is readable
    if !std::path::Path::new(file_path).exists() {
        return Err("File does not exist".to_string());
    }

    // Check file extension
    if !file_path.to_lowercase().ends_with(".zip") {
        return Err("File must be a ZIP archive".to_string());
    }

    // Try to open as ZIP
    let file = File::open(file_path).map_err(|e| format!("Cannot open file: {}", e))?;

    let archive = ZipArchive::new(file).map_err(|e| format!("Invalid ZIP file: {}", e))?;

    // Check if ZIP is not empty
    if archive.len() == 0 {
        return Err("ZIP file is empty".to_string());
    }

    Ok(())
}

/// Validate soundpack structure within ZIP file and return soundpack name and config content
pub async fn validate_soundpack_structure(file_path: &str) -> Result<(String, String), String> {
    let file = File::open(file_path).map_err(|e| format!("Cannot open ZIP file: {}", e))?;

    let mut archive = ZipArchive::new(file).map_err(|e| format!("Invalid ZIP archive: {}", e))?;

    let mut audio_found = false;
    let mut config_method = "single".to_string();
    let mut soundpack_name = "Unknown".to_string();

    // The same entry the installer will read, found by the same rules.
    let config_index = locate_pack_config(&mut archive)?;
    let config_content = read_pack_config(&mut archive, config_index)?;
    if let Ok(config) = serde_json::from_str::<Value>(&config_content) {
        if let Some(name) = config.get("name").and_then(|v| v.as_str()) {
            soundpack_name = name.to_string();
        }
        if let Some(method) = config.get("method").and_then(|v| v.as_str()) {
            config_method = method.to_string();
        }
    }

    // Check for audio files
    for i in 0..archive.len() {
        let file = archive
            .by_index(i)
            .map_err(|e| format!("Cannot read file in archive: {}", e))?;
        let file_name = file.name().to_string();
        if is_archive_noise(&file_name) {
            continue;
        }
        let file_lower = file_name.to_lowercase();
        if file_lower.ends_with(".ogg")
            || file_lower.ends_with(".wav")
            || file_lower.ends_with(".mp3")
            || file_lower.ends_with(".flac")
        {
            audio_found = true;
        }
    }

    // If single method, ensure at least one audio file is present
    if config_method == "single" && !audio_found {
        return Err("No audio files found in soundpack".to_string());
    }

    // Use existing validator to validate config content
    // In the temp folder, not the working folder: an install under Program
    // Files runs from a folder it cannot write to.
    let temp_config_path = std::env::temp_dir()
        .join(format!("mechvibes-validate-{}.json", Uuid::new_v4()))
        .to_string_lossy()
        .to_string();
    std::fs::write(&temp_config_path, &config_content)
        .map_err(|e| format!("Cannot write temp config: {}", e))?;

    let validation_result = validate_soundpack_config(&temp_config_path);

    // Clean up temp file
    let _ = std::fs::remove_file(&temp_config_path);

    // Check validation result
    match validation_result.status {
        SoundpackValidationStatus::Valid => Ok((soundpack_name, config_content)),
        SoundpackValidationStatus::VersionOneNeedsConversion => {
            Ok((soundpack_name, config_content)) // V1 is acceptable, will be converted
        }
        SoundpackValidationStatus::InvalidStructure(msg) => {
            Err(format!("Invalid soundpack structure: {}", msg))
        }
        SoundpackValidationStatus::MissingRequiredFields(fields) => {
            Err(format!("Missing required fields: {}", fields.join(", ")))
        }
        SoundpackValidationStatus::InvalidVersion => {
            Err("Invalid or unsupported soundpack version".to_string())
        }
        SoundpackValidationStatus::RequiresNewerAppVersion(_) => {
            Err("This soundpack requires a newer version of Mechvibes".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exact `config_version` spelling every bundled pack uses: a string,
    /// not a number. Reading it as a number only is what made real V2 packs
    /// fall through to the "missing fields" branch.
    const REAL_PACK_CONFIG: &str =
        r#"{
            "audio_file": "sound.ogg",
            "config_version": "2",
            "created_at": "2025-06-17T12:23:39.537516300+00:00",
            "definition_method": "single",
            "author": "Mechvibes",
            "definitions": {
                "AltLeft": { "timing": [[45750.0, 45832.0], [45832.0, 45914.0]] },
                "Escape": { "timing": [[2894.0, 3007.0], [3007.0, 3120.0]] }
            },
            "id": "keyboad-cherrymx-black-abs",
            "name": "CherryMX Black - ABS keycaps",
            "options": { "random_pitch": false, "recommended_volume": 1.0 },
            "icon": "black.jpg",
            "tags": []
        }"#;

    fn validate_json(contents: &str) -> SoundpackValidationResult {
        let path = std::env
            ::temp_dir()
            .join(format!("mechvibes-validator-{}-{}.json", std::process::id(), uuid::Uuid::new_v4()));
        std::fs::write(&path, contents).expect("write temp config");
        let result = validate_soundpack_config(path.to_str().expect("utf-8 temp path"));
        std::fs::remove_file(&path).ok();
        result
    }

    fn version_of(raw: &str) -> Option<u32> {
        read_config_version(&serde_json::from_str(raw).expect("valid json"))
    }

    #[test]
    fn config_version_is_read_from_both_number_and_string_forms() {
        assert_eq!(version_of(r#"{"config_version": 2}"#), Some(2));
        assert_eq!(version_of(r#"{"config_version": "2"}"#), Some(2));
        assert_eq!(version_of(r#"{"config_version": 1}"#), Some(1));
        assert_eq!(version_of(r#"{"config_version": "1"}"#), Some(1));
        assert_eq!(version_of(r#"{"config_version": 3}"#), Some(3));
        assert_eq!(version_of(r#"{"config_version": "3"}"#), Some(3));
    }

    #[test]
    fn an_unreadable_config_version_is_treated_as_absent() {
        assert_eq!(version_of(r#"{}"#), None, "absent");
        assert_eq!(version_of(r#"{"config_version": null}"#), None, "null");
        assert_eq!(version_of(r#"{"config_version": "banana"}"#), None, "garbage text");
        assert_eq!(version_of(r#"{"config_version": ""}"#), None, "empty string");
        assert_eq!(version_of(r#"{"config_version": -1}"#), None, "negative");
        assert_eq!(version_of(r#"{"config_version": 2.5}"#), None, "fractional");
    }

    /// The defect this pins: a real bundled pack must reach the V2 branch and
    /// be reported valid, with its version recognised as 2.
    #[test]
    fn a_real_bundled_pack_validates_as_v2() {
        let result = validate_json(REAL_PACK_CONFIG);

        assert_eq!(
            result.status,
            SoundpackValidationStatus::Valid,
            "a shipped pack must validate; got: {}",
            result.message
        );
        assert_eq!(result.config_version, Some(2), "the string \"2\" must be understood as version 2");
        assert!(result.is_valid_v2);
        assert!(!result.can_be_converted, "a valid V2 pack needs no conversion");
    }

    /// Every pack in the repository must be loadable. This is the regression
    /// net for the whole shipped corpus, not just the one sampled above.
    #[test]
    fn every_bundled_soundpack_validates() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("soundpacks");

        let mut checked = 0;
        let mut dirs = vec![root.clone()];
        while let Some(dir) = dirs.pop() {
            let entries = match std::fs::read_dir(&dir) {
                Ok(entries) => entries,
                Err(_) => {
                    continue;
                }
            };

            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_dir() {
                    dirs.push(path);
                } else if path.file_name().and_then(|n| n.to_str()) == Some("config.json") {
                    let result = validate_soundpack_config(path.to_str().expect("utf-8 path"));
                    assert_eq!(
                        result.status,
                        SoundpackValidationStatus::Valid,
                        "bundled pack {} must validate; got: {}",
                        path.display(),
                        result.message
                    );
                    assert!(result.is_valid_v2, "bundled pack {} must be valid V2", path.display());
                    checked += 1;
                }
            }
        }

        assert!(checked > 0, "expected to find bundled soundpack configs under {}", root.display());
    }

    /// `defs` is accepted as an alias so a config using the shorter spelling is
    /// not rejected for a naming difference alone.
    #[test]
    fn the_defs_spelling_is_accepted_as_an_alias() {
        let result = validate_json(
            r#"{
                "config_version": 2,
                "name": "alias pack",
                "author": "someone",
                "defs": { "Escape": [[0.0, 100.0]] }
            }"#
        );

        assert_eq!(result.status, SoundpackValidationStatus::Valid, "got: {}", result.message);
    }

    /// A version from the future must say so, in both spellings, rather than
    /// blaming the author for fields this build simply does not know about.
    #[test]
    fn a_newer_config_version_asks_the_user_to_update() {
        for raw in ["3", "\"3\"", "99"] {
            let result = validate_json(
                &format!(r#"{{"config_version": {}, "name": "future", "author": "a"}}"#, raw)
            );

            assert!(
                matches!(result.status, SoundpackValidationStatus::RequiresNewerAppVersion(_)),
                "config_version {} must report a newer app version, got {:?}",
                raw,
                result.status
            );
            assert_eq!(result.message, "This soundpack requires a newer version of Mechvibes");
            assert!(!result.can_be_converted, "a future format cannot be converted by this build");
            assert!(!result.is_valid_v2);
        }
    }

    /// A future pack that happens to carry a V1-looking body must still be
    /// refused as too new, rather than being downgraded and converted.
    #[test]
    fn a_newer_version_is_not_mistaken_for_a_convertible_v1_pack() {
        let result = validate_json(
            r#"{
                "config_version": "3",
                "name": "future",
                "defines": { "1": [0, 100] },
                "sound": "sound.ogg"
            }"#
        );

        assert!(
            matches!(result.status, SoundpackValidationStatus::RequiresNewerAppVersion(_)),
            "got {:?}",
            result.status
        );
        assert!(!result.can_be_converted, "converting a format we cannot read would corrupt it");
    }

    /// V1 packs must keep taking the conversion path untouched.
    #[test]
    fn a_v1_pack_still_asks_to_be_converted() {
        let result = validate_json(
            r#"{
                "config_version": 1,
                "name": "old pack",
                "defines": { "1": [0, 100] },
                "sound": "sound.ogg"
            }"#
        );

        assert_eq!(result.status, SoundpackValidationStatus::VersionOneNeedsConversion);
        assert!(result.can_be_converted);
    }

    /// An unversioned V1 pack is recognised by its shape alone.
    #[test]
    fn an_unversioned_v1_pack_is_detected_by_structure() {
        let result = validate_json(
            r#"{ "name": "old", "defines": { "1": [0, 100] }, "sound": "sound.ogg" }"#
        );

        assert_eq!(result.status, SoundpackValidationStatus::VersionOneNeedsConversion);
        assert!(result.can_be_converted);
    }

    /// A config that is neither shape still reports what it is missing, and
    /// names the field by the spelling packs actually use.
    #[test]
    fn a_config_with_no_definitions_reports_the_missing_field() {
        let result = validate_json(r#"{ "name": "empty" }"#);

        match result.status {
            SoundpackValidationStatus::MissingRequiredFields(fields) => {
                assert!(
                    fields.iter().any(|f| f.contains("definitions")),
                    "the missing field should be named as packs spell it, got {:?}",
                    fields
                );
            }
            other => panic!("expected missing required fields, got {:?}", other),
        }
    }

    /// A key of a multi pack that names its own file may leave out its timing
    /// and play the whole file. Without a file of its own it still needs one,
    /// and a single pack always does.
    #[test]
    fn only_a_multi_key_with_its_own_file_may_leave_out_timing() {
        let multi = r#"{"name":"M","author":"A","config_version":"2","definition_method":"multi","definitions":{"KeyA":{"audio_file":"a.ogg"}}}"#;
        assert_eq!(validate_json(multi).status, SoundpackValidationStatus::Valid);

        let multi_without_file = r#"{"name":"M","author":"A","config_version":"2","definition_method":"multi","definitions":{"KeyA":{}}}"#;
        assert_ne!(validate_json(multi_without_file).status, SoundpackValidationStatus::Valid);

        let single = r#"{"name":"S","author":"A","config_version":"2","definition_method":"single","audio_file":"a.ogg","definitions":{"KeyA":{"audio_file":"a.ogg"}}}"#;
        assert_ne!(validate_json(single).status, SoundpackValidationStatus::Valid);
    }

    /// An in-memory ZIP holding `entries` with small placeholder contents.
    fn archive_of(entries: &[&str]) -> ZipArchive<std::io::Cursor<Vec<u8>>> {
        use std::io::Write;
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for name in entries {
            writer.start_file(*name, zip::write::SimpleFileOptions::default()).expect("entry");
            writer.write_all(br#"{"name":"x"}"#).expect("contents");
        }
        ZipArchive::new(writer.finish().expect("finish")).expect("archive")
    }

    #[test]
    fn names_windows_cannot_store_are_not_portable() {
        for name in ["sound.ogg", "Config.json", "con-sound.ogg", "LPT10.wav", "a b.ogg", "Âm thanh.ogg"] {
            assert!(is_portable_name(name), "{} must be allowed", name);
        }
        for name in [
            "CON", "nul.txt", "Com1.wav", "aux", "a<b.ogg", "a|b", "what?.ogg", "x.", "x ", "a\u{1}b", "..", ".", "a/b",
            "a\\b", "C:x",
        ] {
            assert!(!is_portable_name(name), "{:?} must be refused", name);
        }
    }

    #[test]
    fn archive_clutter_is_skipped() {
        for name in ["__MACOSX/pack/._config.json", "pack/.DS_Store", "._sound.ogg", "Thumbs.db", "pack\\desktop.ini"] {
            assert!(is_archive_noise(name), "{} is clutter", name);
            assert_eq!(entry_file_name(name), Ok(None));
        }
        assert!(!is_archive_noise("pack/config.json"));
    }

    #[test]
    fn only_a_file_named_config_json_is_the_config() {
        let mut archive = archive_of(&["pack/myconfig.json", "pack/sound.ogg"]);
        assert!(locate_pack_config(&mut archive).is_err(), "myconfig.json is not config.json");

        let mut archive = archive_of(&["__MACOSX/pack/._config.json", "a/.DS_Store", ".DS_Store", "pack/config.json"]);
        assert_eq!(locate_pack_config(&mut archive), Ok(3), "clutter is ignored, even when repeated");
    }

    /// Two entries that would land on one name, even differing only in case,
    /// would leave which one is installed to the order they are written in.
    #[test]
    fn two_files_that_end_up_with_one_name_are_refused() {
        for entries in [
            ["config.json", "pack/Config.json"],
            ["a/config.json", "b/config.json"],
            ["a/sound.ogg", "b/SOUND.ogg"],
        ] {
            let mut archive = archive_of(&entries);
            let error = locate_pack_config(&mut archive).expect_err("a name collision must be refused");
            assert!(error.contains("more than one file named"), "{:?}: {}", entries, error);
        }
    }

    #[test]
    fn a_file_name_windows_cannot_store_is_refused() {
        let mut archive = archive_of(&["config.json", "pack/NUL.ogg"]);
        assert!(locate_pack_config(&mut archive).is_err());
    }

    /// The validator reads the config before the installer does, so it has to
    /// stop at the same limit, or a config that unpacks to gigabytes would be
    /// read whole here first.
    #[test]
    fn the_validator_stops_at_the_config_size_limit() {
        use std::io::Write;
        let path = std::env::temp_dir().join(format!("mechvibes-big-config-{}.zip", Uuid::new_v4()));
        let mut writer = zip::ZipWriter::new(File::create(&path).expect("zip file"));
        writer.start_file("config.json", zip::write::SimpleFileOptions::default()).expect("entry");
        writer.write_all(&vec![b' '; (MAX_CONFIG_BYTES as usize) + 1]).expect("contents");
        writer.start_file("sound.ogg", zip::write::SimpleFileOptions::default()).expect("audio");
        writer.finish().expect("finish");

        let runtime = tokio::runtime::Builder::new_current_thread().build().expect("runtime");
        let result = runtime.block_on(validate_soundpack_structure(path.to_str().unwrap()));
        let _ = std::fs::remove_file(&path);
        assert!(result.expect_err("oversized config").contains("too large"));
    }
}
