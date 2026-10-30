use std::path::PathBuf;
use crate::utils::constants::APP_NAME;

#[cfg(target_os = "windows")]
use winreg::enums::*;
#[cfg(target_os = "windows")]
use winreg::RegKey;

/// The Windows Run value name written by builds from before the app was
/// renamed from MechvibesDX.
#[cfg(target_os = "windows")]
const LEGACY_RUN_VALUE: &str = "MechvibesDX";

/// Get the current executable path
fn get_exe_path() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| format!("Failed to get executable path: {}", e))
}

/// The Run command line for `exe_path`, in one of the two shapes
/// [`exe_path_from_run_command`] reads back.
fn run_command(exe_path: &str, minimized: bool) -> String {
    if minimized { format!("\"{}\" --minimized", exe_path) } else { exe_path.to_string() }
}

/// Enable auto startup on Windows
#[cfg(target_os = "windows")]
pub fn enable_auto_startup() -> Result<(), String> {
    let exe_path = get_exe_path()?;
    let exe_path_str = exe_path.to_str().ok_or("Failed to convert executable path to string")?;

    // Check if we should start minimized. Read from the config authority, not
    // the file: the Settings toggle calls this immediately after writing
    // `start_minimized`, and a re-parse could race that write.
    let config = crate::state::config_writer::current();
    let command = run_command(exe_path_str, config.start_minimized);

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run_key = hkcu
        .open_subkey_with_flags("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run", KEY_WRITE)
        .map_err(|e| format!("Failed to open registry key: {}", e))?;

    run_key
        .set_value(APP_NAME, &command)
        .map_err(|e| format!("Failed to set registry value: {}", e))?;

    crate::always_print!("✅ Auto startup enabled: {}", command);
    Ok(())
}

/// Disable auto startup on Windows
#[cfg(target_os = "windows")]
pub fn disable_auto_startup() -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run_key = hkcu
        .open_subkey_with_flags("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run", KEY_WRITE)
        .map_err(|e| format!("Failed to open registry key: {}", e))?;

    match run_key.delete_value(APP_NAME) {
        Ok(_) => {
            crate::always_print!("✅ Auto startup disabled");
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Entry doesn't exist, which is fine
            crate::always_print!("ℹ️ Auto startup was not enabled");
            Ok(())
        }
        Err(e) => Err(format!("Failed to delete registry value: {}", e)),
    }
}

/// Pull the executable path back out of a registry Run command line.
///
/// `enable_auto_startup` writes two shapes depending on `start_minimized`:
/// a bare path, or `"path" --minimized`. Comparing the whole value against
/// the bare path only ever matches the first shape, so with "start minimized"
/// on the check reported "not enabled" for an entry it had just written -
/// and `AppConfig::load` then "synced" `auto_start` back to false and saved.
/// Both shapes have to round-trip.
fn exe_path_from_run_command(command: &str) -> &str {
    let command = command.trim();

    // Quoted form: take what is between the first pair of quotes and ignore
    // any arguments that follow.
    if let Some(rest) = command.strip_prefix('"') {
        return match rest.find('"') {
            Some(end) => &rest[..end],
            // Unbalanced quote - no sane reading, treat the remainder as path.
            None => rest,
        };
    }

    // Unquoted form is written without arguments, so the whole value is the
    // path. Splitting on whitespace here would break paths like
    // `C:\Program Files\...`.
    command
}

/// Whether a Run command written before the rename launches the copy that is
/// running now. The rename changed the executable's file name but not its
/// folder, so the folders are compared; a command for a copy somewhere else
/// belongs to that copy and is left alone.
fn is_same_install(legacy_command: &str, current_exe: &str) -> bool {
    fn folder(path: &str) -> Option<&str> {
        path.rfind(['\\', '/']).map(|end| &path[..end]).filter(|folder| !folder.is_empty())
    }
    match (folder(exe_path_from_run_command(legacy_command)), folder(current_exe)) {
        (Some(legacy), Some(current)) => legacy.eq_ignore_ascii_case(current),
        _ => false,
    }
}

/// Moves a start-at-login entry from the pre-rename value name to the current
/// one, pointed at the renamed executable.
///
/// Without this the enabled check finds nothing under the new name, and the
/// config sync then turns auto start off for everyone who had it on.
#[cfg(target_os = "windows")]
pub fn migrate_legacy_entry() {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let Ok(run_key) = hkcu.open_subkey_with_flags(
        "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run",
        KEY_READ | KEY_WRITE
    ) else {
        return;
    };
    let Ok(legacy_command) = run_key.get_value::<String, _>(LEGACY_RUN_VALUE) else {
        return;
    };
    let Some(exe_path) = get_exe_path().ok().and_then(|path| path.to_str().map(str::to_string)) else {
        return;
    };
    if !is_same_install(&legacy_command, &exe_path) {
        return;
    }

    let command = run_command(&exe_path, legacy_command.contains("--minimized"));
    if let Err(e) = run_key.set_value(APP_NAME, &command) {
        crate::always_eprint!("⚠️ Could not move the auto startup entry to {}: {}", APP_NAME, e);
        return;
    }
    match run_key.delete_value(LEGACY_RUN_VALUE) {
        Ok(_) => crate::always_print!("🔄 Auto startup entry moved from {} to {}", LEGACY_RUN_VALUE, APP_NAME),
        Err(e) => crate::always_eprint!("⚠️ Could not remove the {} auto startup entry: {}", LEGACY_RUN_VALUE, e),
    }
}

/// Only Windows builds before the rename wrote a startup entry under the old
/// name; macOS login items are tied to the bundle identifier, which did not
/// change.
#[cfg(not(target_os = "windows"))]
pub fn migrate_legacy_entry() {}

/// Check if auto startup is currently enabled
#[cfg(target_os = "windows")]
pub fn is_auto_startup_enabled() -> bool {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run_key = match hkcu.open_subkey("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run") {
        Ok(key) => key,
        Err(_) => {
            return false;
        }
    };

    match run_key.get_value::<String, _>(APP_NAME) {
        Ok(value) => {
            let current_exe = get_exe_path().unwrap_or_default();
            let current_exe_str = match current_exe.to_str() {
                Some(path) if !path.is_empty() => path,
                // Without a path to compare against, "" would match a "" value
                // and claim auto start is on for an entry pointing elsewhere.
                _ => {
                    return false;
                }
            };

            // Windows paths are case-insensitive, and the registry value was
            // written by a possibly different-cased invocation of this exe.
            exe_path_from_run_command(&value).eq_ignore_ascii_case(current_exe_str)
        }
        Err(_) => false,
    }
}

/// macOS: register the app as a login item with `SMAppService`.
///
/// `SMAppService.mainApp` is the supported API (macOS 13+). The app then shows
/// up in System Settings > General > Login Items, where the user can see and
/// remove it. It launches the app with no arguments, so "start minimized" is
/// decided from the saved config at startup (`main.rs`), not a `--minimized`
/// flag as on Windows.
///
/// It only works from an app bundle in a normal location (e.g.
/// /Applications). A bare `target/release/mechvibes` or `dx serve` run has
/// no bundle, so registering fails and reports not-enabled.
#[cfg(target_os = "macos")]
mod macos {
    use objc2::rc::Retained;
    use objc2::runtime::{ AnyClass, AnyObject };
    use objc2::msg_send;
    use objc2_foundation::NSError;

    // Nothing in the Rust code references the framework directly (everything
    // goes through the Objective-C runtime), so link it explicitly or the
    // SMAppService class does not exist at runtime.
    #[link(name = "ServiceManagement", kind = "framework")]
    unsafe extern "C" {}

    /// `SMAppServiceStatus`
    const STATUS_ENABLED: isize = 1;
    const STATUS_REQUIRES_APPROVAL: isize = 2;

    fn service_class() -> Result<&'static AnyClass, String> {
        // Missing before macOS 13.
        AnyClass::get(c"SMAppService").ok_or_else(||
            "Start at login needs macOS 13 or later".to_string()
        )
    }

    fn main_app_service() -> Result<Retained<AnyObject>, String> {
        let class = service_class()?;
        // SAFETY: `+mainAppService` takes no arguments and returns an object.
        let service: Option<Retained<AnyObject>> = unsafe { msg_send![class, mainAppService] };
        service.ok_or_else(|| "SMAppService.mainApp is unavailable".to_string())
    }

    fn status(service: &AnyObject) -> isize {
        // SAFETY: `-status` takes no arguments and returns an NSInteger.
        unsafe { msg_send![service, status] }
    }

    pub fn is_enabled() -> bool {
        match main_app_service() {
            Ok(service) => status(&service) == STATUS_ENABLED,
            Err(_) => false,
        }
    }

    pub fn enable() -> Result<(), String> {
        let service = main_app_service()?;
        if status(&service) != STATUS_ENABLED {
            // SAFETY: `-registerAndReturnError:` returns BOOL and fills in an
            // NSError on failure, which `msg_send!` turns into the Result.
            let result: Result<(), Retained<NSError>> = unsafe {
                msg_send![&*service, registerAndReturnError: _]
            };
            result.map_err(|e| format!("Failed to register login item: {}", e.localizedDescription()))?;
        }

        // The user can have switched this app off under Login Items. register()
        // succeeds but it stays disabled until they approve it, so take them there.
        if status(&service) == STATUS_REQUIRES_APPROVAL {
            crate::always_print!("ℹ️ Login item needs approval in System Settings > General > Login Items");
            if let Ok(class) = service_class() {
                // SAFETY: class method with no arguments and no return value.
                unsafe {
                    let _: () = msg_send![class, openSystemSettingsLoginItems];
                }
            }
            return Err("Allow Mechvibes in System Settings > General > Login Items".to_string());
        }

        crate::always_print!("✅ Auto startup enabled (macOS login item)");
        Ok(())
    }

    pub fn disable() -> Result<(), String> {
        let service = main_app_service()?;
        if status(&service) == 0 {
            // SMAppServiceStatusNotRegistered
            crate::always_print!("ℹ️ Auto startup was not enabled");
            return Ok(());
        }
        // SAFETY: as in `enable`.
        let result: Result<(), Retained<NSError>> = unsafe {
            msg_send![&*service, unregisterAndReturnError: _]
        };
        result.map_err(|e| format!("Failed to remove login item: {}", e.localizedDescription()))?;
        crate::always_print!("✅ Auto startup disabled");
        Ok(())
    }
}

/// Set auto startup state (enable or disable)
pub fn set_auto_startup(enable: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        if enable { enable_auto_startup() } else { disable_auto_startup() }
    }

    #[cfg(target_os = "macos")]
    {
        if enable { macos::enable() } else { macos::disable() }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Err("Auto startup is only supported on Windows".to_string())
    }
}

/// Get current auto startup state
pub fn get_auto_startup_state() -> bool {
    #[cfg(target_os = "windows")]
    {
        is_auto_startup_enabled()
    }

    #[cfg(target_os = "macos")]
    {
        macos::is_enabled()
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::{ exe_path_from_run_command, is_same_install, run_command };

    /// A test binary is not an app bundle, so it can never be a registered
    /// login item. This also proves the SMAppService lookup and the framework
    /// link work at runtime without registering anything.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_bare_binary_is_not_a_registered_login_item() {
        assert!(!super::get_auto_startup_state());
    }

    /// Mirrors the unquoted shape `enable_auto_startup` writes when
    /// `start_minimized` is off.
    #[test]
    fn a_bare_path_is_returned_unchanged() {
        let path = r"C:\Program Files\Mechvibes\mechvibes.exe";
        assert_eq!(exe_path_from_run_command(path), path);
    }

    /// The shape that used to be unrecognizable: with "start minimized" on the
    /// value carries quotes and an argument, and the old whole-string compare
    /// reported "not enabled" for an entry this app had just written.
    #[test]
    fn a_quoted_path_with_the_minimized_flag_yields_just_the_path() {
        let path = r"C:\Program Files\Mechvibes\mechvibes.exe";
        let command = format!("\"{}\" --minimized", path);
        assert_eq!(exe_path_from_run_command(&command), path);
    }

    /// A quoted path with no arguments must also round-trip, since that is
    /// what a user or an older build may have left behind.
    #[test]
    fn a_quoted_path_without_arguments_yields_just_the_path() {
        let path = r"C:\Apps\mechvibes.exe";
        assert_eq!(exe_path_from_run_command(&format!("\"{}\"", path)), path);
    }

    /// Spaces inside an unquoted path must not be treated as an argument
    /// separator, or every "Program Files" install compares unequal and
    /// self-disables exactly like the quoted bug did.
    #[test]
    fn spaces_in_an_unquoted_path_do_not_truncate_it() {
        let path = r"C:\Program Files\Mechvibes\mechvibes.exe";
        assert_eq!(exe_path_from_run_command(path), path);
    }

    /// The registry value is written by whatever casing launched the app;
    /// the enabled check compares case-insensitively, so both forms must
    /// reduce to a path that compares equal to the real one.
    #[test]
    fn both_written_forms_match_the_exe_case_insensitively() {
        let exe = r"C:\Program Files\Mechvibes\mechvibes.exe";
        let bare = r"c:\program files\mechvibes\MECHVIBES.EXE";
        let minimized = format!("\"{}\" --minimized", bare);

        assert!(exe_path_from_run_command(bare).eq_ignore_ascii_case(exe));
        assert!(exe_path_from_run_command(&minimized).eq_ignore_ascii_case(exe));
    }

    /// A Run entry pointing at some other program must not be read as "our
    /// auto start is on", or the config sync would flip `auto_start` true.
    #[test]
    fn a_different_executable_does_not_match() {
        let exe = r"C:\Program Files\Mechvibes\mechvibes.exe";
        let other = "\"C:\\Other\\thing.exe\" --minimized";
        assert!(!exe_path_from_run_command(other).eq_ignore_ascii_case(exe));
    }

    /// Both shapes `run_command` writes must read back as the same path.
    #[test]
    fn run_commands_round_trip_through_the_reader() {
        let exe = r"C:\Program Files\MechvibesDX\mechvibes.exe";
        assert_eq!(exe_path_from_run_command(&run_command(exe, false)), exe);
        assert_eq!(exe_path_from_run_command(&run_command(exe, true)), exe);
        assert!(run_command(exe, true).ends_with(" --minimized"));
    }

    /// An entry written before the rename points at the old file name in the
    /// same folder the installer upgrades in place, in either written shape.
    #[test]
    fn an_entry_for_the_old_file_name_in_the_same_folder_is_this_install() {
        let current = r"C:\Users\someone\AppData\Local\Programs\MechvibesDX\mechvibes.exe";
        let old = r"C:\Users\someone\AppData\Local\Programs\MechvibesDX\mechvibes-dx.exe";
        assert!(is_same_install(old, current));
        assert!(is_same_install(&format!("\"{}\" --minimized", old), current));
        assert!(is_same_install(&old.to_uppercase(), current));
    }

    /// A copy in another folder owns its own entry; moving it would point
    /// that copy's start-at-login at this one.
    #[test]
    fn an_entry_for_a_copy_in_another_folder_is_not_this_install() {
        let current = r"C:\Users\someone\AppData\Local\Programs\MechvibesDX\mechvibes.exe";
        assert!(!is_same_install(r"D:\Portable\mechvibes-dx.exe", current));
        assert!(!is_same_install("mechvibes-dx.exe", current));
        assert!(!is_same_install("", current));
    }
}
