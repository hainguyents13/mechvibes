/// Path and file system utility functions
use crate::state::paths;
use std::fs;
use std::process::Command;

/// Check if data directory exists
pub fn data_dir_exists() -> bool {
    paths::data::config_json().parent().unwrap().exists()
}

/// Check if config file exists
pub fn config_file_exists() -> bool {
    paths::data::config_json().exists()
}

/// Get absolute path for data directory
pub fn get_data_dir_absolute() -> String {
    paths::data::config_json().parent().unwrap().to_string_lossy().to_string()
}

/// Get absolute path for config file
pub fn get_config_file_absolute() -> String {
    paths::data::config_json().to_string_lossy().to_string()
}

/// Get absolute path for soundpacks directory (built-in soundpacks)
pub fn get_soundpacks_dir_absolute() -> String {
    paths::soundpacks::get_builtin_soundpacks_dir()
        .to_string_lossy()
        .to_string()
}

/// Get absolute path for custom soundpacks directory (system app data)
pub fn get_custom_soundpacks_dir_absolute() -> String {
    paths::soundpacks::get_custom_soundpacks_dir()
        .to_string_lossy()
        .to_string()
}

// ===== FILE SYSTEM UTILITIES =====

/// Open a path in the system file manager
pub fn open_path(path_to_open: &str) -> Result<(), String> {
    let result = if cfg!(target_os = "windows") {
        Command::new("explorer").arg(&path_to_open).spawn()
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg(&path_to_open).spawn()
    } else {
        // Linux and other Unix-like systems
        Command::new("xdg-open").arg(&path_to_open).spawn()
    };

    match result {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Failed to open path: {}", e)),
    }
}

/// Check if a directory exists
pub fn directory_exists(path: &str) -> bool {
    std::path::Path::new(path).exists()
}

/// Create directory recursively if it doesn't exist
pub fn ensure_directory_exists(path: impl AsRef<std::path::Path>) -> Result<(), String> {
    let path_ref = path.as_ref();
    fs::create_dir_all(path_ref).map_err(|e| {
        format!(
            "Failed to create directory '{}': {}",
            path_ref.display(),
            e
        )
    })
}

/// Read file contents as string
pub fn read_file_contents(path: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("Failed to read file '{}': {}", path, e))
}

/// Write string contents to file
pub fn write_file_contents(path: &str, contents: &str) -> Result<(), String> {
    fs::write(path, contents).map_err(|e| format!("Failed to write file '{}': {}", path, e))
}

/// Copy a picked image to the custom images directory and return the asset URL
/// Returns a URL in the format: /custom-images/img-{hash}.{ext}
/// The copy is named after its content, so picking the same image twice keeps
/// one file.
pub fn copy_to_custom_images(source_path: &str) -> Result<String, String> {
    use std::path::Path;

    let source = Path::new(source_path);

    // Validate source file exists
    if !source.exists() {
        return Err(format!("Source file does not exist: {}", source_path));
    }

    let bytes = fs::read(source).map_err(|e| format!("Failed to read file: {}", e))?;

    // The picked file's own extension is used only when the bytes do not
    // name a known image type.
    let extension = source.extension().and_then(|ext| ext.to_str());
    crate::utils::custom_images::save(&bytes, extension).map(|(url, _)| url)
}
