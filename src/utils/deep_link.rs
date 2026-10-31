//! `mechvibes://install/<id>` links from the sound pack library and
//! `mechvibes://theme/<id>` links from the theme library.
//!
//! A link is untrusted input, since any page can open one. Only the exact
//! forms `mechvibes://install/mv-<32 hex>` and `mechvibes://theme/th-<32 hex>`
//! are accepted, the id only ever goes to the library API at
//! `LIBRARY_BASE_URL`, and nothing is installed until the user confirms it in
//! the app.
//!
//! A link reaches the app in one of two ways. When the app is not running it
//! is on the command line of the launch it causes. When it is, the operating
//! system still starts a second copy with the link, which hands it over
//! through a small file in the data folder and wakes the running copy (see
//! `single_instance`) before exiting.
//!
//! A theme file opened from the file manager arrives the same two ways, as a
//! path or a `file://` URL in place of the link. `classify_arg` tells the two
//! apart, and never takes anything with a scheme of its own for a path, so a
//! page that opens a link cannot pass it off as a file. The file itself is
//! only read by the running copy, and nothing is put on until the user
//! confirms it.

use crate::libs::cli_args::{ MOUSE_SOUNDPACK_ARG, SOUNDPACK_ARG };
use crate::state::paths;
use crate::utils::constants::APP_PROTOCOL;
use crate::utils::theme_share::FILE_EXTENSION;
use std::ffi::OsString;
use std::path::{ Path, PathBuf };
use std::sync::{ Mutex, OnceLock };

/// What a library link asks the app to install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryLink {
    Pack(String),
    Theme(String),
}

/// The pack or theme a library link names, or `None` for anything else.
///
/// A query or fragment is ignored, so the website can tag its links
/// (`?utm_source=...`) without breaking them; nothing in it is read. A
/// trailing `/` is allowed because some browsers add one. The scheme is
/// matched without regard to case, as URL schemes are; the id is not, since
/// library ids are lowercase hex.
pub fn parse_link(link: &str) -> Option<LibraryLink> {
    let link = link.trim();
    let (scheme, rest) = link.split_once("://")?;
    if !scheme.eq_ignore_ascii_case(APP_PROTOCOL) {
        return None;
    }
    let rest = rest.split(['?', '#']).next().unwrap_or_default();
    let (kind, id) = rest.split_once('/')?;
    let id = id.strip_suffix('/').unwrap_or(id);
    match kind {
        "install" if is_library_id(id) => Some(LibraryLink::Pack(id.to_string())),
        "theme" if is_theme_id(id) => Some(LibraryLink::Theme(id.to_string())),
        _ => None,
    }
}

/// `mv-` followed by exactly 32 lowercase hex digits.
pub fn is_library_id(id: &str) -> bool {
    has_hex_id(id, "mv-")
}

/// `th-` followed by exactly 32 lowercase hex digits.
pub fn is_theme_id(id: &str) -> bool {
    has_hex_id(id, "th-")
}

fn has_hex_id(id: &str, prefix: &str) -> bool {
    id.strip_prefix(prefix).is_some_and(|hex| {
        hex.len() == 32 && hex.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

/// The first launch argument that looks like a link to this app, as given.
/// Whether it is a valid library link is decided later, by `parse_link`.
pub fn link_from_args<I>(args: I) -> Option<String> where I: IntoIterator<Item = OsString> {
    let prefix = format!("{}://", APP_PROTOCOL);
    args.into_iter()
        .skip(1)
        .filter_map(|arg| arg.into_string().ok())
        // Compared as bytes: slicing the string at the prefix length would
        // panic on an argument with a multi-byte character there, such as a
        // Vietnamese pack name passed to --soundpack.
        .find(|arg| {
            arg.as_bytes()
                .get(..prefix.len())
                .is_some_and(|head| head.eq_ignore_ascii_case(prefix.as_bytes()))
        })
}

/// What one launch argument, or one handed-over string, asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchArg {
    /// Anything starting `mechvibes:`, as given. Never a path, whatever follows.
    AppLink(String),
    /// A theme file on this computer, made absolute.
    ThemeFile(PathBuf),
    Other,
}

/// Sort a launch argument into a link, a theme file, or neither.
///
/// - `mechvibes:`, in any case and with or without `//`, is a link.
/// - `file:` is a file URL, taken only without a host or with `localhost`,
///   so a URL naming another computer never reaches out to it.
/// - Any other `<scheme>:` is refused. A one-letter scheme is a Windows drive
///   letter (`C:\...`), so that is still a path.
/// - Anything else is a path, and a theme file when it ends in `.mvtheme`.
pub fn classify_arg(arg: &str) -> LaunchArg {
    let path = match scheme_of(arg.trim_start()) {
        Some(scheme) if scheme.eq_ignore_ascii_case(APP_PROTOCOL) => {
            return LaunchArg::AppLink(arg.to_string());
        }
        Some(scheme) if scheme.eq_ignore_ascii_case("file") => {
            match local_file_url_path(arg.trim()) {
                Some(path) => path,
                None => {
                    return LaunchArg::Other;
                }
            }
        }
        Some(_) => {
            return LaunchArg::Other;
        }
        None => PathBuf::from(arg),
    };
    if !is_theme_file_name(&path) {
        return LaunchArg::Other;
    }
    match std::path::absolute(&path) {
        Ok(path) => LaunchArg::ThemeFile(path),
        Err(_) => LaunchArg::Other,
    }
}

/// The scheme `text` starts with, if it has one longer than a drive letter:
/// a letter, then letters, digits, `+`, `-` or `.`, then `:`.
fn scheme_of(text: &str) -> Option<&str> {
    let (scheme, _) = text.split_once(':')?;
    let mut chars = scheme.chars();
    let first = chars.next()?;
    let valid =
        scheme.len() > 1 &&
        first.is_ascii_alphabetic() &&
        chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    valid.then_some(scheme)
}

/// The path a `file:` URL names, only when it names one on this computer.
fn local_file_url_path(text: &str) -> Option<PathBuf> {
    let url = url::Url::parse(text).ok()?;
    if !matches!(url.host_str(), None | Some("") | Some("localhost")) {
        return None;
    }
    url.to_file_path().ok()
}

/// Whether `path` ends in `.mvtheme`, in any case.
pub(crate) fn is_theme_file_name(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case(FILE_EXTENSION))
}

/// The first theme file among the launch arguments.
///
/// Skips `argv[0]`, flags, and the value given to `--soundpack` or
/// `--mouse-soundpack`, which is a pack name even if it ends in `.mvtheme`.
/// An argument that is not valid Unicode is skipped: a handed-over path
/// travels as text.
pub fn theme_file_from_args<I>(args: I) -> Option<PathBuf> where I: IntoIterator<Item = OsString> {
    let mut args = args.into_iter().skip(1).peekable();
    while let Some(arg) = args.next() {
        let Some(arg) = arg.to_str() else {
            crate::always_eprint!("⚠️  Ignoring a launch argument that is not valid text");
            continue;
        };
        if arg == SOUNDPACK_ARG || arg == MOUSE_SOUNDPACK_ARG {
            // The same rule `cli_args` uses: a following flag is not a value.
            if args.peek().and_then(|next| next.to_str()).is_some_and(|next| !next.starts_with("--")) {
                args.next();
            }
            continue;
        }
        if arg.starts_with("--") {
            continue;
        }
        if let LaunchArg::ThemeFile(path) = classify_arg(arg) {
            return Some(path);
        }
    }
    None
}

/// How long a handed-over link stays good. One the running copy never picked
/// up, because it was closing just then, must not pop up days later.
const HAND_OVER_LIFETIME: std::time::Duration = std::time::Duration::from_secs(120);

/// Leave `link` for the copy that is already running. Written to a temporary
/// file and renamed into place, so the running copy never reads half of it.
pub fn hand_over(link: &str) -> Result<(), String> {
    let path = paths::data::pending_deep_link();
    let partial = path.with_extension(format!("{}.partial", std::process::id()));
    std::fs
        ::write(&partial, link)
        .and_then(|()| std::fs::rename(&partial, &path))
        .map_err(|e| format!("Could not hand the link over: {}", e))
}

/// Take a link a second launch left behind, if there is one and it is recent.
pub fn take_handed_over() -> Option<String> {
    let path = paths::data::pending_deep_link();
    let fresh = std::fs
        ::metadata(&path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .is_some_and(|age| age <= HAND_OVER_LIFETIME);
    let link = std::fs::read_to_string(&path).ok();
    let _ = std::fs::remove_file(&path);
    link.filter(|_| fresh)
}

type Stream<T> = (
    tokio::sync::mpsc::UnboundedSender<T>,
    Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<T>>>,
);

fn new_stream<T>() -> Stream<T> {
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    (sender, Mutex::new(Some(receiver)))
}

/// One stream of ids per kind of link, each read by its own dialog.
fn links(theme: bool) -> &'static Stream<String> {
    static PACKS: OnceLock<Stream<String>> = OnceLock::new();
    static THEMES: OnceLock<Stream<String>> = OnceLock::new();
    (if theme { &THEMES } else { &PACKS }).get_or_init(new_stream)
}

/// Theme files to show in the import dialog, from the Import button and
/// anywhere else a file can come from.
fn theme_files() -> &'static Stream<PathBuf> {
    static FILES: OnceLock<Stream<PathBuf>> = OnceLock::new();
    FILES.get_or_init(new_stream)
}

/// Pass a library id to the window, which asks the user before installing.
/// Safe to call before the window exists: ids wait until it takes them.
pub fn deliver(link: LibraryLink) {
    let _ = match link {
        LibraryLink::Pack(id) => links(false).0.send(id),
        LibraryLink::Theme(id) => links(true).0.send(id),
    };
}

/// Hand a raw link or theme file path to the window if it is a valid library
/// link or a theme file, and log it otherwise.
pub fn deliver_link(link: &str) {
    match classify_arg(link) {
        LaunchArg::AppLink(app_link) => {
            if let Some(library_link) = parse_link(&app_link) {
                deliver(library_link);
                return;
            }
        }
        LaunchArg::ThemeFile(path) => {
            deliver_theme_file(path);
            return;
        }
        LaunchArg::Other => {}
    }
    // Shortened and stripped of control characters: it came from whatever
    // page opened it.
    let shown: String = link
        .chars()
        .filter(|c| !c.is_control())
        .take(120)
        .collect();
    crate::always_eprint!(
        "⚠️  Ignoring a link that is not a sound pack or theme link, or a theme file: {}",
        shown
    );
}

/// The stream of sound pack ids for the window. Only the first caller gets it.
pub fn take_pack_receiver() -> Option<tokio::sync::mpsc::UnboundedReceiver<String>> {
    links(false).1.lock().ok()?.take()
}

/// The stream of theme ids for the window. Only the first caller gets it.
pub fn take_theme_receiver() -> Option<tokio::sync::mpsc::UnboundedReceiver<String>> {
    links(true).1.lock().ok()?.take()
}

/// Pass a theme file to the window, which reads it and asks the user before
/// putting it on. Safe to call before the window exists, like `deliver`.
pub fn deliver_theme_file(path: PathBuf) {
    let _ = theme_files().0.send(path);
}

/// The stream of theme files for the window. Only the first caller gets it.
pub fn take_theme_file_receiver() -> Option<tokio::sync::mpsc::UnboundedReceiver<PathBuf>> {
    theme_files().1.lock().ok()?.take()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "mv-0123456789abcdef0123456789abcdef";
    const THEME_ID: &str = "th-0123456789abcdef0123456789abcdef";

    fn install_id(link: &str) -> Option<String> {
        match parse_link(link)? {
            LibraryLink::Pack(id) => Some(id),
            LibraryLink::Theme(_) => None,
        }
    }

    #[test]
    fn an_install_link_gives_its_id() {
        assert_eq!(install_id(&format!("mechvibes://install/{}", ID)).as_deref(), Some(ID));
        assert_eq!(install_id(&format!("MechVibes://install/{}/", ID)).as_deref(), Some(ID));
    }

    #[test]
    fn a_theme_link_gives_its_id() {
        let theme = Some(LibraryLink::Theme(THEME_ID.to_string()));
        assert_eq!(parse_link(&format!("mechvibes://theme/{}", THEME_ID)), theme);
        assert_eq!(parse_link(&format!("MechVibes://theme/{}/?utm_source=library", THEME_ID)), theme);
    }

    /// Each kind of link takes only its own kind of id.
    #[test]
    fn a_theme_link_takes_only_a_theme_id() {
        for link in [
            format!("mechvibes://theme/{}", ID),
            format!("mechvibes://install/{}", THEME_ID),
            format!("mechvibes://theme/{}/extra", THEME_ID),
            format!("mechvibes://theme/../{}", THEME_ID),
            "mechvibes://theme/th-0123456789ABCDEF0123456789ABCDEF".to_string(),
            "mechvibes://theme/".to_string(),
            "mechvibes://theme".to_string(),
        ] {
            assert_eq!(parse_link(&link), None, "{} must be refused", link);
        }
    }

    /// The website may tag its links; the tags are dropped, never read.
    #[test]
    fn a_query_or_fragment_is_ignored() {
        for link in [
            format!("mechvibes://install/{}?utm_source=library&utm_medium=web", ID),
            format!("mechvibes://install/{}/?ref=pack-page", ID),
            format!("mechvibes://install/{}#install", ID),
        ] {
            assert_eq!(install_id(&link).as_deref(), Some(ID), "{} must give the id", link);
        }
    }

    #[test]
    fn anything_else_is_refused() {
        for link in [
            format!("https://install/{}", ID),
            format!("mechvibes://open/{}", ID),
            format!("mechvibes://install/{}/extra", ID),
            format!("mechvibes://install/../{}", ID),
            format!("mechvibes://install/?id={}", ID),
            format!("mechvibes://install/{}x?y=1", ID),
            "mechvibes://install/mv-0123456789ABCDEF0123456789ABCDEF".to_string(),
            "mechvibes://install/mv-0123".to_string(),
            "mechvibes://install/default-keyboard-eg-oreo".to_string(),
            "mechvibes://install/".to_string(),
            "mechvibes:install/x".to_string(),
        ] {
            assert_eq!(install_id(&link), None, "{} must be refused", link);
        }
    }

    #[test]
    fn the_link_is_found_among_the_launch_arguments() {
        let args = ["mechvibes.exe", "--minimized", "mechvibes://install/x"].map(OsString::from);
        assert_eq!(link_from_args(args).as_deref(), Some("mechvibes://install/x"));

        let args = ["mechvibes.exe", "--minimized"].map(OsString::from);
        assert_eq!(link_from_args(args), None);

        // argv[0] is the program, never a link.
        let args = ["mechvibes://install/x"].map(OsString::from);
        assert_eq!(link_from_args(args), None);
    }

    /// A multi-byte character where the prefix ends used to panic at startup.
    #[test]
    fn an_argument_with_accented_letters_is_not_a_link_and_does_not_panic() {
        let args = ["mechvibes.exe", "--soundpack", "Âm thanh cơ học", "ơơơơơơơơơơơơơ"].map(OsString::from);
        assert_eq!(link_from_args(args), None);
    }

    /// A theme file where this platform keeps absolute paths.
    #[cfg(windows)]
    const THEME_PATH: &str = r"C:\Themes\Synthwave.mvtheme";
    #[cfg(not(windows))]
    const THEME_PATH: &str = "/home/me/Themes/Synthwave.mvtheme";

    fn launch(args: &[&str]) -> Option<PathBuf> {
        theme_file_from_args(std::iter::once("mechvibes.exe").chain(args.iter().copied()).map(OsString::from))
    }

    fn is_theme_file(arg: &str) -> bool {
        matches!(classify_arg(arg), LaunchArg::ThemeFile(_))
    }

    #[test]
    fn a_theme_file_on_the_command_line_is_found() {
        assert_eq!(launch(&["--minimized", THEME_PATH]), Some(PathBuf::from(THEME_PATH)));
        assert_eq!(launch(&["--minimized"]), None);
        // argv[0] is the program, never a theme.
        assert_eq!(theme_file_from_args([OsString::from(THEME_PATH)]), None);
        // A path typed relative to a terminal is made absolute, since the copy
        // it may be handed to runs somewhere else.
        let found = launch(&["Synthwave.mvtheme"]).expect("a relative path is a theme file");
        assert!(found.is_absolute());
        assert_eq!(found, std::env::current_dir().unwrap().join("Synthwave.mvtheme"));
    }

    #[test]
    fn a_file_url_with_spaces_becomes_a_path() {
        #[cfg(windows)]
        let (url, path) = ("file:///C:/My%20Themes/Neon%20Night.mvtheme", r"C:\My Themes\Neon Night.mvtheme");
        #[cfg(not(windows))]
        let (url, path) = ("file:///home/me/My%20Themes/Neon%20Night.mvtheme", "/home/me/My Themes/Neon Night.mvtheme");
        assert_eq!(classify_arg(url), LaunchArg::ThemeFile(PathBuf::from(path)));
        assert_eq!(launch(&[url]), Some(PathBuf::from(path)));
    }

    #[test]
    fn the_extension_is_matched_without_case() {
        assert!(is_theme_file("Synthwave.MVTHEME"));
        assert!(is_theme_file("Synthwave.MvTheme"));
        for other in ["Synthwave.zip", "Synthwave.mvtheme.zip", "Synthwave", "mvtheme", ""] {
            assert_eq!(classify_arg(other), LaunchArg::Other, "{:?} is not a theme file", other);
        }
    }

    #[test]
    fn a_sound_pack_argument_is_not_a_theme_file() {
        assert_eq!(launch(&["--soundpack", "odd-name.mvtheme"]), None);
        assert_eq!(launch(&["--mouse-soundpack", "odd-name.mvtheme"]), None);
        // A flag after --soundpack is not its value, so a file after that still counts.
        assert_eq!(launch(&["--soundpack", "--minimized", THEME_PATH]), Some(PathBuf::from(THEME_PATH)));
        assert_eq!(launch(&["--soundpack", "eg-oreo", THEME_PATH]), Some(PathBuf::from(THEME_PATH)));
        assert_eq!(launch(&["--some-flag.mvtheme"]), None);
    }

    #[test]
    fn a_library_link_is_not_a_theme_file() {
        let link = format!("mechvibes://theme/{}", THEME_ID);
        assert_eq!(launch(&[&link]), None);
        assert_eq!(classify_arg(&link), LaunchArg::AppLink(link.clone()));
    }

    #[test]
    fn a_non_unicode_argument_is_skipped() {
        #[cfg(windows)]
        let bad = {
            use std::os::windows::ffi::OsStringExt;
            OsString::from_wide(&[0xd800])
        };
        #[cfg(not(windows))]
        let bad = {
            use std::os::unix::ffi::OsStringExt;
            OsString::from_vec(vec![0xff])
        };
        let args = vec![OsString::from("mechvibes.exe"), bad.clone(), OsString::from(THEME_PATH)];
        assert_eq!(theme_file_from_args(args), Some(PathBuf::from(THEME_PATH)));
        assert_eq!(theme_file_from_args(vec![OsString::from("mechvibes.exe"), bad]), None);
    }

    /// A browser may pass an app link in its opaque form. Whatever follows
    /// the scheme, it must never be read as a file.
    #[test]
    fn an_opaque_app_link_is_never_a_path() {
        for link in ["mechvibes:x.mvtheme", "MECHVIBES:x.mvtheme", "Mechvibes://x.mvtheme", " mechvibes:x.mvtheme"] {
            assert!(matches!(classify_arg(link), LaunchArg::AppLink(_)), "{:?} must stay a link", link);
            assert_eq!(launch(&[link]), None, "{:?} must not be a theme file", link);
        }
    }

    #[test]
    fn a_file_url_with_a_host_is_refused() {
        assert_eq!(classify_arg("file://server/share/x.mvtheme"), LaunchArg::Other);
        assert_eq!(classify_arg("FILE://server/share/x.mvtheme"), LaunchArg::Other);
        assert!(is_theme_file("file:///C:/x.mvtheme"));
        assert!(is_theme_file("file://localhost/C:/x.mvtheme"));
        #[cfg(not(windows))]
        assert_eq!(classify_arg("file://localhost/home/me/x.mvtheme"), LaunchArg::ThemeFile(PathBuf::from("/home/me/x.mvtheme")));
    }

    #[test]
    fn an_unknown_scheme_is_not_a_path() {
        for arg in ["https://a/x.mvtheme", "foo:x.mvtheme", "ms-settings:x.mvtheme", "a1+b.c-d:x.mvtheme"] {
            assert_eq!(classify_arg(arg), LaunchArg::Other, "{:?} must be refused", arg);
        }
    }

    #[test]
    fn a_drive_letter_is_a_path() {
        assert!(is_theme_file(r"C:\x.mvtheme"));
        assert!(is_theme_file("d:/Themes/x.mvtheme"));
    }

    /// The window's streams, taken once for every test that delivers, and
    /// locked so those tests take turns.
    struct Streams {
        packs: tokio::sync::mpsc::UnboundedReceiver<String>,
        themes: tokio::sync::mpsc::UnboundedReceiver<String>,
        files: tokio::sync::mpsc::UnboundedReceiver<PathBuf>,
    }

    impl Streams {
        fn drain(&mut self) {
            while self.packs.try_recv().is_ok() {}
            while self.themes.try_recv().is_ok() {}
            while self.files.try_recv().is_ok() {}
        }
    }

    fn streams() -> std::sync::MutexGuard<'static, Streams> {
        static STREAMS: OnceLock<Mutex<Streams>> = OnceLock::new();
        let mut streams = STREAMS.get_or_init(|| {
            Mutex::new(Streams {
                packs: take_pack_receiver().expect("only these tests take the streams"),
                themes: take_theme_receiver().expect("only these tests take the streams"),
                files: take_theme_file_receiver().expect("only these tests take the streams"),
            })
        })
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        streams.drain();
        streams
    }

    #[test]
    fn a_handed_over_path_goes_to_the_theme_file_stream() {
        let mut streams = streams();
        deliver_link(THEME_PATH);
        assert_eq!(streams.files.try_recv().ok(), Some(PathBuf::from(THEME_PATH)));
        assert!(streams.themes.try_recv().is_err());
        assert!(streams.packs.try_recv().is_err());
    }

    #[test]
    fn a_handed_over_link_still_goes_to_the_library_stream() {
        let mut streams = streams();
        deliver_link(&format!("mechvibes://theme/{}", THEME_ID));
        assert_eq!(streams.themes.try_recv().ok().as_deref(), Some(THEME_ID));
        deliver_link(&format!("mechvibes://install/{}", ID));
        assert_eq!(streams.packs.try_recv().ok().as_deref(), Some(ID));
        assert!(streams.files.try_recv().is_err());
    }

    #[test]
    fn a_handed_over_path_with_another_extension_is_ignored() {
        let mut streams = streams();
        deliver_link(&THEME_PATH.replace(".mvtheme", ".zip"));
        deliver_link("mechvibes:x.mvtheme");
        deliver_link("file://server/share/x.mvtheme");
        assert!(streams.files.try_recv().is_err());
        assert!(streams.themes.try_recv().is_err());
        assert!(streams.packs.try_recv().is_err());
    }
}
