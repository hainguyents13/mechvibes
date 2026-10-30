//! `mechvibes://install/<id>` links from the sound pack library.
//!
//! A link is untrusted input, since any page can open one. Only the exact
//! form `mechvibes://install/mv-<32 hex>` is accepted, the id only ever goes
//! to the library API at `LIBRARY_BASE_URL`, and nothing is installed until
//! the user confirms it in the app.
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

/// The library id in an install link, or `None` for anything else.
///
/// A trailing `/` is allowed because some browsers add one. The scheme is
/// matched without regard to case, as URL schemes are; the id is not, since
/// library ids are lowercase hex.
pub fn install_id(link: &str) -> Option<String> {
    let link = link.trim();
    let (scheme, rest) = link.split_once("://")?;
    if !scheme.eq_ignore_ascii_case(APP_PROTOCOL) {
        return None;
    }
    let id = rest.strip_prefix("install/")?;
    let id = id.strip_suffix('/').unwrap_or(id);
    is_library_id(id).then(|| id.to_string())
}

/// `mv-` followed by exactly 32 lowercase hex digits.
pub fn is_library_id(id: &str) -> bool {
    id.strip_prefix("mv-").is_some_and(|hex| {
        hex.len() == 32 && hex.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

/// The first launch argument that looks like a link to this app, as given.
/// Whether it is a valid install link is decided later, by `install_id`.
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

fn links() -> &'static Links {
    static LINKS: OnceLock<Links> = OnceLock::new();
    LINKS.get_or_init(|| {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        (sender, Mutex::new(Some(receiver)))
    })
}

/// Pass a library id to the window, which asks the user before installing.
/// Safe to call before the window exists: ids wait until it takes them.
pub fn deliver(id: String) {
    let _ = links().0.send(id);
}

/// Hand a raw link to the window if it is a valid install link, and log it
/// otherwise.
pub fn deliver_link(link: &str) {
    match install_id(link) {
        Some(id) => deliver(id),
        None => {
            // Shortened and stripped of control characters: it came from
            // whatever page opened it.
            let shown: String = link
                .chars()
                .filter(|c| !c.is_control())
                .take(120)
                .collect();
            crate::always_eprint!("⚠️  Ignoring a link that is not a sound pack install link: {}", shown);
        }
    }
}

/// The stream of ids for the window. Only the first caller gets it.
pub fn take_receiver() -> Option<tokio::sync::mpsc::UnboundedReceiver<String>> {
    links().1.lock().ok()?.take()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "mv-0123456789abcdef0123456789abcdef";

    #[test]
    fn an_install_link_gives_its_id() {
        assert_eq!(install_id(&format!("mechvibes://install/{}", ID)).as_deref(), Some(ID));
        assert_eq!(install_id(&format!("MechVibes://install/{}/", ID)).as_deref(), Some(ID));
    }

    #[test]
    fn anything_else_is_refused() {
        for link in [
            format!("https://install/{}", ID),
            format!("mechvibes://open/{}", ID),
            format!("mechvibes://install/{}/extra", ID),
            format!("mechvibes://install/../{}", ID),
            format!("mechvibes://install/{}?x=1", ID),
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
