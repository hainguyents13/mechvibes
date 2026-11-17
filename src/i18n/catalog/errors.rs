//! Text that reaches the screen from outside the component tree: the
//! notice templates and the messages the audio and device layers surface.
//!
//! `Display` stays a stable English description for logs; only what the user
//! reads is translated, at the site that has the catalog in hand.
//!
//! Reserved by the feature plan so one slice owns this file: the keys arrive with
//! the migration of the components listed in `odd/tasks/i18n-multi-language.md`.

zone! {
    /// See the module comment.
    ErrorStrings {}
}
