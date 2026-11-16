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
    /// Words shared by several screens.
    pub common: catalog::CommonStrings,
}

const STRINGS_EN: Strings = Strings { common: catalog::common::EN };
const STRINGS_ZH: Strings = Strings { common: catalog::common::ZH };
const STRINGS_ES: Strings = Strings { common: catalog::common::ES };

/// The catalog for `language`.
///
/// The match is exhaustive on purpose: adding a variant to [`Language`] stops
/// compiling here until its catalog is listed.
#[allow(dead_code)] // reachable from `i18n::use_strings` once the config carries the language
pub(super) fn for_language(language: Language) -> &'static Strings {
    match language {
        Language::En => &STRINGS_EN,
        Language::Zh => &STRINGS_ZH,
        Language::Es => &STRINGS_ES,
    }
}
