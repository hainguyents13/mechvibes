//! Shares a whole look as one `.mvtheme` file: the colours, the logo and the
//! background, with their images inside, so a friend gets the same look with
//! one import.
//!
//! The file is a ZIP holding `theme.json` and the images it names. Whatever
//! comes out of a file is treated as untrusted: theme CSS keeps only colour
//! declarations, colours keep only colour functions, and images are checked
//! by their bytes and stored under names this app picks. A shared file can
//! change how the app looks, and nothing else.

use crate::libs::theme::{ BuiltInTheme, Theme };
use crate::state::config::{ AppConfig, BackgroundCustomization, LogoCustomization };
use crate::state::themes::ThemesConfig;
use crate::utils::soundpack_validator::is_portable_name;
use serde::{ Deserialize, Serialize };
use std::collections::HashMap;
use std::io::{ Cursor, Read, Write };
use std::path::Path;

pub const FILE_EXTENSION: &str = "mvtheme";

/// The layout `theme.json` is written in. A file from a later layout is
/// refused rather than half read.
const FORMAT: u32 = 1;
const MANIFEST_NAME: &str = "theme.json";
const MAX_MANIFEST_BYTES: u64 = 256 * 1024;
/// Per image. Wallpapers are the large ones, and 10 MB covers a 4K photo.
const MAX_IMAGE_BYTES: u64 = 10 * 1024 * 1024;
/// Three images at their limit plus the manifest.
pub const MAX_FILE_BYTES: u64 = 3 * MAX_IMAGE_BYTES + MAX_MANIFEST_BYTES;
const MAX_NAME_CHARS: usize = 64;
const MAX_CSS_VALUE_CHARS: usize = 200;

/// The colour functions a colour may use. `url()`, `image()` and the like are
/// not here, so a shared colour can never load anything.
const CSS_FUNCTIONS: [&str; 16] = [
    "var", "calc", "min", "max", "clamp", "rgb", "rgba", "hsl", "hsla", "hwb", "lab", "lch",
    "oklab", "oklch", "color", "color-mix",
];

/// The theme colours a look uses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SharedColors {
    BuiltIn(BuiltInTheme),
    Custom {
        css: String,
    },
}

/// `theme.json`. The logo and background image fields name files inside the
/// `.mvtheme`; a part is left out when the person sharing has it switched off.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Manifest {
    format: u32,
    name: String,
    colors: SharedColors,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    logo: Option<LogoCustomization>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    background: Option<BackgroundCustomization>,
}

/// A look read from a file or gathered from this app, with its images held in
/// memory under the names the logo and background fields use.
#[derive(Debug, Clone, PartialEq)]
pub struct SharedTheme {
    pub name: String,
    pub colors: SharedColors,
    pub logo: Option<LogoCustomization>,
    pub background: Option<BackgroundCustomization>,
    images: HashMap<String, Vec<u8>>,
}

impl SharedTheme {
    /// The colours' name as the import dialog shows it.
    pub fn colors_label(&self) -> String {
        match &self.colors {
            SharedColors::BuiltIn(theme) => format!("{:?}", theme),
            SharedColors::Custom { .. } => "Custom colors".to_string(),
        }
    }

    /// Give the look another name, tidied the way a name from a file is.
    /// Keeps the old one when nothing is left of `name` after tidying.
    pub fn rename(&mut self, name: &str) {
        if let Some(name) = clean_name(name) {
            self.name = name;
        }
    }
}

/// The name a share starts with: the custom theme's name, or the built-in one.
pub fn default_name(config: &AppConfig, themes: &ThemesConfig) -> String {
    match &config.theme {
        Theme::Custom(id) =>
            themes
                .get_theme_by_id(id)
                .map(|theme| theme.name.clone())
                .unwrap_or_else(|| "My theme".to_string()),
        Theme::BuiltIn(theme) => format!("My {:?} look", theme),
    }
}

// ===== Sharing =====

/// Gather the look this app shows now. Images are read from wherever they are
/// set, a web address included, so the file works without the internet and
/// without the person who made it.
pub async fn gather(
    name: &str,
    config: &AppConfig,
    themes: &ThemesConfig
) -> Result<SharedTheme, String> {
    let name = clean_name(name).ok_or_else(|| "Give the theme a name first.".to_string())?;
    let colors = match &config.theme {
        Theme::BuiltIn(theme) => SharedColors::BuiltIn(theme.clone()),
        Theme::Custom(id) => {
            let theme = themes
                .get_theme_by_id(id)
                .ok_or_else(|| "The selected theme is missing. Pick a theme and try again.".to_string())?;
            SharedColors::Custom { css: theme.css.clone() }
        }
    };

    let mut images = HashMap::new();
    let logo = if config.enable_logo_customization {
        let mut logo = config.logo_customization.clone();
        logo.background_image = gather_image(
            logo.use_background_image,
            logo.background_image.as_deref(),
            "logo",
            &mut images
        ).await?;
        logo.use_background_image = logo.background_image.is_some();
        logo.muted_background_image = gather_image(
            logo.use_muted_background_image,
            logo.muted_background_image.as_deref(),
            "logo-muted",
            &mut images
        ).await?;
        logo.use_muted_background_image = logo.muted_background_image.is_some();
        Some(logo)
    } else {
        None
    };
    let background = if config.enable_background_customization {
        let mut background = config.background_customization.clone();
        background.background_image = gather_image(
            background.use_image,
            background.background_image.as_deref(),
            "background",
            &mut images
        ).await?;
        background.use_image = background.background_image.is_some();
        Some(background)
    } else {
        None
    };

    Ok(SharedTheme { name, colors, logo, background, images })
}

/// Add the image at `value` to `images` as `<stem>.<ext>` and return that name.
/// An image the look does not show is left out.
async fn gather_image(
    in_use: bool,
    value: Option<&str>,
    stem: &str,
    images: &mut HashMap<String, Vec<u8>>
) -> Result<Option<String>, String> {
    let Some(value) = value.map(str::trim).filter(|value| in_use && !value.is_empty()) else {
        return Ok(None);
    };
    let bytes = load_image(value).await?;
    let extension = image_extension(&bytes).ok_or_else(||
        format!("'{}' is not a PNG, JPEG, GIF, WebP or BMP image.", value)
    )?;
    let file_name = format!("{}.{}", stem, extension);
    images.insert(file_name.clone(), bytes);
    Ok(Some(file_name))
}

/// The bytes behind an image setting: a picked image in this app's images
/// folder, a web address, or a path typed in by hand.
async fn load_image(value: &str) -> Result<Vec<u8>, String> {
    if let Some(file_name) = value.strip_prefix("/custom-images/") {
        if !is_portable_name(file_name) {
            return Err(format!("Could not read the image '{}'.", value));
        }
        let path = crate::state::paths::data::custom_images_dir().join(file_name);
        return read_image_file(&path);
    }
    if value.starts_with("https://") || value.starts_with("http://") {
        return download_image(value).await;
    }
    read_image_file(Path::new(value))
}

fn read_image_file(path: &Path) -> Result<Vec<u8>, String> {
    let file = std::fs::File
        ::open(path)
        .map_err(|_| format!("Could not read the image '{}'. Pick it again and retry.", path.display()))?;
    let mut bytes = Vec::new();
    file
        .take(MAX_IMAGE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Could not read the image '{}': {}", path.display(), e))?;
    if (bytes.len() as u64) > MAX_IMAGE_BYTES {
        return Err(format!("The image '{}' is larger than 10 MB. Use a smaller one.", path.display()));
    }
    Ok(bytes)
}

async fn download_image(url: &str) -> Result<Vec<u8>, String> {
    let failed = || format!("Could not download the image {}. Check your connection, or pick the image from your computer.", url);
    let mut response = crate::utils::library_client
        ::client()?
        .get(url)
        .send().await
        .map_err(|_| failed())?;
    if !response.status().is_success() {
        return Err(failed());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| failed())? {
        bytes.extend_from_slice(&chunk);
        if (bytes.len() as u64) > MAX_IMAGE_BYTES {
            return Err(format!("The image {} is larger than 10 MB. Use a smaller one.", url));
        }
    }
    Ok(bytes)
}

/// Write the look to `path` as a `.mvtheme`, its images cut down first. Slow
/// for a large wallpaper, so call it off the UI thread.
pub fn save(theme: &SharedTheme, path: &Path) -> Result<(), String> {
    let bytes = to_bytes(&shrink_images(theme))?;
    std::fs::write(path, bytes).map_err(|e| format!("Could not save the file: {}", e))
}

/// The look with its images cut down to what the app can show, so a shared
/// file stays small enough to send. An image that cannot be read, or that
/// comes out no smaller, is kept as it is.
fn shrink_images(theme: &SharedTheme) -> SharedTheme {
    let background = theme.background.as_ref().and_then(|b| b.background_image.as_deref());
    let mut renamed = HashMap::new();
    let mut images = HashMap::new();
    for (name, bytes) in &theme.images {
        let bytes = shrink_image(bytes, background == Some(name.as_str())).unwrap_or_else(|| bytes.clone());
        let stem = name.rsplit_once('.').map_or(name.as_str(), |(stem, _)| stem);
        let new_name = format!("{}.{}", stem, image_extension(&bytes).unwrap_or("png"));
        renamed.insert(name.clone(), new_name.clone());
        images.insert(new_name, bytes);
    }
    let mut shrunk = SharedTheme {
        name: theme.name.clone(),
        colors: theme.colors.clone(),
        logo: theme.logo.clone(),
        background: theme.background.clone(),
        images,
    };
    relink_images(&mut shrunk, &renamed);
    shrunk
}

/// The wallpaper fills a window 470pt wide and at least 600pt tall with
/// `center / cover`, so it is cut to that widest shape and sized for a 2x
/// screen up to 1080pt tall. A taller window only trims more off the sides.
const BACKGROUND_SHAPE: f64 = 470.0 / 600.0;
const BACKGROUND_BOX: (u32, u32) = (1692, 2160);
/// The logo box follows the font, so logo images are only shrunk, never cut.
const LOGO_BOX: (u32, u32) = (960, 960);
/// Larger images are kept as they are rather than decoded into memory.
const MAX_SHRINK_PIXELS: u64 = 40_000_000;
const JPEG_QUALITY: u8 = 85;

/// The image cut and shrunk to its box, as JPEG, or PNG when it has
/// transparency or is a PNG logo (JPEG would blur its edges). `None` when it
/// is better kept as it is: an animation (resizing would keep one frame), one
/// too large or damaged to decode, or one that does not come out smaller.
fn shrink_image(bytes: &[u8], background: bool) -> Option<Vec<u8>> {
    use image::{ codecs::jpeg::JpegEncoder, imageops::FilterType, ImageFormat };

    if is_animated(bytes) {
        return None;
    }
    let (width, height) = image::io::Reader
        ::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()?;
    if width == 0 || height == 0 || (width as u64) * (height as u64) > MAX_SHRINK_PIXELS {
        return None;
    }
    // The decoder ignores the EXIF orientation a camera writes, while the
    // window honours it. Turned upright here, since the copy has no EXIF.
    let mut image = upright(image::load_from_memory(bytes).ok()?, jpeg_orientation(bytes));
    let (width, height) = (image.width(), image.height());
    let (box_width, box_height) = if background {
        let crop_width = width.min(((height as f64) * BACKGROUND_SHAPE).round() as u32).max(1);
        let crop_height = height.min(((width as f64) / BACKGROUND_SHAPE).round() as u32).max(1);
        image = image.crop_imm((width - crop_width) / 2, (height - crop_height) / 2, crop_width, crop_height);
        BACKGROUND_BOX
    } else {
        LOGO_BOX
    };
    if image.width() > box_width || image.height() > box_height {
        // Keeps the shape and fits inside the box.
        image = image.resize(box_width, box_height, FilterType::Lanczos3);
    }

    let transparent = image.color().has_alpha() && image.to_rgba8().pixels().any(|pixel| pixel[3] < 255);
    let png_logo = !background && image_extension(bytes) == Some("png");
    let mut out = Vec::new();
    if transparent || png_logo {
        image.write_to(&mut Cursor::new(&mut out), ImageFormat::Png).ok()?;
    } else {
        JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY).encode_image(&image.to_rgb8()).ok()?;
    }
    (out.len() < bytes.len()).then_some(out)
}

/// A GIF, an animated PNG (an `acTL` chunk before the image data) or an
/// animated WebP (the animation flag of its `VP8X` header).
fn is_animated(bytes: &[u8]) -> bool {
    match image_extension(bytes) {
        Some("gif") => true,
        Some("png") => {
            let find = |chunk: &[u8]| bytes.windows(4).position(|window| window == chunk);
            matches!((find(b"acTL"), find(b"IDAT")), (Some(actl), Some(idat)) if actl < idat)
        }
        Some("webp") => bytes.get(12..16) == Some(b"VP8X") && bytes.get(20).is_some_and(|flags| flags & 0x02 != 0),
        _ => false,
    }
}

/// `image` turned the way an EXIF orientation (1 to 8) says it is shown.
fn upright(image: image::DynamicImage, orientation: u16) -> image::DynamicImage {
    match orientation {
        2 => image.fliph(),
        3 => image.rotate180(),
        4 => image.flipv(),
        5 => image.rotate90().fliph(),
        6 => image.rotate90(),
        7 => image.rotate270().fliph(),
        8 => image.rotate270(),
        _ => image,
    }
}

/// The EXIF orientation of a JPEG, or 1 (as stored) when it has none.
fn jpeg_orientation(bytes: &[u8]) -> u16 {
    if image_extension(bytes) != Some("jpg") {
        return 1;
    }
    // Segments after the start marker: 0xFF, a marker, a two byte length
    // that counts itself, then the data. The image data starts at 0xDA.
    let mut at = 2;
    while let (Some(0xff), Some(&marker)) = (bytes.get(at), bytes.get(at + 1)) {
        let Some(length) = bytes.get(at + 2..at + 4).map(|b| u16::from_be_bytes([b[0], b[1]]) as usize) else {
            break;
        };
        let Some(data) = bytes.get(at + 4..at + 2 + length).filter(|_| length >= 2) else {
            break;
        };
        if marker == 0xda {
            break;
        }
        if marker == 0xe1 {
            if let Some(orientation) = exif_orientation(data) {
                return orientation;
            }
        }
        at += 2 + length;
    }
    1
}

/// The orientation tag (0x0112) in the first IFD of an EXIF block.
fn exif_orientation(app1: &[u8]) -> Option<u16> {
    let tiff = app1.strip_prefix(b"Exif\0\0")?;
    let big_endian = match tiff.get(0..2)? {
        b"MM" => true,
        b"II" => false,
        _ => {
            return None;
        }
    };
    let u16_at = |at: usize| {
        let b = tiff.get(at..at + 2)?;
        Some(if big_endian { u16::from_be_bytes([b[0], b[1]]) } else { u16::from_le_bytes([b[0], b[1]]) })
    };
    let b = tiff.get(4..8)?;
    let ifd = (if big_endian { u32::from_be_bytes([b[0], b[1], b[2], b[3]]) } else { u32::from_le_bytes([b[0], b[1], b[2], b[3]]) }) as usize;
    for entry in 0..u16_at(ifd)? as usize {
        let at = ifd + 2 + entry * 12;
        if u16_at(at)? == 0x0112 {
            return u16_at(at + 8).filter(|orientation| (1..=8).contains(orientation));
        }
    }
    None
}

fn to_bytes(theme: &SharedTheme) -> Result<Vec<u8>, String> {
    let manifest = Manifest {
        format: FORMAT,
        name: theme.name.clone(),
        colors: theme.colors.clone(),
        logo: theme.logo.clone(),
        background: theme.background.clone(),
    };
    let json = serde_json
        ::to_vec_pretty(&manifest)
        .map_err(|e| format!("Could not write the theme: {}", e))?;

    let failed = |e: zip::result::ZipError| format!("Could not write the theme: {}", e);
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file(MANIFEST_NAME, zip::write::SimpleFileOptions::default()).map_err(failed)?;
    zip.write_all(&json).map_err(|e| format!("Could not write the theme: {}", e))?;
    // Images are compressed already; deflating them again only costs time.
    let stored = zip::write::SimpleFileOptions
        ::default()
        .compression_method(zip::CompressionMethod::Stored);
    let mut names: Vec<&String> = theme.images.keys().collect();
    names.sort();
    for name in names {
        zip.start_file(name.as_str(), stored).map_err(failed)?;
        zip.write_all(&theme.images[name]).map_err(|e| format!("Could not write the theme: {}", e))?;
    }
    Ok(zip.finish().map_err(failed)?.into_inner())
}

// ===== Importing =====

/// Read and check a `.mvtheme`. Nothing is installed yet; the user sees what
/// the file holds first.
pub fn open(path: &Path) -> Result<SharedTheme, String> {
    let size = std::fs::metadata(path).map_err(|e| format!("Could not open the file: {}", e))?.len();
    if size > MAX_FILE_BYTES {
        return Err("This theme file is too large.".to_string());
    }
    let bytes = std::fs::read(path).map_err(|e| format!("Could not open the file: {}", e))?;
    from_bytes(&bytes)
}

/// Read and check a `.mvtheme` held in memory, such as one downloaded from
/// the theme library. Same checks as `open`.
pub fn from_bytes(bytes: &[u8]) -> Result<SharedTheme, String> {
    if (bytes.len() as u64) > MAX_FILE_BYTES {
        return Err("This theme file is too large.".to_string());
    }
    let not_a_theme = || "This is not a Mechvibes theme file.".to_string();
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| not_a_theme())?;
    let manifest = read_entry(&mut archive, MANIFEST_NAME, MAX_MANIFEST_BYTES)?.ok_or_else(not_a_theme)?;
    let format = serde_json
        ::from_slice::<serde_json::Value>(&manifest)
        .map_err(|_| not_a_theme())?
        .get("format")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(not_a_theme)?;
    if format > (FORMAT as u64) {
        return Err("This theme was made with a newer Mechvibes. Update the app to use it.".to_string());
    }
    let manifest: Manifest = serde_json
        ::from_slice(&manifest)
        .map_err(|_| "This theme file is damaged.".to_string())?;

    let name = clean_name(&manifest.name).ok_or_else(|| "This theme file is damaged.".to_string())?;
    let colors = match manifest.colors {
        SharedColors::BuiltIn(theme) => SharedColors::BuiltIn(theme),
        SharedColors::Custom { css } => {
            let css = safe_theme_css(&css);
            if css.is_empty() {
                return Err("This theme has no colors this app can use.".to_string());
            }
            SharedColors::Custom { css }
        }
    };

    let mut images = HashMap::new();
    let logo = match manifest.logo {
        Some(mut logo) => {
            let defaults = LogoCustomization::default();
            for (color, default) in [
                (&mut logo.border_color, defaults.border_color),
                (&mut logo.text_color, defaults.text_color),
                (&mut logo.shadow_color, defaults.shadow_color),
                (&mut logo.background_color, defaults.background_color),
                (&mut logo.muted_background, defaults.muted_background),
            ] {
                *color = safe_color(color, default)?;
            }
            logo.background_image = open_image(&mut archive, logo.background_image, &mut images)?;
            logo.use_background_image &= logo.background_image.is_some();
            logo.muted_background_image = open_image(&mut archive, logo.muted_background_image, &mut images)?;
            logo.use_muted_background_image &= logo.muted_background_image.is_some();
            Some(logo)
        }
        None => None,
    };
    let background = match manifest.background {
        Some(mut background) => {
            background.background_color = safe_color(
                &background.background_color,
                BackgroundCustomization::default().background_color
            )?;
            background.background_image = open_image(&mut archive, background.background_image, &mut images)?;
            background.use_image &= background.background_image.is_some();
            Some(background)
        }
        None => None,
    };

    Ok(SharedTheme { name, colors, logo, background, images })
}

/// The image a manifest field names, checked and kept in `images`.
fn open_image<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: Option<String>,
    images: &mut HashMap<String, Vec<u8>>
) -> Result<Option<String>, String> {
    let Some(name) = name else {
        return Ok(None);
    };
    let damaged = || "This theme file is damaged: an image is missing or unreadable.".to_string();
    if !is_portable_name(&name) || name == MANIFEST_NAME {
        return Err(damaged());
    }
    let bytes = read_entry(archive, &name, MAX_IMAGE_BYTES)?.ok_or_else(damaged)?;
    if image_extension(&bytes).is_none() {
        return Err(damaged());
    }
    images.insert(name.clone(), bytes);
    Ok(Some(name))
}

/// The entry called `name`, or `None` when the file has none. Refuses one that
/// unpacks past `limit` before it is held in memory.
fn read_entry<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
    limit: u64
) -> Result<Option<Vec<u8>>, String> {
    let entry = match archive.by_name(name) {
        Ok(entry) => entry,
        Err(zip::result::ZipError::FileNotFound) => {
            return Ok(None);
        }
        Err(_) => {
            return Err("This theme file is damaged.".to_string());
        }
    };
    let mut bytes = Vec::new();
    entry
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "This theme file is damaged.".to_string())?;
    if (bytes.len() as u64) > limit {
        return Err("This theme file is too large.".to_string());
    }
    Ok(Some(bytes))
}

/// Copy the look's images into this app's images folder and point the logo
/// and background at the copies. Each copy gets a new name, so importing never
/// replaces an image already in use. Returns the files written, so a caller
/// that fails afterwards can take them back with `remove_images`.
pub fn install_images(theme: &mut SharedTheme) -> Result<Vec<std::path::PathBuf>, String> {
    let dir = crate::state::paths::data::custom_images_dir();
    crate::utils::path::ensure_directory_exists(&dir)?;
    let stem: String = theme.name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();

    let mut installed = HashMap::new();
    let mut written = Vec::new();
    for (name, bytes) in &theme.images {
        let extension = image_extension(bytes).unwrap_or("png");
        let file_name = format!("{}_{}.{}", stem, &uuid::Uuid::new_v4().simple().to_string()[..8], extension);
        let path = dir.join(&file_name);
        if let Err(e) = std::fs::write(&path, bytes) {
            let _ = std::fs::remove_file(&path);
            remove_images(&written);
            return Err(format!("Could not save the theme's images: {}", e));
        }
        written.push(path);
        installed.insert(name.clone(), format!("/custom-images/{}", file_name));
    }

    relink_images(theme, &installed);
    theme.images.clear();
    Ok(written)
}

/// Point the logo and background image fields at the new names in `names`.
/// A field whose image has no new name is cleared.
fn relink_images(theme: &mut SharedTheme, names: &HashMap<String, String>) {
    let relink = |image: &mut Option<String>| {
        *image = image.as_ref().and_then(|name| names.get(name).cloned());
    };
    if let Some(logo) = &mut theme.logo {
        relink(&mut logo.background_image);
        relink(&mut logo.muted_background_image);
    }
    if let Some(background) = &mut theme.background {
        relink(&mut background.background_image);
    }
}

/// Take back images `install_images` saved, when the import stops after it.
pub fn remove_images(paths: &[std::path::PathBuf]) {
    for path in paths {
        let _ = std::fs::remove_file(path);
    }
}

/// Put the look on `config`. A part the file leaves out is switched off, so
/// the result looks as it did for the person who shared it; the settings for
/// that part stay as they were for when it is switched back on.
pub fn apply_to_config(theme: &SharedTheme, theme_choice: Theme, config: &mut AppConfig) {
    config.theme = theme_choice;
    config.enable_logo_customization = theme.logo.is_some();
    if let Some(logo) = &theme.logo {
        config.logo_customization = logo.clone();
    }
    config.enable_background_customization = theme.background.is_some();
    if let Some(background) = &theme.background {
        config.background_customization = background.clone();
    }
}

// ===== Checks =====

pub(crate) fn clean_name(name: &str) -> Option<String> {
    let name: String = name
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let name: String = name.chars().take(MAX_NAME_CHARS).collect();
    let name = name.trim().to_string();
    (!name.is_empty()).then_some(name)
}

/// The kind of image `bytes` holds, read from its first bytes rather than a
/// name the file could get wrong.
fn image_extension(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Some("jpg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("gif")
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else if bytes.starts_with(b"BM") {
        Some("bmp")
    } else {
        None
    }
}

/// `value` when it is a plain colour or length: one that cannot end its
/// declaration, open a rule or a comment, leave a bracket open for what comes
/// after it, or call anything but a colour function.
pub fn safe_css_value(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > MAX_CSS_VALUE_CHARS {
        return None;
    }
    if
        value.contains("/*") ||
        value.contains("*/") ||
        value
            .chars()
            .any(|c| {
                c.is_control() ||
                    matches!(c, ';' | '{' | '}' | '[' | ']' | '<' | '>' | '"' | '\'' | '\\' | '@' | '`')
            })
    {
        return None;
    }
    let mut word_start = None;
    let mut depth: usize = 0;
    for (i, c) in value.char_indices() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            word_start.get_or_insert(i);
            continue;
        }
        if c == '(' {
            if let Some(start) = word_start {
                let function = value[start..i].to_ascii_lowercase();
                if !CSS_FUNCTIONS.contains(&function.as_str()) {
                    return None;
                }
            }
            depth += 1;
        }
        if c == ')' {
            depth = depth.checked_sub(1)?;
        }
        word_start = None;
    }
    (depth == 0).then(|| value.to_string())
}

/// A colour from a shared file. Configs from before the defaults became theme
/// variables can hold an empty colour, which means the default.
fn safe_color(value: &str, default: String) -> Result<String, String> {
    if value.trim().is_empty() {
        return Ok(default);
    }
    safe_css_value(value).ok_or_else(|| "This theme file has a color this app cannot use.".to_string())
}

/// The colour declarations of a theme's CSS, one per line. Anything else,
/// rules, at-rules and declarations whose value is not a plain colour or
/// length, is dropped.
pub fn safe_theme_css(css: &str) -> String {
    let mut without_comments = String::new();
    let mut rest = css;
    // A comment reads as a space, as in CSS itself, so removing one never
    // joins the text around it into a new comment.
    while let Some(start) = rest.find("/*") {
        without_comments.push_str(&rest[..start]);
        without_comments.push(' ');
        rest = match rest[start + 2..].find("*/") {
            Some(end) => &rest[start + 2 + end + 2..],
            None => "",
        };
    }
    without_comments.push_str(rest);

    without_comments
        .split(';')
        .filter_map(|declaration| {
            let (property, value) = declaration.split_once(':')?;
            let property = property.trim();
            let custom = property
                .strip_prefix("--")
                .is_some_and(|name| {
                    !name.is_empty() &&
                        name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                });
            if !custom && property != "color-scheme" {
                return None;
            }
            Some(format!("{}: {};", property, safe_css_value(value)?))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nrest-of-a-png";
    const JPEG: &[u8] = b"\xff\xd8\xff\xe0rest-of-a-jpeg";

    fn look() -> SharedTheme {
        let mut logo = LogoCustomization::default();
        logo.background_image = Some("logo.png".to_string());
        logo.use_background_image = true;
        let background = BackgroundCustomization {
            background_color: "#101010".to_string(),
            background_image: Some("background.jpg".to_string()),
            use_image: true,
        };
        SharedTheme {
            name: "Night drive".to_string(),
            colors: SharedColors::Custom { css: "--color-primary: oklch(55% 0.3 240);".to_string() },
            logo: Some(logo),
            background: Some(background),
            images: HashMap::from([
                ("logo.png".to_string(), PNG.to_vec()),
                ("background.jpg".to_string(), JPEG.to_vec()),
            ]),
        }
    }

    fn zip_with(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes) in entries {
            zip.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn a_shared_look_reads_back_the_same() {
        let theme = look();
        assert_eq!(from_bytes(&to_bytes(&theme).unwrap()).unwrap(), theme);
    }

    #[test]
    fn a_built_in_look_without_logo_or_background_reads_back() {
        let theme = SharedTheme {
            name: "Plain".to_string(),
            colors: SharedColors::BuiltIn(BuiltInTheme::Synthwave),
            logo: None,
            background: None,
            images: HashMap::new(),
        };
        let read = from_bytes(&to_bytes(&theme).unwrap()).unwrap();
        assert_eq!(read, theme);
        assert_eq!(read.colors_label(), "Synthwave");
    }

    #[test]
    fn a_file_from_a_newer_layout_asks_for_an_update() {
        let bytes = zip_with(&[(MANIFEST_NAME, br#"{"format": 2, "name": "x"}"#)]);
        assert!(from_bytes(&bytes).unwrap_err().contains("newer Mechvibes"));
    }

    #[test]
    fn a_file_that_is_not_a_theme_is_refused() {
        assert!(from_bytes(b"not a zip").unwrap_err().contains("not a Mechvibes theme"));
        let bytes = zip_with(&[("config.json", b"{}")]);
        assert!(from_bytes(&bytes).unwrap_err().contains("not a Mechvibes theme"));
    }

    #[test]
    fn an_image_that_is_missing_or_not_an_image_is_refused() {
        let mut theme = look();
        theme.images.remove("background.jpg");
        assert!(from_bytes(&to_bytes(&theme).unwrap()).unwrap_err().contains("image"));

        let mut theme = look();
        theme.images.insert("background.jpg".to_string(), b"<svg onload=alert(1)>".to_vec());
        assert!(from_bytes(&to_bytes(&theme).unwrap()).unwrap_err().contains("image"));
    }

    #[test]
    fn an_image_name_cannot_leave_the_file() {
        let mut theme = look();
        theme.logo.as_mut().unwrap().background_image = Some("../logo.png".to_string());
        assert!(from_bytes(&to_bytes(&theme).unwrap()).is_err());
    }

    #[test]
    fn a_colour_that_could_load_something_is_refused() {
        let mut theme = look();
        theme.logo.as_mut().unwrap().border_color = "red; background: url(https://example.com/x)".to_string();
        assert!(from_bytes(&to_bytes(&theme).unwrap()).unwrap_err().contains("color"));
    }

    #[test]
    fn an_empty_colour_from_an_older_config_reads_as_the_default() {
        let mut theme = look();
        theme.background.as_mut().unwrap().background_color = String::new();
        theme.logo.as_mut().unwrap().text_color = " ".to_string();
        let read = from_bytes(&to_bytes(&theme).unwrap()).unwrap();
        assert_eq!(read.background.unwrap().background_color, BackgroundCustomization::default().background_color);
        assert_eq!(read.logo.unwrap().text_color, LogoCustomization::default().text_color);
    }

    #[test]
    fn colours_keep_only_colour_functions() {
        assert_eq!(safe_css_value(" oklch(55% 0.3 240) ").as_deref(), Some("oklch(55% 0.3 240)"));
        assert!(safe_css_value("var(--color-base-200)").is_some());
        assert!(safe_css_value("color-mix(in oklab, red 40%, blue)").is_some());
        assert!(safe_css_value("#ff00aa").is_some());
        assert!(safe_css_value("url(https://example.com/x.png)").is_none());
        assert!(safe_css_value("URL(x)").is_none());
        assert!(safe_css_value("image-set(x 1x)").is_none());
        assert!(safe_css_value("red; color: blue").is_none());
        assert!(safe_css_value("red } body { display: none").is_none());
        assert!(safe_css_value("u\\72 l(x)").is_none());
        assert!(safe_css_value("`; alert(1); `").is_none());
        assert!(safe_css_value("").is_none());
    }

    #[test]
    fn a_colour_cannot_leave_a_comment_or_bracket_open() {
        assert!(safe_css_value("red /* rest").is_none());
        assert!(safe_css_value("red */").is_none());
        assert!(safe_css_value("rgb(1 2 3").is_none());
        assert!(safe_css_value("red)").is_none());
        assert!(safe_css_value("red [").is_none());
        assert!(safe_css_value("calc((1px + 2px) * 2)").is_some());
    }

    #[test]
    fn removing_a_comment_never_joins_a_new_one() {
        let css = safe_theme_css("--a: red//**/*x;\n--b: blue;");
        assert!(!css.contains("/*"), "{}", css);
        assert!(css.ends_with("--b: blue;"));
    }

    #[test]
    fn theme_css_keeps_only_colour_declarations() {
        let css = "/* Colors */\n--color-primary: oklch(55% 0.3 240);\n--radius-box: 0.5rem;\ncolor-scheme: dark;\n\
                   --evil: url(https://example.com/x);\n} body { display: none; }\n@import url(x);\n--last: #fff";
        assert_eq!(
            safe_theme_css(css),
            "--color-primary: oklch(55% 0.3 240);\n--radius-box: 0.5rem;\ncolor-scheme: dark;\n--last: #fff;"
        );
    }

    #[test]
    fn names_are_tidied_and_bounded() {
        assert_eq!(clean_name("  Night \n drive\u{0}  ").as_deref(), Some("Night drive"));
        assert_eq!(clean_name(&"a".repeat(100)).unwrap().chars().count(), MAX_NAME_CHARS);
        assert_eq!(clean_name(" \t "), None);
    }

    #[test]
    fn images_are_recognised_by_their_bytes() {
        assert_eq!(image_extension(PNG), Some("png"));
        assert_eq!(image_extension(JPEG), Some("jpg"));
        assert_eq!(image_extension(b"GIF89a..."), Some("gif"));
        assert_eq!(image_extension(b"RIFF\0\0\0\0WEBPVP8 "), Some("webp"));
        assert_eq!(image_extension(b"<svg/>"), None);
    }

    /// A PNG of noise, so it compresses badly the way a photo does. `alpha`
    /// below 255 makes it see-through.
    fn noisy_png(width: u32, height: u32, alpha: u8) -> Vec<u8> {
        let mut seed: u32 = 0x9e37_79b9;
        let image = image::RgbaImage::from_fn(width, height, |_, _| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let [r, g, b, _] = seed.to_le_bytes();
            image::Rgba([r, g, b, alpha])
        });
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        bytes
    }

    /// The noise as a best quality JPEG, the way a camera saves a photo.
    fn noisy_jpeg(width: u32, height: u32) -> Vec<u8> {
        let rgb = image::load_from_memory(&noisy_png(width, height, 255)).unwrap().to_rgb8();
        let mut bytes = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 100).encode_image(&rgb).unwrap();
        bytes
    }

    fn dimensions(bytes: &[u8]) -> (u32, u32) {
        image::load_from_memory(bytes).map(|image| (image.width(), image.height())).unwrap()
    }

    #[test]
    fn a_wide_wallpaper_is_cut_to_the_window_shape_as_jpeg() {
        let out = shrink_image(&noisy_png(3000, 1000, 255), true).expect("smaller");
        assert_eq!(image_extension(&out), Some("jpg"));
        assert_eq!(dimensions(&out), (783, 1000), "470:600 around the centre, not enlarged");
    }

    #[test]
    fn a_huge_wallpaper_is_sized_for_a_2x_screen() {
        let out = shrink_image(&noisy_png(2400, 3000, 255), true).expect("smaller");
        assert_eq!(dimensions(&out), (1692, 2160));
    }

    #[test]
    fn a_see_through_logo_is_shrunk_to_fit_but_never_cut() {
        let out = shrink_image(&noisy_png(2000, 1000, 128), false).expect("smaller");
        assert_eq!(image_extension(&out), Some("png"));
        assert_eq!(dimensions(&out), (960, 480));
    }

    #[test]
    fn an_image_is_kept_when_shrinking_cannot_help() {
        assert_eq!(shrink_image(b"GIF89a-an-animation", true), None, "a GIF keeps its frames");
        assert_eq!(shrink_image(PNG, true), None, "unreadable");
        let tiny = noisy_png(4, 4, 255);
        assert_eq!(shrink_image(&tiny, false), None, "already smaller than any JPEG");
    }

    #[test]
    fn sharing_renames_shrunk_images_and_points_the_look_at_them() {
        let mut theme = look();
        theme.images.insert("background.jpg".to_string(), noisy_png(1200, 1200, 255));
        let shrunk = shrink_images(&theme);
        assert_eq!(shrunk.background.as_ref().unwrap().background_image.as_deref(), Some("background.jpg"));
        assert_eq!(dimensions(&shrunk.images["background.jpg"]), (940, 1200));

        // A JPEG logo comes back as JPEG, under the name of its new type.
        let logo = noisy_jpeg(1200, 1200);
        theme.images.remove("logo.png");
        theme.images.insert("logo.jpg".to_string(), logo);
        theme.logo.as_mut().unwrap().background_image = Some("logo.jpg".to_string());
        theme.images.insert("background.jpg".to_string(), noisy_png(1200, 1200, 255));
        let shrunk = shrink_images(&theme);
        assert_eq!(shrunk.background.as_ref().unwrap().background_image.as_deref(), Some("background.jpg"));
        assert!(!shrunk.images.values().any(|bytes| image_extension(bytes) == Some("png")));
        assert_eq!(dimensions(&shrunk.images["logo.jpg"]), (960, 960));
        let read_back = from_bytes(&to_bytes(&shrunk).unwrap()).unwrap();
        assert_eq!(read_back, shrunk);
    }

    #[test]
    fn a_png_logo_stays_a_png_so_its_edges_stay_sharp() {
        let out = shrink_image(&noisy_png(1200, 1200, 255), false).expect("smaller");
        assert_eq!(image_extension(&out), Some("png"));
        assert_eq!(dimensions(&out), (960, 960));
    }

    /// A JPEG with an EXIF block holding `orientation`, written in either byte
    /// order, the way cameras and phones store a photo taken sideways.
    fn jpeg_with_orientation(width: u32, height: u32, orientation: u16, big_endian: bool) -> Vec<u8> {
        let jpeg = noisy_jpeg(width, height);
        let (u16b, u32b): (fn(u16) -> [u8; 2], fn(u32) -> [u8; 4]) = if big_endian {
            (u16::to_be_bytes, u32::to_be_bytes)
        } else {
            (u16::to_le_bytes, u32::to_le_bytes)
        };
        let mut tiff = Vec::new();
        tiff.extend_from_slice(if big_endian { b"MM" } else { b"II" });
        tiff.extend_from_slice(&u16b(42));
        tiff.extend_from_slice(&u32b(8));
        tiff.extend_from_slice(&u16b(1));
        tiff.extend_from_slice(&u16b(0x0112));
        tiff.extend_from_slice(&u16b(3));
        tiff.extend_from_slice(&u32b(1));
        tiff.extend_from_slice(&u16b(orientation));
        tiff.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
        let mut app1 = b"Exif\0\0".to_vec();
        app1.extend_from_slice(&tiff);
        let mut out = jpeg[..2].to_vec();
        out.extend_from_slice(&[0xff, 0xe1]);
        out.extend_from_slice(&((app1.len() + 2) as u16).to_be_bytes());
        out.extend_from_slice(&app1);
        out.extend_from_slice(&jpeg[2..]);
        out
    }

    #[test]
    fn the_exif_orientation_is_read_in_either_byte_order() {
        assert_eq!(jpeg_orientation(&jpeg_with_orientation(8, 8, 6, false)), 6);
        assert_eq!(jpeg_orientation(&jpeg_with_orientation(8, 8, 8, true)), 8);
        assert_eq!(jpeg_orientation(&jpeg_with_orientation(8, 8, 9, true)), 1, "not an orientation");
        assert_eq!(jpeg_orientation(JPEG), 1, "no EXIF");
        assert_eq!(jpeg_orientation(PNG), 1);
    }

    /// A photo taken upright but stored sideways is cut as it is shown.
    #[test]
    fn a_sideways_photo_is_turned_upright_before_it_is_cut() {
        let photo = jpeg_with_orientation(1200, 400, 6, false);
        let out = shrink_image(&photo, true).expect("smaller");
        assert_eq!(dimensions(&out), (400, 511), "shown 400 x 1200, cut to 470:600");
    }

    #[test]
    fn animations_are_recognised() {
        let mut apng = PNG[..8].to_vec();
        apng.extend_from_slice(b"\0\0\0\x08acTL....\0\0\0\0IDAT");
        assert!(is_animated(&apng));
        let mut still = PNG[..8].to_vec();
        still.extend_from_slice(b"\0\0\0\0IDAT then acTL in the data");
        assert!(!is_animated(&still));

        let webp = |flags: u8| [b"RIFF\0\0\0\0WEBPVP8X".as_slice(), &[10, 0, 0, 0, flags]].concat();
        assert!(is_animated(&webp(0x02)));
        assert!(!is_animated(&webp(0x10)));
        assert!(is_animated(b"GIF89a"));
    }

    #[test]
    fn applying_switches_off_the_parts_a_look_leaves_out() {
        let mut config = AppConfig::default();
        config.enable_logo_customization = true;
        config.logo_customization.text_color = "red".to_string();
        let theme = SharedTheme { logo: None, ..look() };
        apply_to_config(&theme, Theme::BuiltIn(BuiltInTheme::Dark), &mut config);
        assert_eq!(config.theme, Theme::BuiltIn(BuiltInTheme::Dark));
        assert!(!config.enable_logo_customization);
        assert_eq!(config.logo_customization.text_color, "red");
        assert!(config.enable_background_customization);
        assert_eq!(config.background_customization.background_color, "#101010");
    }
}
