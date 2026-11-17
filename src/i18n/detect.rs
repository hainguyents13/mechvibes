//! Where the operating system's language comes from, in two layers.
//!
//! 1. The POSIX environment: `LC_ALL`, then `LC_MESSAGES`, then `LANG`. A value
//!    that is empty, `C` or `POSIX` answers "no language" and the next variable
//!    is tried, which is the precedence this repo already had in
//!    `utils::telemetry`.
//! 2. `sys-locale`, which asks the platform: the preferred UI language on
//!    Windows, `NSLocale` on macOS, `setlocale` on Unix.
//!
//! The second layer is not belt and braces. A Windows app started from the
//! desktop inherits no `LANG`, and a macOS app started from Finder inherits
//! none either, so the environment alone would answer "unknown" on exactly the
//! platforms where most people start the app from an icon, and the app would
//! open in English regardless of the system language.
//!
//! The telemetry keeps its own private copy of layer 1
//! (`utils::telemetry::locale`) on purpose: it feeds an anonymous launch metric
//! whose continuity nobody asked to change, so unifying the two would trade a
//! real risk for cosmetics.

/// Reads one environment variable.
///
/// Injected rather than read from `std::env` inside the function so the
/// precedence can be tested: `std::env::set_var` is `unsafe` in edition 2024
/// and races between tests running in parallel.
pub type EnvLookup<'a> = dyn Fn(&str) -> Option<String> + 'a;

/// The POSIX locale variables, in the order they take precedence.
const PREFERENCE_ORDER: [&str; 3] = ["LC_ALL", "LC_MESSAGES", "LANG"];

/// A value that means "no language chosen": an empty string (some setups export
/// `LC_ALL=` explicitly) or the two C-locale spellings.
fn is_no_language(value: &str) -> bool {
    // `C.UTF-8` and `POSIX.UTF-8` are the same "no language" as `C`, and stripping
    // the codeset first is what lets them fall through to `LANG` and then to the
    // platform. Matching the whole string would treat them as a language named
    // `C.UTF-8`, which no catalog has, so the answer would be English anyway —
    // but it would answer before the platform got its chance.
    let base = value.split(['.', '@']).next().unwrap_or(value).trim();

    base.is_empty() || base.eq_ignore_ascii_case("C") || base.eq_ignore_ascii_case("POSIX")
}

/// Layer 1: the first environment variable that names a language.
pub fn env_locale_from(get: &EnvLookup) -> Option<String> {
    PREFERENCE_ORDER.iter().find_map(|key| {
        let value = get(key)?;
        let trimmed = value.trim();
        if is_no_language(trimmed) {
            return None;
        }
        Some(trimmed.to_string())
    })
}

/// The OS locale as a BCP-47-shaped tag (`es-AR`, `zh-CN`, `en-US`), or `None`
/// when neither layer knows.
pub fn system_locale() -> Option<String> {
    env_locale_from(&|key| std::env::var(key).ok()).or_else(sys_locale::get_locale)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds an [`EnvLookup`] from a list of pairs.
    fn env(pairs: &[(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        let pairs: Vec<(&'static str, String)> =
            pairs.iter().map(|(k, v)| (*k, v.to_string())).collect();
        move |key| {
            pairs
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| v.clone())
        }
    }

    #[test]
    fn lc_all_wins_over_lc_messages_and_lang() {
        let get = env(&[("LC_ALL", "es_ES.UTF-8"), ("LANG", "en_US.UTF-8")]);
        assert_eq!(env_locale_from(&get).as_deref(), Some("es_ES.UTF-8"));
    }

    #[test]
    fn lc_messages_wins_over_lang_when_lc_all_is_absent() {
        let get = env(&[("LC_MESSAGES", "zh_CN.UTF-8"), ("LANG", "en_US.UTF-8")]);
        assert_eq!(env_locale_from(&get).as_deref(), Some("zh_CN.UTF-8"));
    }

    /// The case that made this module two-layer in the first place: `sys-locale`
    /// reads `LANGUAGE` first and has no fallthrough, so `LC_ALL=C` used to
    /// swallow a perfectly good `LANG`.
    #[test]
    fn a_literal_c_locale_falls_through_to_the_next_variable() {
        let get = env(&[("LC_ALL", "C"), ("LANG", "es_AR.UTF-8")]);
        assert_eq!(env_locale_from(&get).as_deref(), Some("es_AR.UTF-8"));
    }

    #[test]
    fn a_c_locale_with_a_codeset_also_falls_through() {
        let get = env(&[("LC_ALL", "C.UTF-8"), ("LANG", "es_AR.UTF-8")]);
        assert_eq!(env_locale_from(&get).as_deref(), Some("es_AR.UTF-8"));
    }

    #[test]
    fn posix_and_empty_values_also_fall_through() {
        let get = env(&[("LC_ALL", "  "), ("LC_MESSAGES", "POSIX"), ("LANG", "zh_CN")]);
        assert_eq!(env_locale_from(&get).as_deref(), Some("zh_CN"));
    }

    #[test]
    fn the_value_is_trimmed() {
        let get = env(&[("LANG", "  es_MX.UTF-8\n")]);
        assert_eq!(env_locale_from(&get).as_deref(), Some("es_MX.UTF-8"));
    }

    #[test]
    fn nothing_set_answers_none() {
        let get = env(&[]);
        assert_eq!(env_locale_from(&get), None);
    }

    #[test]
    fn everything_set_to_a_no_language_value_answers_none() {
        let get = env(&[("LC_ALL", "C"), ("LC_MESSAGES", ""), ("LANG", "POSIX")]);
        assert_eq!(env_locale_from(&get), None);
    }
}
