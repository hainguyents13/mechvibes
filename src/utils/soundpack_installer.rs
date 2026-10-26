use serde_json::Value;
use std::fs::File;
use std::io::Read;
use std::path::{ Component, Path, PathBuf };
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

/// Read and parse the `config.json` inside a soundpack ZIP.
fn read_config_from_zip(file_path: &str) -> Result<Value, String> {
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

            let config: Value = serde_json
                ::from_str(&config_content)
                .map_err(|e| format!("Failed to parse config.json: {}", e))?;

            return Ok(config);
        }
    }

    Err("No config.json found in ZIP file".to_string())
}

/// The raw ID stored in `config.json`, and whether it had to be minted here.
///
/// One place decides both, because the two callers need the second half for
/// different reasons: the resolver only wants the value, and the installer has
/// to write a minted ID back into the config it saves, or a later scan would
/// mint a different one for the same folder.
fn resolve_soundpack_id(config: &Value) -> (String, bool) {
    match config.get("id").and_then(|v| v.as_str()) {
        Some(id) if !id.trim().is_empty() => (id.to_string(), false),
        _ => (format!("imported-{}", Uuid::new_v4()), true),
    }
}

/// The raw ID stored in `config.json`, generating one when it is absent or blank.
fn soundpack_id_from_config(config: &Value) -> String {
    resolve_soundpack_id(config).0
}

/// Extract soundpack ID from ZIP without extracting files.
///
/// The ID is validated here as well as at the install step: this is what the
/// import dialog resolves before anything is written, and an ID that is not a
/// single folder name must never look acceptable to it.
pub fn get_soundpack_id_from_zip(file_path: &str) -> Result<String, String> {
    let config = read_config_from_zip(file_path)?;
    let soundpack_id = soundpack_id_from_config(&config);
    validate_soundpack_id(&soundpack_id)?;
    Ok(soundpack_id)
}

/// Resolve the soundpack type exactly as the installer does.
fn resolve_soundpack_type(
    config: &Value,
    target_type: Option<crate::state::soundpack::SoundpackType>
) -> &'static str {
    if let Some(target) = target_type {
        match target {
            crate::state::soundpack::SoundpackType::Keyboard => "keyboard",
            crate::state::soundpack::SoundpackType::Mouse => "mouse",
        }
    } else if determine_soundpack_type(config) {
        "mouse"
    } else {
        "keyboard"
    }
}

/// Resolve the directory an import of `file_path` would install into.
///
/// Reads the ZIP's `config.json` and resolves the type the same way
/// `extract_and_install_soundpack_with_type` does, then joins the raw ID.
/// `base` is normally `get_custom_soundpacks_dir()`, so the returned path is
/// exactly what a repeated import would overwrite.
pub fn resolve_soundpack_install_dir(
    file_path: &str,
    target_type: Option<crate::state::soundpack::SoundpackType>,
    base: &Path
) -> Result<PathBuf, String> {
    let config = read_config_from_zip(file_path)?;
    let soundpack_id = soundpack_id_from_config(&config);
    // This is where an untrusted ID becomes a path, so it is validated here too:
    // the dialog asks this before the installer ever runs, and an ID such as
    // `..` would otherwise resolve to a directory outside the sound packs one.
    validate_soundpack_id(&soundpack_id)?;
    let soundpack_type = resolve_soundpack_type(&config, target_type);
    Ok(base.join(soundpack_type).join(&soundpack_id))
}

/// Why an import cannot go ahead.
///
/// The two collisions need different advice, so a caller has to be able to tell
/// them apart: an installed pack can be removed by the user, a built-in one
/// cannot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SoundpackConflict {
    /// A pack already lives at the directory the import would extract into.
    AlreadyInstalled(PathBuf),
    /// The ID belongs to a built-in pack. The import would install fine and the
    /// built-in one would then shadow it in the list, so it would never play.
    ShadowsBuiltin { folder_path: String },
}

/// What to tell the user when the destination is already taken.
fn already_installed_message(soundpack_id: &str) -> String {
    format!(
        "A sound pack with ID '{}' already exists.\nPlease remove the existing sound pack and try again.",
        soundpack_id
    )
}

/// The collision an install into `install_dir` would hit, or `None` when it is
/// safe.
///
/// Two collisions count. An existing directory at the resolved install path:
/// extraction writes into the pack's own folder, so it would replace the
/// installed pack file by file. And an ID that belongs to a built-in pack: the
/// import lands in the custom folder and succeeds, but the built-in one shadows
/// it in the list, so the imported pack never plays and the user cannot tell
/// why. A built-in pack is not on disk under the install base, so an existence
/// check alone can never see it.
///
/// Both callers ask this one function because they need the same answer: the
/// dialog needs to tell the two cases apart for its own message, and the
/// installer only needs to refuse. Two copies of these conditions would drift.
fn soundpack_conflict_at(install_dir: &Path, folder_path: &str) -> Option<SoundpackConflict> {
    if install_dir.exists() {
        return Some(SoundpackConflict::AlreadyInstalled(install_dir.to_path_buf()));
    }
    if crate::state::paths::soundpacks::is_builtin_soundpack(folder_path) {
        return Some(SoundpackConflict::ShadowsBuiltin {
            folder_path: folder_path.to_string(),
        });
    }
    None
}

/// What to tell the user when an import has to be refused.
fn conflict_message(conflict: &SoundpackConflict, soundpack_id: &str) -> String {
    match conflict {
        SoundpackConflict::AlreadyInstalled(_) => already_installed_message(soundpack_id),
        SoundpackConflict::ShadowsBuiltin { folder_path } => {
            let builtin_id = folder_path.rsplit('/').next().unwrap_or(folder_path);
            format!(
                "The ID '{}' belongs to a sound pack that ships with MechvibesDX ({}).\nImporting it would install a pack that never plays.\nPlease use a different ID.",
                builtin_id,
                folder_path
            )
        }
    }
}

/// The collision installing `file_path` would hit, or `None` when it is safe.
pub fn check_soundpack_conflict(
    file_path: &str,
    target_type: Option<crate::state::soundpack::SoundpackType>,
    base: &Path
) -> Result<Option<SoundpackConflict>, String> {
    let install_dir = resolve_soundpack_install_dir(file_path, target_type, base)?;

    let config = read_config_from_zip(file_path)?;
    let folder_path = format!(
        "{}/{}",
        resolve_soundpack_type(&config, target_type),
        soundpack_id_from_config(&config)
    );

    Ok(soundpack_conflict_at(&install_dir, &folder_path))
}

/// Extract and install soundpack from ZIP file with specified target type
// Structure to hold soundpack information after extraction
#[derive(Debug, Clone)]
pub struct SoundpackInfo {
    pub name: String,
    pub id: String,
    /// The key the pack is recorded under: `{type}/{id}`.
    pub folder_path: String,
}

pub fn extract_and_install_soundpack_with_type(
    file_path: &str,
    target_type: Option<crate::state::soundpack::SoundpackType>
) -> Result<SoundpackInfo, String> {
    let info = extract_and_install_into(
        file_path,
        target_type,
        &crate::state::paths::soundpacks::get_custom_soundpacks_dir()
    )?;

    // Stamp the arrival so the lists can sort by it and badge it as new. This
    // sits in the caller rather than in the install itself because it writes the
    // shared application config: installing into a scratch directory, as the
    // tests do, must not touch that.
    crate::state::config_writer::apply(|config| {
        crate::state::soundpack_library::mark_added(
            &mut config.soundpack_added_at,
            &info.folder_path
        );
    });

    Ok(info)
}

/// Extract and install into `soundpacks_dir`.
///
/// Split from the public entry point so the install path can be exercised
/// against a scratch directory: what may be overwritten, and what has to be
/// taken back out when an import fails, is only observable where the files are
/// written, and the real data directory must not be the only place to see it.
fn extract_and_install_into(
    file_path: &str,
    target_type: Option<crate::state::soundpack::SoundpackType>,
    soundpacks_dir: &Path
) -> Result<SoundpackInfo, String> {
    // Open ZIP file
    let file = File::open(file_path).map_err(|e| format!("Failed to open ZIP file: {}", e))?;
    let mut archive = ZipArchive::new(file).map_err(|e|
        format!("Failed to read ZIP archive: {}", e)
    )?;

    // Find config.json to determine soundpack info
    let mut config_content = String::new();
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

    // Resolve the ID the same way the import dialog does, from the parsed
    // config, so the two cannot drift apart.
    let (soundpack_id, id_was_generated) = resolve_soundpack_id(&config);

    // A minted ID is this pack's identity from now on, so it has to be written
    // into the config that gets saved: a later scan of the folder would
    // otherwise mint a different one for it.
    if id_was_generated {
        config["id"] = Value::String(soundpack_id.clone());
    }
    validate_soundpack_id(&soundpack_id)?;

    // Determine soundpack type - use target type if provided, otherwise auto-detect
    let soundpack_type = resolve_soundpack_type(&config, target_type);

    // Determine installation directory using soundpack type and ID
    // Custom soundpacks go to system app data directory
    let install_dir = soundpacks_dir.join(soundpack_type).join(&soundpack_id);

    // The key the scanner records for a pack, and the same string a built-in
    // pack is registered under. The dialog asks this before anything is written
    // so it can show its own message; asking it here as well is what covers
    // every other caller, including the built-in collision, which no existence
    // check can see.
    let folder_path = format!("{}/{}", soundpack_type, soundpack_id);
    if let Some(conflict) = soundpack_conflict_at(&install_dir, &folder_path) {
        return Err(conflict_message(&conflict, &soundpack_id));
    }

    // Create the parent, then the leaf: the leaf's own creation is the step that
    // settles the conflict, so an import that raced past the check above still
    // cannot write into a folder it does not own.
    if let Some(parent) = install_dir.parent() {
        std::fs::create_dir_all(parent).map_err(|e|
            format!("Failed to create soundpack directory: {}", e)
        )?;
    }
    match std::fs::create_dir(&install_dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(already_installed_message(&soundpack_id));
        }
        Err(e) => return Err(format!("Failed to create soundpack directory: {}", e)),
    }

    // Everything below writes into a folder this call created, so a failure has
    // to take it back out: a half-made folder would make the next import report
    // "already exists" for a pack that never shows up in the list. Removing it
    // is safe precisely because the checks above proved it was not there.
    if let Err(error) = extract_files_into(&install_dir, file_path, &config) {
        let _ = std::fs::remove_dir_all(&install_dir);
        return Err(error);
    }

    Ok(SoundpackInfo {
        name: soundpack_name,
        id: soundpack_id,
        folder_path,
    })
}

/// Write every file of `file_path` into `install_dir`, then the saved config.
///
/// Split out so the caller can take the folder back out when any part of it
/// fails, instead of leaving a half-made pack behind.
fn extract_files_into(install_dir: &Path, file_path: &str, config: &Value) -> Result<(), String> {
    // Extract all files
    let mut archive = ZipArchive::new(
        File::open(file_path).map_err(|e| format!("Failed to reopen ZIP: {}", e))?
    ).map_err(|e| format!("Failed to reread ZIP archive: {}", e))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read archive entry: {}", e))?;
        let entry_name = file.name().to_string();

        // Skip directories; place every file at the root of the install folder
        let Some(filename) = entry_file_name(&entry_name)? else {
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
        ::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize updated config: {}", e))?;
    std::fs
        ::write(&config_path, updated_config)
        .map_err(|e| format!("Failed to write updated config.json: {}", e))?;

    Ok(())
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
    use crate::state::soundpack::SoundpackType;
    use std::io::Write;

    const MOUSE_CONFIG: &str =
        r#"{"id":"custom-sound-pack-77777732","defs":{"MouseLeft":{"type":"single","file":"a.ogg"}}}"#;
    const KEYBOARD_CONFIG: &str =
        r#"{"id":"thocky","defs":{"KeyA":{"type":"single","file":"a.ogg"}}}"#;

    /// A unique scratch directory per test, so the real data dir is never read.
    fn scratch_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(
            format!("mechvibes-soundpack-test-{}", Uuid::new_v4())
        );
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    /// Build a ZIP in memory and write it to `dir`, returning its path.
    fn zip_with_config(dir: &Path, name: &str, config: &str) -> String {
        let zip_path = dir.join(name);
        let file = File::create(&zip_path).expect("zip file");
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file("config.json", zip::write::SimpleFileOptions::default())
            .expect("start config entry");
        writer.write_all(config.as_bytes()).expect("write config");
        writer.finish().expect("finish zip");
        zip_path.to_string_lossy().to_string()
    }

    #[test]
    fn mouse_config_without_target_type_resolves_to_mouse_dir() {
        let base = scratch_dir();
        let pack_zip = zip_with_config(&base, "pack.zip", MOUSE_CONFIG);
        let install_dir = resolve_soundpack_install_dir(&pack_zip, None, &base).expect(
            "resolve"
        );
        assert_eq!(install_dir, base.join("mouse").join("custom-sound-pack-77777732"));
    }

    #[test]
    fn explicit_target_type_wins_for_mouse_and_keyboard() {
        let base = scratch_dir();
        let pack_zip = zip_with_config(&base, "pack.zip", MOUSE_CONFIG);

        let mouse_dir = resolve_soundpack_install_dir(
            &pack_zip,
            Some(SoundpackType::Mouse),
            &base
        ).expect("resolve mouse");
        assert_eq!(mouse_dir, base.join("mouse").join("custom-sound-pack-77777732"));

        let keyboard_dir = resolve_soundpack_install_dir(
            &pack_zip,
            Some(SoundpackType::Keyboard),
            &base
        ).expect("resolve keyboard");
        assert_eq!(keyboard_dir, base.join("keyboard").join("custom-sound-pack-77777732"));
    }

    #[test]
    fn keyboard_config_auto_detects_keyboard() {
        let base = scratch_dir();
        let pack_zip = zip_with_config(&base, "pack.zip", KEYBOARD_CONFIG);
        let install_dir = resolve_soundpack_install_dir(&pack_zip, None, &base).expect(
            "resolve"
        );
        assert_eq!(install_dir, base.join("keyboard").join("thocky"));
    }

    #[test]
    fn a_free_destination_is_not_a_conflict() {
        let base = scratch_dir();
        let pack_zip = zip_with_config(&base, "pack.zip", MOUSE_CONFIG);

        assert_eq!(check_soundpack_conflict(&pack_zip, None, &base).expect("free check"), None);
    }

    #[test]
    fn an_existing_destination_is_a_conflict() {
        let base = scratch_dir();
        let pack_zip = zip_with_config(&base, "pack.zip", MOUSE_CONFIG);
        let install_dir = resolve_soundpack_install_dir(&pack_zip, None, &base).expect(
            "resolve"
        );

        std::fs::create_dir_all(&install_dir).expect("existing pack dir");

        assert_eq!(
            check_soundpack_conflict(&pack_zip, None, &base).expect("taken check"),
            Some(SoundpackConflict::AlreadyInstalled(install_dir))
        );
    }

    #[test]
    fn a_builtin_id_is_a_conflict_even_though_the_custom_folder_is_free() {
        let base = scratch_dir();
        // `mouse/ping` ships with the app, so nothing exists under the custom
        // folder and an existence check alone would let the import through. The
        // built-in pack would then shadow the imported one in the list.
        let pack_zip = zip_with_config(
            &base,
            "pack.zip",
            r#"{"id":"ping","type":"mouse","defs":{"MouseLeft":{"type":"single","file":"a.ogg"}}}"#
        );

        assert_eq!(
            check_soundpack_conflict(&pack_zip, None, &base).expect("builtin check"),
            Some(SoundpackConflict::ShadowsBuiltin { folder_path: "mouse/ping".to_string() })
        );
    }

    #[test]
    fn the_installer_refuses_a_destination_that_is_already_taken() {
        let base = scratch_dir();
        let pack_zip = zip_with_config(&base, "pack.zip", MOUSE_CONFIG);
        let install_dir = base.join("mouse").join("custom-sound-pack-77777732");

        std::fs::create_dir_all(&install_dir).expect("existing pack dir");
        let error = extract_and_install_into(&pack_zip, None, &base).expect_err(
            "an existing destination must be refused"
        );
        assert!(
            error.contains("custom-sound-pack-77777732"),
            "the error must name the ID so the user can find the pack, got: {error}"
        );
    }

    #[test]
    fn the_installer_refuses_an_id_a_builtin_pack_shadows() {
        let base = scratch_dir();
        // `mouse/ping` ships with the app, so the custom folder is free and an
        // existence check alone lets the import through: the built-in pack then
        // shadows the imported one in the list and the user cannot tell why.
        let pack_zip = zip_with_config(
            &base,
            "pack.zip",
            r#"{"id":"ping","type":"mouse","defs":{"MouseLeft":{"type":"single","file":"a.ogg"}}}"#
        );

        let error = extract_and_install_into(&pack_zip, None, &base).expect_err(
            "an ID a built-in pack owns must be refused"
        );
        assert!(
            error.contains("ping"),
            "the error must name the pack it collided with, got: {error}"
        );
        assert!(
            !base.join("mouse").join("ping").exists(),
            "a refused import must not leave a folder behind"
        );
    }

    #[test]
    fn a_failed_extraction_takes_the_half_made_folder_back_out() {
        let base = scratch_dir();
        // A name the extractor refuses fails the import after the folder was
        // created, which is what used to leave it behind: the next import then
        // reported "already exists" for a pack that never appeared in the list.
        let pack_zip = zip_with_a_refused_entry(&base, "pack.zip", MOUSE_CONFIG);
        let install_dir = base.join("mouse").join("custom-sound-pack-77777732");

        extract_and_install_into(&pack_zip, None, &base).expect_err(
            "a refused entry name must fail the import"
        );
        assert!(
            !install_dir.exists(),
            "the folder this import created must be removed when it fails"
        );
    }

    #[test]
    fn a_first_install_into_an_empty_data_directory_works() {
        let base = scratch_dir();
        // Nothing under `base` exists yet, so this is the path a first import
        // takes: the parent has to be created before the leaf, or create_dir
        // would fail with NotFound.
        let pack_zip = zip_with_config_and_audio(&base, "pack.zip", MOUSE_CONFIG);
        let install_dir = base.join("mouse").join("custom-sound-pack-77777732");

        let info = extract_and_install_into(&pack_zip, None, &base).expect(
            "a first install must succeed"
        );
        assert_eq!(info.id, "custom-sound-pack-77777732");
        assert_eq!(info.folder_path, "mouse/custom-sound-pack-77777732");
        assert_eq!(
            file_names(&install_dir),
            vec!["audio.ogg".to_string(), "config.json".to_string()],
            "the pack must hold exactly the files the ZIP carried"
        );
        let saved: Value = serde_json
            ::from_str(
                &std::fs::read_to_string(install_dir.join("config.json")).expect(
                    "read the saved config"
                )
            )
            .expect("the saved config must be valid JSON");
        assert_eq!(
            saved.get("id").and_then(|v| v.as_str()),
            Some("custom-sound-pack-77777732"),
            "the saved config must carry the ID the folder is named after"
        );
    }

    #[test]
    fn a_refused_import_leaves_the_installed_pack_alone() {
        let base = scratch_dir();
        // The ZIP carries an `audio.ogg` of its own, so a guard that let the
        // import through would overwrite the installed one.
        let pack_zip = zip_with_config_and_audio(&base, "pack.zip", MOUSE_CONFIG);
        let install_dir = base.join("mouse").join("custom-sound-pack-77777732");
        std::fs::create_dir_all(&install_dir).expect("existing pack dir");
        let installed = install_dir.join("audio.ogg");
        std::fs::write(&installed, b"original").expect("write installed file");

        extract_and_install_into(&pack_zip, None, &base).expect_err(
            "an existing destination must be refused"
        );
        assert_eq!(
            std::fs::read(&installed).expect("the installed file must survive"),
            b"original",
            "a refused import must not rewrite the pack that is already installed"
        );
        assert_eq!(
            file_names(&install_dir),
            vec!["audio.ogg".to_string()],
            "a refused import must not add or remove anything in the folder"
        );
    }

    /// The file names inside `dir`, sorted, so a test can state exactly what a
    /// folder holds.
    fn file_names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs
            ::read_dir(dir)
            .expect("read dir")
            .map(|entry| entry.expect("dir entry").file_name().to_string_lossy().to_string())
            .collect();
        names.sort();
        names
    }

    /// A ZIP with a config plus one audio file, so an import into a pack that
    /// already has an `audio.ogg` would overwrite it.
    fn zip_with_config_and_audio(dir: &Path, name: &str, config: &str) -> String {
        let zip_path = dir.join(name);
        let file = File::create(&zip_path).expect("zip file");
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file("config.json", zip::write::SimpleFileOptions::default())
            .expect("start config entry");
        writer.write_all(config.as_bytes()).expect("write config");
        writer
            .start_file("audio.ogg", zip::write::SimpleFileOptions::default())
            .expect("start audio entry");
        writer.write_all(b"from the zip").expect("write audio");
        writer.finish().expect("finish zip");
        zip_path.to_string_lossy().to_string()
    }

    /// A ZIP with a valid config plus one entry the extractor refuses, so the
    /// import fails after the install folder has been created.
    fn zip_with_a_refused_entry(dir: &Path, name: &str, config: &str) -> String {
        let zip_path = dir.join(name);
        let file = File::create(&zip_path).expect("zip file");
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file("config.json", zip::write::SimpleFileOptions::default())
            .expect("start config entry");
        writer.write_all(config.as_bytes()).expect("write config");
        writer
            .start_file("C:evil.dll", zip::write::SimpleFileOptions::default())
            .expect("start refused entry");
        writer.write_all(b"x").expect("write refused entry");
        writer.finish().expect("finish zip");
        zip_path.to_string_lossy().to_string()
    }

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
