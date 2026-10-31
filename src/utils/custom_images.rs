//! Images the user picks for the logo or background, and the ones a theme
//! brings, kept in this app's images folder under a name taken from their
//! content: `img-<32 hex of the SHA-256>.<ext>`.
//!
//! The same image saved twice is one file, and a name never says anything a
//! file brought with it. Each save writes its own partial file first and then
//! puts it into place, so a reader never sees half an image, even when two
//! saves of the same image run at once.

use sha2::{ Digest, Sha256 };
use std::fs;
use std::path::{ Path, PathBuf };
use std::sync::atomic::{ AtomicU64, Ordering };

/// Gives every partial file in this process its own name.
static NEXT_PARTIAL: AtomicU64 = AtomicU64::new(0);

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
        bytes,
        fallback_ext
    )?;
    Ok((format!("/custom-images/{}", file_name), created))
}

/// `save` into `dir`. Returns the file name and whether this call created it.
pub(crate) fn save_in(
    dir: &Path,
    bytes: &[u8],
    fallback_ext: Option<&str>
) -> Result<(String, bool), String> {
    let extension = match crate::utils::theme_share::image_extension(bytes) {
        Some(extension) => extension.to_string(),
        None =>
            fallback_ext
                .and_then(clean_extension)
                .ok_or_else(|| "This is not a PNG, JPEG, GIF, WebP or BMP image.".to_string())?,
    };
    let hash = crate::utils::update_installer::hex_encode(&Sha256::digest(bytes));
    let file_name = format!("img-{}.{}", &hash[..32], extension);
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

    fn files_in(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs
            ::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn the_same_image_is_saved_once() {
        let dir = temp_dir("once");
        let (first, created) = save_in(&dir, PNG, None).unwrap();
        assert!(created);
        let (second, created) = save_in(&dir, PNG, None).unwrap();
        assert!(!created);
        assert_eq!(first, second);
        assert_eq!(files_in(&dir), [first]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn names_come_from_the_bytes_not_the_file_name() {
        let dir = temp_dir("names");
        let hash = crate::utils::update_installer::hex_encode(&Sha256::digest(PNG));
        let (name, _) = save_in(&dir, PNG, Some("jpg")).unwrap();
        assert_eq!(name, format!("img-{}.png", &hash[..32]));

        let mut other = PNG.to_vec();
        other.push(b'!');
        let (other_name, _) = save_in(&dir, &other, Some("png")).unwrap();
        assert_ne!(name, other_name);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unrecognised_picked_image_keeps_its_extension() {
        let dir = temp_dir("fallback");
        let (name, created) = save_in(&dir, b"an image the bytes do not name", Some("JPEG")).unwrap();
        assert!(created);
        assert!(name.starts_with("img-") && name.ends_with(".jpeg"), "{}", name);
        assert!(save_in(&dir, b"not an image", None).is_err());
        assert!(save_in(&dir, b"not an image", Some("../x")).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

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
                        save_in(&dir, &bytes, None).unwrap()
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
    fn a_cut_short_image_with_the_same_name_is_replaced() {
        let dir = temp_dir("cut");
        let (name, _) = save_in(&dir, PNG, None).unwrap();
        fs::write(dir.join(&name), &PNG[..5]).unwrap();
        let (again, created) = save_in(&dir, PNG, None).unwrap();
        assert_eq!(again, name);
        assert!(!created, "the name was there before");
        assert_eq!(fs::read(dir.join(&name)).unwrap(), PNG);
        assert_eq!(files_in(&dir), [name]);
        let _ = fs::remove_dir_all(&dir);
    }
}
