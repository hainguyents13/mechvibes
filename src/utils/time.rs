//! Relative time: "Just now", "2 minutes ago", "3 weeks ago".
//!
//! **The thresholds are Rust's; the wording is the catalog's.** Which
//! bucket a timestamp falls into (minute, hour, day, week, month, year) and how
//! wide each one is belongs in this file, because it is arithmetic and not
//! language. The sentence for a bucket is a `_one`/`_other` pair in
//! [`crate::i18n::catalog::HomeStrings`], because word order and inflection are
//! language.
//!
//! **`now` is a parameter, not a clock read.** [`format_relative_time_in`] is a
//! pure function of `(wording, timestamp, now)`, so a test can pin every
//! boundary without sleeping or racing the wall clock.
//!
//! **The count is never used to build a sentence.** Each member of a pair
//! already carries its own number — the singular is the finished `"1 minute
//! ago"`, the plural is the template `"{} minutes ago"` — so the code picks a
//! member with [`plural`] and always runs it through [`fill`]. `fill` leaves a
//! template with no `{}` untouched, which is why one code path serves both
//! members and no language has to fit "number + noun + ago" word order.

use crate::i18n::catalog::HomeStrings;
use crate::i18n::{ fill, plural };

/// Formats `timestamp` (seconds since the Unix epoch) in the wording of
/// `strings`, relative to `now` (also seconds since the epoch).
///
/// `now` is injected so the function stays pure: no clock read, no sleeping, and
/// a test can walk every threshold. A timestamp after `now` (clock skew, a file
/// touched in the future) reads as "Just now" instead of panicking on the
/// subtraction.
///
/// Examples, in English: `"Just now"`, `"2 minutes ago"`, `"1 hour ago"`,
/// `"3 days ago"`.
pub fn format_relative_time_in(strings: &HomeStrings, timestamp: u64, now: u64) -> String {
    let diff = now.saturating_sub(timestamp);

    if diff < 60 {
        return strings.time_just_now.to_string();
    }

    // One pair and one count per bucket, then the same two lines for all of
    // them. Nothing here knows how the sentence is worded.
    let (one, other, count) = if diff < 3600 {
        (strings.time_minute_one, strings.time_minute_other, diff / 60)
    } else if diff < 86_400 {
        (strings.time_hour_one, strings.time_hour_other, diff / 3600)
    } else if diff < 604_800 {
        (strings.time_day_one, strings.time_day_other, diff / 86_400)
    } else if diff < 2_592_000 {
        // 30 days
        (strings.time_week_one, strings.time_week_other, diff / 604_800)
    } else if diff < 31_536_000 {
        // 365 days
        (strings.time_month_one, strings.time_month_other, diff / 2_592_000)
    } else {
        (strings.time_year_one, strings.time_year_other, diff / 31_536_000)
    };

    fill(plural(one, other, count as usize), &[&count])
}

/// Seconds since the Unix epoch, for the callers that read the clock in a
/// component body and pass the result to [`format_relative_time_in`].
///
/// The formatter itself never reads the clock, so its tests can pin every
/// threshold instead of racing it.
pub fn now_secs() -> u64 {
    std::time::SystemTime
        ::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Language;

    /// Fixed, so no test here reads the clock: every case passes the
    /// distance from `NOW` it wants and the formatter subtracts it.
    const NOW: u64 = 1_700_000_000;

    fn ago(strings: &HomeStrings, seconds: u64) -> String {
        format_relative_time_in(strings, NOW.saturating_sub(seconds), NOW)
    }

    fn english(seconds: u64) -> String {
        ago(&Language::En.strings().home, seconds)
    }

    fn chinese(seconds: u64) -> String {
        ago(&Language::Zh.strings().home, seconds)
    }

    fn spanish(seconds: u64) -> String {
        ago(&Language::Es.strings().home, seconds)
    }

    /// Every threshold of the ladder, in English and byte for byte: this is the
    /// text the app shipped, and the catalog must not have re-worded it.
    #[test]
    fn english_pins_every_threshold() {
        let cases = [
            (0, "Just now"),
            (1, "Just now"),
            (59, "Just now"),
            // First threshold: seconds become a minute.
            (60, "1 minute ago"),
            (61, "1 minute ago"),
            (119, "1 minute ago"),
            (120, "2 minutes ago"),
            (3599, "59 minutes ago"),
            (3600, "1 hour ago"),
            (7199, "1 hour ago"),
            (7200, "2 hours ago"),
            (86_399, "23 hours ago"),
            (86_400, "1 day ago"),
            (172_800, "2 days ago"),
            (604_799, "6 days ago"),
            (604_800, "1 week ago"),
            (1_209_600, "2 weeks ago"),
            (2_591_999, "4 weeks ago"),
            (2_592_000, "1 month ago"),
            (5_184_000, "2 months ago"),
            (31_535_999, "12 months ago"),
            (31_536_000, "1 year ago"),
            (63_072_000, "2 years ago"),
        ];
        for (seconds, expected) in cases {
            assert_eq!(english(seconds), expected, "{seconds} seconds before now");
        }
    }

    /// A timestamp far in the past is the large-value case: the year bucket has
    /// no ceiling, and `saturating_sub` has to survive the subtraction. `NOW` is
    /// too small to reach these by subtraction, so `now` moves instead.
    #[test]
    fn a_large_value_still_formats() {
        let en = &Language::En.strings().home;
        assert_eq!(format_relative_time_in(en, 0, NOW), "53 years ago");
        assert_eq!(format_relative_time_in(en, 0, 31_536_000 * 120), "120 years ago");
        assert_eq!(format_relative_time_in(en, 0, u64::MAX), "584942417355 years ago");
        assert_eq!(format_relative_time_in(en, 0, 0), "Just now");
    }

    /// A timestamp after `now` — clock skew, a file dated in the future — reads
    /// as "Just now" rather than underflowing.
    #[test]
    fn a_future_timestamp_reads_as_just_now() {
        let expected = [("en", "Just now"), ("zh", "刚刚"), ("es", "Ahora mismo")];
        let languages = [Language::En, Language::Zh, Language::Es];
        for (language, (tag, text)) in languages.iter().zip(expected) {
            let strings = &language.strings().home;
            for timestamp in [NOW + 1, NOW + 3600, NOW + 31_536_000, u64::MAX] {
                assert_eq!(format_relative_time_in(strings, timestamp, NOW), text, "{tag} future");
            }
        }
    }

    /// Chinese has no plural inflection, so the same wording serves 1 and 8 —
    /// only the number in front of it changes.
    #[test]
    fn chinese_pins_every_threshold() {
        let cases = [
            (0, "刚刚"),
            (59, "刚刚"),
            (60, "1 分钟前"),
            (120, "2 分钟前"),
            (3599, "59 分钟前"),
            (3600, "1 小时前"),
            (7200, "2 小时前"),
            (86_400, "1 天前"),
            (172_800, "2 天前"),
            (604_800, "1 周前"),
            (1_209_600, "2 周前"),
            (2_592_000, "1 个月前"),
            (5_184_000, "2 个月前"),
            (31_536_000, "1 年前"),
            (63_072_000, "2 年前"),
        ];
        for (seconds, expected) in cases {
            assert_eq!(chinese(seconds), expected, "zh {seconds} seconds before now");
        }
    }

    /// Spanish inflects, so the singular and the plural are different words and
    /// both are pinned at the boundary that switches between them.
    #[test]
    fn spanish_pins_every_threshold() {
        let cases = [
            (0, "Ahora mismo"),
            (59, "Ahora mismo"),
            (60, "Hace 1 minuto"),
            (120, "Hace 2 minutos"),
            (3599, "Hace 59 minutos"),
            (3600, "Hace 1 hora"),
            (7200, "Hace 2 horas"),
            (86_400, "Hace 1 día"),
            (172_800, "Hace 2 días"),
            (604_800, "Hace 1 semana"),
            (1_209_600, "Hace 2 semanas"),
            (2_592_000, "Hace 1 mes"),
            (5_184_000, "Hace 2 meses"),
            (31_536_000, "Hace 1 año"),
            (63_072_000, "Hace 2 años"),
        ];
        for (seconds, expected) in cases {
            assert_eq!(spanish(seconds), expected, "es {seconds} seconds before now");
        }
    }

    /// The three languages must actually differ: a catalog wired to the wrong
    /// const would render English text under a Chinese config and every other
    /// test here would still pass.
    #[test]
    fn each_language_reads_its_own_catalog() {
        let en = english(7200);
        let zh = chinese(7200);
        let es = spanish(7200);
        assert_ne!(en, zh);
        assert_ne!(en, es);
        assert_ne!(zh, es);
        assert_eq!(en, "2 hours ago");
    }
}
