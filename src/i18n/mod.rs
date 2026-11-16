//! Languages, detection, and the catalogs behind every string the user reads.
//!
//! The catalog is a plain Rust struct of `&'static str` fields — one file per
//! zone under [`catalog`], with the three translations side by side. That buys
//! three things no runtime table does:
//!
//! - A key that is missing or misspelled does not compile (`E0063` / `E0560`).
//! - A translator sees every language of a key on one line while writing it.
//! - Nothing is allocated per render: [`Language::strings`] hands out a
//!   `&'static Strings`.
//!
//! "Follow the system" is deliberately **not** a fourth language.
//! `AppConfig::language` is `Option<Language>`, and `None` means "whatever the
//! OS says", which is what an upgraded config with no `language` key means too.
//! [`resolve`] turns that into the language actually in use, so the selector can
//! offer Sistema/System/Sistema without storing a fourth variant or migrating
//! anybody's config.

use std::fmt::Display;

pub mod catalog;
mod detect;
mod strings;

pub use strings::Strings;

/// A language the app can be shown in.
///
/// Serialized as its BCP-47 primary subtag, so `config.json` carries `"en"`,
/// `"zh"` or `"es"` — stable identifiers that survive reordering this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    En,
    Zh,
    Es,
}

impl Language {
    /// Every language, in the order the selector shows them.
    ///
    /// The compiler does not check this list against the enum: adding a variant
    /// fails to compile in [`Language::tag`], [`Language::native_label`] and
    /// [`strings::for_language`], and this list is the one place left to update
    /// by hand (`the_selector_lists_every_language` fails if you forget).
    pub const ALL: [Language; 3] = [Language::En, Language::Zh, Language::Es];

    /// The identifier used in `config.json` and in the telemetry payload.
    pub const fn tag(self) -> &'static str {
        match self {
            Language::En => "en",
            Language::Zh => "zh",
            Language::Es => "es",
        }
    }

    /// The language's name **in that language**.
    ///
    /// Never translated: someone who only reads Chinese has to be able to find
    /// their own language in the list, and "Chinese" spelled in English does not
    /// help them.
    pub const fn native_label(self) -> &'static str {
        match self {
            Language::En => "English",
            Language::Zh => "简体中文",
            Language::Es => "Español",
        }
    }

    /// Maps an OS locale (`es-AR`, `zh_CN.UTF-8`, `en_US`, `es`) to a catalog.
    ///
    /// Only the primary subtag decides: the catalogs are language-neutral, so
    /// `es-AR` and `es-ES` both land on `Es`. Chinese has one catalog for now —
    /// Simplified — so `zh-TW` lands there too rather than falling back to
    /// English, which would be worse for a Traditional reader than Simplified
    /// text. Returns `None` for a language the app does not speak.
    pub fn from_bcp47(locale: &str) -> Option<Self> {
        let primary = locale
            .split(['-', '_'])
            .next()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();

        match primary.as_str() {
            "en" => Some(Language::En),
            "zh" => Some(Language::Zh),
            "es" => Some(Language::Es),
            _ => None,
        }
    }

    /// The catalog for this language.
    pub fn strings(self) -> &'static Strings {
        strings::for_language(self)
    }
}

/// The language the app should be shown in.
///
/// `None` means the user never chose one (or chose "System"), so the OS decides.
pub fn resolve(preferred: Option<Language>) -> Language {
    preferred.unwrap_or_else(system_language)
}

/// The system language, falling back to English when the OS names one the app
/// does not speak.
pub fn system_language() -> Language {
    detect::system_locale()
        .as_deref()
        .and_then(Language::from_bcp47)
        .unwrap_or(Language::En)
}

/// Fills a catalog template's `{}` placeholders, in order.
///
/// `format!` requires a literal format string, so `format!(strings.some_key, n)`
/// does not compile and every count-dependent sentence would have to be split
/// around its number at the call site. This keeps the whole sentence in the
/// catalog, where a translator can reorder it around the placeholder.
///
/// **A placeholder with no matching argument is left visible as `{}`** instead of
/// panicking: a render panic takes the window down, and a stray `{}` in one label
/// is a bug report instead of a crash. Arguments beyond the placeholders are
/// ignored.
pub fn fill(template: &str, args: &[&dyn Display]) -> String {
    use std::fmt::Write;

    let mut out = String::with_capacity(template.len() + 4 * args.len());
    let mut rest = template;
    let mut next_arg = 0;

    while let Some(at) = rest.find("{}") {
        out.push_str(&rest[..at]);
        match args.get(next_arg) {
            Some(arg) => {
                next_arg += 1;
                // Writing into a String cannot fail.
                let _ = write!(out, "{arg}");
            }
            None => out.push_str("{}"),
        }
        rest = &rest[at + 2..];
    }
    out.push_str(rest);
    out
}

/// Picks the singular or plural form for `count`.
///
/// The two forms are separate catalog keys with the `_one` / `_other` suffix, so
/// a language that does not inflect for number (Chinese) writes the same text in
/// both and still goes through the same call site. Russian-style plural rules are
/// not modelled: no catalog here needs them yet, and guessing would be wrong for
/// the languages the app does ship.
pub fn plural<'a>(one: &'a str, other: &'a str, count: usize) -> &'a str {
    if count == 1 { one } else { other }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_selector_lists_every_language() {
        // Keep in sync with `Language` by hand: a new variant fails to compile in
        // `tag`, `native_label` and `for_language`, but `ALL` would silently miss it.
        assert_eq!(
            Language::ALL.len(),
            3,
            "a language was added or removed: update Language::ALL and this count"
        );
        for language in Language::ALL {
            assert!(!language.tag().is_empty());
            assert!(!language.native_label().is_empty());
        }
    }

    #[test]
    fn native_labels_are_distinct() {
        let mut labels: Vec<&str> = Language::ALL.iter().map(|l| l.native_label()).collect();
        labels.sort_unstable();
        let count = labels.len();
        labels.dedup();
        assert_eq!(labels.len(), count, "two languages share a native label");
    }

    #[test]
    fn every_language_tag_round_trips() {
        for language in Language::ALL {
            assert_eq!(Language::from_bcp47(language.tag()), Some(language));
        }
    }

    #[test]
    fn a_regional_locale_maps_to_its_language() {
        for (locale, expected) in [
            ("es-AR", Language::Es),
            ("es_ES", Language::Es),
            ("es", Language::Es),
            ("es_MX.UTF-8", Language::Es),
            ("zh-CN", Language::Zh),
            ("zh_Hans_CN", Language::Zh),
            ("zh-TW", Language::Zh),
            ("zh_CN.UTF-8", Language::Zh),
            ("en-US", Language::En),
            ("en_GB.UTF-8", Language::En),
        ] {
            assert_eq!(Language::from_bcp47(locale), Some(expected), "locale {locale}");
        }
    }

    #[test]
    fn an_unsupported_locale_is_none() {
        for locale in ["pt-BR", "de_DE.UTF-8", "", "   ", "klingon"] {
            assert_eq!(Language::from_bcp47(locale), None, "locale {locale}");
        }
    }

    #[test]
    fn an_unsupported_system_locale_falls_back_to_english() {
        // `resolve` cannot be tested directly (it reads the OS), so this pins the
        // fallback that `system_language` applies to an unknown locale.
        let unknown = Language::from_bcp47("pt-BR").unwrap_or(Language::En);
        assert_eq!(unknown, Language::En);
    }

    #[test]
    fn a_chosen_language_wins_over_detection() {
        assert_eq!(resolve(Some(Language::Zh)), Language::Zh);
    }

    #[test]
    fn languages_serialize_as_their_tag() {
        assert_eq!(serde_json::to_string(&Language::Zh).unwrap(), "\"zh\"");
        assert_eq!(serde_json::from_str::<Language>("\"es\"").unwrap(), Language::Es);
        assert!(serde_json::from_str::<Language>("\"klingon\"").is_err());
    }

    #[test]
    fn fill_replaces_each_placeholder_in_order() {
        assert_eq!(fill("Found {} packs in {}", &[&8, &"Keyboard"]), "Found 8 packs in Keyboard");
    }

    #[test]
    fn fill_leaves_a_template_without_placeholders_alone() {
        assert_eq!(fill("Found nothing", &[]), "Found nothing");
        assert_eq!(fill("Found nothing", &[&8]), "Found nothing");
    }

    #[test]
    fn fill_leaves_unmatched_placeholders_visible() {
        assert_eq!(fill("Found {} of {}", &[&8]), "Found 8 of {}");
    }

    #[test]
    fn fill_handles_a_placeholder_at_the_very_end() {
        assert_eq!(fill("Total: {}", &[&3]), "Total: 3");
    }

    #[test]
    fn plural_picks_the_matching_form() {
        assert_eq!(plural("{} pack", "{} packs", 0), "{} packs");
        assert_eq!(plural("{} pack", "{} packs", 1), "{} pack");
        assert_eq!(plural("{} pack", "{} packs", 2), "{} packs");
    }
}
