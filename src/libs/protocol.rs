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
    env::var_os("APPIMAGE").is_some()
}

/// Make an AppImage the handler for `mechvibes://` links.
///
/// The `.deb` declares the handler in its own desktop file. An AppImage
/// installs nothing, so it writes a handler entry for the user each time it
/// starts, pointing at the AppImage file itself: the path it runs from inside
/// is a mount point that changes every launch. The entry is hidden from the
/// app menu, which already has the AppImage or the package's own entry.
#[cfg(target_os = "linux")]
pub fn register_protocol() -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;

    let home = env::var("HOME")?;
    let entry_name = format!("{}-url-handler.desktop", APP_NAME_LOWERCASE);
    let apps_dir = format!("{}/.local/share/applications", home);
    let desktop_file_path = format!("{}/{}", apps_dir, entry_name);
    let exe_path = env::var_os("APPIMAGE").map(std::path::PathBuf::from).ok_or("not an AppImage")?;

    let desktop_content = format!(
        r#"[Desktop Entry]
Name={}
Comment=Mechanical keyboard sound simulator
Exec="{}" %u
Icon={}
Type=Application
NoDisplay=true
MimeType=x-scheme-handler/{};
"#,
        APP_NAME,
        exe_path.to_string_lossy(),
        APP_NAME_LOWERCASE,
        APP_PROTOCOL
    );

    // Rewritten only when it changed, so an unchanged AppImage starts without
    // touching the desktop database.
    if fs::read_to_string(&desktop_file_path).ok().as_deref() == Some(desktop_content.as_str()) {
        return Ok(());
    }
    crate::always_print!("🐧 Registering {}// protocol on Linux...", APP_PROTOCOL);
    fs::create_dir_all(&apps_dir)?;
    fs::write(&desktop_file_path, desktop_content)?;

    let _ = Command::new("update-desktop-database").arg(&apps_dir).output();
    let _ = Command::new("xdg-mime")
        .args(["default", &entry_name, &format!("x-scheme-handler/{}", APP_PROTOCOL)])
        .output();

    crate::always_print!("✅ Protocol {}// registered successfully", APP_PROTOCOL);
    Ok(())
}

