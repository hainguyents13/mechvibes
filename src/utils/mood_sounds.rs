//! Sounds the user adds to Mood from files of their own: music, lo-fi, any
//! background they want on repeat.
//!
//! A picked file is copied into this copy's Mood sounds folder, so moving or
//! deleting the original later does not silence it. The copy is named from
//! its content, `sound-<32 hex of the SHA-256>.<ext>`, so adding the same file
//! twice finds the one already there. A file is checked by decoding its first
//! sample before it is kept, so the list never holds something the player
//! cannot play.

use crate::i18n::catalog::ErrorStrings;
use crate::i18n::fill;
use crate::state::config::CustomMoodSound;
use sha2::{ Digest, Sha256 };
use std::fs::{ self, File };
use std::io::{ BufReader, Read, Write };
use std::path::{ Path, PathBuf };

/// The kinds of file the player decodes, also offered by the file picker.
pub const SUPPORTED_EXTENSIONS: &[&str] = &["mp3", "wav", "ogg", "flac", "m4a"];

/// The longest name kept from the picked file, in characters.
const MAX_NAME_CHARS: usize = 60;

/// Why a picked file was not added to the list.
///
/// The two halves of every failure live apart on purpose. `Display` is the
/// stable English description the log gets, and it never follows the UI
/// language. [`MoodSoundError::ui_message`] is the sentence the user reads,
/// composed from the catalog the caller hands in — this module has no way to
/// know which language is in effect, and asking it would put a config read on
/// the path that runs in a background task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoodSoundError {
    /// The extension is not one the player decodes.
    UnsupportedFile,
    /// The Mood sounds folder could not be created. The path helper that owns
    /// the sentence words it from the same catalog.
    Directory(crate::utils::path::DirectoryError),
    /// The picked file could not be read.
    FileOpen(String),
    /// Reading it stopped halfway.
    FileRead(String),
    /// The copy could not be written, or could not be moved into its final name.
    SaveFailed(String),
    /// The copy is there but could not be opened for decoding.
    SoundOpen(String),
    /// The file opened but is not a format the decoder accepts.
    NotPlayable,
    /// The file decodes to no samples at all.
    EmptySound,
}

impl MoodSoundError {
    /// The sentence for this failure in the language `errors` carries.
    ///
    /// The caller resolves the language and passes the `errors` zone; nothing
    /// here reads the config.
    pub fn ui_message(&self, errors: &ErrorStrings) -> String {
        match self {
            MoodSoundError::UnsupportedFile => errors.mood_sound_pick_supported_file.to_string(),
            // The folder's own sentence, which the path helper words.
            MoodSoundError::Directory(cause) => cause.ui_message(errors),
            MoodSoundError::FileOpen(cause) => fill(errors.mood_sound_open_failed, &[cause]),
            MoodSoundError::FileRead(cause) => fill(errors.mood_sound_read_failed, &[cause]),
            MoodSoundError::SaveFailed(cause) => fill(errors.mood_sound_save_failed, &[cause]),
            MoodSoundError::SoundOpen(cause) => fill(errors.mood_sound_open_sound_failed, &[cause]),
            MoodSoundError::NotPlayable => errors.mood_sound_not_playable.to_string(),
            MoodSoundError::EmptySound => errors.mood_sound_empty.to_string(),
        }
    }
}

impl std::fmt::Display for MoodSoundError {
    /// Always English: the catalog for it is named here, never read from config,
    /// so a log line or a test reads the same in every language.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.ui_message(&crate::i18n::Language::En.strings().errors))
    }
}

/// Copy `source` into the Mood sounds folder and describe it for the list.
///
/// The failure is typed so the screen can word it from the catalog with
/// [`MoodSoundError::ui_message`].
pub fn add_typed(source: &Path) -> Result<CustomMoodSound, MoodSoundError> {
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

/// [`add_typed`] into `dir`.
pub(crate) fn add_in(dir: &Path, source: &Path) -> Result<CustomMoodSound, MoodSoundError> {
    let extension = supported_extension(source).ok_or(MoodSoundError::UnsupportedFile)?;
    crate::utils::path::ensure_directory_exists_typed(dir).map_err(MoodSoundError::Directory)?;

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
        return Err(MoodSoundError::SaveFailed(e.to_string()));
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
fn copy_hashing(source: &Path, target: &Path) -> Result<String, MoodSoundError> {
    let mut reader = File::open(source).map_err(|e| MoodSoundError::FileOpen(e.to_string()))?;
    let mut writer = File::create(target).map_err(|e| MoodSoundError::SaveFailed(e.to_string()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|e| MoodSoundError::FileRead(e.to_string()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        writer
            .write_all(&buffer[..read])
            .map_err(|e| MoodSoundError::SaveFailed(e.to_string()))?;
    }
    writer.flush().map_err(|e| MoodSoundError::SaveFailed(e.to_string()))?;
    Ok(crate::utils::update_installer::hex_encode(&hasher.finalize()))
}

/// Whether the player can decode `path`: it opens, and gives a first sample.
fn check_playable(path: &Path) -> Result<(), MoodSoundError> {
    let file = File::open(path).map_err(|e| MoodSoundError::SoundOpen(e.to_string()))?;
    let mut decoder = rodio::Decoder
        ::new(BufReader::new(file))
        .map_err(|_| MoodSoundError::NotPlayable)?;
    match decoder.next() {
        Some(_) => Ok(()),
        None => Err(MoodSoundError::EmptySound),
    }
}

/// Shown in place of a file name that has nothing printable left.
///
/// It comes from the **English** catalog on purpose. `CustomMoodSound::name` is
/// data: it is written to `config.json` when the sound is added, so taking it
/// from the language in effect would put the language of that moment into the
/// user's list for good. The wording still lives in the catalog, next to the
/// notices this module produces. A translated fallback belongs at the render
/// site, which is the only place that can follow a language change.
fn fallback_name() -> &'static str {
    crate::i18n::Language::En.strings().errors.mood_sound_fallback_name
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
    if cleaned.is_empty() { fallback_name().to_string() } else { cleaned }
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
        assert_eq!(display_name(Path::new("   .ogg")), fallback_name());
        let long = format!("{}.mp3", "a".repeat(200));
        assert_eq!(display_name(Path::new(&long)).chars().count(), MAX_NAME_CHARS);
    }

    /// The descriptions are the contract: the log and the cached metadata read
    /// the same in every language, and only `ui_message` follows the catalog.
    #[test]
    fn a_description_is_english_whatever_language_the_catalog_is_in() {
        use crate::i18n::Language;

        let failures = [
            MoodSoundError::UnsupportedFile,
            MoodSoundError::Directory(crate::utils::path::DirectoryError {
                path: PathBuf::from("/x"),
                cause: "denied".to_string(),
            }),
            MoodSoundError::FileOpen("no such file".to_string()),
            MoodSoundError::FileRead("interrupted".to_string()),
            MoodSoundError::SaveFailed("disk full".to_string()),
            MoodSoundError::SoundOpen("denied".to_string()),
            MoodSoundError::NotPlayable,
            MoodSoundError::EmptySound,
        ];
        for failure in &failures {
            let english = failure.to_string();
            let from_catalog = failure.ui_message(&Language::En.strings().errors);
            assert_eq!(english, from_catalog, "the English catalog is the description");
            for language in [Language::Zh, Language::Es] {
                assert_eq!(
                    failure.to_string(),
                    english,
                    "`{failure:?}` changed with {language:?}: a description must not follow the UI language"
                );
            }
        }
        assert_eq!(
            MoodSoundError::UnsupportedFile
                .ui_message(&Language::Es.strings().errors),
            "Elige un archivo MP3, WAV, OGG, FLAC o M4A.",
            "what the user reads does follow the language"
        );
    }
}
