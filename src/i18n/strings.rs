//! The composed catalog: one field per zone, one `const` per language.
//!
//! [`Strings`] is what components read. It is assembled from the zone `const`s
//! in [`catalog`] so that `strings()` can hand out a `&'static Strings` without
//! allocating anything per render.

use super::catalog;
use super::Language;

/// Every user-visible string in the app, grouped by zone.
///
/// Components reach this through `use_strings()` once the config carries the
/// language (`i18n::use_strings`), and non-component code such as the tray
/// through [`crate::i18n::Language::strings`].
pub struct Strings {
    /// The application shell: title bar, header, volume slider, app info.
    pub chrome: catalog::ChromeStrings,
    /// Words shared by several screens.
    pub common: catalog::CommonStrings,
    /// The dock's labels.
    pub nav: catalog::NavStrings,
    /// The Settings page.
    pub settings: catalog::SettingsStrings,
    /// The Home page.
    pub home: catalog::HomeStrings,
    /// The Sound packs page and its modals.
    pub soundpacks: catalog::SoundpacksStrings,
    /// Customize: themes, colors, background and logo.
    pub customize: catalog::CustomizeStrings,
    /// Mood and the built-in ambience texts.
    pub mood: catalog::MoodStrings,
    /// The device selectors.
    pub devices: catalog::DevicesStrings,
    /// The tray menu.
    pub tray: catalog::TrayStrings,
    /// Text that reaches the screen from outside the component tree.
    pub errors: catalog::ErrorStrings,
}

const STRINGS_EN: Strings = Strings {
    chrome: catalog::chrome::EN,
    common: catalog::common::EN,
    nav: catalog::nav::EN,
    settings: catalog::settings::EN,
    home: catalog::home::EN,
    soundpacks: catalog::soundpacks::EN,
    customize: catalog::customize::EN,
    mood: catalog::mood::EN,
    devices: catalog::devices::EN,
    tray: catalog::tray::EN,
    errors: catalog::errors::EN,
};
const STRINGS_ZH: Strings = Strings {
    chrome: catalog::chrome::ZH,
    common: catalog::common::ZH,
    nav: catalog::nav::ZH,
    settings: catalog::settings::ZH,
    home: catalog::home::ZH,
    soundpacks: catalog::soundpacks::ZH,
    customize: catalog::customize::ZH,
    mood: catalog::mood::ZH,
    devices: catalog::devices::ZH,
    tray: catalog::tray::ZH,
    errors: catalog::errors::ZH,
};
const STRINGS_ES: Strings = Strings {
    chrome: catalog::chrome::ES,
    common: catalog::common::ES,
    nav: catalog::nav::ES,
    settings: catalog::settings::ES,
    home: catalog::home::ES,
    soundpacks: catalog::soundpacks::ES,
    customize: catalog::customize::ES,
    mood: catalog::mood::ES,
    devices: catalog::devices::ES,
    tray: catalog::tray::ES,
    errors: catalog::errors::ES,
};

/// The catalog for `language`.
///
/// The match is exhaustive on purpose: adding a variant to [`Language`] stops
/// compiling here until its catalog is listed.
pub(super) fn for_language(language: Language) -> &'static Strings {
    match language {
        Language::En => &STRINGS_EN,
        Language::Zh => &STRINGS_ZH,
        Language::Es => &STRINGS_ES,
    }
}
