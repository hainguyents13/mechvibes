//! Sounds the user adds to Mood from files of their own: music, lo-fi, any
//! background they want on repeat.
//!
//! A picked file is copied into this copy's Mood sounds folder, so moving or
//! deleting the original later does not silence it. The copy is named from
//! its content, `sound-<32 hex of the SHA-256>.<ext>`, so adding the same file
//! twice finds the one already there. A file is checked by decoding its first
//! sample before it is kept, so the list never holds something the player
//! cannot play.

use crate::state::config::CustomMoodSound;
use sha2::{ Digest, Sha256 };
use std::fs::{ self, File };
use std::io::{ BufReader, Read, Write };
use std::path::{ Path, PathBuf };

/// The kinds of file the player decodes, also offered by the file picker.
pub const SUPPORTED_EXTENSIONS: &[&str] = &["mp3", "wav", "ogg", "flac", "m4a"];

/// The longest name kept from the picked file, in characters.
const MAX_NAME_CHARS: usize = 60;

/// Shown in place of a file name that has nothing printable left.
const FALLBACK_NAME: &str = "My sound";

/// Copy `source` into the Mood sounds folder and describe it for the list.
pub fn add(source: &Path) -> Result<CustomMoodSound, String> {
    add_in(&crate::state::paths::data::mood_sounds_dir(), source)
}

/// Where the copy named `file_name` lives.
pub fn path_of(file_name: &str) -> PathBuf {
    crate::state::paths::data::mood_sounds_dir().join(file_name)
}

/// Remove the copy named `file_name`, when it is one this module made.
pub fn remove(file_name: &str) {
    remove_in(&crate::state::paths::data::mood_sounds_dir(), file_name);
}

/// `add` into `dir`.
pub(crate) fn add_in(dir: &Path, source: &Path) -> Result<CustomMoodSound, String> {
    let extension = supported_extension(source).ok_or_else(||
        "Pick an MP3, WAV, OGG, FLAC or M4A file.".to_string()
    )?;
    crate::utils::path::ensure_directory_exists(dir)?;

    // Copy and hash in one pass, so a long music file is read only once.
    let partial = dir.join(format!("sound-{}.partial", uuid::Uuid::new_v4().simple()));
    let hash = match copy_hashing(source, &partial) {
        Ok(hash) => hash,
        Err(e) => {
            let _ = fs::remove_file(&partial);
            return Err(e);
        }
    };
    if let Err(e) = check_playable(&partial) {
        let _ = fs::remove_file(&partial);
        return Err(e);
    }

    let short_hash = &hash[..32];
    let file_name = format!("sound-{}.{}", short_hash, extension);
    let path = dir.join(&file_name);
    if path.is_file() {
        // The same content is already here.
        let _ = fs::remove_file(&partial);
    } else if let Err(e) = fs::rename(&partial, &path) {
        let _ = fs::remove_file(&partial);
        return Err(format!("Could not save the sound: {}", e));
    }

    Ok(CustomMoodSound {
        id: format!("custom-{}", short_hash),
        name: display_name(source),
        file_name,
    })
}

/// `remove` from `dir`. Only a plain file named the way `add` names one is
/// removed, so an edited config can never point this at another file.
pub(crate) fn remove_in(dir: &Path, file_name: &str) {
    if !is_sound_name(file_name) {
        return;
    }
    let path = dir.join(file_name);
    if fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_file())
        && let Err(e) = fs::remove_file(&path)
    {
        crate::always_eprint!("⚠️ Could not remove the Mood sound {}: {}", path.display(), e);
    }
}

/// The picked file's extension, lowercased, when the player can decode it.
fn supported_extension(source: &Path) -> Option<String> {
    let extension = source.extension()?.to_str()?.to_ascii_lowercase();
    SUPPORTED_EXTENSIONS.contains(&extension.as_str()).then_some(extension)
}

/// Copy `source` to `target` and return the hex SHA-256 of what was copied.
fn copy_hashing(source: &Path, target: &Path) -> Result<String, String> {
    let mut reader = File::open(source).map_err(|e| format!("Could not open the file: {}", e))?;
    let mut writer = File::create(target).map_err(|e| format!("Could not save the sound: {}", e))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer).map_err(|e| format!("Could not read the file: {}", e))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        writer.write_all(&buffer[..read]).map_err(|e| format!("Could not save the sound: {}", e))?;
    }
    writer.flush().map_err(|e| format!("Could not save the sound: {}", e))?;
    Ok(crate::utils::update_installer::hex_encode(&hasher.finalize()))
}

/// Whether the player can decode `path`: it opens, and gives a first sample.
fn check_playable(path: &Path) -> Result<(), String> {
    let file = File::open(path).map_err(|e| format!("Could not open the sound: {}", e))?;
    let mut decoder = rodio::Decoder
        ::new(BufReader::new(file))
        .map_err(|_| "This file is not a sound Mechvibes can play.".to_string())?;
    match decoder.next() {
        Some(_) => Ok(()),
        None => Err("This sound file is empty.".to_string()),
    }
}

/// The picked file's name without its extension, trimmed and kept short.
fn display_name(source: &Path) -> String {
    let stem = source
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_default();
    let cleaned: String = stem
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(MAX_NAME_CHARS)
        .collect();
    let cleaned = cleaned.trim().to_string();
    if cleaned.is_empty() { FALLBACK_NAME.to_string() } else { cleaned }
}

/// Whether `name` is `sound-<32 lowercase hex>.<supported extension>`.
fn is_sound_name(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("sound-") else {
        return false;
    };
    let Some((hash, extension)) = rest.split_once('.') else {
        return false;
    };
    hash.len() == 32 &&
        hash.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')) &&
        SUPPORTED_EXTENSIONS.contains(&extension)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An empty folder of its own under the system temp folder.
    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mechvibes-mood-{}-{}", name, uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A short real WAV file at `path`, different for each `seed`.
    fn write_wav(path: &Path, seed: i16) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 8000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for i in 0..800i16 {
            writer.write_sample(i.wrapping_mul(seed)).unwrap();
        }
        writer.finalize().unwrap();
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
    fn a_picked_sound_is_copied_and_named_from_its_file() {
        let (picked, dir) = (temp_dir("picked"), temp_dir("store"));
        let source = picked.join("Lofi Beats.WAV");
        write_wav(&source, 3);

        let sound = add_in(&dir, &source).unwrap();
        assert_eq!(sound.name, "Lofi Beats");
        assert!(sound.id.starts_with("custom-") && sound.id.len() == 39, "{}", sound.id);
        assert!(is_sound_name(&sound.file_name), "{}", sound.file_name);
        assert!(sound.file_name.ends_with(".wav"));
        assert_eq!(fs::read(dir.join(&sound.file_name)).unwrap(), fs::read(&source).unwrap());

        // The original can go away; the copy stays.
        fs::remove_file(&source).unwrap();
        assert!(dir.join(&sound.file_name).is_file());
        assert_eq!(files_in(&dir), [sound.file_name]);
        let _ = fs::remove_dir_all(&picked);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_same_sound_added_twice_is_one_file() {
        let (picked, dir) = (temp_dir("twice"), temp_dir("twice-store"));
        let (first, renamed) = (picked.join("a.wav"), picked.join("b.wav"));
        write_wav(&first, 5);
        fs::copy(&first, &renamed).unwrap();

        let one = add_in(&dir, &first).unwrap();
        let two = add_in(&dir, &renamed).unwrap();
        assert_eq!(one.id, two.id);
        assert_eq!(files_in(&dir), [one.file_name]);
        let _ = fs::remove_dir_all(&picked);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_the_player_cannot_decode_is_refused_and_leaves_nothing() {
        let (picked, dir) = (temp_dir("bad"), temp_dir("bad-store"));
        let fake = picked.join("song.mp3");
        fs::write(&fake, b"this is not an mp3 at all").unwrap();
        assert!(add_in(&dir, &fake).is_err());

        let text = picked.join("notes.txt");
        fs::write(&text, b"hello").unwrap();
        assert!(add_in(&dir, &text).is_err());
        assert!(files_in(&dir).is_empty(), "no partial file is left behind");
        let _ = fs::remove_dir_all(&picked);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn removing_only_touches_sounds_this_module_named() {
        let (picked, dir) = (temp_dir("remove"), temp_dir("remove-store"));
        let source = picked.join("rain.wav");
        write_wav(&source, 7);
        let sound = add_in(&dir, &source).unwrap();
        fs::write(dir.join("keep.mp3"), b"x").unwrap();

        remove_in(&dir, "keep.mp3");
        remove_in(&dir, "../rain.wav");
        remove_in(&dir, &sound.file_name);
        assert_eq!(files_in(&dir), ["keep.mp3"]);
        assert!(source.is_file());
        let _ = fs::remove_dir_all(&picked);
        let _ = fs::remove_dir_all(&dir);
    }

    /// Checks real files of every offered kind decode. Point
    /// `MOOD_SOUND_SAMPLES` at a folder holding `sample.<ext>` for each one,
    /// then run with `--ignored`.
    #[test]
    #[ignore]
    fn every_offered_kind_of_file_decodes() {
        let samples = PathBuf::from(std::env::var("MOOD_SOUND_SAMPLES").expect("MOOD_SOUND_SAMPLES is set"));
        for extension in SUPPORTED_EXTENSIONS {
            let path = samples.join(format!("sample.{}", extension));
            assert!(check_playable(&path).is_ok(), "{} does not decode", path.display());
        }
    }

    #[test]
    fn names_are_trimmed_kept_short_and_never_empty() {
        assert_eq!(display_name(Path::new("  Chill Mix  .mp3")), "Chill Mix");
        assert_eq!(display_name(Path::new("   .ogg")), FALLBACK_NAME);
        let long = format!("{}.mp3", "a".repeat(200));
        assert_eq!(display_name(Path::new(&long)).chars().count(), MAX_NAME_CHARS);
    }
}
