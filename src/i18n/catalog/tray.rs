//! The system tray menu, and the theme file dialogs opened from outside a
//! component.
//!
//! The tray is not a Dioxus component, so it cannot call `use_strings()`: it
//! resolves this catalog from `state::config_writer::current`, the same
//! authority the Settings screen writes the choice through. The theme dialogs in
//! `components/theme_share.rs` are read the same way — they open from a click,
//! not from a render — and share this zone with the menu because both are read
//! outside the component tree.
//!
//! The macOS permission rows live here too (`accessibility`, `input_monitoring`,
//! `permission_*`). They are the one part of this zone read by two files: the
//! tray decides which words, `libs/permissions.rs` lays them out and colours
//! them, because the string it colours has to be the exact one the item shows.
//!
//! Two decisions the keys encode:
//!
//! - **The brand is not a key.** `show` holds the phrase around the app name and
//!   the call site fills `APP_NAME` in, so no language has to spell "Mechvibes"
//!   and a translator cannot change the brand by accident.
//! - **A menu entry names an action, not a state.** The two mute entries are
//!   checkable: the checkmark says on or off, and their wording stays the same
//!   either way. A label that followed the state flipped between
//!   "Mute"/"Unmute" every time it was refreshed.

zone! {
    /// See the module comment.
    ///
    /// `allow(dead_code)` **only off macOS**: the five permission keys are read
    /// by `libs/permissions.rs`, which is `#[cfg(target_os = "macos")]`, so
    /// nothing reads them on Linux or Windows. An unconditional allow would also
    /// hide a genuinely unused key added to this zone later, and gating the whole
    /// zone would skip the `ENTRIES` invariants on those platforms — this is the
    /// narrow version of both.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    TrayStrings {
        // ---- The menu ----
        show: "Show {}", "显示 {}", "Mostrar {}",
        mute_sounds: "Mute sounds", "静音音效", "Silenciar sonidos",
        // "Mood" is the name of the page this mutes, so it follows that page's
        // translation (`nav.mood`) rather than inventing a second one.
        mute_mood: "Mute Mood", "静音氛围", "Silenciar ambiente",
        // These three name other people's products: the proper nouns stay as
        // they are in every language and only the noun around them is
        // translated.
        github: "GitHub Repository", "GitHub 仓库", "Repositorio de GitHub",
        discord: "Discord Community", "Discord 社区", "Comunidad de Discord",
        website: "Official Website", "官方网站", "Sitio web oficial",
        exit: "Exit", "退出", "Salir",

        // ---- The theme file dialogs ----
        // A dialog title is a sentence in its own right, not a word borrowed
        // from the page that opened it: "Share" and "Import" are buttons, these
        // say what the OS window is for.
        dialog_save_theme: "Save theme", "保存主题", "Guardar tema",
        dialog_import_theme: "Import theme", "导入主题", "Importar tema",
        // The file-type filter the dialog lists. It names the app's own theme
        // file, so the brand stays and the noun is translated.
        dialog_theme_files: "Mechvibes theme", "Mechvibes 主题", "Tema de Mechvibes",

        // ---- The macOS permission rows ----
        // Two lines in the tray menu that say whether Accessibility and Input
        // Monitoring are granted. Each row is one of two whole sentences, with
        // `{}` where the permission's name goes, so a language can put the name
        // where its own grammar wants it. `permission_granted_word` is the word
        // inside the granted sentence that is drawn green: it is its own key so
        // the colour never has to guess what a translation put where, and it has
        // to end that sentence (`every_language_ends_...` below).
        accessibility: "Accessibility", "辅助功能", "Accesibilidad",
        input_monitoring: "Input Monitoring", "输入监控", "Monitoreo de entrada",
        permission_granted: "{}: Granted", "{}：已授权", "{}: Concedido",
        permission_grant: "Grant {}…", "授予{}权限…", "Conceder {}…",
        permission_granted_word: "Granted", "已授权", "Concedido",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Language;

    /// The text the tray showed before this migration, word for word. English is
    /// the language the app already ships in, so this is a move into the catalog
    /// and not a copy edit.
    #[test]
    fn english_keeps_the_permission_rows_users_already_had() {
        assert_eq!(EN.accessibility, "Accessibility");
        assert_eq!(EN.input_monitoring, "Input Monitoring");
        assert_eq!(EN.permission_granted_word, "Granted");
        assert_eq!(crate::i18n::fill(EN.permission_granted, &[&EN.accessibility]), "Accessibility: Granted");
        assert_eq!(crate::i18n::fill(EN.permission_grant, &[&EN.accessibility]), "Grant Accessibility…");
        assert_eq!(
            crate::i18n::fill(EN.permission_granted, &[&EN.input_monitoring]),
            "Input Monitoring: Granted"
        );
        assert_eq!(
            crate::i18n::fill(EN.permission_grant, &[&EN.input_monitoring]),
            "Grant Input Monitoring…"
        );
    }

    /// `permissions::granted_word_range` finds the green word by its suffix,
    /// because the row is one line of text and AppKit addresses a range inside
    /// it. A translation that put the word anywhere else would colour the wrong
    /// characters, and only on macOS — where no test on Linux can see it.
    #[test]
    fn every_language_ends_its_granted_line_with_the_granted_word() {
        for language in Language::ALL {
            let strings = &language.strings().tray;
            let line = crate::i18n::fill(strings.permission_granted, &[&strings.accessibility]);
            assert!(
                line.ends_with(strings.permission_granted_word),
                "{}: {line:?} has to end with {:?}, which is the part drawn green",
                language.tag(),
                strings.permission_granted_word
            );
        }
    }
}
