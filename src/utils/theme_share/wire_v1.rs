//! `theme.json` of format 1, exactly as Mechvibes 3.3 reads and writes it.
//!
//! Never edit a field here: files made by 3.3 must keep reading the same, and
//! files made now must keep opening in 3.3. A change of shape is a new format.
//! The app's own types convert to and from these at the edge, so a setting
//! added to the app later does not quietly change the file.

use super::SharedColors;
use crate::libs::theme::BuiltInTheme;
use crate::state::config::{ BackgroundCustomization, LogoCustomization };
use serde::{ Deserialize, Serialize };

/// `theme.json`. The logo and background image fields name files inside the
/// `.mvtheme`; a part is left out when the person sharing has it switched off.
#[derive(Serialize, Deserialize)]
pub(super) struct ManifestV1 {
    pub format: u32,
    pub name: String,
    pub colors: ColorsV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo: Option<LogoV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<BackgroundV1>,
}

/// A built-in theme goes by the name 3.3 gave its variant, such as
/// `"Synthwave"`, so renaming a variant cannot rename it in files.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ColorsV1 {
    BuiltIn(String),
    Custom {
        css: String,
    },
}

/// Same names, types and order as `LogoCustomization` had in 3.3. The image
/// fields are written as `null` when empty, as 3.3 does.
#[derive(Serialize, Deserialize)]
pub(super) struct LogoV1 {
    pub border_color: String,
    pub text_color: String,
    pub shadow_color: String,
    pub background_color: String,
    pub background_image: Option<String>,
    pub use_background_image: bool,
    pub muted_background: String,
    pub muted_background_image: Option<String>,
    pub use_muted_background_image: bool,
    pub dimmed_when_muted: bool,
}

/// Same names, types and order as `BackgroundCustomization` had in 3.3.
#[derive(Serialize, Deserialize)]
pub(super) struct BackgroundV1 {
    pub background_color: String,
    pub background_image: Option<String>,
    pub use_image: bool,
}

/// The built-in theme a file names, or `None` for a name this app has never
/// had.
pub(super) fn built_in_from_wire(name: &str) -> Option<BuiltInTheme> {
    BuiltInTheme::all()
        .into_iter()
        .find(|theme| format!("{:?}", theme) == name)
}

impl ColorsV1 {
    pub fn from_shared(colors: &SharedColors) -> Self {
        match colors {
            SharedColors::BuiltIn(theme) => Self::BuiltIn(format!("{:?}", theme)),
            SharedColors::Custom { css } => Self::Custom { css: css.clone() },
        }
    }

    /// `None` when a built-in name is unknown, which makes the file damaged.
    pub fn into_shared(self) -> Option<SharedColors> {
        match self {
            Self::BuiltIn(name) => built_in_from_wire(&name).map(SharedColors::BuiltIn),
            Self::Custom { css } => Some(SharedColors::Custom { css }),
        }
    }
}

impl LogoV1 {
    /// Written without `..` on purpose: a field added to `LogoCustomization`
    /// stops this compiling, so whoever adds it decides whether it belongs
    /// in the file (which would make a new format) or not.
    pub fn from_config(logo: &LogoCustomization) -> Self {
        let LogoCustomization {
            border_color,
            text_color,
            shadow_color,
            background_color,
            background_image,
            use_background_image,
            muted_background,
            muted_background_image,
            use_muted_background_image,
            dimmed_when_muted,
        } = logo;
        Self {
            border_color: border_color.clone(),
            text_color: text_color.clone(),
            shadow_color: shadow_color.clone(),
            background_color: background_color.clone(),
            background_image: background_image.clone(),
            use_background_image: *use_background_image,
            muted_background: muted_background.clone(),
            muted_background_image: muted_background_image.clone(),
            use_muted_background_image: *use_muted_background_image,
            dimmed_when_muted: *dimmed_when_muted,
        }
    }

    /// A setting the app gains later, which no format 1 file has, takes its
    /// default.
    #[allow(clippy::needless_update)]
    pub fn into_config(self) -> LogoCustomization {
        LogoCustomization {
            border_color: self.border_color,
            text_color: self.text_color,
            shadow_color: self.shadow_color,
            background_color: self.background_color,
            background_image: self.background_image,
            use_background_image: self.use_background_image,
            muted_background: self.muted_background,
            muted_background_image: self.muted_background_image,
            use_muted_background_image: self.use_muted_background_image,
            dimmed_when_muted: self.dimmed_when_muted,
            ..LogoCustomization::default()
        }
    }
}

impl BackgroundV1 {
    /// Written without `..` on purpose, as `LogoV1::from_config` is.
    pub fn from_config(background: &BackgroundCustomization) -> Self {
        let BackgroundCustomization { background_color, background_image, use_image } = background;
        Self {
            background_color: background_color.clone(),
            background_image: background_image.clone(),
            use_image: *use_image,
        }
    }

    /// A setting the app gains later takes its default.
    #[allow(clippy::needless_update)]
    pub fn into_config(self) -> BackgroundCustomization {
        BackgroundCustomization {
            background_color: self.background_color,
            background_image: self.background_image,
            use_image: self.use_image,
            ..BackgroundCustomization::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every built-in theme name 3.3 wrote, copied by hand from 3.3 rather
    /// than read from the enum, so renaming or dropping a variant shows up
    /// here instead of in people's files.
    const V3_3_BUILT_IN_NAMES: [&str; 36] = [
        "Light", "Dark", "System", "Cupcake", "Bumblebee", "Emerald", "Corporate", "Synthwave",
        "Retro", "Cyberpunk", "Valentine", "Halloween", "Garden", "Forest", "Aqua", "Lofi",
        "Pastel", "Fantasy", "Wireframe", "Black", "Luxury", "Dracula", "Cmyk", "Autumn",
        "Business", "Acid", "Lemonade", "Night", "Coffee", "Winter", "Dim", "Nord", "Sunset",
        "Abyss", "Silk", "Caramellatte",
    ];

    #[test]
    fn every_v3_3_built_in_name_still_reads() {
        for name in V3_3_BUILT_IN_NAMES {
            let colors = ColorsV1::BuiltIn(name.to_string())
                .into_shared()
                .unwrap_or_else(|| panic!("'{}' no longer reads", name));
            match ColorsV1::from_shared(&colors) {
                ColorsV1::BuiltIn(written) => assert_eq!(written, name, "'{}' is written back under another name", name),
                ColorsV1::Custom { .. } => panic!("'{}' is written back as custom colors", name),
            }
        }
    }
}
