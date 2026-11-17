//! The "Update now" button and its state machine.
//!
//! Downloading is a deliberate user action: a background check only ever
//! *notifies*, and nothing is transferred until this button is clicked. One
//! click then carries the update all the way through - download, verify,
//! launch the installer, close the app - with no second confirmation:
//!
//! ```text
//! Idle       "Update now"
//!   click -> Downloading  spinner + "Downloading new version..." (disabled)
//!         -> installs and exits automatically once verified
//!         -> Failed       reason + "Retry", until a newer version appears
//! ```
//!
//! An installer left staged by an interrupted session is picked up on the next
//! launch (`restore_staged_update`), so clicking again reuses it rather than
//! downloading a second time.
//!
//! `Failed` records the version it failed on. It persists as a "Retry" button
//! rather than clearing itself, so a failure is never silently forgotten, and
//! a release newer than that version resets it to "Update now" - retrying a
//! version that is no longer the latest would be the wrong offer. The browser
//! link that predates the staged-download flow sits on its own line below
//! `Retry`, because retrying cannot help a release with no installer asset.
//!
//! **Every sentence here comes from the `home` zone at render time**, including
//! the failure reason: `UpdateStage::Failed` carries a `String` that the update
//! layers build, and those layers keep their `Display` a stable English
//! description (it also reaches the log). [`update_failure_text`] is where that
//! text meets the catalog.

use crate::i18n::catalog::HomeStrings;
use crate::utils::auto_updater::{
    clear_failed_update,
    download_and_stage_update,
    get_update_stage,
    install_staged_update,
    AutoUpdater,
    UpdateInfo,
    UpdateStage,
};
use dioxus::desktop::use_window;
use dioxus::prelude::*;
use lucide_dioxus::{ Download, RefreshCw };

/// The update flow's failure text, in the user's language.
///
/// `UpdateStage::Failed { reason }` arrives as a `String` built two layers down:
/// three sentences `auto_updater` writes itself, and `StageError`'s `Display`
/// for everything else. Neither layer may translate, and neither may read the
/// config - their text is also what the log carries - so the mapping happens
/// here, where the catalog is.
///
/// The **frame** of each sentence is translated; the **detail** inside it is
/// not. A download error's inner message, a filesystem error or a hash pair is a
/// diagnostic, and a translated diagnostic cannot be searched for. Anything this
/// function does not recognise passes through untouched, because an English
/// detail beats a blank warning.
fn update_failure_text(strings: &HomeStrings, reason: &str) -> String {
    // The three sentences the update flow words itself.
    match reason {
        "No installer asset in this release" => return strings.update_failed_no_asset.to_string(),
        "This release has no published checksums; use the download page." => {
            return strings.update_failed_no_checksums.to_string();
        }
        "The downloaded installer failed verification and was discarded." => {
            return strings.update_failed_checksum_discarded.to_string();
        }
        _ => {}
    }

    // `StageError`'s `Display` frames. Matched by prefix, with the detail kept
    // verbatim: the alternative is translating a hash or an OS error.
    if let Some(detail) = reason.strip_prefix("Download failed: ") {
        return crate::i18n::fill(strings.update_failed_download, &[&detail]);
    }
    if let Some(detail) = reason.strip_prefix("Checksum unavailable: ") {
        return crate::i18n::fill(strings.update_failed_checksum_unavailable, &[&detail]);
    }
    if let Some(detail) = reason.strip_prefix("Checksum mismatch: expected ") {
        if let Some((expected, actual)) = detail.split_once(", got ") {
            return crate::i18n::fill(strings.update_failed_checksum_mismatch, &[&expected, &actual]);
        }
    }
    if let Some(detail) = reason.strip_prefix("I/O error: ") {
        return crate::i18n::fill(strings.update_failed_io, &[&detail]);
    }
    if let Some(detail) = reason.strip_prefix("Unsupported: ") {
        return crate::i18n::fill(strings.update_failed_unsupported, &[&detail]);
    }

    reason.to_string()
}

/// Renders the install control for an available update.
///
/// On platforms without a silent installer this collapses to a plain link to
/// the release page - there is no in-place upgrade path to offer there.
#[component]
pub fn UpdateInstallButton(info: UpdateInfo) -> Element {
    let window = use_window();

    // Read at render time, never inside a memo: the config signal this reads is
    // what re-renders the button when the language changes. There is no memo in
    // this component - the stage is a signal holding updater state, not text.
    let strings = crate::i18n::use_strings();

    // The stage is written from an async task with no handle on any signal,
    // so it is polled. Half a second is well under the time any step takes
    // and costs a single Mutex read.
    let mut stage = use_signal(get_update_stage);

    // A failure sticks: the button becomes "Retry" and stays that way until
    // it succeeds or a newer version appears. `Failed` carries the version it
    // failed on, so a release newer than that one clears it back to a plain
    // "Update now" - retrying a version that is no longer the latest would be
    // the wrong offer.
    let latest_version = info.latest_version.clone();
    use_future(move || {
        let latest_version = latest_version.clone();
        async move {
            loop {
                if let UpdateStage::Failed { version, .. } = get_update_stage() {
                    if version != latest_version {
                        clear_failed_update();
                    }
                }

                let current = get_update_stage();
                if *stage.peek() != current {
                    stage.set(current);
                }
                futures_timer::Delay::new(std::time::Duration::from_millis(500)).await;
            }
        }
    });

    let version = info.latest_version.clone();
    let release_page = AutoUpdater::releases_page_url(&version);

    // No silent installer on this platform: link out, never offer a download
    // that could not be applied anyway.
    if !crate::utils::update_installer::silent_update_supported() {
        return rsx! {
          a {
            href: "{release_page}",
            target: "_blank",
            class: "btn btn-success btn-sm",
            Download { class: "w-4 h-4 mr-1" }
            {crate::i18n::fill(strings.home.update_get_version, &[&format!("v{version}")])}
          }
        };
    }

    let fallback_link = rsx! {
      a {
        href: "{release_page}",
        target: "_blank",
        class: "link link-hover text-xs",
        "{strings.home.update_open_page}"
      }
    };

    // Shared by the first attempt and the retry: one click downloads,
    // verifies, runs the installer and closes the app.
    let start_update = {
        let info = info.clone();
        let window = window.clone();
        move |_| {
            let info = info.clone();
            let window = window.clone();
            spawn(async move {
                download_and_stage_update(&info).await;

                // `download_and_stage_update` reports through the shared stage
                // rather than a return value, and it has several early exits
                // (already downloading, no asset, transfer failed). Installing
                // is therefore gated on reading the stage back, not on the
                // await returning.
                if !matches!(get_update_stage(), UpdateStage::Ready { .. }) {
                    return;
                }

                match install_staged_update() {
                    Ok(()) => {
                        // Installer is running detached; close the app the
                        // same way the tray Exit does, which also drops the
                        // input worker's stdin and takes that child with it.
                        window.close();
                    }
                    Err(e) => {
                        crate::always_eprint!("❌ Could not start the update installer: {}", e);
                    }
                }
            });
        }
    };

    match stage() {
        UpdateStage::Downloading { version } =>
            rsx! {
          button { class: "btn btn-success btn-sm", disabled: true,
            span { class: "loading loading-spinner loading-xs mr-1" }
            "{strings.home.update_downloading}"
          }
          // The version number keeps its `v` at the call site: the prefix is
          // part of the version, not of a sentence.
          div { class: "text-xs text-base-content/50 mt-1", "v{version}" }
        },

        // Verified and on disk. The click that started the download installs
        // from here without asking again, so this is only ever shown for the
        // moment between verification and the app exiting - or if launching
        // the installer failed, in which case clicking retries it.
        UpdateStage::Ready { version, .. } =>
            rsx! {
          button { class: "btn btn-success btn-sm", disabled: true,
            span { class: "loading loading-spinner loading-xs mr-1" }
            "{strings.home.update_installing}"
          }
          div { class: "text-xs text-base-content/50 mt-1",
            {crate::i18n::fill(strings.home.update_ready, &[&format!("v{version}")])}
          }
        },

        // Sticks until the retry succeeds or a newer version supersedes it.
        // The browser link sits on its own line below the button, not beside
        // it: a retry cannot help when the release has no installer asset or
        // the network is blocked outright, so the way out has to stay visible
        // without crowding the button.
        UpdateStage::Failed { reason, .. } =>
            rsx! {
          div { class: "flex flex-col items-start gap-2",
            div { class: "text-sm text-warning", {update_failure_text(&strings.home, &reason)} }
            button {
              class: "btn btn-warning btn-sm",
              onclick: start_update,
              RefreshCw { class: "w-4 h-4 mr-1" }
              "{strings.common.retry}"
            }
            {fallback_link}
          }
        },

        UpdateStage::Idle =>
            rsx! {
          button {
            class: "btn btn-success btn-sm",
            onclick: start_update,
            Download { class: "w-4 h-4 mr-1" }
            "{strings.home.update_now}"
          }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Language;
    use crate::utils::update_installer::StageError;

    fn home(language: Language) -> &'static HomeStrings {
        &language.strings().home
    }

    /// The three sentences `auto_updater` writes itself must be the exact ones
    /// this screen recognises. If that layer re-words one, the mapping stops
    /// matching and the user is shown English - which no other test here can
    /// see, because the failure arm falls back to the raw text on purpose.
    #[test]
    fn the_authored_reasons_are_still_the_ones_the_updater_writes() {
        let source = include_str!("../utils/auto_updater.rs");
        for reason in [
            "No installer asset in this release",
            "This release has no published checksums; use the download page.",
            "The downloaded installer failed verification and was discarded.",
        ] {
            assert!(source.contains(reason), "auto_updater no longer writes {reason:?}");
        }
    }

    #[test]
    fn the_authored_reasons_reach_the_screen_translated() {
        let cases = [
            (
                "No installer asset in this release",
                "No installer asset in this release",
                "此版本没有安装程序文件",
                "Esta versión no incluye instalador",
            ),
            (
                "This release has no published checksums; use the download page.",
                "This release has no published checksums; use the download page.",
                "此版本没有发布校验和，请使用下载页面。",
                "Esta versión no publica sumas de verificación; usa la página de descarga.",
            ),
            (
                "The downloaded installer failed verification and was discarded.",
                "The downloaded installer failed verification and was discarded.",
                "下载的安装程序未通过校验，已被丢弃。",
                "El instalador descargado no pasó la verificación y se descartó.",
            ),
        ];
        for (reason, en, zh, es) in cases {
            assert_eq!(update_failure_text(home(Language::En), reason), en);
            assert_eq!(update_failure_text(home(Language::Zh), reason), zh);
            assert_eq!(update_failure_text(home(Language::Es), reason), es);
        }
    }

    /// The `StageError` frames are matched through their real `Display`, so a
    /// re-worded prefix in `update_installer` fails here instead of silently
    /// dropping that failure back to English.
    #[test]
    fn a_stage_error_keeps_its_frame_and_its_detail() {
        let es = home(Language::Es);
        let zh = home(Language::Zh);

        let download = StageError::Download("connection reset".to_string()).to_string();
        assert_eq!(update_failure_text(es, &download), "Error de descarga: connection reset");
        assert_eq!(update_failure_text(zh, &download), "下载失败：connection reset");

        let unavailable = StageError::ChecksumUnavailable("no SHA256SUMS.txt".to_string()).to_string();
        assert_eq!(
            update_failure_text(es, &unavailable),
            "Suma de verificación no disponible: no SHA256SUMS.txt"
        );

        let mismatch = StageError::ChecksumMismatch {
            expected: "aa".to_string(),
            actual: "bb".to_string(),
        }
        .to_string();
        assert_eq!(
            update_failure_text(es, &mismatch),
            "Suma de verificación distinta: se esperaba aa, se obtuvo bb"
        );

        let io = StageError::Io("Staged installer is gone".to_string()).to_string();
        assert_eq!(update_failure_text(es, &io), "Error de E/S: Staged installer is gone");

        let unsupported = StageError::Unsupported("no AppImage".to_string()).to_string();
        assert_eq!(update_failure_text(es, &unsupported), "No compatible: no AppImage");
    }

    /// English is what the screen shipped, so the mapping must not re-word it
    /// either: the `Display` frames are reproduced verbatim.
    #[test]
    fn english_reproduces_the_updaters_own_sentences() {
        let en = home(Language::En);
        for error in [
            StageError::Download("connection reset".to_string()),
            StageError::ChecksumUnavailable("no SHA256SUMS.txt".to_string()),
            StageError::ChecksumMismatch { expected: "aa".to_string(), actual: "bb".to_string() },
            StageError::Io("disk full".to_string()),
            StageError::Unsupported("no installer".to_string()),
        ] {
            let reason = error.to_string();
            assert_eq!(update_failure_text(en, &reason), reason, "{reason}");
        }
    }

    /// An unrecognised reason is a diagnostic from a layer this screen does not
    /// own. It must reach the user untouched rather than blank or half-mapped.
    #[test]
    fn an_unknown_reason_passes_through() {
        for language in Language::ALL {
            let strings = home(language);
            for reason in ["Something new went wrong", "Checksum mismatch: no separator"] {
                assert_eq!(update_failure_text(strings, reason), reason);
            }
            assert_eq!(update_failure_text(strings, ""), "");
        }
    }
}
