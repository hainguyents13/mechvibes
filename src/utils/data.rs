use crate::i18n::catalog::ErrorStrings;
use crate::i18n::fill;
use serde::{ Deserialize, Serialize };
/// Data serialization and file management utilities
use std::fs;
use std::path::{ Path, PathBuf };

/// Why a JSON file could not be written.
///
/// The wording is split from the cause on purpose. `Display` is the stable
/// English description a log and the callers that only print it keep using, and
/// it never follows the UI language. [`WriteJsonError::ui_message`] is the
/// sentence the screen shows, composed from the catalog the caller hands in:
/// this helper runs on the window's thread, on the audio thread and in tokio
/// tasks, and cannot know which language is in effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteJsonError {
    /// The folder the file lives in could not be created.
    Directory(crate::utils::path::DirectoryError),
    /// The data could not be turned into JSON. The `String` is the cause.
    Serialize(String),
    /// The temporary file next to the target could not be written.
    WriteFile {
        path: PathBuf,
        cause: String,
    },
    /// The temporary file could not be put in the target's place.
    Replace {
        path: PathBuf,
        cause: String,
    },
}

impl WriteJsonError {
    /// The sentence for this failure in the language `errors` carries.
    ///
    /// The caller resolves the language and passes the `errors` zone; nothing
    /// here reads the config.
    pub fn ui_message(&self, errors: &ErrorStrings) -> String {
        match self {
            // The folder's own sentence: the path helper words this one.
            WriteJsonError::Directory(cause) => cause.ui_message(errors),
            WriteJsonError::Serialize(cause) => fill(errors.json_serialize_failed, &[cause]),
            WriteJsonError::WriteFile { path, cause } =>
                fill(errors.file_write_failed, &[&path.display(), cause]),
            WriteJsonError::Replace { path, cause } =>
                fill(errors.file_replace_failed, &[&path.display(), cause]),
        }
    }
}

impl std::fmt::Display for WriteJsonError {
    /// Always English: the catalog for it is named here, never read from config,
    /// so a log line or a test reads the same in every language.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.ui_message(&crate::i18n::Language::En.strings().errors))
    }
}

/// Generic function to load JSON data from file
pub fn load_json_from_file<T>(file_path: &Path) -> Result<T, String>
    where T: for<'de> Deserialize<'de>
{
    let contents = fs
        ::read_to_string(file_path)
        .map_err(|e| format!("Failed to read file '{}': {}", file_path.display(), e))?;

    serde_json
        ::from_str::<T>(&contents)
        .map_err(|e| format!("Failed to parse JSON from '{}': {}", file_path.display(), e))
}

/// Generic function to save data as JSON to file
pub fn save_json_to_file<T>(data: &T, file_path: &Path) -> Result<(), String> where T: Serialize {
    // Ensure parent directory exists
    if let Some(parent) = file_path.parent() {
        fs
            ::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory '{}': {}", parent.display(), e))?;
    }

    let contents = serde_json
        ::to_string_pretty(data)
        .map_err(|e| format!("Failed to serialize data: {}", e))?;

    fs::write(file_path, contents).map_err(|e|
        format!("Failed to write file '{}': {}", file_path.display(), e)
    )
}

/// Save data as JSON, leaving either the old file or the complete new one at
/// `file_path` - never a partial document.
///
/// `fs::write` truncates first and then writes, so a crash, a full disk or a
/// power loss mid-write leaves a truncated file. For `config.json` that is not
/// a cosmetic problem: an unparseable config is what routes the load path into
/// the `.corrupt` rescue, costing the user every setting until they restore it
/// by hand. Writing a sibling temp file and renaming keeps the swap atomic on
/// both Windows (`MoveFileEx` with replace semantics, which `fs::rename` uses)
/// and POSIX.
///
/// The temp file is a sibling rather than in the system temp dir so the rename
/// stays within one filesystem, where it is atomic.
///
/// Kept in its `String` shape for the callers that only log or pass it on;
/// delegates to [`save_json_to_file_atomically_typed`], so the wording is not
/// duplicated. A caller that shows the failure calls that one and words it with
/// [`WriteJsonError::ui_message`].
pub fn save_json_to_file_atomically<T>(data: &T, file_path: &Path) -> Result<(), String>
    where T: Serialize
{
    save_json_to_file_atomically_typed(data, file_path).map_err(|e| e.to_string())
}

/// The same save as [`save_json_to_file_atomically`], with the failure typed so
/// the screen can word it from the catalog.
pub fn save_json_to_file_atomically_typed<T>(
    data: &T,
    file_path: &Path
) -> Result<(), WriteJsonError> where T: Serialize {
    if let Some(parent) = file_path.parent() {
        crate::utils::path::ensure_directory_exists_typed(parent).map_err(WriteJsonError::Directory)?;
    }

    let contents = serde_json
        ::to_string_pretty(data)
        .map_err(|e| WriteJsonError::Serialize(e.to_string()))?;

    // Process id in the name so two copies of the app pointed at the same data
    // directory cannot write each other's temp file half-way through.
    let temp_path = {
        let mut name = file_path.as_os_str().to_os_string();
        name.push(format!(".{}.tmp", std::process::id()));
        PathBuf::from(name)
    };

    fs::write(&temp_path, contents).map_err(|e| WriteJsonError::WriteFile {
        path: temp_path.clone(),
        cause: e.to_string(),
    })?;

    fs::rename(&temp_path, file_path).map_err(|e| {
        // Leaving the temp file behind would accumulate one per failed save.
        let _ = fs::remove_file(&temp_path);
        WriteJsonError::Replace {
            path: file_path.to_path_buf(),
            cause: e.to_string(),
        }
    })
}
