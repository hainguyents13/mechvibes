use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Puts `soundpacks/` next to the built executable (`target/<profile>/`).
///
/// The app resolves its resources relative to the executable (see
/// `get_app_root` in src/state/paths.rs), so a bare `cargo build` binary with
/// no `soundpacks/` beside it loads zero packs and plays nothing. Packaged
/// builds (.app, AppImage, installer) bundle their own copy; this only makes
/// the plain `target/<profile>/mechvibes` run out of the box.
///
/// Best effort: a failure here must never fail the build.
fn sync_soundpacks() {
    println!("cargo:rerun-if-changed=soundpacks");

    let (Some(out_dir), Some(manifest_dir)) = (
        std::env::var_os("OUT_DIR").map(PathBuf::from),
        std::env::var_os("CARGO_MANIFEST_DIR").map(PathBuf::from),
    ) else {
        return;
    };

    // OUT_DIR is target/<profile>/build/<pkg>-<hash>/out, so the profile
    // directory (the one holding the executable) is three levels up.
    let Some(profile_dir) = out_dir.ancestors().nth(3) else {
        return;
    };

    // `dx serve` builds under target/dx/ and runs with the project root as its
    // app root, so it never reads this copy.
    if profile_dir.components().any(|c| c.as_os_str() == "dx") {
        return;
    }

    let src = manifest_dir.join("soundpacks");
    if !src.is_dir() {
        return;
    }

    if let Err(e) = copy_changed(&src, &profile_dir.join("soundpacks")) {
        println!("cargo:warning=could not copy soundpacks next to the executable: {e}");
    }
}

/// Copies `src` into `dst`, skipping files whose size and mtime already match
/// so repeat builds don't re-copy ~18 MB of audio. Never deletes anything.
fn copy_changed(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_changed(&from, &to)?;
            continue;
        }
        let from_meta = entry.metadata()?;
        let up_to_date = fs::metadata(&to)
            .map(|m| {
                m.len() == from_meta.len() && matches!(
                    (m.modified(), from_meta.modified()),
                    (Ok(a), Ok(b)) if a >= b
                )
            })
            .unwrap_or(false);
        if !up_to_date {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// Writes the stylesheet the app inlines to `OUT_DIR/style.css`.
///
/// On macOS the window draws with the WebKit built into the OS, not the one
/// Safari updates bring along. On macOS 12 that WebKit predates CSS nesting,
/// so every nested rule in the Tailwind/daisyUI output (tabs, toggles, pack
/// lists) is dropped and the window comes up half empty even with Safari 17.6
/// installed. For macOS builds the sheet is rewritten for Safari 15: nested
/// rules are flattened, colors get fallbacks where they can be computed, and
/// each `color-mix()` becomes the color that makes up most of it (see
/// `build/old_webkit_colors.rs`). Windows and Linux keep the sheet exactly as
/// written.
fn write_stylesheet() -> io::Result<()> {
    const SOURCE: &str = "assets/style.css";
    println!("cargo:rerun-if-changed={SOURCE}");
    println!("cargo:rerun-if-changed=build/old_webkit_colors.rs");

    let css = fs::read_to_string(SOURCE)?;
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR is set by cargo"))
        .join("style.css");

    let for_macos = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos");
    let css = if for_macos { lower_for_old_webkit(&css).map_err(io::Error::other)? } else { css };
    fs::write(out, css)
}

fn lower_for_old_webkit(css: &str) -> Result<String, String> {
    use lightningcss::stylesheet::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet};
    use lightningcss::targets::{Browsers, Targets};

    // Version bytes are major, minor, patch: Safari 15.0.
    let targets = Targets::from(Browsers { safari: Some(15 << 16), ..Browsers::default() });

    let css = old_webkit_colors::replace_color_mix(css);
    let mut sheet = StyleSheet::parse(&css, ParserOptions::default())
        .map_err(|e| format!("could not parse assets/style.css: {e}"))?;
    sheet
        .minify(MinifyOptions { targets, ..MinifyOptions::default() })
        .map_err(|e| format!("could not lower assets/style.css: {e}"))?;
    let printed = sheet
        .to_css(PrinterOptions { targets, ..PrinterOptions::default() })
        .map_err(|e| format!("could not print assets/style.css: {e}"))?;
    Ok(printed.code)
}

#[path = "build/old_webkit_colors.rs"]
mod old_webkit_colors;

fn main() -> io::Result<()> {
    sync_soundpacks();
    write_stylesheet()?;

    // Only compile resources on Windows
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();

        // Build the resources again when an icon changes. A build script
        // that names any file only runs again for the files it names.
        println!("cargo:rerun-if-changed=assets/icon.ico");
        println!("cargo:rerun-if-changed=assets/theme-file.ico");

        // Set application icon
        res.set_icon("assets/icon.ico");
        // The .mvtheme file icon. The installer points the file type at it
        // by this id: `mechvibes.exe,-2`.
        res.set_icon_with_id("assets/theme-file.ico", "2");

        // Set application metadata
        res.set("ProductName", "Mechvibes");
        res.set("FileDescription", "Mechvibes - Interactive Sound Simulator");
        res.set("CompanyName", "Hai Nguyen");
        res.set("LegalCopyright", "Copyright (C) 2026 Hai Nguyen");

        // Compile the resource file
        res.compile()?;
    }

    Ok(())
}
