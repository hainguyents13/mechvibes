//! The catalogs, one module per zone of the UI.
//!
//! A zone is a group of screens that share vocabulary and are edited together:
//! `settings`, `soundpacks`, `tray` and so on. Each zone file holds the struct
//! every language fills, the three `const`s that fill it, and `ENTRIES`, which
//! lists every key with its three translations so the invariants below can check
//! them without reflection.
//!
//! Zones exist so that two people (or two agents) can translate different parts
//! of the app at the same time without touching the same file.

/// Declares one catalog zone.
///
/// ```ignore
/// zone! {
///     /// Words shared by several screens.
///     CommonStrings {
///         cancel: "Cancel", "取消", "Cancelar",
///         close: "Close", "关闭", "Cerrar",
///     }
/// }
/// ```
///
/// One line per key, one column per language, always in the same order:
/// English, Chinese, Spanish. The struct, the three `const`s and `ENTRIES` all
/// come from that single list, so a language cannot be missing a key: a key
/// added to the list without a column is a macro arity error, and a key present
/// in the struct but absent from a `const` cannot happen by construction.
///
/// **Count-dependent text uses two keys with the `_one` / `_other` suffix**, and
/// call sites go through [`crate::i18n::plural`]. Two keys rather than a tuple
/// keeps this list flat, and the call site names both, so neither can be
/// forgotten.
///
/// **Adding a language** means one more column per zone file (eleven today) plus
/// the matcher and `ENTRIES` shape here, the registry in `strings.rs` and
/// `Language` itself. That is mechanical and the compiler stops at every line
/// that is missing a column; it is the price of seeing all translations of a key
/// on one line while writing them.
macro_rules! zone {
    (
        $( #[$meta:meta] )*
        $ty:ident {
            $( $key:ident : $en:literal , $zh:literal , $es:literal ),* $(,)?
        }
    ) => {
        $( #[$meta] )*
        pub struct $ty {
            $( pub $key: &'static str, )*
        }

        pub(crate) const EN: $ty = $ty { $( $key: $en, )* };
        pub(crate) const ZH: $ty = $ty { $( $key: $zh, )* };
        pub(crate) const ES: $ty = $ty { $( $key: $es, )* };

        /// Every key with its three translations, as
        /// `(key, english, chinese, spanish)`.
        ///
        /// Test-only: nothing renders from these, they exist so the invariants in
        /// `catalog` can check every language without reflection.
        #[cfg(test)]
        pub(super) const ENTRIES: &[(&str, &str, &str, &str)] = &[
            $( (stringify!($key), $en, $zh, $es), )*
        ];
    };
}

pub mod chrome;
pub mod common;
pub mod customize;
pub mod devices;
pub mod errors;
pub mod home;
pub mod mood;
pub mod nav;
pub mod settings;
pub mod soundpacks;
pub mod tray;

pub use chrome::ChromeStrings;
pub use common::CommonStrings;
pub use customize::CustomizeStrings;
pub use devices::DevicesStrings;
pub use errors::ErrorStrings;
pub use home::HomeStrings;
pub use mood::MoodStrings;
pub use nav::NavStrings;
pub use settings::SettingsStrings;
pub use soundpacks::SoundpacksStrings;
pub use tray::TrayStrings;

/// Every zone's `ENTRIES` with its name, so the invariants cover the whole
/// catalog and a failure names where the entry lives.
#[cfg(test)]
pub(super) const ZONES: &[(&str, &[(&str, &str, &str, &str)])] = &[
    ("chrome", chrome::ENTRIES),
    ("common", common::ENTRIES),
    ("customize", customize::ENTRIES),
    ("devices", devices::ENTRIES),
    ("errors", errors::ENTRIES),
    ("home", home::ENTRIES),
    ("mood", mood::ENTRIES),
    ("nav", nav::ENTRIES),
    ("settings", settings::ENTRIES),
    ("soundpacks", soundpacks::ENTRIES),
    ("tray", tray::ENTRIES),
];

/// Keys whose translation is legitimately identical to the English one.
///
/// This is not a way to silence a failure. It is for text that has no
/// translation *in principle* (a product name, a key combination) or whose
/// correct translation happens to coincide in that language. Each entry needs a
/// reason on the same line, so the next reader can judge it.
#[cfg(test)]
pub(super) const IDENTICAL_IS_INTENTIONAL: &[(&str, &str, &str)] = &[
    // (key, language tag, reason). The language is part of the entry on purpose:
    // a Spanish coincidence says nothing about Chinese, and exempting the key
    // outright would let an untranslated Chinese value through.
    (
        "error",
        "es",
        "Spanish spells it `Error`; German (Fehler) and French (Erreur) do not, so it stays a catalog key",
    ),
    (
        "color",
        "es",
        "the Spanish for it is `Color`; a hardcoded literal could not be translated where the word differs at all",
    ),
    (
        "section_general",
        "es",
        "`General` names the section identically in Spanish; recorded so the next reader does not `fix` it into a worse word",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Language;

    fn is_allowlisted(key: &str, language: &str) -> bool {
        IDENTICAL_IS_INTENTIONAL
            .iter()
            .any(|(allowed, allowed_language, _)| *allowed == key && *allowed_language == language)
    }

    #[test]
    fn the_catalog_is_not_empty() {
        let total: usize = ZONES.iter().map(|(_, entries)| entries.len()).sum();
        assert!(total > 0, "the catalog has no entries at all");
    }

    #[test]
    fn no_translation_is_blank() {
        for (zone, entries) in ZONES {
            for (key, en, zh, es) in *entries {
                for (language, value) in [("en", en), ("zh", zh), ("es", es)] {
                    assert!(
                        !value.trim().is_empty(),
                        "{zone}.{key} is blank in {language}; every language needs a value"
                    );
                }
            }
        }
    }

    /// A trailing space inside a catalog value is invisible in review and gets
    /// eaten by editors, so the separator belongs at the call site, not here.
    #[test]
    fn no_translation_has_surrounding_whitespace() {
        for (zone, entries) in ZONES {
            for (key, en, zh, es) in *entries {
                for (language, value) in [("en", en), ("zh", zh), ("es", es)] {
                    assert!(
                        value.trim() == *value,
                        "{zone}.{key} has surrounding whitespace in {language}; \
                         move the spacing to the call site"
                    );
                }
            }
        }
    }

    /// `fill` walks the placeholders in order, so a translation that drops or
    /// reorders one silently loses or misplaces information.
    #[test]
    fn every_translation_keeps_the_english_placeholders() {
        for (zone, entries) in ZONES {
            for (key, en, zh, es) in *entries {
                let expected = en.matches("{}").count();
                for (language, value) in [("zh", zh), ("es", es)] {
                    assert_eq!(
                        value.matches("{}").count(),
                        expected,
                        "{zone}.{key} has a different number of `{{}}` placeholders in {language} \
                         than in English ({value:?} vs {en:?})"
                    );
                }
            }
        }
    }

    #[test]
    fn every_translation_differs_from_the_base_language() {
        for (zone, entries) in ZONES {
            for (key, en, zh, es) in *entries {
                for (language, value) in [("zh", zh), ("es", es)] {
                    if is_allowlisted(key, language) {
                        continue;
                    }
                    assert_ne!(
                        value, en,
                        "{zone}.{key} is identical to the English text in {language}. Either \
                         translate it, or add `(\"{key}\", \"{language}\", \"why\")` to \
                         IDENTICAL_IS_INTENTIONAL as a reviewed decision."
                    );
                }
            }
        }
    }

    #[test]
    fn the_allowlist_has_no_stale_entries() {
        let keys: Vec<&str> = ZONES
            .iter()
            .flat_map(|(_, entries)| entries.iter().map(|(key, ..)| *key))
            .collect();
        for (key, language, reason) in IDENTICAL_IS_INTENTIONAL {
            assert!(keys.contains(key), "IDENTICAL_IS_INTENTIONAL names `{key}`, which is not a catalog key");
            assert!(
                crate::i18n::Language::ALL.iter().any(|l| l.tag() == *language),
                "`{key}` names `{language}`, which is not a language tag"
            );
            assert!(!reason.trim().is_empty(), "`{key}` needs a reason in IDENTICAL_IS_INTENTIONAL");
        }
    }

    /// Each language must reach its **own** catalog. A `for_language` arm that
    /// returned the wrong `const` (or a copy-pasted one) would keep compiling and
    /// would silently show English to a Chinese reader, which no textual
    /// assertion on a single key would catch.
    #[test]
    fn every_language_resolves_to_its_own_catalog() {
        for (index, language) in Language::ALL.iter().enumerate() {
            for other in &Language::ALL[index + 1..] {
                assert!(
                    !std::ptr::eq(language.strings(), other.strings()),
                    "{} and {} resolve to the same catalog",
                    language.tag(),
                    other.tag()
                );
            }
        }
    }
}
