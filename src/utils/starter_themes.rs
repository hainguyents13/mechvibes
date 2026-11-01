//! The themes that come with the app, shown as cards next to the user's own.
//!
//! Each one is a `.mvtheme` file in `assets/starter-themes/`, built into the
//! app and read with the same checks as any theme file. A starter is never in
//! the saved themes list: the config points at one as "starter:<slug>", and
//! its images are saved to the images folder only when it is put on. A
//! pointer to a starter a later version no longer has counts as none.

use crate::state::paths;
use crate::utils::custom_images;
use crate::utils::theme_share::{ self, Look, SharedTheme };
use std::path::Path;
use std::sync::OnceLock;

/// How the config's `active_theme_id` names a starter.
pub const POINTER_PREFIX: &str = "starter:";

/// The starters, in the order the gallery shows them.
const FILES: [(&str, &[u8]); 4] = [
    ("retro-beige", include_bytes!("../../assets/starter-themes/retro-beige.mvtheme")),
    ("midnight-blue", include_bytes!("../../assets/starter-themes/midnight-blue.mvtheme")),
    ("rgb-night", include_bytes!("../../assets/starter-themes/rgb-night.mvtheme")),
    ("rainy-cafe", include_bytes!("../../assets/starter-themes/rainy-cafe.mvtheme")),
];

/// The address the window loads a starter's images from, for its card.
pub const IMAGE_ROUTE: &str = "starter-themes";

#[derive(Debug, PartialEq)]
pub struct StarterTheme {
    pub slug: &'static str,
    /// The file as it comes with the app, which Share writes out unchanged.
    pub bytes: &'static [u8],
    pub theme: SharedTheme,
}

static STARTERS: OnceLock<Vec<StarterTheme>> = OnceLock::new();

/// Every starter, in catalog order. A file that fails the checks is left out
/// and logged, so one bad file never takes the others with it.
pub fn all() -> &'static [StarterTheme] {
    STARTERS.get_or_init(|| read(&FILES))
}

fn read(files: &[(&'static str, &'static [u8])]) -> Vec<StarterTheme> {
    files
        .iter()
        .filter_map(|&(slug, bytes)| {
            match theme_share::from_bytes(bytes) {
                Ok(theme) => Some(StarterTheme { slug, bytes, theme }),
                Err(e) => {
                    crate::always_eprint!("❌ The starter theme '{}' could not be read: {}", slug, e);
                    None
                }
            }
        })
        .collect()
}

/// An image a starter's card shows, with its media type. Only a name the
/// starter's own file holds is found.
pub fn image(slug: &str, name: &str) -> Option<(&'static [u8], &'static str)> {
    let bytes = all()
        .iter()
        .find(|starter| starter.slug == slug)?
        .theme.image(name)?;
    Some((bytes, media_type(bytes)?))
}

/// The media type of an image, read from its first bytes.
fn media_type(bytes: &[u8]) -> Option<&'static str> {
    Some(match theme_share::image_extension(bytes)? {
        "png" => "image/png",
        "jpg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        _ => {
            return None;
        }
    })
}

impl StarterTheme {
    /// What `active_theme_id` holds while this starter is on.
    pub fn pointer(&self) -> String {
        format!("{}{}", POINTER_PREFIX, self.slug)
    }

    pub fn name(&self) -> &str {
        &self.theme.name
    }

    /// The look putting this starter on shows, with its images where saving
    /// them puts them. Nothing is written, so the look on screen can be
    /// compared with it at any time.
    pub fn look(&self) -> Look {
        self.theme.look_at(|_, bytes| custom_images::url_for(bytes))
    }

    /// The look its card shows, with its images loaded straight from the app.
    pub fn preview_look(&self) -> Look {
        self.theme.look_at(|name, _| Some(format!("/{}/{}/{}", IMAGE_ROUTE, self.slug, name)))
    }

    /// Save its images to the images folder, where `look` says they are.
    pub fn save_images(&self) -> Result<(), String> {
        self.save_images_in(&paths::data::custom_images_dir(), &paths::data::owned_images_json()).map(|_| ())
    }

    /// `save_images` into `dir`, noting the images it made in the list of
    /// owned images at `owned`. Returns the names of the images this call
    /// made.
    pub(crate) fn save_images_in(&self, dir: &Path, owned: &Path) -> Result<Vec<String>, String> {
        let mut made = Vec::new();
        for (_, bytes) in self.theme.images() {
            let (name, created) = custom_images
                ::save_in(dir, owned, bytes, None)
                .map_err(|e| format!("Could not save the theme's images. {}", e))?;
            if created {
                made.push(name);
            }
        }
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::customize::colors_editor_modal::THEME_DEFAULT_CSS;
    use crate::utils::theme_share::{ safe_theme_css, SharedColors };
    use std::io::Read;

    const MAX_FILE_BYTES: usize = 150 * 1024;
    const MAX_TOTAL_BYTES: usize = 600 * 1024;

    #[test]
    fn every_starter_reads() {
        for (slug, bytes) in FILES {
            theme_share::from_bytes(bytes).unwrap_or_else(|e| panic!("'{}' does not read: {}", slug, e));
        }
        assert_eq!(all().len(), FILES.len());
        let names: Vec<&str> = all().iter().map(StarterTheme::name).collect();
        assert_eq!(names, ["Retro Beige", "Midnight Blue", "RGB Night", "Rainy Cafe"]);
    }

    #[test]
    fn a_damaged_starter_is_left_out() {
        let read = read(&[("broken", b"not a theme"), FILES[0]]);
        assert_eq!(read.len(), 1);
        assert_eq!(read[0].slug, FILES[0].0);
    }

    #[test]
    fn starter_slugs_are_unique() {
        let mut slugs: Vec<&str> = FILES.iter().map(|(slug, _)| *slug).collect();
        slugs.sort();
        slugs.dedup();
        assert_eq!(slugs.len(), FILES.len());
        for slug in slugs {
            assert!(slug.chars().all(|c| c.is_ascii_lowercase() || c == '-'), "'{}' is a plain slug", slug);
        }
    }

    #[test]
    fn a_starter_is_pointed_at_by_its_slug() {
        assert_eq!(all()[1].pointer(), "starter:midnight-blue");
    }

    /// The images of `starter` that are neither WebP nor JPEG.
    fn heavy_images(starter: &StarterTheme) -> Vec<&str> {
        starter.theme
            .images()
            .filter(|(_, bytes)| !matches!(theme_share::image_extension(bytes), Some("webp" | "jpg")))
            .map(|(name, _)| name)
            .collect()
    }

    #[test]
    fn starter_images_are_webp_or_jpeg() {
        for starter in all() {
            assert_eq!(heavy_images(starter), Vec::<&str>::new(), "{}", starter.slug);
            for (name, bytes) in starter.theme.images() {
                assert_eq!(image(starter.slug, name).map(|(served, _)| served), Some(bytes));
            }
        }
        // Today no starter has an image, so this shows the check would catch one.
        let theme = theme_share::from_bytes(include_bytes!("theme_share/fixtures/v1/custom-full.mvtheme")).unwrap();
        assert_eq!(heavy_images(&StarterTheme { slug: "night", bytes: &[], theme }), ["logo.png"]);
    }

    #[test]
    fn only_a_starters_own_images_are_served() {
        assert_eq!(image("retro-beige", "theme.json"), None);
        assert_eq!(image("retro-beige", "../config.json"), None);
        assert_eq!(image("nobody", "background.webp"), None);
    }

    #[test]
    fn starters_stay_within_the_size_budget() {
        for (slug, bytes) in FILES {
            assert!(bytes.len() <= MAX_FILE_BYTES, "'{}' is {} bytes", slug, bytes.len());
        }
        let total: usize = FILES.iter().map(|(_, bytes)| bytes.len()).sum();
        assert!(total <= MAX_TOTAL_BYTES, "the starters take {} bytes", total);
    }

    /// Variables the new-colors template sets, so a starter never leaves one
    /// to whatever theme was on before.
    #[test]
    fn starter_colors_set_every_default_variable() {
        let property = |line: &str| line.split_once(':').map(|(property, _)| property.trim().to_string());
        let defaults: Vec<String> = safe_theme_css(THEME_DEFAULT_CSS).lines().filter_map(property).collect();
        assert!(defaults.len() > 20, "the template has its variables");
        for starter in all() {
            let SharedColors::Custom { css } = &starter.theme.colors else {
                panic!("'{}' has colors of its own", starter.slug);
            };
            let set: Vec<String> = css.lines().filter_map(property).collect();
            for variable in &defaults {
                assert!(set.contains(variable), "'{}' leaves out {}", starter.slug, variable);
            }
        }
    }

    #[test]
    fn starter_names_have_no_dash_punctuation() {
        for starter in all() {
            assert!(!starter.name().contains(['\u{2013}', '\u{2014}']), "{}", starter.name());
        }
    }

    /// The files are what Share writes for the same theme, so a starter
    /// shared from its card opens in every Mechvibes that reads format 1.
    #[test]
    fn starter_files_are_written_as_share_writes_them() {
        for starter in all() {
            let mut archive = zip::ZipArchive::new(std::io::Cursor::new(starter.bytes)).unwrap();
            let mut manifest = Vec::new();
            archive.by_name("theme.json").unwrap().read_to_end(&mut manifest).unwrap();
            assert_eq!(
                String::from_utf8(manifest).unwrap(),
                String::from_utf8(theme_share::manifest_json(&starter.theme).unwrap()).unwrap(),
                "{}",
                starter.slug
            );
            let names: Vec<&str> = archive.file_names().collect();
            assert_eq!(names.len(), 1 + starter.theme.images().count(), "{} holds only what it names", starter.slug);
        }
    }

    #[test]
    fn every_starter_has_a_logo_style() {
        for starter in all() {
            assert!(starter.look().logo.is_some(), "{}", starter.slug);
        }
    }

    #[test]
    fn a_starters_card_loads_its_images_from_the_app() {
        for starter in all() {
            if starter.theme.images().next().is_none() {
                assert_eq!(starter.look(), starter.preview_look(), "{}", starter.slug);
            }
        }
        let theme = theme_share::from_bytes(include_bytes!("theme_share/fixtures/v1/custom-full.mvtheme")).unwrap();
        let starter = StarterTheme { slug: "night", bytes: &[], theme };
        let shown = starter.preview_look().background.unwrap().background_image.unwrap();
        assert!(shown.starts_with("/starter-themes/night/"), "{}", shown);
        let worn = starter.look().background.unwrap().background_image.unwrap();
        assert!(worn.starts_with("/custom-images/img-"), "{}", worn);
    }
}
