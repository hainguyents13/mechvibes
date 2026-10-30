//! Command-line flag parsing for the launch modes `main()` chooses between.
//!
//! Kept as a pure function over an argument list, with no environment access,
//! so every flag combination is testable without spawning a process. `main()`
//! passes `std::env::args_os()` in and branches on the result.
//!
//! Unknown arguments are deliberately *not* an error. The app is launched by
//! the installer, by the Windows auto-start entry and by `dx serve`, each of
//! which may append arguments of its own; refusing to start on an unrecognized
//! one would turn a cosmetic surprise into a launch failure.

use std::ffi::OsString;

/// The `--headless` flag: run without a window, webview or tray.
pub const HEADLESS_ARG: &str = "--headless";

/// The `--soundpack <id>` flag: override the keyboard pack for this run only.
pub const SOUNDPACK_ARG: &str = "--soundpack";

/// The `--mouse-soundpack <id>` flag: same, for the mouse pack.
pub const MOUSE_SOUNDPACK_ARG: &str = "--mouse-soundpack";

/// Everything `main()` needs to know from the command line.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct CliArgs {
    /// Run without any UI.
    pub headless: bool,
    /// Start with the window hidden (auto-start uses this).
    pub minimized: bool,
    /// Session-only keyboard soundpack override. Never persisted.
    pub soundpack: Option<String>,
    /// Session-only mouse soundpack override. Never persisted.
    pub mouse_soundpack: Option<String>,
}

/// Parses the launch flags out of an argument list including `argv[0]`.
///
/// `OsString` rather than `String` because `std::env::args()` panics on an
/// argument that is not valid Unicode, which would kill the app before any
/// logging exists to say why - the same reasoning as the `--input-worker`
/// check in `main()`. A non-Unicode argument simply matches no flag here.
///
/// A value-taking flag given without a value (`--soundpack` as the last
/// argument) yields `None` for that override rather than an error: the run
/// continues with the configured pack, which is a better outcome than refusing
/// to start over a typo.
pub fn parse<I>(args: I) -> CliArgs where I: IntoIterator<Item = OsString> {
    let mut parsed = CliArgs::default();
    // Skips argv[0]. Peekable because a value-taking flag must inspect the
    // next argument before deciding to consume it: a following `--flag` has
    // to be left in place so the loop still sees it.
    let mut args = args.into_iter().skip(1).peekable();

    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some(HEADLESS_ARG) => {
                parsed.headless = true;
            }
            Some("--minimized") => {
                parsed.minimized = true;
            }
            Some(SOUNDPACK_ARG) => {
                parsed.soundpack = take_value(&mut args);
            }
            Some(MOUSE_SOUNDPACK_ARG) => {
                parsed.mouse_soundpack = take_value(&mut args);
            }
            _ => {}
        }
    }

    parsed
}

/// Takes the value that follows a value-taking flag.
///
/// A following argument that itself looks like a flag is left in the iterator
/// rather than swallowed, so `--soundpack --headless` still runs headless
/// instead of dropping the mode and hunting for a pack named `--headless`.
fn take_value<I>(args: &mut std::iter::Peekable<I>) -> Option<String>
    where I: Iterator<Item = OsString>
{
    let value = args.peek()?.to_str()?;
    if value.starts_with("--") {
        return None;
    }
    let value = value.to_string();
    args.next();
    Some(value)
}

/// Normalizes a soundpack id the way the loader expects it.
///
/// Soundpack ids are `keyboard/<name>` or `mouse/<name>` (see
/// `soundpack_loader::determine_soundpack_type`, which classifies a pack by
/// this prefix and rejects a keyboard pack loaded into the mouse slot). A bare
/// name from the command line gets `default_prefix` so `--soundpack eg-oreo`
/// works, which is what anyone typing it expects.
pub fn qualify_soundpack_id(id: &str, default_prefix: &str) -> String {
    let id = id.trim();
    if id.starts_with("keyboard/") || id.starts_with("mouse/") {
        return id.to_string();
    }
    // Backslashes reach the loader from hand-edited configs, so accept them
    // here too rather than double-prefixing an already-qualified id.
    if id.starts_with("keyboard\\") || id.starts_with("mouse\\") {
        return id.replace('\\', "/");
    }
    format!("{}{}", default_prefix, id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: &[&str]) -> Vec<OsString> {
        std::iter::once("mechvibes.exe")
            .chain(items.iter().copied())
            .map(OsString::from)
            .collect()
    }

    #[test]
    fn no_flags_means_a_plain_gui_launch() {
        // The default launch must stay exactly as it was: this is the path
        // every existing user takes by double-clicking the icon.
        assert_eq!(parse(args(&[])), CliArgs::default());
    }

    #[test]
    fn headless_flag_is_recognized() {
        let parsed = parse(args(&[HEADLESS_ARG]));
        assert!(parsed.headless);
        assert_eq!(parsed.soundpack, None);
    }

    #[test]
    fn soundpack_flag_takes_the_following_value() {
        let parsed = parse(args(&[HEADLESS_ARG, SOUNDPACK_ARG, "eg-oreo"]));
        assert!(parsed.headless);
        assert_eq!(parsed.soundpack.as_deref(), Some("eg-oreo"));
    }

    #[test]
    fn mouse_soundpack_is_parsed_independently_of_the_keyboard_one() {
        let parsed = parse(
            args(&[SOUNDPACK_ARG, "eg-oreo", MOUSE_SOUNDPACK_ARG, "mouse/ping"])
        );
        assert_eq!(parsed.soundpack.as_deref(), Some("eg-oreo"));
        assert_eq!(parsed.mouse_soundpack.as_deref(), Some("mouse/ping"));
    }

    #[test]
    fn flag_order_does_not_matter() {
        let parsed = parse(args(&[SOUNDPACK_ARG, "eg-oreo", HEADLESS_ARG]));
        assert!(parsed.headless);
        assert_eq!(parsed.soundpack.as_deref(), Some("eg-oreo"));
    }

    #[test]
    fn a_value_flag_with_no_value_does_not_eat_the_next_flag() {
        // `--soundpack --headless` must still run headless. Swallowing the
        // flag as a pack name would drop the mode the user asked for and then
        // fail to find a pack called "--headless".
        let parsed = parse(args(&[SOUNDPACK_ARG, HEADLESS_ARG]));
        assert!(parsed.headless, "the following flag must still be parsed as a flag");
        assert_eq!(parsed.soundpack, None);
    }

    #[test]
    fn a_trailing_value_flag_is_ignored_rather_than_fatal() {
        let parsed = parse(args(&[HEADLESS_ARG, SOUNDPACK_ARG]));
        assert!(parsed.headless);
        assert_eq!(parsed.soundpack, None, "a missing value falls back to the configured pack");
    }

    #[test]
    fn unknown_arguments_are_ignored() {
        // The installer, the auto-start entry and `dx serve` all append
        // arguments of their own; none may keep the app from starting.
        let parsed = parse(args(&["--some-future-flag", HEADLESS_ARG, "stray-positional"]));
        assert!(parsed.headless);
    }

    #[test]
    fn minimized_is_still_recognized() {
        // Pre-existing behavior: the Windows auto-start entry passes this.
        assert!(parse(args(&["--minimized"])).minimized);
        assert!(!parse(args(&["--minimized"])).headless);
    }

    #[test]
    fn non_unicode_arguments_do_not_panic() {
        // `std::env::args()` panics on these, which is why the parser takes
        // `OsString`. Reaching the end of this test is the assertion.
        #[cfg(windows)]
        let bad = {
            use std::os::windows::ffi::OsStringExt;
            // An unpaired surrogate: valid UTF-16, not valid Unicode.
            OsString::from_wide(&[0xd800])
        };
        #[cfg(not(windows))]
        let bad = {
            use std::os::unix::ffi::OsStringExt;
            OsString::from_vec(vec![0xff])
        };

        let argv = vec![OsString::from("mechvibes"), bad, OsString::from(HEADLESS_ARG)];
        assert!(parse(argv).headless);
    }

    #[test]
    fn a_bare_name_is_qualified_as_a_keyboard_pack() {
        assert_eq!(qualify_soundpack_id("eg-oreo", "keyboard/"), "keyboard/eg-oreo");
        assert_eq!(qualify_soundpack_id("ping", "mouse/"), "mouse/ping");
    }

    #[test]
    fn an_already_qualified_id_is_left_alone() {
        // Double-prefixing would produce `keyboard/keyboard/eg-oreo`, which
        // resolves to no directory at all.
        assert_eq!(qualify_soundpack_id("keyboard/eg-oreo", "keyboard/"), "keyboard/eg-oreo");
        assert_eq!(qualify_soundpack_id("mouse/ping", "keyboard/"), "mouse/ping");
    }

    #[test]
    fn backslash_ids_are_normalized_rather_than_prefixed() {
        // The loader accepts either separator; the command line should too.
        assert_eq!(qualify_soundpack_id("keyboard\\eg-oreo", "keyboard/"), "keyboard/eg-oreo");
    }

    #[test]
    fn surrounding_whitespace_is_trimmed() {
        // Quoted arguments from a shell frequently carry it.
        assert_eq!(qualify_soundpack_id("  eg-oreo  ", "keyboard/"), "keyboard/eg-oreo");
    }
}
