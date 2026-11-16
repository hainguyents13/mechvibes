//! The Settings page.
//!
//! Only the language section lives here so far, because it is the part this
//! slice renders. The rest of the page (~45 strings) lands with the settings
//! zone; adding keys there is the same edit as adding them here.

zone! {
    /// Settings page copy.
    SettingsStrings {
        language_section_title: "Language", "语言", "Idioma",
        language_system: "System", "跟随系统", "Sistema",
    }
}
