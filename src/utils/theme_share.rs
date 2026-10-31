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
use std::collections::HashMap;
use std::io::{ Cursor, Read, Write };
use std::path::{ Path, PathBuf };
use wire_v1::{ BackgroundV1, ColorsV1, LogoV1, ManifestV1 };

mod preview;
mod wire_v1;

pub use preview::preview_stripes;

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

/// The theme colours a look uses. In a file they are written as
/// `wire_v1::ColorsV1`.
#[derive(Debug, Clone, PartialEq)]
pub enum SharedColors {
    BuiltIn(BuiltInTheme),
    Custom {
        css: String,
    },
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

    /// What the look brings for the logo, as the import dialog shows it, or
    /// `None` when it leaves the logo out.
    pub fn logo_label(&self) -> Option<&'static str> {
        self.logo.as_ref().map(|logo| {
            if logo.use_background_image || logo.use_muted_background_image {
                "Logo with images"
            } else {
                "Logo style"
            }
        })
    }

    /// What the look brings for the background, as the import dialog shows
    /// it, or `None` when it leaves the background out.
    pub fn background_label(&self) -> Option<&'static str> {
        self.background
            .as_ref()
            .map(|background| if background.use_image { "Background image" } else { "Background color" })
    }

    /// Give the look another name, tidied the way a name from a file is.
    /// Keeps the old one when nothing is left of `name` after tidying.
    pub fn rename(&mut self, name: &str) {
        if let Some(name) = clean_name(name) {
            self.name = name;
        }
    }

    /// Drops the unticked parts and the images only they used, so putting the
    /// look on brings nothing of a part the user chose to keep their own of.
    /// The colours have no images and stay; the caller leaves them out.
    pub fn keep_parts(&mut self, parts: ImportParts) {
        if !parts.logo {
            self.logo = None;
        }
        if !parts.background {
            self.background = None;
        }
        let mut used = Vec::new();
        if let Some(logo) = &self.logo {
            used.extend(logo.background_image.clone());
            used.extend(logo.muted_background_image.clone());
        }
        if let Some(background) = &self.background {
            used.extend(background.background_image.clone());
        }
        self.images.retain(|name, _| used.contains(name));
    }
}

/// What a theme looks like, without a name or image bytes: the colours, and
/// the logo and background when they are switched on.
#[derive(Debug, Clone, PartialEq)]
pub struct Look {
    pub colors: LookColors,
    /// `None` when the part is switched off.
    pub logo: Option<LogoCustomization>,
    pub background: Option<BackgroundCustomization>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LookColors {
    Known(SharedColors),
    /// `config.theme` is `Custom(id)` but the colors list has no such id. The
    /// screen falls back to System; the look itself never claims to be System.
    Missing(String),
}

impl Look {
    /// The look the app shows now.
    pub fn from_config(config: &AppConfig, themes: &ThemesConfig) -> Look {
        let colors = match &config.theme {
            Theme::BuiltIn(theme) => LookColors::Known(SharedColors::BuiltIn(theme.clone())),
            Theme::Custom(id) =>
                match themes.get_theme_by_id(id) {
                    Some(theme) => LookColors::Known(SharedColors::Custom { css: theme.css.clone() }),
                    None => LookColors::Missing(id.clone()),
                }
        };
        Look {
            colors,
            logo: config.enable_logo_customization.then(|| config.logo_customization.clone()),
            background: config.enable_background_customization.then(|| config.background_customization.clone()),
        }
    }
}

/// The parts of a theme the user chose to put on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportParts {
    pub colors: bool,
    pub logo: bool,
    pub background: bool,
}

impl ImportParts {
    pub const ALL: Self = Self { colors: true, logo: true, background: true };

    pub fn has(&self, part: Part) -> bool {
        match part {
            Part::Colors => self.colors,
            Part::Logo => self.logo,
            Part::Background => self.background,
        }
    }

    /// These parts with `part` ticked or not.
    pub fn with(mut self, part: Part, ticked: bool) -> Self {
        match part {
            Part::Colors => self.colors = ticked,
            Part::Logo => self.logo = ticked,
            Part::Background => self.background = ticked,
        }
        self
    }

    /// At least one part is ticked, so using the theme would change something.
    pub fn any(&self) -> bool {
        self.colors || self.logo || self.background
    }
}

/// One part of a theme, as the import dialog lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Colors,
    Logo,
    Background,
}

/// A line of the import dialog's checklist.
#[derive(Debug, Clone, PartialEq)]
pub struct PartRow {
    pub part: Part,
    /// What the theme brings for this part, or the part's name when it
    /// brings nothing.
    pub label: String,
    /// What ticking it does to the user's own part, when that is worth saying.
    pub note: Option<&'static str>,
}

/// The checklist the import dialog shows for `theme`, one line per part,
/// read from the theme itself so what is listed is what gets put on. `config`
/// says which of the user's own parts are on, and so get replaced.
pub fn checklist(theme: &SharedTheme, config: &AppConfig) -> [PartRow; 3] {
    let row = |part: Part, name: &str, label: Option<&str>, yours_on: bool, replaces: &'static str| PartRow {
        part,
        label: label.unwrap_or(name).to_string(),
        note: match label {
            Some(_) => yours_on.then_some(replaces),
            None => Some("Not in this theme. Will turn off yours."),
        },
    };
    [
        PartRow { part: Part::Colors, label: format!("Colors: {}", theme.colors_label()), note: None },
        row(Part::Logo, "Logo", theme.logo_label(), config.enable_logo_customization, "Replaces your logo"),
        row(
            Part::Background,
            "Background",
            theme.background_label(),
            config.enable_background_customization,
            "Replaces your background"
        ),
    ]
}

/// The name a share starts with: the custom theme's name, or the built-in one.
pub fn default_name(config: &AppConfig, themes: &ThemesConfig) -> String {
    match &config.theme {
        Theme::Custom(id) =>
            themes
                .get_theme_by_id(id)
                .map(|theme| theme.name.clone())
                .unwrap_or_else(|| "My theme".to_string()),
        Theme::BuiltIn(theme) => format!("My {:?} theme", theme),
    }
}

// ===== Sharing =====

/// Gather `look`, usually the one this app shows now. Images are read from
/// wherever they are set, a web address included, so the file works without
/// the internet and without the person who made it.
pub async fn gather(name: &str, look: &Look) -> Result<SharedTheme, String> {
    let name = clean_name(name).ok_or_else(|| "Give the theme a name first.".to_string())?;
    let colors = match &look.colors {
        LookColors::Known(colors) => colors.clone(),
        LookColors::Missing(_) => {
            return Err("The selected theme is missing. Pick a theme and try again.".to_string());
        }
    };

    let mut images = HashMap::new();
    let logo = if let Some(logo) = &look.logo {
        let mut logo = logo.clone();
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
    let background = if let Some(background) = &look.background {
        let mut background = background.clone();
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

/// Where Share writes, and the name the theme in the file takes, from the path
/// the save dialog gave. A dialog that leaves the extension off gets it added
/// after what the user typed, never in place of part of it, so "Neon v1.2"
/// stays "Neon v1.2". When that lands on a file the dialog never asked about,
/// it is refused rather than written over.
pub fn share_target(chosen: &Path) -> Result<(PathBuf, String), String> {
    let is_theme_file = chosen
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(FILE_EXTENSION));
    let name = if is_theme_file { chosen.file_stem() } else { chosen.file_name() };
    let name = name.map(|name| name.to_string_lossy().to_string()).unwrap_or_default();
    if is_theme_file {
        return Ok((chosen.to_path_buf(), name));
    }
    let mut path = chosen.as_os_str().to_os_string();
    path.push(format!(".{}", FILE_EXTENSION));
    let path = PathBuf::from(path);
    if path.symlink_metadata().is_ok() {
        return Err(format!("There is already a file called \"{}.{}\". Share again and pick another name.", name, FILE_EXTENSION));
    }
    Ok((path, name))
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
    let manifest = ManifestV1 {
        format: FORMAT,
        name: theme.name.clone(),
        colors: ColorsV1::from_shared(&theme.colors),
        logo: theme.logo.as_ref().map(LogoV1::from_config),
        background: theme.background.as_ref().map(BackgroundV1::from_config),
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
///
/// A folder or a pipe is refused before it is opened, since opening a pipe
/// waits until something writes to it. The checks run again on the opened
/// file itself, so it cannot be swapped between the check and the read, and
/// at most one byte past the limit is read, so a file that keeps growing is
/// refused rather than read without end.
pub fn open(path: &Path) -> Result<SharedTheme, String> {
    let failed = |e: std::io::Error| format!("Could not open the file: {}", e);
    if !std::fs::metadata(path).map_err(failed)?.is_file() {
        return Err("This is not a Mechvibes theme file.".to_string());
    }
    let file = std::fs::File::open(path).map_err(failed)?;
    if !file.metadata().map_err(failed)?.is_file() {
        return Err("This is not a Mechvibes theme file.".to_string());
    }
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(failed)?;
    if (bytes.len() as u64) > MAX_FILE_BYTES {
        return Err("This theme file is too large.".to_string());
    }
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
    let manifest: ManifestV1 = serde_json
        ::from_slice(&manifest)
        .map_err(|_| "This theme file is damaged.".to_string())?;

    let name = clean_name(&manifest.name).ok_or_else(|| "This theme file is damaged.".to_string())?;
    let colors = match manifest.colors.into_shared().ok_or_else(|| "This theme file is damaged.".to_string())? {
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
    let logo = match manifest.logo.map(LogoV1::into_config) {
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
    let background = match manifest.background.map(BackgroundV1::into_config) {
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
/// and background at the copies. Each copy is named after its content, so the
/// same image imported twice is one file. Returns only the files this call
/// created, so a caller that fails afterwards can take them back with
/// `remove_images` without removing an image something else already uses.
pub fn install_images(theme: &mut SharedTheme) -> Result<Vec<std::path::PathBuf>, String> {
    install_images_in(theme, &crate::state::paths::data::custom_images_dir())
}

fn install_images_in(theme: &mut SharedTheme, dir: &Path) -> Result<Vec<std::path::PathBuf>, String> {
    let mut installed = HashMap::new();
    let mut created = Vec::new();
    for (name, bytes) in &theme.images {
        match crate::utils::custom_images::save_in(dir, bytes, None) {
            Ok((file_name, is_new)) => {
                if is_new {
                    created.push(dir.join(&file_name));
                }
                installed.insert(name.clone(), format!("/custom-images/{}", file_name));
            }
            Err(e) => {
                remove_images(&created);
                return Err(format!("Could not save the theme's images. {}", e));
            }
        }
    }

    relink_images(theme, &installed);
    theme.images.clear();
    Ok(created)
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

/// Put the ticked `parts` of the look on `config`, with `colors` as the theme
/// its colours became. A ticked part the file leaves out is switched off, so
/// the result looks as it did for the person who shared it; the settings for
/// that part stay as they were for when it is switched back on. An unticked
/// part, and colours given as `None`, leave the config as it was.
pub fn apply_to_config(theme: &SharedTheme, colors: Option<Theme>, parts: ImportParts, config: &mut AppConfig) {
    if let (true, Some(choice)) = (parts.colors, colors) {
        config.theme = choice;
    }
    if parts.logo {
        config.enable_logo_customization = theme.logo.is_some();
        if let Some(logo) = &theme.logo {
            config.logo_customization = logo.clone();
        }
    }
    if parts.background {
        config.enable_background_customization = theme.background.is_some();
        if let Some(background) = &theme.background {
            config.background_customization = background.clone();
        }
    }
}

// ===== Checks =====

pub(crate) fn clean_name(name: &str) -> Option<String> {
    clean_text(name, MAX_NAME_CHARS)
}

/// `text` on one line, without control characters and cut to `max_chars`,
/// or `None` when nothing is left.
pub(crate) fn clean_text(text: &str, max_chars: usize) -> Option<String> {
    let text: String = text
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let text: String = text.chars().take(max_chars).collect();
    let text = text.trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// The kind of image `bytes` holds, read from its first bytes rather than a
/// name the file could get wrong.
pub(crate) fn image_extension(bytes: &[u8]) -> Option<&'static str> {
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
    safe_css_value(value)
        .filter(|value| is_one_value(value))
        .ok_or_else(|| "This theme file has a color this app cannot use.".to_string())
}

/// Whether `value` is one value rather than a list or several: no comma or
/// space outside a function's brackets. The logo puts its colours in a
/// `box-shadow`, which takes a list, so `red, 0 0 0 100vmax black` would add
/// a shadow of its own over the page.
pub(crate) fn is_one_value(value: &str) -> bool {
    let mut depth: usize = 0;
    for c in value.trim().chars() {
        match c {
            '(' => {
                depth += 1;
            }
            ')' => {
                depth = depth.saturating_sub(1);
            }
            ',' if depth == 0 => {
                return false;
            }
            c if c.is_whitespace() && depth == 0 => {
                return false;
            }
            _ => {}
        }
    }
    true
}

/// The declarations of a theme's CSS this app knows, one per line: the
/// `--color-*` colours, `color-scheme` and the daisyUI sizes, each with a
/// value it can use. Anything else, rules, at-rules, other properties, a
/// colour that draws nothing and a size out of range, is dropped, so a theme
/// can change how the app looks but cannot hide its text or blow up its
/// controls.
pub fn safe_theme_css(css: &str) -> String {
    without_comments(css)
        .split(';')
        .filter_map(|declaration| {
            let (property, value) = declaration.split_once(':')?;
            let property = property.trim();
            let value = safe_css_value(value)?;
            theme_value_fits(property, &value).then(|| format!("{}: {};", property, value))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// How many declarations `css` holds, kept or not, so a caller can tell
/// whether `safe_theme_css` left any out.
pub(crate) fn theme_css_declarations(css: &str) -> usize {
    without_comments(css)
        .split(';')
        .filter(|declaration| !declaration.trim().is_empty())
        .count()
}

fn without_comments(css: &str) -> String {
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
}

/// The colour functions whose alpha comes after a `/`.
const ALPHA_FUNCTIONS: [&str; 10] = [
    "rgb", "rgba", "hsl", "hsla", "hwb", "lab", "lch", "oklab", "oklch", "color",
];

/// Whether `property` is a theme variable this app knows and `value`, already
/// through `safe_css_value`, is one it can use.
fn theme_value_fits(property: &str, value: &str) -> bool {
    // A value never needs `!important`, and `transparent !important` would
    // still draw nothing.
    if value.contains('!') {
        return false;
    }
    if let Some(name) = property.strip_prefix("--color-") {
        return !name.is_empty() &&
            name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') &&
            is_one_value(value) &&
            !is_invisible_color(value);
    }
    let length_within = |low: f64, high: f64| length_px(value).is_some_and(|px| (low..=high).contains(&px));
    match property {
        "color-scheme" => value == "light" || value == "dark",
        "--radius-selector" | "--radius-field" | "--radius-box" => length_within(0.0, 32.0),
        "--size-selector" | "--size-field" => length_within(2.0, 8.0),
        "--border" => length_within(0.5, 2.0),
        "--depth" | "--noise" => value == "0" || value == "1",
        _ => false,
    }
}

/// A colour that draws nothing: `transparent`, a hex colour whose alpha is
/// zero, or a colour function whose alpha is not a number above zero (`0`,
/// `-1`, `none`, `calc(0)` all draw nothing). The whole value is searched, so
/// such a colour cannot hide in a `var()` fallback, a `color-mix()` or the
/// colour a relative colour starts from.
pub(crate) fn is_invisible_color(value: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    let word = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_';
    let draws_nothing = value
        .split(|c: char| !word(c) && c != '#')
        .any(|token| {
            token == "transparent" ||
                token.strip_prefix('#').is_some_and(|hex| {
                    (hex.len() == 4 && hex.ends_with('0')) || (hex.len() == 8 && hex.ends_with("00"))
                })
        });
    if draws_nothing {
        return true;
    }

    let mut word_start = None;
    for (open, c) in value.char_indices() {
        if word(c) {
            word_start.get_or_insert(open);
            continue;
        }
        let function = word_start.take().map(|start| &value[start..open]);
        if c == '(' && function.is_some_and(|function| ALPHA_FUNCTIONS.contains(&function)) {
            let alpha = function_alpha(&value[open + 1..]);
            if alpha.is_some_and(|alpha| !is_visible_alpha(alpha)) {
                return true;
            }
        }
    }
    false
}

/// The alpha of a colour function, given the text after its `(`: what follows
/// a `/`, or the fourth of the comma separated values of the older
/// `rgba(r, g, b, a)` form, at the function's own level.
fn function_alpha(after_open: &str) -> Option<&str> {
    let mut depth: usize = 0;
    let mut slash = None;
    let mut commas = Vec::new();
    let mut end = after_open.len();
    for (i, c) in after_open.char_indices() {
        match c {
            '(' => {
                depth += 1;
            }
            ')' if depth == 0 => {
                end = i;
                break;
            }
            ')' => {
                depth -= 1;
            }
            '/' if depth == 0 => {
                slash = Some(i);
            }
            ',' if depth == 0 => commas.push(i),
            _ => {}
        }
    }
    let at = match (slash, commas.as_slice()) {
        (Some(at), _) => at,
        (None, [_, _, at]) => *at,
        _ => {
            return None;
        }
    };
    Some(after_open[at + 1..end].trim())
}

/// An alpha that draws something: a number or percentage above zero, or
/// `alpha`, which keeps the alpha of the colour a relative colour starts from.
fn is_visible_alpha(alpha: &str) -> bool {
    if alpha == "alpha" {
        return true;
    }
    let number = alpha.strip_suffix('%').unwrap_or(alpha);
    number.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | '+' | '-' | 'e')) &&
        number.parse::<f64>().is_ok_and(|n| n.is_finite() && n > 0.0)
}

/// A plain length in px: `0`, or a number in `px` or `rem` (16px).
fn length_px(value: &str) -> Option<f64> {
    if value == "0" {
        return Some(0.0);
    }
    match value.strip_suffix("rem") {
        Some(number) => plain_number(number).map(|rem| rem * 16.0),
        None => plain_number(value.strip_suffix("px")?),
    }
}

/// A number written with digits and at most one point, such as `2`, `0.5`
/// or `.25`. No sign, exponent or anything else.
fn plain_number(text: &str) -> Option<f64> {
    let (whole, fraction) = text.split_once('.').unwrap_or((text, "0"));
    let digits = |part: &str| part.chars().all(|c| c.is_ascii_digit());
    if text.is_empty() || fraction.is_empty() || !digits(whole) || !digits(fraction) {
        return None;
    }
    text.parse().ok()
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

    /// The `theme.json` of a `.mvtheme`, as JSON.
    fn manifest_json(bytes: &[u8]) -> serde_json::Value {
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let manifest = read_entry(&mut archive, MANIFEST_NAME, MAX_MANIFEST_BYTES).unwrap().unwrap();
        serde_json::from_slice(&manifest).unwrap()
    }

    fn keys(value: &serde_json::Value) -> Vec<&str> {
        let mut keys: Vec<&str> = value.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort();
        keys
    }

    /// The file keeps the keys format 1 has, whatever the app's own settings
    /// grow into: 3.3 refuses a file missing one of these as damaged.
    #[test]
    fn written_parts_keep_exactly_the_format_one_keys() {
        let full = manifest_json(&to_bytes(&look()).unwrap());
        assert_eq!(keys(&full), ["background", "colors", "format", "logo", "name"]);
        assert_eq!(
            keys(&full["logo"]),
            [
                "background_color",
                "background_image",
                "border_color",
                "dimmed_when_muted",
                "muted_background",
                "muted_background_image",
                "shadow_color",
                "text_color",
                "use_background_image",
                "use_muted_background_image",
            ]
        );
        assert_eq!(keys(&full["background"]), ["background_color", "background_image", "use_image"]);
        assert_eq!(full["format"], 1);
        assert_eq!(full["colors"], serde_json::json!({ "custom": { "css": "--color-primary: oklch(55% 0.3 240);" } }));

        // An empty image is written as null, as 3.3 does, and a part that is
        // switched off is left out rather than written as null.
        let mut plain = look();
        plain.logo.as_mut().unwrap().background_image = None;
        plain.images.remove("logo.png");
        plain.background = None;
        plain.colors = SharedColors::BuiltIn(BuiltInTheme::Synthwave);
        let plain = manifest_json(&to_bytes(&plain).unwrap());
        assert_eq!(keys(&plain), ["colors", "format", "logo", "name"]);
        assert!(plain["logo"]["background_image"].is_null());
        assert_eq!(keys(&plain["logo"]).len(), 10);
        assert_eq!(plain["colors"], serde_json::json!({ "built_in": "Synthwave" }));
    }

    /// A file from `theme_share/fixtures/v1/` and what Mechvibes 3.3 made of
    /// it. The README there says how they were made.
    macro_rules! v3_3_file {
        ($name:literal) => {
            (
                $name,
                include_bytes!(concat!("theme_share/fixtures/v1/", $name, ".mvtheme")).as_slice(),
                include_str!(concat!("theme_share/fixtures/v1/", $name, ".expected.json")),
            )
        };
    }

    fn v3_3_files() -> Vec<(&'static str, &'static [u8], serde_json::Value)> {
        [
            v3_3_file!("custom-full"),
            v3_3_file!("built-in-plain"),
            v3_3_file!("logo-colors-only"),
            v3_3_file!("background-color-only"),
            v3_3_file!("older-empty-colors"),
            v3_3_file!("unknown-fields"),
            v3_3_file!("library-rewritten"),
        ]
            .into_iter()
            .map(|(name, bytes, expected)| (name, bytes, as_this_build_reads(name, serde_json::from_str(expected).unwrap())))
            .collect()
    }

    /// The one place this build reads a 3.3 file differently on purpose:
    /// theme CSS keeps only the variables this app knows, so the
    /// `--sparkle-size` line of `unknown-fields` is left out.
    fn as_this_build_reads(name: &str, mut expected: serde_json::Value) -> serde_json::Value {
        if name == "unknown-fields" {
            for snapshot in ["read", "written_again"] {
                let css = &mut expected[snapshot]["colors"]["custom"]["css"];
                let kept = css.as_str().unwrap().replace("\n--sparkle-size: 3rem;", "");
                assert_ne!(css.as_str().unwrap(), kept, "{}: the line 3.3 kept", snapshot);
                *css = kept.into();
            }
        }
        expected
    }

    /// `base` with the theme, logo and background settings a snapshot's
    /// `start` or `applied` holds.
    fn with_look(base: &AppConfig, look: &serde_json::Value) -> AppConfig {
        let field = |key: &str| look[key].clone();
        let mut config = base.clone();
        config.theme = serde_json::from_value(field("theme")).unwrap();
        config.enable_logo_customization = serde_json::from_value(field("enable_logo_customization")).unwrap();
        config.logo_customization = serde_json::from_value(field("logo_customization")).unwrap();
        config.enable_background_customization = serde_json::from_value(field("enable_background_customization")).unwrap();
        config.background_customization = serde_json::from_value(field("background_customization")).unwrap();
        config
    }

    #[test]
    fn format_one_files_from_v3_3_read_the_same() {
        use sha2::{ Digest, Sha256 };

        for (name, bytes, expected) in v3_3_files() {
            let read = from_bytes(bytes).unwrap_or_else(|e| panic!("{}: {}", name, e));
            let expected = &expected["read"];

            assert_eq!(read.name, expected["name"], "{}: name", name);
            let colors = match &read.colors {
                SharedColors::BuiltIn(theme) => serde_json::json!({ "built_in": format!("{:?}", theme) }),
                SharedColors::Custom { css } => serde_json::json!({ "custom": { "css": css } }),
            };
            assert_eq!(colors, expected["colors"], "{}: colors", name);
            let logo: Option<LogoCustomization> = serde_json::from_value(expected["logo"].clone()).unwrap();
            assert_eq!(read.logo, logo, "{}: logo", name);
            let background: Option<BackgroundCustomization> = serde_json
                ::from_value(expected["background"].clone())
                .unwrap();
            assert_eq!(read.background, background, "{}: background", name);
            let images: serde_json::Map<String, serde_json::Value> = read.images
                .iter()
                .map(|(image, bytes)| {
                    (image.clone(), crate::utils::update_installer::hex_encode(&Sha256::digest(bytes)).into())
                })
                .collect();
            assert_eq!(serde_json::Value::Object(images), expected["images"], "{}: images", name);
        }
    }

    #[test]
    fn format_one_files_apply_the_same_as_v3_3() {
        let base = AppConfig::default();
        for (name, bytes, expected) in v3_3_files() {
            let theme = from_bytes(bytes).unwrap_or_else(|e| panic!("{}: {}", name, e));
            let choice = match &theme.colors {
                SharedColors::BuiltIn(built_in) => Theme::BuiltIn(built_in.clone()),
                SharedColors::Custom { .. } => Theme::Custom("imported-colors".to_string()),
            };
            let mut config = with_look(&base, &expected["start"]);
            apply_to_config(&theme, Some(choice), ImportParts::ALL, &mut config);
            assert_eq!(config, with_look(&base, &expected["applied"]), "{}", name);
        }
    }

    /// A file read and written again holds what 3.3 wrote back, so a file
    /// that passes through this build still opens in 3.3 as it did.
    #[test]
    fn writing_a_v3_3_file_again_keeps_its_fields() {
        for (name, bytes, expected) in v3_3_files() {
            let written = to_bytes(&from_bytes(bytes).unwrap_or_else(|e| panic!("{}: {}", name, e))).unwrap();
            assert_eq!(manifest_json(&written), expected["written_again"], "{}", name);
        }
    }

    #[test]
    fn an_unknown_built_in_name_is_damaged() {
        let manifest = br#"{"format": 1, "name": "x", "colors": {"built_in": "Neon"}}"#;
        assert_eq!(from_bytes(&zip_with(&[(MANIFEST_NAME, manifest)])).unwrap_err(), "This theme file is damaged.");
        let manifest = br#"{"format": 1, "name": "x", "colors": {"built_in": "Synthwave"}}"#;
        assert!(from_bytes(&zip_with(&[(MANIFEST_NAME, manifest)])).is_ok());
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
        let css = safe_theme_css("--color-a: red//**/*x;\n--color-b: blue;");
        assert!(!css.contains("/*"), "{}", css);
        assert!(css.ends_with("--color-b: blue;"));
    }

    #[test]
    fn theme_css_keeps_only_colour_declarations() {
        let css = "/* Colors */\n--color-primary: oklch(55% 0.3 240);\n--radius-box: 0.5rem;\ncolor-scheme: dark;\n\
                   --evil: url(https://example.com/x);\n} body { display: none; }\n@import url(x);\n--color-last: #fff";
        assert_eq!(
            safe_theme_css(css),
            "--color-primary: oklch(55% 0.3 240);\n--radius-box: 0.5rem;\ncolor-scheme: dark;\n--color-last: #fff;"
        );
    }

    #[test]
    fn a_theme_cannot_hide_the_text() {
        let css = "--color-base-content: transparent;\n--color-primary: #0000;\n\
                   --color-base-100: oklch(50% 0.1 30 / 0);\n--size-field: 999rem;\n--evil: red;\n\
                   --color-primary: oklch(55% 0.3 240);\n--radius-box: 0.5rem;\ncolor-scheme: dark;";
        assert_eq!(
            safe_theme_css(css),
            "--color-primary: oklch(55% 0.3 240);\n--radius-box: 0.5rem;\ncolor-scheme: dark;"
        );
    }

    #[test]
    fn colours_that_draw_nothing_are_recognised() {
        for invisible in [
            "transparent",
            "TRANSPARENT",
            "#0000",
            "#ffffff00",
            "rgb(0 0 0 / 0)",
            "rgba(255, 0, 0, 0)",
            "hsla(10, 50%, 50%, 0.0)",
            "oklch(50% 0.1 30 / 0%)",
            "oklch(from var(--color-primary) l c h / .0)",
        ] {
            assert!(is_invisible_color(invisible), "{}", invisible);
        }
        for visible in [
            "#000",
            "#00000001",
            "red",
            "rgb(0 0 0)",
            "rgba(0, 0, 0, 0.5)",
            "oklch(50% calc(0.2 / 2) 0)",
            "oklch(from var(--color-primary) l c h / 0.05)",
            "var(--color-base-100)",
            "color-mix(in oklab, red 0%, blue)",
        ] {
            assert!(!is_invisible_color(visible), "{}", visible);
        }
    }

    #[test]
    fn a_colour_cannot_draw_nothing_by_another_spelling() {
        for invisible in [
            "oklch(50% 0.1 30 / -1)",
            "oklch(50% 0.1 30 / none)",
            "oklch(50% 0.1 30 / calc(0))",
            "rgb(0 0 0 / +0)",
            "rgb(0 0 0 / 0e0)",
            "var(--color-nothing, transparent)",
            "var(--color-nothing, #0000)",
            "color-mix(in oklab, transparent, transparent)",
            "oklch(from transparent l c h)",
            "oklch(from rgb(0 0 0 / 0) l c h / alpha)",
        ] {
            assert!(is_invisible_color(invisible), "{}", invisible);
        }
        assert!(!is_invisible_color("oklch(from var(--color-primary) l c h / alpha)"));
        assert!(!is_invisible_color("rgb(0 0 0 / 1e0)"));
        assert_eq!(safe_theme_css("--color-base-content: transparent !important;"), "");
        assert_eq!(safe_theme_css("--color-base-content: red !important;"), "");
    }

    #[test]
    fn a_colour_is_one_colour_not_a_list() {
        let list = "red, 0 0 0 100vmax rgb(0 0 0)";
        assert_eq!(safe_theme_css(&format!("--color-primary: {};", list)), "");
        assert_eq!(safe_theme_css("--color-primary: red blue;"), "");
        assert_eq!(
            safe_theme_css("--color-primary: color-mix(in oklab, red 40%, blue);"),
            "--color-primary: color-mix(in oklab, red 40%, blue);"
        );

        let mut theme = look();
        theme.logo.as_mut().unwrap().shadow_color = list.to_string();
        assert_eq!(
            from_bytes(&to_bytes(&theme).unwrap()).unwrap_err(),
            "This theme file has a color this app cannot use."
        );
        let mut theme = look();
        theme.background.as_mut().unwrap().background_color = list.to_string();
        assert!(from_bytes(&to_bytes(&theme).unwrap()).is_err());

        let mut theme = look();
        theme.logo.as_mut().unwrap().shadow_color = "oklch(from var(--color-primary) l c h / 0.05)".to_string();
        assert!(from_bytes(&to_bytes(&theme).unwrap()).is_ok(), "spaces inside a function are fine");
    }

    #[test]
    fn theme_sizes_must_stay_in_range() {
        let kept = |declaration: &str| !safe_theme_css(declaration).is_empty();
        assert!(kept("--radius-box: 0;") && kept("--radius-box: 2rem;") && kept("--radius-field: 32px;"));
        assert!(!kept("--radius-box: 2.5rem;") && !kept("--radius-box: -1px;") && !kept("--radius-box: 1em;"));
        assert!(kept("--size-field: 0.125rem;") && kept("--size-selector: 8px;"));
        assert!(!kept("--size-field: 0;") && !kept("--size-field: 1px;") && !kept("--size-field: 1e1px;"));
        assert!(kept("--border: 0.5px;") && kept("--border: 2px;"));
        assert!(!kept("--border: 0;") && !kept("--border: 3px;"));
        assert!(kept("--depth: 1;") && kept("--noise: 0;"));
        assert!(!kept("--depth: 2;") && !kept("--noise: 0.5;"));
        assert!(kept("color-scheme: light;") && !kept("color-scheme: normal;"));
        assert!(!kept("--color-: red;") && !kept("--color-a_b: red;") && !kept("--radius-other: 1px;"));
    }

    #[test]
    fn the_default_colors_template_passes_the_filter() {
        use crate::components::customize::colors_editor_modal::THEME_DEFAULT_CSS;

        let declarations: Vec<&str> = THEME_DEFAULT_CSS.lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with("/*"))
            .collect();
        assert!(declarations.len() > 20);
        assert_eq!(safe_theme_css(THEME_DEFAULT_CSS), declarations.join("\n"));
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
        apply_to_config(&theme, Some(Theme::BuiltIn(BuiltInTheme::Dark)), ImportParts::ALL, &mut config);
        assert_eq!(config.theme, Theme::BuiltIn(BuiltInTheme::Dark));
        assert!(!config.enable_logo_customization);
        assert_eq!(config.logo_customization.text_color, "red");
        assert!(config.enable_background_customization);
        assert_eq!(config.background_customization.background_color, "#101010");
    }

    /// A config with its own colours, logo and background, all switched on.
    fn my_config() -> AppConfig {
        let mut config = AppConfig::default();
        config.theme = Theme::BuiltIn(BuiltInTheme::Synthwave);
        config.enable_logo_customization = true;
        config.logo_customization.text_color = "red".to_string();
        config.enable_background_customization = true;
        config.background_customization.background_color = "#202020".to_string();
        config
    }

    #[test]
    fn all_parts_ticked_matches_the_old_import() {
        // What the import did before parts could be picked: every part of the
        // file on, a part it leaves out switched off with its settings kept.
        let mut config = AppConfig::default();
        config.enable_logo_customization = true;
        config.logo_customization.text_color = "red".to_string();
        let theme = SharedTheme { logo: None, ..look() };
        let mut expected = config.clone();
        apply_to_config(&theme, Some(Theme::BuiltIn(BuiltInTheme::Dark)), ImportParts::ALL, &mut config);

        expected.theme = Theme::BuiltIn(BuiltInTheme::Dark);
        expected.enable_logo_customization = false;
        expected.logo_customization.text_color = "red".to_string();
        expected.enable_background_customization = true;
        expected.background_customization = theme.background.clone().unwrap();
        assert_eq!(config, expected);
    }

    #[test]
    fn unticked_parts_leave_the_config_alone() {
        let theme = look();
        let mine = my_config();

        let mut config = mine.clone();
        let parts = ImportParts { logo: false, ..ImportParts::ALL };
        apply_to_config(&theme, Some(Theme::Custom("night".to_string())), parts, &mut config);
        assert_eq!(config.enable_logo_customization, mine.enable_logo_customization);
        assert_eq!(config.logo_customization, mine.logo_customization);
        assert_eq!(config.theme, Theme::Custom("night".to_string()));
        assert_eq!(config.background_customization, theme.background.clone().unwrap());

        let mut config = mine.clone();
        let parts = ImportParts { colors: false, ..ImportParts::ALL };
        apply_to_config(&theme, None, parts, &mut config);
        assert_eq!(config.theme, mine.theme);
        apply_to_config(&theme, Some(Theme::Custom("night".to_string())), parts, &mut config);
        assert_eq!(config.theme, mine.theme, "unticked colours stay even when a choice is passed");

        let mut config = mine.clone();
        let none = ImportParts { colors: false, logo: false, background: false };
        apply_to_config(&theme, Some(Theme::Custom("night".to_string())), none, &mut config);
        assert_eq!(config, mine);
    }

    #[test]
    fn a_ticked_absent_part_switches_yours_off_and_keeps_its_settings() {
        let theme = SharedTheme { logo: None, background: None, ..look() };
        let mine = my_config();
        let mut config = mine.clone();
        apply_to_config(&theme, None, ImportParts { colors: false, ..ImportParts::ALL }, &mut config);
        assert!(!config.enable_logo_customization);
        assert!(!config.enable_background_customization);
        assert_eq!(config.logo_customization, mine.logo_customization);
        assert_eq!(config.background_customization, mine.background_customization);
        assert_eq!(config.theme, mine.theme);
    }

    #[test]
    fn unticked_parts_bring_no_images() {
        let mut theme = look();
        theme.keep_parts(ImportParts { background: false, ..ImportParts::ALL });
        assert_eq!(theme.background, None);
        assert_eq!(theme.images.keys().collect::<Vec<_>>(), ["logo.png"]);

        let mut theme = look();
        theme.keep_parts(ImportParts { logo: false, ..ImportParts::ALL });
        assert_eq!(theme.logo, None);
        assert_eq!(theme.images.keys().collect::<Vec<_>>(), ["background.jpg"]);

        // An image both parts use stays while one of them is kept.
        let mut theme = look();
        theme.background.as_mut().unwrap().background_image = Some("logo.png".to_string());
        theme.images.remove("background.jpg");
        theme.keep_parts(ImportParts { logo: false, ..ImportParts::ALL });
        assert_eq!(theme.images.keys().collect::<Vec<_>>(), ["logo.png"]);

        let mut theme = look();
        theme.keep_parts(ImportParts::ALL);
        assert_eq!(theme, look());
    }

    /// The import dialog lists what the file holds, so a library theme shows
    /// what will be put on rather than what the library page said.
    #[test]
    fn what_a_theme_holds_is_read_from_the_file() {
        let theme = look();
        assert_eq!(theme.colors_label(), "Custom colors");
        assert_eq!(theme.logo_label(), Some("Logo with images"));
        assert_eq!(theme.background_label(), Some("Background image"));

        let mut theme = SharedTheme { colors: SharedColors::BuiltIn(BuiltInTheme::Synthwave), background: None, ..look() };
        theme.logo.as_mut().unwrap().use_background_image = false;
        assert_eq!(theme.colors_label(), "Synthwave");
        assert_eq!(theme.logo_label(), Some("Logo style"));
        assert_eq!(theme.background_label(), None);

        theme.background = Some(BackgroundCustomization { use_image: false, ..look().background.unwrap() });
        assert_eq!(theme.background_label(), Some("Background color"));
    }

    #[test]
    fn a_part_that_is_on_says_it_will_be_replaced() {
        let theme = look();
        let mut config = AppConfig::default();
        config.enable_logo_customization = true;
        let [colors, logo, background] = checklist(&theme, &config);
        assert_eq!((colors.part, colors.label.as_str(), colors.note), (Part::Colors, "Colors: Custom colors", None));
        assert_eq!((logo.part, logo.label.as_str(), logo.note), (Part::Logo, "Logo with images", Some("Replaces your logo")));
        assert_eq!((background.part, background.label.as_str(), background.note), (Part::Background, "Background image", None));

        config.enable_logo_customization = false;
        config.enable_background_customization = true;
        let [_, logo, background] = checklist(&theme, &config);
        assert_eq!(logo.note, None);
        assert_eq!(background.note, Some("Replaces your background"));
    }

    #[test]
    fn a_library_part_the_api_left_out_is_not_installed() {
        // The library listed a logo (`has_logo: true`), but the file it served
        // has none. The checklist only ever reads the file.
        let dir = temp_dir("library");
        let mut theme = SharedTheme { logo: None, ..look() };
        let [_, logo, _] = checklist(&theme, &my_config());
        assert_eq!(logo.label, "Logo");
        assert_eq!(logo.note, Some("Not in this theme. Will turn off yours."));

        theme.keep_parts(ImportParts::ALL);
        let created = install_images_in(&mut theme, &dir).unwrap();
        let background = theme.background.as_ref().unwrap().background_image.clone().unwrap();
        assert_eq!(created, [dir.join(background.trim_start_matches("/custom-images/"))], "no logo image is saved");
        let mut config = my_config();
        apply_to_config(&theme, None, ImportParts::ALL, &mut config);
        assert!(!config.enable_logo_customization);
        assert_eq!(config.logo_customization, my_config().logo_customization, "nothing of a logo is put on");
        let _ = std::fs::remove_dir_all(&dir);

        // The library said there is no logo (`has_logo: false`), but the file
        // has one: the dialog lists the file's logo, which is what gets put on.
        let [_, logo, _] = checklist(&look(), &AppConfig::default());
        assert_eq!(logo.label, "Logo with images");
        assert_eq!(logo.note, None);
    }

    #[test]
    fn parts_are_ticked_one_at_a_time() {
        let parts = ImportParts::ALL.with(Part::Logo, false);
        assert_eq!(parts, ImportParts { logo: false, ..ImportParts::ALL });
        assert!(!parts.has(Part::Logo) && parts.has(Part::Colors) && parts.has(Part::Background));
        assert!(parts.any());

        let none = parts.with(Part::Colors, false).with(Part::Background, false);
        assert!(!none.any(), "nothing ticked leaves nothing to use");
        assert!(none.with(Part::Background, true).any());
    }

    #[test]
    fn the_look_on_screen_reads_custom_colors_from_the_list() {
        let mut themes = ThemesConfig::default();
        let id = themes.import_theme("Night", "--color-primary: red;").unwrap();
        let mut config = my_config();
        config.theme = Theme::Custom(id);
        config.enable_background_customization = false;

        let look = Look::from_config(&config, &themes);
        assert_eq!(look.colors, LookColors::Known(SharedColors::Custom { css: "--color-primary: red;".to_string() }));
        assert_eq!(look.logo, Some(config.logo_customization.clone()));
        assert_eq!(look.background, None, "a part that is off is not in the look");

        config.theme = Theme::BuiltIn(BuiltInTheme::Dark);
        let look = Look::from_config(&config, &themes);
        assert_eq!(look.colors, LookColors::Known(SharedColors::BuiltIn(BuiltInTheme::Dark)));
    }

    #[test]
    fn missing_custom_colors_read_as_missing_not_system() {
        let mut config = my_config();
        config.theme = Theme::Custom("gone".to_string());
        let look = Look::from_config(&config, &ThemesConfig::default());
        assert_eq!(look.colors, LookColors::Missing("gone".to_string()));
    }

    #[tokio::test]
    async fn sharing_missing_colors_is_refused() {
        let mut config = my_config();
        config.theme = Theme::Custom("gone".to_string());
        let look = Look::from_config(&config, &ThemesConfig::default());
        assert_eq!(
            gather("Mine", &look).await.unwrap_err(),
            "The selected theme is missing. Pick a theme and try again."
        );

        config.theme = Theme::BuiltIn(BuiltInTheme::Dark);
        let shared = gather("Mine", &Look::from_config(&config, &ThemesConfig::default())).await.unwrap();
        assert_eq!(shared.colors, SharedColors::BuiltIn(BuiltInTheme::Dark));
        assert_eq!(shared.logo, Some(config.logo_customization.clone()));
    }

    #[test]
    fn a_shared_name_says_theme_not_look() {
        let mut themes = ThemesConfig::default();
        let id = themes.import_theme("Night", "--color-primary: red;").unwrap();
        let mut config = my_config();

        config.theme = Theme::BuiltIn(BuiltInTheme::Dark);
        assert_eq!(default_name(&config, &themes), "My Dark theme");
        config.theme = Theme::Custom(id);
        assert_eq!(default_name(&config, &themes), "Night");
        config.theme = Theme::Custom("gone".to_string());
        assert_eq!(default_name(&config, &themes), "My theme");
    }

    /// An empty folder of its own under the system temp folder.
    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("mechvibes-theme-{}-{}", name, uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn installing_reports_only_files_it_created() {
        let dir = temp_dir("install");
        let (in_use, _) = crate::utils::custom_images::save_in(&dir, PNG, None).unwrap();

        let mut theme = look();
        let created = install_images_in(&mut theme, &dir).unwrap();
        let logo = theme.logo.as_ref().unwrap().background_image.clone().unwrap();
        let background = theme.background.as_ref().unwrap().background_image.clone().unwrap();
        assert_eq!(logo, format!("/custom-images/{}", in_use), "the same image is not saved again");
        assert!(background.starts_with("/custom-images/img-") && background.ends_with(".jpg"), "{}", background);
        assert_eq!(created, [dir.join(background.trim_start_matches("/custom-images/"))]);
        assert!(theme.images.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn undoing_an_import_keeps_an_image_already_in_use() {
        let dir = temp_dir("undo");
        let (in_use, _) = crate::utils::custom_images::save_in(&dir, PNG, None).unwrap();

        let mut theme = look();
        let created = install_images_in(&mut theme, &dir).unwrap();
        remove_images(&created);
        assert_eq!(std::fs::read(dir.join(&in_use)).unwrap(), PNG);
        let left: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
        assert_eq!(left.len(), 1, "only the image that was there before");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_folder_is_not_opened_as_a_theme() {
        let dir = temp_dir("folder");
        let folder = dir.join("looks-like-a.mvtheme");
        std::fs::create_dir_all(&folder).unwrap();
        assert_eq!(open(&folder).unwrap_err(), "This is not a Mechvibes theme file.");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn a_pipe_is_refused_without_waiting_for_a_writer() {
        let dir = temp_dir("pipe");
        let pipe = dir.join("cool.mvtheme");
        let made = std::process::Command::new("mkfifo").arg(&pipe).status().unwrap();
        assert!(made.success());
        assert_eq!(open(&pipe).unwrap_err(), "This is not a Mechvibes theme file.");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sharing_adds_the_extension_without_cutting_the_name_or_replacing_a_file() {
        let dir = temp_dir("share");
        std::fs::write(dir.join("Neon v1.mvtheme"), b"another theme").unwrap();
        std::fs::write(dir.join("notes.mvtheme"), b"another theme").unwrap();

        assert_eq!(
            share_target(&dir.join("Neon v1.2")).unwrap(),
            (dir.join("Neon v1.2.mvtheme"), "Neon v1.2".to_string())
        );
        assert!(share_target(&dir.join("notes")).is_err(), "the dialog never asked about notes.mvtheme");
        // A name the dialog showed with its extension was already asked about.
        assert_eq!(
            share_target(&dir.join("notes.mvtheme")).unwrap(),
            (dir.join("notes.mvtheme"), "notes".to_string())
        );
        assert_eq!(share_target(&dir.join("Night.MVTHEME")).unwrap().1, "Night");
        assert_eq!(std::fs::read(dir.join("Neon v1.mvtheme")).unwrap(), b"another theme");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_over_the_limit_is_refused_without_reading_it_all() {
        let dir = temp_dir("large");
        let path = dir.join("large.mvtheme");
        std::fs::File::create(&path).unwrap().set_len(MAX_FILE_BYTES * 4).unwrap();
        assert_eq!(open(&path).unwrap_err(), "This theme file is too large.");

        let theme = look();
        let path = dir.join("fine.mvtheme");
        std::fs::write(&path, to_bytes(&theme).unwrap()).unwrap();
        assert_eq!(open(&path).unwrap(), theme);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
