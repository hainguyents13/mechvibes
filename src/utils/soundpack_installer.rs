use crate::utils::path;
use serde_json::Value;
use std::fs::File;
use std::io::Read;
use std::path::{ Component, Path };
use uuid::Uuid;
use zip::ZipArchive;

/// True when `name` is exactly one ordinary path component: no separators, no
/// `.` or `..`, no root and no Windows drive prefix. Anything else would let a
/// value from the ZIP point outside the folder it is joined onto.
fn is_single_path_component(name: &str) -> bool {
    let mut components = Path::new(name).components();
    matches!((components.next(), components.next()), (Some(Component::Normal(_)), None)) &&
        !name.contains(['/', '\\', ':'])
}

/// The ID becomes a folder name under the custom sound packs directory, so it
/// has to stay a single folder name.
fn validate_soundpack_id(id: &str) -> Result<(), String> {
    if is_single_path_component(id) {
        Ok(())
    } else {
        Err(format!("The sound pack ID '{}' is not a valid folder name.", id))
    }
}

/// Every file is placed at the root of the install folder, so only the last
/// segment of the entry name is used. ZIPs made on Windows can separate folders
/// with `\`, so both separators count. `Ok(None)` means a folder entry.
fn entry_file_name(entry_name: &str) -> Result<Option<&str>, String> {
    let name = entry_name.rsplit(['/', '\\']).next().unwrap_or_default();
    if name.is_empty() {
        return Ok(None);
    }
    if is_single_path_component(name) {
        Ok(Some(name))
    } else {
        Err(format!("The ZIP contains a file with an unsafe name: '{}'", entry_name))
    }
}

/// Check if a soundpack ID already exists in the app state
pub fn check_soundpack_id_conflict(
    id: &str,
    soundpacks: &[crate::state::soundpack::SoundpackMetadata]
) -> bool {
    soundpacks.iter().any(|pack| pack.id == id)
}

/// Extract soundpack ID from ZIP without extracting files
pub fn get_soundpack_id_from_zip(file_path: &str) -> Result<String, String> {
    let file = File::open(file_path).map_err(|e| format!("Failed to open ZIP file: {}", e))?;
    let mut archive = ZipArchive::new(file).map_err(|e|
        format!("Failed to read ZIP archive: {}", e)
    )?;

    // Find config.json to determine soundpack ID
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read archive entry: {}", e))?;
        let file_path = file.name().to_string();

        if file_path.ends_with("config.json") {
            let mut config_content = String::new();
            file
                .read_to_string(&mut config_content)
                .map_err(|e| format!("Failed to read config.json: {}", e))?;

            // Extract ID from config content only
            let config: Value = serde_json
                ::from_str(&config_content)
                .map_err(|e| format!("Failed to parse config.json: {}", e))?;

            // Check if the config already contains an ID field
            if let Some(id) = config.get("id").and_then(|v| v.as_str()) {
                if !id.trim().is_empty() {
                    validate_soundpack_id(id)?;
                    return Ok(id.to_string());
                }
            }

            // If no ID in config, generate a UUID-based ID
            return Ok(format!("imported-{}", Uuid::new_v4()));
        }
    }

    Err("No config.json found in ZIP file".to_string())
}

/// Extract and install soundpack from ZIP file with specified target type
// Structure to hold soundpack information after extraction
#[derive(Debug, Clone)]
pub struct SoundpackInfo {
    pub name: String,
    pub id: String,
}

pub fn extract_and_install_soundpack_with_type(
    file_path: &str,
    target_type: Option<crate::state::soundpack::SoundpackType>
) -> Result<SoundpackInfo, String> {
    // Open ZIP file
    let file = File::open(file_path).map_err(|e| format!("Failed to open ZIP file: {}", e))?;
    let mut archive = ZipArchive::new(file).map_err(|e|
        format!("Failed to read ZIP archive: {}", e)
    )?;

    // Find config.json to determine soundpack info
    let mut config_content = String::new();
    let mut soundpack_id = String::new();
    let mut found_config = false;

    // First pass: find and read config.json
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read archive entry: {}", e))?;
        let file_path = file.name().to_string();

        // Look for config.json in any directory level
        if file_path.ends_with("config.json") {
            file
                .read_to_string(&mut config_content)
                .map_err(|e| format!("Failed to read config.json: {}", e))?;
            found_config = true;
            break;
        }
    }

    if !found_config {
        return Err("No config.json found in ZIP file".to_string());
    }

    // Parse config to get soundpack info
    let mut config: Value = serde_json
        ::from_str(&config_content)
        .map_err(|e| format!("Failed to parse config.json: {}", e))?;

    let soundpack_name = config
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown Soundpack")
        .to_string();

    // Extract ID from config content only
    if let Some(id) = config.get("id").and_then(|v| v.as_str()) {
        if !id.trim().is_empty() {
            soundpack_id = id.to_string();
        }
    }

    // If no ID in config, generate a UUID-based ID
    if soundpack_id.is_empty() {
        soundpack_id = format!("imported-{}", Uuid::new_v4());
        // Add the generated ID to the config
        config["id"] = Value::String(soundpack_id.clone());
    }
    validate_soundpack_id(&soundpack_id)?;

    // Determine soundpack type - use target type if provided, otherwise auto-detect
    let soundpack_type = if let Some(target) = target_type {
        match target {
            crate::state::soundpack::SoundpackType::Keyboard => "keyboard",
            crate::state::soundpack::SoundpackType::Mouse => "mouse",
        }
    } else {
        // Auto-detect from config
        let is_mouse_soundpack = determine_soundpack_type(&config);
        if is_mouse_soundpack {
            "mouse"
        } else {
            "keyboard"
        }
    };

    // Determine installation directory using soundpack type and ID
    // Custom soundpacks go to system app data directory
    let soundpacks_dir = crate::state::paths::soundpacks::get_custom_soundpacks_dir();
    let install_dir = soundpacks_dir.join(soundpack_type).join(&soundpack_id);

    // Create installation directory
    path
        ::ensure_directory_exists(&install_dir)
        .map_err(|e| format!("Failed to create soundpack directory: {}", e))?;

    // Extract all files
    let mut archive = ZipArchive::new(
        File::open(file_path).map_err(|e| format!("Failed to reopen ZIP: {}", e))?
    ).map_err(|e| format!("Failed to reread ZIP archive: {}", e))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read archive entry: {}", e))?;
        let file_path = file.name().to_string();

        // Skip directories; place every file at the root of the install folder
        let Some(filename) = entry_file_name(&file_path)? else {
            continue;
        };
        let output_path = install_dir.join(filename);

        // Extract file
        let mut output_file = File::create(&output_path).map_err(|e|
            format!("Failed to create output file: {}", e)
        )?;
        std::io
            ::copy(&mut file, &mut output_file)
            .map_err(|e| format!("Failed to extract file: {}", e))?;
    }

    // Write updated config.json with ID if it was generated
    let config_path = install_dir.join("config.json");
    let updated_config = serde_json
        ::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize updated config: {}", e))?;
    std::fs
        ::write(&config_path, updated_config)
        .map_err(|e| format!("Failed to write updated config.json: {}", e))?;

    // Stamp the arrival so the lists can sort by it and badge it as new. The
    // key is `{type}/{id}`, the same folder_path the scanner records.
    let folder_path = format!("{}/{}", soundpack_type, soundpack_id);
    crate::state::config_writer::apply(|config| {
        crate::state::soundpack_library::mark_added(&mut config.soundpack_added_at, &folder_path);
    });

    Ok(SoundpackInfo {
        name: soundpack_name,
        id: soundpack_id,
    })
}

fn determine_soundpack_type(config: &serde_json::Value) -> bool {
    // Check for explicit type field
    if let Some(soundpack_type) = config.get("type") {
        if let Some(type_str) = soundpack_type.as_str() {
            return type_str == "mouse";
        }
    }

    // Check if defs contain mouse-specific keys
    if let Some(defs) = config.get("defs") {
        if let Some(defs_obj) = defs.as_object() {
            for key in defs_obj.keys() {
                if
                    key.starts_with("Mouse") ||
                    key.starts_with("Button") ||
                    key.starts_with("Wheel")
                {
                    return true;
                }
            }
        }
    }

    // Default to keyboard
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_ids_are_accepted() {
        for id in ["cherrymx-black-abs", "imported-1b4e28ba-2fa1-11d2-883f-0016d3cca427", "My Pack 2"] {
            assert!(validate_soundpack_id(id).is_ok(), "{id}");
        }
    }

    #[test]
    fn ids_that_leave_the_sound_packs_folder_are_refused() {
        for id in ["..", ".", "../x", "..\\..\\x", "a/b", "a\\b", "/etc", "C:\\x", "C:x", ""] {
            assert!(validate_soundpack_id(id).is_err(), "{id}");
        }
    }

    #[test]
    fn entries_keep_only_their_file_name() {
        assert_eq!(entry_file_name("config.json"), Ok(Some("config.json")));
        assert_eq!(entry_file_name("pack/audio.ogg"), Ok(Some("audio.ogg")));
        assert_eq!(entry_file_name("pack\\audio.ogg"), Ok(Some("audio.ogg")));
        assert_eq!(entry_file_name("../../audio.ogg"), Ok(Some("audio.ogg")));
        assert_eq!(entry_file_name("..\\..\\evil.dll"), Ok(Some("evil.dll")));
    }

    #[test]
    fn folder_entries_are_skipped() {
        assert_eq!(entry_file_name("pack/"), Ok(None));
        assert_eq!(entry_file_name("pack\\"), Ok(None));
    }

    #[test]
    fn entries_that_escape_the_install_folder_are_refused() {
        for entry in ["..", "pack/..", "pack\\..", "C:evil.dll", "pack/C:evil.dll"] {
            assert!(entry_file_name(entry).is_err(), "{entry}");
        }
    }
}
