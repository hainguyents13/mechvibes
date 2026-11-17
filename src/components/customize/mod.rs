//! The Customize page's parts, and the two lookups that are not per component.

use crate::i18n::catalog::CustomizeStrings;
use crate::libs::theme::BuiltInTheme;

pub mod background_tab;
pub mod colors_editor_modal;
pub mod colors_tab;
pub mod current_theme_row;
pub mod customize_tabs;
pub mod logo_tab;
pub mod look_editor;
pub mod part_panel;
pub mod rename_modal;
pub mod theme_card;
pub mod themes_tab;

pub use background_tab::BackgroundTab;
pub use colors_tab::ColorsTab;
pub use current_theme_row::CurrentThemeRow;
pub use customize_tabs::{ CustomizeTab, CustomizeTabs, CURRENT_TAB };
pub use logo_tab::LogoTab;
pub use themes_tab::ThemesTab;

/// The Customize catalog for the language in effect **right now**.
///
/// For code that runs outside a component body — a toast reported from a
/// model function, a message built inside a spawned task — where
/// `use_strings()` cannot be called. It reads the config every time instead
/// of capturing a value, so a message shown after the user changes the
/// language is in the new one.
///
/// Inside a component, read `use_strings()` instead: it also subscribes the
/// component to the language, which this does not.
pub(crate) fn strings_now() -> &'static CustomizeStrings {
    let strings: &'static crate::i18n::Strings = crate::i18n
        ::resolve(crate::state::config_writer::current().language).strings();
    &strings.customize
}

/// The name of a built-in theme as the screen shows it.
///
/// The three modes — `Light`, `Dark`, `System` — are the app's own words, so
/// they come from the catalog. Every other built-in name is a DaisyUI theme
/// name: it identifies a theme in the files people share and in the CSS the
/// app ships, so it is written the same in every language and stays a
/// `format!("{:?}", …)`. Translating one would stop it matching.
pub(crate) fn theme_name(theme: &BuiltInTheme, strings: &CustomizeStrings) -> String {
    match theme {
        BuiltInTheme::Light => strings.theme_mode_light.to_string(),
        BuiltInTheme::Dark => strings.theme_mode_dark.to_string(),
        BuiltInTheme::System => strings.theme_mode_system.to_string(),
        daisy => format!("{:?}", daisy),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Language;

    /// The split the whole zone depends on: the three modes are the app's
    /// words and follow the language; a DaisyUI name is an identifier and must
    /// come out exactly as the CSS and the theme files spell it, in every
    /// language.
    #[test]
    fn only_the_theme_mode_is_translated() {
        let es = &Language::Es.strings().customize;
        assert_eq!(theme_name(&BuiltInTheme::Light, es), "Claro");
        assert_eq!(theme_name(&BuiltInTheme::Dark, es), "Oscuro");
        assert_eq!(theme_name(&BuiltInTheme::System, es), "Sistema");

        for daisy in [BuiltInTheme::Cupcake, BuiltInTheme::Dracula, BuiltInTheme::Caramellatte] {
            let name = theme_name(&daisy, es);
            assert_eq!(name, format!("{:?}", daisy), "a DaisyUI name is not translated");
            assert_eq!(daisy.to_daisy_theme(), name.to_lowercase(), "and is what the CSS carries");
        }
    }
}
