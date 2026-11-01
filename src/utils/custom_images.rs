//! Images the user picks for the logo or background, and the ones a theme
//! brings, kept in this app's images folder under a name taken from their
//! content: `img-<32 hex of the SHA-256>.<ext>`.
//!
//! The same image saved twice is one file, and a name never says anything a
//! file brought with it. Each save writes its own partial file first and then
//! puts it into place, so a reader never sees half an image, even when two
//! saves of the same image run at once.
//!
//! The images folder is shared by every copy of Mechvibes on the machine (the
//! installed app, a portable copy, a development build), while each copy has
//! its own settings. So each copy keeps a list of the images it made itself,
//! `owned-images.json` in its own data folder, and only ever clears those
//! once nothing has used them for a while.

use chrono::{ DateTime, Utc };
use serde::{ Deserialize, Serialize };
use sha2::{ Digest, Sha256 };
use std::collections::BTreeMap;
use std::fs;
use std::path::{ Path, PathBuf };
use std::sync::atomic::{ AtomicU64, Ordering };
use std::sync::{ Mutex, MutexGuard };

/// Gives every partial file in this process its own name.
static NEXT_PARTIAL: AtomicU64 = AtomicU64::new(0);

/// Held by every save and by anything that removes images this copy made, so
/// an image is never reused and removed at the same moment.
static IMAGES_LOCK: Mutex<()> = Mutex::new(());

fn images_lock() -> MutexGuard<'static, ()> {
    // The lock guards no data, so one a panic left behind is still good.
    IMAGES_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The images this copy of Mechvibes made, and when each was first seen
/// unused (none while something uses it).
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct OwnedImages {
    #[serde(default)]
    images: BTreeMap<String, Option<DateTime<Utc>>>,
}

/// What reading the list of owned images found.
enum OwnedList {
    Read(OwnedImages),
    /// The file is not a list. It has been moved aside, so it is never
    /// written over, and this copy owns no image until it saves a new one.
    Damaged,
    /// The file could not be read, so it must not be written either.
    Unreadable(String),
}

fn read_owned(path: &Path) -> OwnedList {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return OwnedList::Read(OwnedImages::default());
        }
        Err(e) => {
            return OwnedList::Unreadable(e.to_string());
        }
    };
    match serde_json::from_slice(&bytes) {
        Ok(list) => OwnedList::Read(list),
        Err(e) => {
            crate::always_eprint!("⚠️ {} is damaged ({}), so no image counts as made by this copy", path.display(), e);
            if let crate::state::config::Preserved::Failed(e) = crate::state::config::preserve_corrupt_config(path) {
                return OwnedList::Unreadable(format!("could not move the damaged list aside: {}", e));
            }
            OwnedList::Damaged
        }
    }
}

fn write_owned(path: &Path, list: &OwnedImages) {
    if let Err(e) = crate::utils::data::save_json_to_file_atomically(list, path) {
        crate::always_eprint!("⚠️ Could not save {}: {}", path.display(), e);
    }
}

/// Note `file_name` as an image this copy made. A list that cannot be read is
/// left alone: the image then simply never counts as this copy's own.
fn record_owned(owned: &Path, file_name: &str) {
    let mut list = match read_owned(owned) {
        OwnedList::Read(list) => list,
        OwnedList::Damaged => OwnedImages::default(),
        OwnedList::Unreadable(e) => {
            crate::always_eprint!("⚠️ Could not read {}: {}", owned.display(), e);
            return;
        }
    };
    list.images.insert(file_name.to_string(), None);
    write_owned(owned, &list);
}

/// Clear the time `file_name` was first seen unused, when this copy made it.
fn note_in_use(owned: &Path, file_name: &str) {
    if let OwnedList::Read(mut list) = read_owned(owned) {
        if let Some(unused_since @ Some(_)) = list.images.get_mut(file_name) {
            *unused_since = None;
            write_owned(owned, &list);
        }
    }
}

/// The longest picked file extension kept when the bytes say nothing.
const MAX_EXTENSION_CHARS: usize = 8;

/// Saves `bytes` under a name taken from their content and returns the URL the
/// config stores ("/custom-images/img-<32 hex>.<ext>") and whether this call
/// created the file.
///
/// The extension comes from the bytes. `fallback_ext`, the picked file's own
/// extension, is used only when the bytes are not an image this app knows, so
/// a picked image that 3.3 accepted still works.
pub fn save(bytes: &[u8], fallback_ext: Option<&str>) -> Result<(String, bool), String> {
    let (file_name, created) = save_in(
        &crate::state::paths::data::custom_images_dir(),
        &crate::state::paths::data::owned_images_json(),
        bytes,
        fallback_ext
    )?;
    Ok((format!("/custom-images/{}", file_name), created))
}

/// The URL `save` stores for `bytes`, an image of a type this app knows,
/// worked out without writing anything. A theme that comes with the app is
/// compared with the look on screen this way before its images are saved.
pub fn url_for(bytes: &[u8]) -> Option<String> {
    crate::utils::theme_share::image_extension(bytes)?;
    file_name_for(bytes, None).ok().map(|file_name| format!("/custom-images/{}", file_name))
}

/// The name `save` gives `bytes`: the hash of the content, and the extension
/// of the image type, or `fallback_ext` when the bytes do not say.
fn file_name_for(bytes: &[u8], fallback_ext: Option<&str>) -> Result<String, String> {
    let extension = match crate::utils::theme_share::image_extension(bytes) {
        Some(extension) => extension.to_string(),
        None =>
            fallback_ext
                .and_then(clean_extension)
                .ok_or_else(|| "This is not a PNG, JPEG, GIF, WebP or BMP image.".to_string())?,
    };
    let hash = crate::utils::update_installer::hex_encode(&Sha256::digest(bytes));
    Ok(format!("img-{}.{}", &hash[..32], extension))
}

/// `save` into `dir`, noting an image this call created in the list of owned
/// images at `owned`. Returns the file name and whether this call created it.
/// An image that was already there is not noted: another copy of Mechvibes
/// may have made it. One this copy made counts as in use again, since the
/// caller is about to use it, so the clean-up of unused images starts its
/// wait over.
pub(crate) fn save_in(
    dir: &Path,
    owned: &Path,
    bytes: &[u8],
    fallback_ext: Option<&str>
) -> Result<(String, bool), String> {
    let _lock = images_lock();
    let (file_name, created) = put_in(dir, bytes, fallback_ext)?;
    if created {
        record_owned(owned, &file_name);
    } else {
        note_in_use(owned, &file_name);
    }
    Ok((file_name, created))
}

/// How long after start the clean-up of unused images runs, so it never
/// slows the start.
const CLEAR_UNUSED_AFTER_START: std::time::Duration = std::time::Duration::from_secs(120);

/// How long an image this copy made stays after nothing uses it any more.
const UNUSED_IMAGE_GRACE_DAYS: i64 = 7;

/// A while after start, remove the images this copy made that nothing has
/// used for a week, on a thread of its own.
pub fn clear_unused_images_later() {
    let images_dir = crate::state::paths::data::custom_images_dir();
    if !images_dir.is_dir() {
        return;
    }
    std::thread::spawn(move || {
        std::thread::sleep(CLEAR_UNUSED_AFTER_START);
        let config = crate::state::paths::data::config_json();
        let data_dir = config.parent().map(Path::to_path_buf).unwrap_or_default();
        let files = [config, crate::state::paths::data::saved_themes_json()];
        let removed = sweep_orphans(
            &images_dir,
            &crate::state::paths::data::owned_images_json(),
            || references_in(&files, &data_dir),
            chrono::Duration::days(UNUSED_IMAGE_GRACE_DAYS),
            Utc::now()
        );
        if !removed.is_empty() {
            crate::debug_print!("🧹 Removed {} unused images", removed.len());
        }
    });
}

/// The text of `files` and of every copy of a damaged file kept aside in
/// `data_dir` (`*.corrupt*`), as one string. Read as plain text, so an image
/// named only by an entry this version cannot read, or by a field a newer
/// version added, still counts as used. A file that is not there names
/// nothing; any other failure is an `Err`, so nothing is removed.
fn references_in(files: &[PathBuf], data_dir: &Path) -> Result<String, String> {
    let mut paths = files.to_vec();
    match fs::read_dir(data_dir) {
        Ok(entries) => {
            for entry in entries {
                let entry = entry.map_err(|e| format!("{}: {}", data_dir.display(), e))?;
                if entry.file_name().to_string_lossy().contains(".corrupt") {
                    paths.push(entry.path());
                }
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(format!("{}: {}", data_dir.display(), e));
        }
    }

    let mut text = String::new();
    for path in paths {
        match fs::read(&path) {
            Ok(bytes) => text.push_str(&String::from_utf8_lossy(&bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(format!("{}: {}", path.display(), e));
            }
        }
        text.push('\n');
    }
    Ok(text)
}

/// Remove the images in `images_dir` that the list at `owned` says this copy
/// made and that nothing has used for `grace`. Returns the files removed.
///
/// `read_references` returns the text of everything that can name an image.
/// An image counts as used when its name appears anywhere in it. The wait
/// starts the first time a sweep finds an image unused, and the text is read
/// again just before anything is removed. Only names of the form
/// `img-<32 hex>.<ext>` are ever removed, and only plain files, never a link.
/// When the list or the references cannot be read, nothing is removed.
pub fn sweep_orphans(
    images_dir: &Path,
    owned: &Path,
    read_references: impl Fn() -> Result<String, String>,
    grace: chrono::Duration,
    now: DateTime<Utc>
) -> Vec<PathBuf> {
    let _lock = images_lock();
    let mut list = match read_owned(owned) {
        OwnedList::Read(list) => list,
        OwnedList::Damaged => {
            return Vec::new();
        }
        OwnedList::Unreadable(e) => {
            crate::always_eprint!("⚠️ Unused images were left: could not read {}: {}", owned.display(), e);
            return Vec::new();
        }
    };
    let references = match read_references() {
        Ok(references) => references,
        Err(e) => {
            crate::always_eprint!("⚠️ Unused images were left: could not read {}", e);
            return Vec::new();
        }
    };
    let before = list.clone();

    // An image someone removed by hand needs no more watching.
    list.images.retain(|name, _| !is_gone(&images_dir.join(name)));
    for (name, unused_since) in list.images.iter_mut() {
        if references.contains(name.as_str()) {
            *unused_since = None;
        } else if unused_since.is_none() {
            *unused_since = Some(now);
        }
    }
    let due: Vec<String> = list.images
        .iter()
        .filter(|(name, unused_since)| {
            is_hashed_name(name) && unused_since.is_some_and(|since| now - since > grace)
        })
        .map(|(name, _)| name.clone())
        .collect();

    let mut removed = Vec::new();
    if !due.is_empty() {
        // Something may have started using an image since the first read.
        match read_references() {
            Ok(references) => {
                for name in due {
                    if references.contains(name.as_str()) {
                        list.images.insert(name, None);
                        continue;
                    }
                    match remove_plain_file(&images_dir.join(&name)) {
                        Removal::Removed(path) => {
                            list.images.remove(&name);
                            removed.push(path);
                        }
                        Removal::WasGone => {
                            list.images.remove(&name);
                        }
                        Removal::Kept => {}
                    }
                }
            }
            Err(e) => crate::always_eprint!("⚠️ Unused images were left: could not read {}", e),
        }
    }

    if list != before {
        write_owned(owned, &list);
    }
    removed
}

/// What became of an image the sweep set out to remove.
enum Removal {
    Removed(PathBuf),
    WasGone,
    Kept,
}

/// Remove `path` when it is a plain file. A link or a folder is never
/// followed or removed.
fn remove_plain_file(path: &Path) -> Removal {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() =>
            match fs::remove_file(path) {
                Ok(()) => Removal::Removed(path.to_path_buf()),
                Err(e) => {
                    crate::always_eprint!("⚠️ Could not remove the unused image {}: {}", path.display(), e);
                    Removal::Kept
                }
            }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Removal::WasGone,
        _ => Removal::Kept,
    }
}

/// Whether nothing is at `path` any more.
fn is_gone(path: &Path) -> bool {
    matches!(fs::symlink_metadata(path), Err(e) if e.kind() == std::io::ErrorKind::NotFound)
}

/// Whether `name` is `img-<32 lowercase hex>.<1 to 5 lowercase letters or
/// digits>`, the only form `save` gives an image.
fn is_hashed_name(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("img-") else {
        return false;
    };
    let Some((hash, extension)) = rest.split_once('.') else {
        return false;
    };
    hash.len() == 32 &&
        hash.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')) &&
        (1..=5).contains(&extension.len()) &&
        extension.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}

/// Put `bytes` into `dir` under the name taken from their content. Another
/// copy of Mechvibes may save the same image into the same folder at the same
/// time, so this never relies on the lock.
fn put_in(dir: &Path, bytes: &[u8], fallback_ext: Option<&str>) -> Result<(String, bool), String> {
    let file_name = file_name_for(bytes, fallback_ext)?;
    let path = dir.join(&file_name);

    match fs::metadata(&path) {
        // The same name means the same content, unless an earlier write was
        // cut short. A file of the right size is that image already.
        Ok(existing) if existing.is_file() && existing.len() == (bytes.len() as u64) => {
            Ok((file_name, false))
        }
        Ok(_) => {
            let partial = write_partial(dir, &file_name, bytes)?;
            replace_with(&partial, &path, bytes.len())?;
            Ok((file_name, false))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            crate::utils::path::ensure_directory_exists(dir)?;
            let created = add_in_place(dir, &file_name, bytes)?;
            Ok((file_name, created))
        }
        Err(e) => Err(format!("Could not save the image: {}", e)),
    }
}

/// Put `bytes` in place as `file_name` without replacing a file another save
/// put there in the meantime. Returns whether this call created the file, so
/// only one of two saves of the same image at once says it did, and taking an
/// import back never removes the file the other save stored.
fn add_in_place(dir: &Path, file_name: &str, bytes: &[u8]) -> Result<bool, String> {
    let path = dir.join(file_name);
    let partial = write_partial(dir, file_name, bytes)?;
    // A link, unlike a rename, fails when the name is taken.
    match fs::hard_link(&partial, &path) {
        Ok(()) => {
            let _ = fs::remove_file(&partial);
            Ok(true)
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            if is_whole(&path, bytes.len()) {
                let _ = fs::remove_file(&partial);
            } else {
                replace_with(&partial, &path, bytes.len())?;
            }
            Ok(false)
        }
        // A folder that cannot hold links gets the rename instead.
        Err(_) => {
            replace_with(&partial, &path, bytes.len())?;
            Ok(true)
        }
    }
}

/// Write `bytes` to a partial file of this call's own next to `file_name`, so
/// a reader never sees half an image under the real name.
fn write_partial(dir: &Path, file_name: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    let partial = dir.join(
        format!(
            "{}.{}.{}.partial",
            file_name,
            std::process::id(),
            NEXT_PARTIAL.fetch_add(1, Ordering::Relaxed)
        )
    );
    if let Err(e) = fs::write(&partial, bytes) {
        let _ = fs::remove_file(&partial);
        return Err(format!("Could not save the image: {}", e));
    }
    Ok(partial)
}

/// Rename `partial` over `path`. When the rename fails because another save
/// of the same image got there first, that save's file is kept.
fn replace_with(partial: &Path, path: &Path, len: usize) -> Result<(), String> {
    if let Err(e) = fs::rename(partial, path) {
        let _ = fs::remove_file(partial);
        if !is_whole(path, len) {
            return Err(format!("Could not save the image: {}", e));
        }
    }
    Ok(())
}

/// Whether `path` is a file of `len` bytes: the image its name says, whole.
fn is_whole(path: &Path, len: usize) -> bool {
    fs::metadata(path).is_ok_and(|m| m.is_file() && m.len() == (len as u64))
}

/// A picked file's extension, lowercased, when it is a short plain word.
fn clean_extension(extension: &str) -> Option<String> {
    let extension = extension.trim().to_ascii_lowercase();
    (
        !extension.is_empty() &&
        extension.len() <= MAX_EXTENSION_CHARS &&
        extension.chars().all(|c| c.is_ascii_alphanumeric())
    ).then_some(extension)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nrest-of-a-png";

    /// An empty folder of its own under the system temp folder.
    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mechvibes-images-{}-{}", name, uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Where a test keeps the list of the images it made in `dir`: next to
    /// the images, so removing `dir` removes it too.
    fn owned_list(dir: &Path) -> PathBuf {
        dir.join("owned-images.json")
    }

    /// The images in `dir`.
    fn files_in(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs
            ::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
            .filter(|name| name != "owned-images.json")
            .collect();
        names.sort();
        names
    }

    /// The images the list at `owned` says this copy made.
    fn owned_names(owned: &Path) -> Vec<String> {
        match read_owned(owned) {
            OwnedList::Read(list) => list.images.into_keys().collect(),
            _ => panic!("the list of owned images cannot be read"),
        }
    }

    #[test]
    fn the_same_image_is_saved_once() {
        let dir = temp_dir("once");
        let (first, created) = save_in(&dir, &owned_list(&dir), PNG, None).unwrap();
        assert!(created);
        let (second, created) = save_in(&dir, &owned_list(&dir), PNG, None).unwrap();
        assert!(!created);
        assert_eq!(first, second);
        assert_eq!(files_in(&dir), [first]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn names_come_from_the_bytes_not_the_file_name() {
        let dir = temp_dir("names");
        let hash = crate::utils::update_installer::hex_encode(&Sha256::digest(PNG));
        let (name, _) = save_in(&dir, &owned_list(&dir), PNG, Some("jpg")).unwrap();
        assert_eq!(name, format!("img-{}.png", &hash[..32]));

        let mut other = PNG.to_vec();
        other.push(b'!');
        let (other_name, _) = save_in(&dir, &owned_list(&dir), &other, Some("png")).unwrap();
        assert_ne!(name, other_name);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unrecognised_picked_image_keeps_its_extension() {
        let dir = temp_dir("fallback");
        let (name, created) = save_in(&dir, &owned_list(&dir), b"an image the bytes do not name", Some("JPEG")).unwrap();
        assert!(created);
        assert!(name.starts_with("img-") && name.ends_with(".jpeg"), "{}", name);
        assert!(save_in(&dir, &owned_list(&dir), b"not an image", None).is_err());
        assert!(save_in(&dir, &owned_list(&dir), b"not an image", Some("../x")).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn only_images_this_call_made_are_recorded_as_owned() {
        let dir = temp_dir("owned");
        let owned = owned_list(&dir);

        // Another copy of Mechvibes saved this image into the shared folder.
        let (theirs, _) = put_in(&dir, PNG, None).unwrap();
        let (again, created) = save_in(&dir, &owned, PNG, None).unwrap();
        assert_eq!((again.as_str(), created), (theirs.as_str(), false));
        assert!(owned_names(&owned).is_empty(), "an image that was already there is not this copy's");

        let mut other = PNG.to_vec();
        other.push(b'!');
        let (mine, created) = save_in(&dir, &owned, &other, None).unwrap();
        assert!(created);
        assert_eq!(owned_names(&owned), [mine.clone()]);
        save_in(&dir, &owned, &other, None).unwrap();
        assert_eq!(owned_names(&owned), [mine], "noted once");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_damaged_owned_list_is_kept_aside_and_started_again() {
        let dir = temp_dir("damaged-list");
        let owned = owned_list(&dir);
        fs::write(&owned, b"not a list").unwrap();

        let (name, _) = save_in(&dir, &owned, PNG, None).unwrap();
        assert_eq!(owned_names(&owned), [name]);
        assert_eq!(fs::read(dir.join("owned-images.json.corrupt")).unwrap(), b"not a list");
        let _ = fs::remove_dir_all(&dir);
    }

    /// Two copies of Mechvibes can save into the shared folder at once, and
    /// the lock only covers this one, so the save itself must stand a race.
    #[test]
    fn two_saves_of_the_same_image_at_once_leave_one_whole_file_created_once() {
        let bytes: Vec<u8> = PNG.iter()
            .copied()
            .chain((0..200_000u32).map(|i| (i % 251) as u8))
            .collect();
        for _ in 0..10 {
            let dir = temp_dir("race");
            let start = std::sync::Arc::new(std::sync::Barrier::new(2));
            let saves: Vec<_> = (0..2)
                .map(|_| {
                    let (dir, bytes, start) = (dir.clone(), bytes.clone(), start.clone());
                    std::thread::spawn(move || {
                        start.wait();
                        put_in(&dir, &bytes, None).unwrap()
                    })
                })
                .collect();
            let results: Vec<(String, bool)> = saves
                .into_iter()
                .map(|save| save.join().unwrap())
                .collect();
            assert_eq!(results[0].0, results[1].0);
            assert_eq!(
                results.iter().filter(|(_, created)| *created).count(),
                1,
                "only one save may take the file back out"
            );
            assert_eq!(files_in(&dir), [results[0].0.clone()]);
            assert_eq!(fs::read(dir.join(&results[0].0)).unwrap(), bytes);
            let _ = fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn the_url_of_an_image_is_known_before_it_is_saved() {
        let dir = temp_dir("url");
        let url = url_for(PNG).unwrap();
        let (name, _) = save_in(&dir, &owned_list(&dir), PNG, None).unwrap();
        assert_eq!(url, format!("/custom-images/{}", name));
        assert_eq!(url_for(b"an image the bytes do not name"), None);
        assert_eq!(files_in(&dir), [name], "working out the url writes nothing");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn applying_a_starter_twice_writes_no_new_images() {
        let dir = temp_dir("starter");
        let theme = crate::utils::theme_share
            ::from_bytes(include_bytes!("theme_share/fixtures/v1/custom-full.mvtheme"))
            .unwrap();
        let starter = crate::utils::starter_themes::StarterTheme { slug: "night", bytes: &[], theme };

        let made = starter.save_images_in(&dir, &owned_list(&dir)).unwrap();
        assert_eq!(made.len(), 3);
        let images = files_in(&dir);
        let look = starter.look();
        let logo = look.logo.unwrap();
        let named = [logo.background_image, logo.muted_background_image, look.background.unwrap().background_image];
        for url in named {
            let url = url.expect("the starter's look names its images");
            let file = url.strip_prefix("/custom-images/").unwrap();
            assert!(dir.join(file).is_file(), "{} is where its card says", url);
        }

        assert!(starter.save_images_in(&dir, &owned_list(&dir)).unwrap().is_empty());
        assert_eq!(files_in(&dir), images);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_installed_theme_lands_where_its_look_says() {
        let dir = temp_dir("installed");
        let theme = crate::utils::theme_share
            ::from_bytes(include_bytes!("theme_share/fixtures/v1/custom-full.mvtheme"))
            .unwrap();
        let card = theme.look_at(|_, bytes| url_for(bytes));

        let mut first = theme.clone();
        let created = crate::utils::theme_share::install_images_in(&mut first, &dir, &owned_list(&dir)).unwrap();
        assert!(!created.is_empty());
        let images = files_in(&dir);
        let worn = first.look_at(|_, _| None);
        assert_eq!(worn, card, "its images land where its card says they are");

        let mut second = theme.clone();
        let created = crate::utils::theme_share::install_images_in(&mut second, &dir, &owned_list(&dir)).unwrap();
        assert!(created.is_empty());
        assert_eq!(files_in(&dir), images);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_cut_short_image_with_the_same_name_is_replaced() {
        let dir = temp_dir("cut");
        let (name, _) = save_in(&dir, &owned_list(&dir), PNG, None).unwrap();
        fs::write(dir.join(&name), &PNG[..5]).unwrap();
        let (again, created) = save_in(&dir, &owned_list(&dir), PNG, None).unwrap();
        assert_eq!(again, name);
        assert!(!created, "the name was there before");
        assert_eq!(fs::read(dir.join(&name)).unwrap(), PNG);
        assert_eq!(files_in(&dir), [name]);
        let _ = fs::remove_dir_all(&dir);
    }

    // ===== Clearing unused images =====

    fn grace() -> chrono::Duration {
        chrono::Duration::days(UNUSED_IMAGE_GRACE_DAYS)
    }

    fn days(count: i64) -> chrono::Duration {
        chrono::Duration::days(count)
    }

    /// A sweep of `dir` where `references` is everything that names images.
    fn sweep(dir: &Path, references: &str, now: DateTime<Utc>) -> Vec<PathBuf> {
        sweep_orphans(dir, &owned_list(dir), || Ok(references.to_string()), grace(), now)
    }

    /// A different image for each `seed`.
    fn png(seed: u8) -> Vec<u8> {
        [PNG, &[seed]].concat()
    }

    /// A sweep reading its references from a data folder of its own, the way
    /// the app reads `config.json`, `saved_themes.json` and the damaged copies.
    fn sweep_with_data(dir: &Path, data_dir: &Path, now: DateTime<Utc>) -> Vec<PathBuf> {
        let files = [data_dir.join("config.json"), data_dir.join("saved_themes.json")];
        sweep_orphans(dir, &owned_list(dir), || references_in(&files, data_dir), grace(), now)
    }

    #[test]
    fn an_image_only_another_layout_made_is_never_swept() {
        let dir = temp_dir("another-layout");
        let (theirs, _) = put_in(&dir, &png(1), None).unwrap();
        let (mine, _) = save_in(&dir, &owned_list(&dir), &png(2), None).unwrap();

        let now = Utc::now();
        assert!(sweep(&dir, "", now).is_empty(), "the first sweep only notes what is unused");
        assert_eq!(sweep(&dir, "", now + days(30)), [dir.join(&mine)]);
        assert!(sweep(&dir, "", now + days(60)).is_empty());
        assert_eq!(files_in(&dir), [theirs]);
        assert!(owned_names(&owned_list(&dir)).is_empty(), "a removed image is no longer listed");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_grace_period_starts_when_an_image_is_first_unused() {
        let dir = temp_dir("grace");
        let (name, _) = save_in(&dir, &owned_list(&dir), PNG, None).unwrap();
        let now = Utc::now();
        // Made a month ago, in use until today.
        let month_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(30 * 24 * 60 * 60);
        fs::File::options().write(true).open(dir.join(&name)).unwrap().set_modified(month_ago).unwrap();
        let in_use = format!("\"background_image\": \"/custom-images/{}\"", name);
        assert!(sweep(&dir, &in_use, now - days(1)).is_empty());

        assert!(sweep(&dir, "", now).is_empty(), "first seen unused today");
        assert!(sweep(&dir, "", now + days(6)).is_empty());
        assert_eq!(sweep(&dir, "", now + days(8)), [dir.join(&name)]);
        assert!(files_in(&dir).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_image_used_again_starts_its_wait_over() {
        let dir = temp_dir("used-again");
        let (name, _) = save_in(&dir, &owned_list(&dir), PNG, None).unwrap();
        let now = Utc::now();
        assert!(sweep(&dir, "", now).is_empty());
        let in_use = format!("/custom-images/{}", name);
        assert!(sweep(&dir, &in_use, now + days(3)).is_empty());
        assert!(sweep(&dir, "", now + days(4)).is_empty(), "unused again from day four");
        assert!(sweep(&dir, "", now + days(10)).is_empty());

        // Saving it again means it is about to be used, before any setting says so.
        save_in(&dir, &owned_list(&dir), PNG, None).unwrap();
        assert!(sweep(&dir, "", now + days(30)).is_empty());
        assert_eq!(sweep(&dir, "", now + days(38)), [dir.join(&name)]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_image_reused_during_the_sweep_is_kept() {
        let dir = temp_dir("reused");
        let (name, _) = save_in(&dir, &owned_list(&dir), PNG, None).unwrap();
        let now = Utc::now();
        assert!(sweep(&dir, "", now).is_empty());

        // Unused at the first read, used by the time the sweep removes it.
        let reads = std::cell::Cell::new(0);
        let read_references = || {
            reads.set(reads.get() + 1);
            Ok(if reads.get() == 1 { String::new() } else { format!("/custom-images/{}", name) })
        };
        let removed = sweep_orphans(&dir, &owned_list(&dir), read_references, grace(), now + days(8));
        assert!(removed.is_empty());
        assert_eq!(reads.get(), 2, "the references are read again before removing");
        assert_eq!(files_in(&dir), [name.clone()]);
        assert!(sweep(&dir, "", now + days(9)).is_empty(), "its wait starts over");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_sweep_keeps_images_a_config_or_saved_theme_names() {
        let (dir, data_dir) = (temp_dir("named"), temp_dir("named-data"));
        let owned = owned_list(&dir);
        let (in_config, _) = save_in(&dir, &owned, &png(1), None).unwrap();
        let (in_saved_theme, _) = save_in(&dir, &owned, &png(2), None).unwrap();
        let (unused, _) = save_in(&dir, &owned, &png(3), None).unwrap();
        fs::write(
            data_dir.join("config.json"),
            format!("{{\"logo_customization\": {{\"background_image\": \"/custom-images/{}\"}}}}", in_config)
        ).unwrap();
        fs::write(
            data_dir.join("saved_themes.json"),
            format!(
                "{{\"format\": 1, \"themes\": [{{\"background\": {{\"background_image\": \"/custom-images/{}\"}}}}]}}",
                in_saved_theme
            )
        ).unwrap();

        let now = Utc::now();
        assert!(sweep_with_data(&dir, &data_dir, now).is_empty());
        assert_eq!(sweep_with_data(&dir, &data_dir, now + days(8)), [dir.join(&unused)]);
        assert_eq!(files_in(&dir), {
            let mut kept = vec![in_config, in_saved_theme];
            kept.sort();
            kept
        });
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn the_sweep_keeps_images_named_only_in_unreadable_entries() {
        let (dir, data_dir) = (temp_dir("unreadable"), temp_dir("unreadable-data"));
        let (name, _) = save_in(&dir, &owned_list(&dir), PNG, None).unwrap();
        let saved_themes = data_dir.join("saved_themes.json");
        fs::write(
            &saved_themes,
            format!(
                "{{\"format\": 1, \"themes\": [{{\"id\": \"a\", \"name\": \"Later\", \"kind\": \"from_the_future\", \"colors\": {{\"built_in\": \"Dark\"}}, \"logo\": {{\"background_image\": \"/custom-images/{}\"}}}}]}}",
                name
            )
        ).unwrap();
        let store = crate::state::saved_themes::SavedThemes::load_from(&saved_themes);
        assert!(store.had_unreadable && store.themes().next().is_none(), "the app cannot read this entry");

        let now = Utc::now();
        assert!(sweep_with_data(&dir, &data_dir, now).is_empty());
        assert!(sweep_with_data(&dir, &data_dir, now + days(30)).is_empty());
        assert_eq!(files_in(&dir), [name]);
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn images_named_in_a_corrupt_file_are_kept() {
        let (dir, data_dir) = (temp_dir("corrupt"), temp_dir("corrupt-data"));
        let owned = owned_list(&dir);
        let (in_corrupt, _) = save_in(&dir, &owned, &png(1), None).unwrap();
        let (in_numbered_corrupt, _) = save_in(&dir, &owned, &png(2), None).unwrap();
        fs::write(data_dir.join("config.json.corrupt"), format!("{{ broken /custom-images/{}", in_corrupt)).unwrap();
        fs::write(
            data_dir.join("saved_themes.json.corrupt.2"),
            format!("not json /custom-images/{}", in_numbered_corrupt)
        ).unwrap();

        let now = Utc::now();
        assert!(sweep_with_data(&dir, &data_dir, now).is_empty());
        assert!(sweep_with_data(&dir, &data_dir, now + days(30)).is_empty());
        assert_eq!(files_in(&dir).len(), 2);
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn references_that_cannot_be_read_remove_nothing() {
        let dir = temp_dir("no-references");
        let (name, _) = save_in(&dir, &owned_list(&dir), PNG, None).unwrap();
        let now = Utc::now();
        assert!(sweep(&dir, "", now).is_empty());
        let removed = sweep_orphans(&dir, &owned_list(&dir), || Err("busy".to_string()), grace(), now + days(30));
        assert!(removed.is_empty());

        // A config that is a folder cannot be read.
        let data_dir = temp_dir("no-references-data");
        fs::create_dir(data_dir.join("config.json")).unwrap();
        assert!(sweep_with_data(&dir, &data_dir, now + days(30)).is_empty());
        assert_eq!(files_in(&dir), [name]);
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn the_sweep_only_touches_hashed_names() {
        let dir = temp_dir("hashed");
        let long_ago = Utc::now() - days(365);
        let names = [
            "photo_1a2b3c4d.png",
            "photo_1700000000.png",
            "img-0123456789abcdef0123456789ABCDEF.png",
            "img-0123456789abcdef.png",
            "img-0123456789abcdef0123456789abcdef.toolong",
        ];
        for name in names {
            fs::write(dir.join(name), PNG).unwrap();
        }
        // A list someone edited by hand, claiming files the app never names.
        let list = OwnedImages {
            images: names
                .iter()
                .map(|name| (name.to_string(), Some(long_ago)))
                .chain([("../outside.png".to_string(), Some(long_ago))])
                .collect(),
        };
        crate::utils::data::save_json_to_file_atomically(&list, &owned_list(&dir)).unwrap();

        assert!(sweep(&dir, "", Utc::now()).is_empty());
        assert_eq!(files_in(&dir).len(), names.len());
        assert!(is_hashed_name(&url_for(PNG).unwrap().replace("/custom-images/", "")));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_damaged_owned_list_deletes_nothing() {
        let dir = temp_dir("damaged-sweep");
        let (name, _) = save_in(&dir, &owned_list(&dir), PNG, None).unwrap();
        let now = Utc::now();
        assert!(sweep(&dir, "", now).is_empty());
        fs::write(owned_list(&dir), b"{ cut short").unwrap();

        assert!(sweep(&dir, "", now + days(30)).is_empty());
        assert!(sweep(&dir, "", now + days(60)).is_empty(), "the moved list owns nothing");
        assert_eq!(files_in(&dir), [name, "owned-images.json.corrupt".to_string()]);
        let _ = fs::remove_dir_all(&dir);
    }
}
