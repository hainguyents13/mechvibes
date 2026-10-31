//! Short messages for the user, reported from anywhere in the app.
//!
//! A failed save used to reach only the log, so the user went on as if their
//! change had stuck. `report` can be called from any thread (the config writer
//! also runs on the audio thread and in tokio tasks), and the window's poll
//! loop picks the messages up with `take_new` and shows them as toasts.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{ AtomicBool, AtomicU64, Ordering };
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeKind {
    Info,
    Error,
}

/// A button the toast offers next to its text.
#[derive(Debug, Clone, PartialEq)]
pub enum NoticeAction {
    /// Show this file in the system file manager.
    ShowFile(PathBuf),
    /// Go back to the look from before the last theme was put on
    /// (`look_undo`).
    Undo,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Notice {
    pub id: u64,
    pub kind: NoticeKind,
    pub text: String,
    pub action: Option<NoticeAction>,
}

/// The window shows at most this many, so no more wait for it either. A
/// burst of the same failure, or a window that is not open, cannot pile up.
const MAX_WAITING: usize = 3;

static WAITING: Mutex<VecDeque<Notice>> = Mutex::new(VecDeque::new());
/// Set once something is waiting, so the poll loop's check on every tick
/// is one atomic load rather than a lock.
static HAS_WAITING: AtomicBool = AtomicBool::new(false);
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Queue a message for the window.
pub fn report(kind: NoticeKind, text: impl Into<String>, action: Option<NoticeAction>) {
    let notice = Notice {
        id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
        kind,
        text: text.into(),
        action,
    };
    {
        let mut waiting = WAITING.lock().unwrap_or_else(|e| e.into_inner());
        waiting.push_back(notice);
        while waiting.len() > MAX_WAITING {
            waiting.pop_front();
        }
    }
    // After the push, so a reader that sees the flag also sees the notice.
    HAS_WAITING.store(true, Ordering::Release);
}

/// The messages reported since the last call, oldest first.
pub fn take_new() -> Vec<Notice> {
    if !HAS_WAITING.swap(false, Ordering::Acquire) {
        return Vec::new();
    }
    WAITING.lock()
        .unwrap_or_else(|e| e.into_inner())
        .drain(..)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_notice_from_another_thread_reaches_the_window() {
        let text = format!("Could not save your changes. {}", uuid::Uuid::new_v4());
        let sent = text.clone();
        std::thread
            ::spawn(move || report(NoticeKind::Error, sent, None))
            .join()
            .unwrap();

        let taken = take_new();
        let notice = taken
            .iter()
            .find(|notice| notice.text == text)
            .expect("the notice reached the window");
        assert_eq!(notice.kind, NoticeKind::Error);
        assert!(!take_new().iter().any(|notice| notice.text == text), "and is taken only once");
    }
}
