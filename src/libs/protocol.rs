use std::env;
use std::process::Command;
use crate::utils::constants::{ APP_PROTOCOL, APP_NAME };

#[cfg(target_os = "linux")]
use crate::utils::constants::APP_NAME_LOWERCASE;

#[allow(dead_code)]
/// Register the mechvibes:// protocol for the application
#[cfg(target_os = "windows")]
pub fn register_protocol() -> Result<(), Box<dyn std::error::Error>> {
    let exe_path = env::current_exe()?;
    let exe_path_str = exe_path.to_string_lossy();
    crate::always_print!("🔗 Registering {}// protocol... {}", APP_PROTOCOL, exe_path_str); // Store formatted strings to avoid temporary value issues
    let icon_path = format!("\"{}\"", exe_path_str);
    let command_path = format!("\"{}\" \"%1\"", exe_path_str); // Registry commands to register the protocol
    let protocol_key = format!("HKCU\\Software\\Classes\\{}", APP_PROTOCOL);
    let protocol_description = format!("{} Protocol", APP_NAME);
    let default_icon_key = format!("{}\\DefaultIcon", protocol_key);
    let shell_command_key = format!("{}\\shell\\open\\command", protocol_key);

    let commands = vec![
        vec!["reg", "add", &protocol_key, "/ve", "/d", &protocol_description, "/f"],
        vec!["reg", "add", &protocol_key, "/v", "URL Protocol", "/d", "", "/f"],
        vec!["reg", "add", &default_icon_key, "/ve", "/d", &icon_path, "/f"],
        vec!["reg", "add", &shell_command_key, "/ve", "/d", &command_path, "/f"]
    ];
    for cmd in commands {
        let output = Command::new(cmd[0])
            .args(&cmd[1..])
            .output()?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            crate::always_eprint!("❌ Registry command failed: {}", error);
        }
    }

    crate::always_print!("✅ Protocol {}// registered successfully", APP_PROTOCOL);
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn register_protocol() -> Result<(), Box<dyn std::error::Error>> {
    crate::always_print!("🍎 Protocol registration on macOS requires app bundle configuration in Info.plist");
    crate::always_print!("Add the following to your Info.plist:");
    crate::always_print!(
        r#"
<key>CFBundleURLTypes</key>    <array>
        <dict>
            <key>CFBundleURLName</key>
            <string>{} Protocol</string>
            <key>CFBundleURLSchemes</key>
            <array>
                <string>{}</string>
            </array>
        </dict>
    </array>
"#,
        APP_NAME,
        APP_PROTOCOL
    );
    Ok(())
}

/// Whether this build has to register `mechvibes://` itself: only an
/// AppImage, since every other Linux install comes with its desktop file.
#[cfg(target_os = "linux")]
pub fn should_self_register() -> bool {
    appimage_file().is_some()
}

/// The AppImage this process runs from. `$APPIMAGE` alone is not enough:
/// every child process inherits it, so the `.deb` started from a terminal
/// inside some other AppImage would see that one's path. It only counts when
/// this executable really runs from an AppImage mount, and names a real file.
#[cfg(target_os = "linux")]
pub fn appimage_file() -> Option<std::path::PathBuf> {
    if !crate::state::paths::is_running_from_appimage() {
        return None;
    }
    env::var_os("APPIMAGE")
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_file())
}

/// Quote `path` for a desktop file's `Exec` key, per the Desktop Entry spec:
/// `"`, `` ` ``, `$` and `\` are escaped inside the quotes, then every `\` is
/// doubled again by the string escaping of the value, and `%` becomes `%%`.
/// `None` for a path with a line break, which would start a new key.
#[cfg(any(target_os = "linux", test))]
fn desktop_exec_quote(path: &str) -> Option<String> {
    if path.contains(['\n', '\r']) {
        return None;
    }
    let mut quoted = String::from("\"");
    for c in path.chars() {
        match c {
            '"' | '`' | '$' => {
                quoted.push_str("\\\\");
                quoted.push(c);
            }
            '\\' => quoted.push_str("\\\\\\\\"),
            '%' => quoted.push_str("%%"),
            c => quoted.push(c),
        }
    }
    quoted.push('"');
    Some(quoted)
}

/// The type of a `.mvtheme` theme file, as `packaging/linux/mechvibes.xml`
/// declares it.
#[cfg(any(target_os = "linux", test))]
const THEME_MIME_TYPE: &str = "application/x-mechvibes-theme";

/// The shared-mime-info file that declares `THEME_MIME_TYPE`. The `.deb`
/// ships the same file.
#[cfg(any(target_os = "linux", test))]
const THEME_MIME_XML: &str = include_str!("../../packaging/linux/mechvibes.xml");

/// Make an AppImage the handler for `mechvibes://` links and `.mvtheme` theme
/// files.
///
/// The `.deb` declares the handler in its own desktop file. An AppImage
/// installs nothing, so it writes a handler entry for the user each time it
/// starts, pointing at the AppImage file itself: the path it runs from inside
/// is a mount point that changes every launch. The entry is hidden from the
/// app menu, which already has the AppImage or the package's own entry. The
/// theme file type is declared for the user the same way.
#[cfg(target_os = "linux")]
pub fn register_protocol() -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;

    let home = env::var("HOME")?;
    let entry_name = format!("{}-url-handler.desktop", APP_NAME_LOWERCASE);
    let apps_dir = format!("{}/.local/share/applications", home);
    let desktop_file_path = format!("{}/{}", apps_dir, entry_name);
    let mime_dir = format!("{}/.local/share/mime", home);
    let mime_packages_dir = format!("{}/packages", mime_dir);
    let mime_file_path = format!("{}/{}.xml", mime_packages_dir, APP_NAME_LOWERCASE);
    let exe_path = appimage_file().ok_or("not an AppImage")?;
    let exec = desktop_exec_quote(&exe_path.to_string_lossy()).ok_or(
        "the AppImage path has a line break in it"
    )?;

    let desktop_content = format!(
        r#"[Desktop Entry]
Name={}
Comment=Mechanical keyboard sound simulator
Exec={} %u
Icon={}
Type=Application
NoDisplay=true
MimeType=x-scheme-handler/{};{};
"#,
        APP_NAME,
        exec,
        APP_NAME_LOWERCASE,
        APP_PROTOCOL,
        THEME_MIME_TYPE
    );

    // Each file is rewritten only when it changed, so an unchanged AppImage
    // starts without touching the desktop or MIME databases.
    let unchanged = |path: &str, content: &str| fs::read_to_string(path).ok().as_deref() == Some(content);
    let entry_changed = !unchanged(desktop_file_path.as_str(), desktop_content.as_str());
    let mime_changed = !unchanged(mime_file_path.as_str(), THEME_MIME_XML);
    if !entry_changed && !mime_changed {
        return Ok(());
    }
    crate::always_print!("🐧 Registering {}// protocol and theme files on Linux...", APP_PROTOCOL);

    // The entry first: links matter most.
    if entry_changed {
        fs::create_dir_all(&apps_dir)?;
        fs::write(&desktop_file_path, desktop_content)?;
        let _ = Command::new("update-desktop-database").arg(&apps_dir).output();
    }
    // A MIME folder that cannot be written must not keep links from opening
    // here, so its error waits until the defaults below are set. The MIME
    // file is then tried again on the next start.
    let mime_written = if mime_changed {
        let written = fs::create_dir_all(&mime_packages_dir).and_then(|()| fs::write(&mime_file_path, THEME_MIME_XML));
        if written.is_ok() {
            // Best effort: a minimal system may not have shared-mime-info.
            let _ = Command::new("update-mime-database").arg(&mime_dir).output();
        }
        written
    } else {
        Ok(())
    };
    for mime_type in [format!("x-scheme-handler/{}", APP_PROTOCOL), THEME_MIME_TYPE.to_string()] {
        let _ = Command::new("xdg-mime").args(["default", &entry_name, &mime_type]).output();
    }
    mime_written?;

    crate::always_print!("✅ Protocol {}// and theme files registered successfully", APP_PROTOCOL);
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::{ desktop_exec_quote, THEME_MIME_TYPE, THEME_MIME_XML };

    /// The type the file manager matches must be the one each desktop entry
    /// says it opens, and it must cover the extension the app writes.
    #[test]
    fn the_theme_file_type_is_declared_the_same_everywhere() {
        assert!(THEME_MIME_XML.contains(&format!("<mime-type type=\"{}\">", THEME_MIME_TYPE)));
        assert!(
            THEME_MIME_XML.contains(
                &format!("<glob pattern=\"*.{}\"/>", crate::utils::theme_share::FILE_EXTENSION)
            )
        );
        let desktop = include_str!("../../mechvibes.desktop");
        assert!(
            desktop
                .lines()
                .any(|line| line.trim_end() == format!("MimeType=x-scheme-handler/mechvibes;{};", THEME_MIME_TYPE))
        );
    }

    #[test]
    fn an_exec_path_is_quoted_and_escaped_for_a_desktop_file() {
        assert_eq!(desktop_exec_quote("/home/me/Mechvibes.AppImage").as_deref(), Some("\"/home/me/Mechvibes.AppImage\""));
        // `$` `"` `` ` `` take a backslash inside the quotes, which the value's
        // own escaping doubles; `%` is doubled; `\` ends up as four.
        assert_eq!(desktop_exec_quote("/a b/$x\"`%").as_deref(), Some(r#""/a b/\\$x\\"\\`%%""#));
        assert_eq!(desktop_exec_quote(r"/a\b").as_deref(), Some(r#""/a\\\\b""#));
        assert_eq!(desktop_exec_quote("/a\nExec=evil"), None);
    }
}
