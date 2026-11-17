//! The system tray menu and the macOS permission rows.
//!
//! The tray is not a Dioxus component, so it resolves this catalog from the
//! config writer rather than through `use_strings()`.
//!
//! Reserved by the feature plan so one slice owns this file: the keys arrive with
//! the migration of the components listed in `odd/tasks/i18n-multi-language.md`.

zone! {
    /// See the module comment.
    TrayStrings {}
}
