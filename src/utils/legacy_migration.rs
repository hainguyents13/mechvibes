//! One-time moves from the apps Mechvibes 3 replaces.
//!
//! Mechvibes 2 (the Electron app) kept the packs people added in its own
//! folder, and on macOS the app used to be called MechvibesDX.app, which
//! dragging the new Mechvibes.app into Applications leaves behind. Each step
//! runs once and leaves a marker in the data folder, so a pack the user deletes
//! afterwards does not come back and the question is not asked twice.

use crate::state::paths;
use crate::state::soundpack::SoundpackType;
use crate::utils::soundpack_installer;
use std::path::{ Path, PathBuf };

/// Where Mechvibes 2 kept the packs people added: `custom` inside Electron's
/// user data folder, which is named after the app. That is `%APPDATA%` on
/// Windows, `~/Library/Application Support` on macOS and `~/.config` on Linux.
fn mechvibes2_custom_dir() -> Option<PathBuf> {
    directories::BaseDirs
        ::new()
        .map(|dirs| dirs.config_dir().join("Mechvibes").join("custom"))
}

/// Bring the packs added to Mechvibes 2 into this app, once.
///
/// The originals are copied, not moved, so nothing is lost if a pack fails to
/// come over. Packs in the old format are converted by the regular scan, the
/// same way an imported one is.
pub fn import_mechvibes2_packs() {
    let Some(legacy_dir) = mechvibes2_custom_dir() else {
        return;
    };
    if !legacy_dir.is_dir() {
        return;
    }
    let marker = paths::data::mechvibes2_import_marker();
    if marker.exists() {
        return;
    }

    let imported = import_packs_from(
        &legacy_dir,
        &paths::soundpacks::get_custom_soundpacks_dir()
    );

    // Written whatever came over, so a pack that could not be installed is not
    // retried on every start.
    if let Err(e) = std::fs::write(&marker, "") {
        crate::always_eprint!("⚠️  Could not record the Mechvibes 2 pack import: {}", e);
    }
    if imported.is_empty() {
        return;
    }
    crate::always_print!("📦 Brought over {} sound pack(s) from Mechvibes 2", imported.len());

    // Stamped as added so they show up with the New badge.
    crate::state::config_writer::apply(|config| {
        for folder_path in &imported {
            crate::state::soundpack_library::mark_added(
                &mut config.soundpack_added_at,
                folder_path
            );
        }
    });

    // The list is read from a cache that is only rebuilt when it is empty, so
    // without this the packs would not appear until the user refreshes it.
    let mut cache = crate::state::soundpack::SoundpackCache::load();
    cache.refresh_from_directory();
    cache.save();
}

/// Install every pack in `legacy_dir` into `soundpacks_dir` and return the
/// folder paths of the ones that went in.
///
/// Mechvibes 2 accepted a pack as a folder with a `config.json` or as a ZIP.
/// It only had keyboard sounds, so every pack goes in as a keyboard pack. A
/// pack that cannot be installed, including one already installed here, is
/// logged and skipped, so it never keeps the others out.
fn import_packs_from(legacy_dir: &Path, soundpacks_dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(legacy_dir) else {
        return Vec::new();
    };
    let mut sources: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .collect();
    sources.sort();

    let mut imported = Vec::new();
    for source in sources {
        let result = if source.is_dir() {
            if !source.join("config.json").is_file() {
                continue;
            }
            soundpack_installer::install_folder_into(
                &source,
                Some(SoundpackType::Keyboard),
                soundpacks_dir
            )
        } else if source.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("zip")) {
            soundpack_installer::extract_and_install_into(
                &source.to_string_lossy(),
                Some(SoundpackType::Keyboard),
                soundpacks_dir
            )
        } else {
            continue;
        };

        match result {
            Ok(info) => imported.push(info.folder_path),
            Err(e) => {
                crate::always_eprint!(
                    "⚠️  Could not bring over {} from Mechvibes 2: {}",
                    source.display(),
                    e
                );
            }
        }
    }
    imported
}

/// Whether `exe` runs from an app installed in an Applications folder, rather
/// than straight from the disk image.
#[cfg(any(target_os = "macos", test))]
fn runs_from_installed_app(exe: &Path) -> bool {
    exe.ancestors().any(|dir| {
        dir.extension().is_some_and(|ext| ext == "app") &&
            dir
                .parent()
                .and_then(|parent| parent.file_name())
                .is_some_and(|name| name == "Applications")
    })
}

/// The old MechvibesDX apps among `candidates` worth offering to remove.
///
/// Nothing is offered when this app runs from the disk image: the old app may
/// then be the only copy the user has installed. Nor is the app this one runs
/// from.
#[cfg(any(target_os = "macos", test))]
fn mechvibesdx_apps_to_offer(exe: &Path, candidates: Vec<PathBuf>) -> Vec<PathBuf> {
    if !runs_from_installed_app(exe) {
        return Vec::new();
    }
    candidates
        .into_iter()
        .filter(|app| !exe.starts_with(app) && app.is_dir())
        .collect()
}

/// Offer, once, to move the old MechvibesDX.app to the Trash.
///
/// Both apps carry the same bundle identifier, so keeping the old one around
/// makes it easy to open the wrong one, and the two then play every key twice.
#[cfg(target_os = "macos")]
pub async fn offer_to_trash_mechvibesdx_app() {
    let marker = paths::data::mechvibesdx_app_marker();
    if marker.exists() {
        return;
    }
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let mut candidates = vec![PathBuf::from("/Applications/MechvibesDX.app")];
    if let Some(dirs) = directories::BaseDirs::new() {
        candidates.push(dirs.home_dir().join("Applications").join("MechvibesDX.app"));
    }
    let old_apps = mechvibesdx_apps_to_offer(&exe, candidates);
    if old_apps.is_empty() {
        return;
    }

    // Recorded before asking, so the question comes once whatever the answer.
    if let Err(e) = std::fs::write(&marker, "") {
        crate::always_eprint!("⚠️  Could not record the MechvibesDX cleanup question: {}", e);
    }

    let answer = rfd::AsyncMessageDialog
        ::new()
        .set_level(rfd::MessageLevel::Info)
        .set_title("Mechvibes")
        .set_description(
            "MechvibesDX is now Mechvibes, and the old MechvibesDX app is still in your Applications folder.\n\nMove it to the Trash? Your settings and sound packs stay."
        )
        .set_buttons(rfd::MessageButtons::YesNo)
        .show().await;
    if answer != rfd::MessageDialogResult::Yes {
        return;
    }

    for app in old_apps {
        if let Err(e) = macos::move_to_trash(&app) {
            crate::always_eprint!("⚠️  Could not move {} to the Trash: {}", app.display(), e);
            rfd::AsyncMessageDialog
                ::new()
                .set_level(rfd::MessageLevel::Warning)
                .set_title("Mechvibes")
                .set_description(
                    "Mechvibes could not move the old app. You can drag MechvibesDX from your Applications folder to the Trash."
                )
                .set_buttons(rfd::MessageButtons::Ok)
                .show().await;
            return;
        }
        crate::always_print!("🗑️ Moved {} to the Trash", app.display());
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use objc2::rc::Retained;
    use objc2::runtime::{ AnyClass, AnyObject };
    use objc2::msg_send;
    use objc2_foundation::{ NSError, NSString };
    use std::path::Path;

    /// Move `path` to the user's Trash, where it can still be put back.
    pub fn move_to_trash(path: &Path) -> Result<(), String> {
        let url_class = AnyClass::get(c"NSURL").ok_or("NSURL is unavailable")?;
        let manager_class = AnyClass::get(c"NSFileManager").ok_or(
            "NSFileManager is unavailable"
        )?;
        let path = NSString::from_str(&path.to_string_lossy());

        // SAFETY: `+fileURLWithPath:` takes an NSString and returns an NSURL.
        let url: Option<Retained<AnyObject>> = unsafe {
            msg_send![url_class, fileURLWithPath: &*path]
        };
        let url = url.ok_or("Could not make a file URL")?;

        // SAFETY: `+defaultManager` takes no arguments and returns an object.
        let manager: Option<Retained<AnyObject>> = unsafe {
            msg_send![manager_class, defaultManager]
        };
        let manager = manager.ok_or("NSFileManager.defaultManager is unavailable")?;

        // SAFETY: `-trashItemAtURL:resultingItemURL:error:` returns BOOL and
        // fills in an NSError on failure, which `msg_send!` turns into the
        // Result. A null `resultingItemURL` means the new location is not
        // wanted.
        let result: Result<(), Retained<NSError>> = unsafe {
            msg_send![
                &*manager,
                trashItemAtURL: &*url,
                resultingItemURL: std::ptr::null_mut::<*mut AnyObject>(),
                error: _
            ]
        };
        result.map_err(|e| format!("{}", e.localizedDescription()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// A unique scratch directory per test, so the real data dir is never read.
    fn scratch_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(
            format!("mechvibes-legacy-test-{}", uuid::Uuid::new_v4())
        );
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    /// A Mechvibes 2 pack in the old format, with its sounds in a subfolder.
    fn folder_pack(legacy: &Path, folder: &str, id: &str) {
        let pack = legacy.join(folder);
        std::fs::create_dir_all(pack.join("sounds")).expect("pack dir");
        std::fs
            ::write(
                pack.join("config.json"),
                format!(
                    r#"{{"id":"{}","name":"{}","key_define_type":"single","sound":"sounds/press.ogg","defines":{{}}}}"#,
                    id,
                    folder
                )
            )
            .expect("config");
        std::fs::write(pack.join("sounds").join("press.ogg"), b"press").expect("sound");
    }

    #[test]
    fn folder_packs_come_over_as_keyboard_packs_with_their_subfolders() {
        let legacy = scratch_dir();
        let base = scratch_dir();
        folder_pack(&legacy, "My Pack", "sound-pack-1");

        let imported = import_packs_from(&legacy, &base);

        assert_eq!(imported, vec!["keyboard/sound-pack-1".to_string()]);
        let installed = base.join("keyboard").join("sound-pack-1");
        assert_eq!(
            std::fs::read(installed.join("sounds").join("press.ogg")).expect("sound copied"),
            b"press"
        );
        let config: serde_json::Value = serde_json
            ::from_str(&std::fs::read_to_string(installed.join("config.json")).expect("config"))
            .expect("valid config");
        assert_eq!(config["sound"], "sounds/press.ogg");
        assert!(legacy.join("My Pack").join("config.json").is_file(), "the original stays");
    }

    #[test]
    fn zip_packs_come_over_too() {
        let legacy = scratch_dir();
        let base = scratch_dir();
        let mut writer = zip::ZipWriter::new(
            std::fs::File::create(legacy.join("zipped.zip")).expect("zip file")
        );
        writer
            .start_file("config.json", zip::write::SimpleFileOptions::default())
            .expect("config entry");
        writer.write_all(br#"{"id":"zipped","name":"Zipped","defines":{}}"#).expect("config");
        writer.finish().expect("finish zip");

        assert_eq!(import_packs_from(&legacy, &base), vec!["keyboard/zipped".to_string()]);
    }

    #[test]
    fn a_pack_already_installed_is_skipped_and_the_rest_still_come_over() {
        let legacy = scratch_dir();
        let base = scratch_dir();
        folder_pack(&legacy, "a", "taken");
        folder_pack(&legacy, "b", "free");
        let taken = base.join("keyboard").join("taken");
        std::fs::create_dir_all(&taken).expect("installed pack");
        std::fs::write(taken.join("mine.ogg"), b"mine").expect("installed file");

        assert_eq!(import_packs_from(&legacy, &base), vec!["keyboard/free".to_string()]);
        assert_eq!(
            std::fs::read(taken.join("mine.ogg")).expect("installed pack untouched"),
            b"mine"
        );
    }

    #[test]
    fn folders_without_a_config_and_stray_files_are_ignored() {
        let legacy = scratch_dir();
        let base = scratch_dir();
        std::fs::create_dir_all(legacy.join("empty")).expect("empty folder");
        std::fs::write(legacy.join("notes.txt"), b"hi").expect("stray file");

        assert!(import_packs_from(&legacy, &base).is_empty());
        assert!(!base.join("keyboard").exists());
    }

    #[test]
    fn only_an_installed_app_counts_as_installed() {
        assert!(
            runs_from_installed_app(Path::new("/Applications/Mechvibes.app/Contents/MacOS/mechvibes"))
        );
        assert!(
            runs_from_installed_app(
                Path::new("/Users/me/Applications/Mechvibes.app/Contents/MacOS/mechvibes")
            )
        );
        assert!(
            !runs_from_installed_app(
                Path::new("/Volumes/Mechvibes/Mechvibes.app/Contents/MacOS/mechvibes")
            )
        );
        assert!(!runs_from_installed_app(Path::new("/Users/me/dev/target/release/mechvibes")));
    }

    #[test]
    fn the_old_app_is_offered_only_from_an_installed_copy_and_never_itself() {
        let root = scratch_dir();
        let applications = root.join("Applications");
        let old_app = applications.join("MechvibesDX.app");
        std::fs::create_dir_all(&old_app).expect("old app");
        let missing = root.join("Elsewhere").join("MechvibesDX.app");
        let candidates = || vec![old_app.clone(), missing.clone()];

        let installed = applications.join("Mechvibes.app/Contents/MacOS/mechvibes");
        assert_eq!(mechvibesdx_apps_to_offer(&installed, candidates()), vec![old_app.clone()]);

        let from_dmg = root.join("Volumes/Mechvibes/Mechvibes.app/Contents/MacOS/mechvibes");
        assert!(mechvibesdx_apps_to_offer(&from_dmg, candidates()).is_empty());

        let from_old_app = old_app.join("Contents/MacOS/mechvibes");
        assert!(mechvibesdx_apps_to_offer(&from_old_app, candidates()).is_empty());
    }
}
