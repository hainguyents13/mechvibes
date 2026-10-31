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

use crate::state::paths;
use crate::utils::constants::APP_PROTOCOL;
use std::ffi::OsString;
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

type Links = (
    tokio::sync::mpsc::UnboundedSender<String>,
    Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<String>>>,
);

/// One stream of ids per kind of link, each read by its own dialog.
fn links(theme: bool) -> &'static Links {
    static PACKS: OnceLock<Links> = OnceLock::new();
    static THEMES: OnceLock<Links> = OnceLock::new();
    (if theme { &THEMES } else { &PACKS }).get_or_init(|| {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        (sender, Mutex::new(Some(receiver)))
    })
}

/// Pass a library id to the window, which asks the user before installing.
/// Safe to call before the window exists: ids wait until it takes them.
pub fn deliver(link: LibraryLink) {
    let _ = match link {
        LibraryLink::Pack(id) => links(false).0.send(id),
        LibraryLink::Theme(id) => links(true).0.send(id),
    };
}

/// Hand a raw link to the window if it is a valid library link, and log it
/// otherwise.
pub fn deliver_link(link: &str) {
    match parse_link(link) {
        Some(link) => deliver(link),
        None => {
            // Shortened and stripped of control characters: it came from
            // whatever page opened it.
            let shown: String = link
                .chars()
                .filter(|c| !c.is_control())
                .take(120)
                .collect();
            crate::always_eprint!("⚠️  Ignoring a link that is not a sound pack or theme link: {}", shown);
        }
    }
}

/// The stream of sound pack ids for the window. Only the first caller gets it.
pub fn take_pack_receiver() -> Option<tokio::sync::mpsc::UnboundedReceiver<String>> {
    links(false).1.lock().ok()?.take()
}

/// The stream of theme ids for the window. Only the first caller gets it.
pub fn take_theme_receiver() -> Option<tokio::sync::mpsc::UnboundedReceiver<String>> {
    links(true).1.lock().ok()?.take()
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
}
