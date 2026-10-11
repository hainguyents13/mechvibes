//! Native macOS window control that does not depend on the web view.
//!
//! The app's windows are normally shown and hidden by a task that runs inside the
//! Dioxus virtual DOM. That task only runs while the virtual DOM is being polled,
//! and the virtual DOM is not polled while the web view has UI updates it has not
//! acknowledged. A hidden macOS web view does not run animation frames, which is
//! when it acknowledges them, so one pending update while the window is hidden can
//! freeze every task, including the one that would show the window again.
//!
//! The functions here talk to AppKit directly, so "Show" from the tray menu works
//! even then. Showing the window also lets the web view run again, which releases
//! the frozen tasks.
#![cfg(target_os = "macos")]

use crate::utils::constants::APP_NAME;
use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::{ AnyClass, AnyObject, Bool };
use objc2_foundation::NSString;

/// Brings the app's main window to the front, restoring it if it was hidden or
/// minimized to the Dock, and activates the app. Returns whether a window was
/// found.
///
/// Must run on the main thread. The tray menu's action does, because AppKit
/// delivers it there.
pub fn show_main_window() -> bool {
    // SAFETY: plain Objective-C messages on the main thread. Every receiver is
    // checked for null before use, and the windows are only read and ordered.
    let found = unsafe {
        let Some(class) = AnyClass::get(c"NSApplication") else {
            return false;
        };
        let app: *mut AnyObject = msg_send![class, sharedApplication];
        if app.is_null() {
            return false;
        }
        let windows: Option<Retained<AnyObject>> = msg_send![app, windows];
        let Some(windows) = windows else {
            return false;
        };
        let count: usize = msg_send![&*windows, count];

        let mut found = false;
        for index in 0..count {
            let window: *mut AnyObject = msg_send![&*windows, objectAtIndex: index];
            if window.is_null() {
                continue;
            }
            // The app also owns helper windows with no title; the main one is
            // the one created with the app's name.
            let title: Option<Retained<NSString>> = msg_send![window, title];
            if title.map(|t| t.to_string()).as_deref() != Some(APP_NAME) {
                continue;
            }
            let minimized: Bool = msg_send![window, isMiniaturized];
            if minimized.as_bool() {
                let _: () = msg_send![window, deminiaturize: std::ptr::null_mut::<AnyObject>()];
            }
            let _: () = msg_send![window, makeKeyAndOrderFront: std::ptr::null_mut::<AnyObject>()];
            found = true;
        }
        found
    };

    if found {
        activate_app();
    }
    found
}

/// macOS: make this app the active one so the restored window comes to the
/// front instead of appearing behind whatever the user was using.
///
/// A tray click does not activate the app, and an app started in the
/// background (a login item) is not frontmost, so `set_visible(true)` leaves the
/// window on screen but behind other apps' windows. `set_focus()` alone does not
/// fix that: it goes through `activateIgnoringOtherApps:`, which macOS 14
/// deprecated and no longer reliably honors.
///
/// No single call was reliable when tried on a background-launched app, so this
/// sends all three, newest last: `NSRunningApplication activateWithOptions:`
/// (all windows, ignoring other apps), the legacy
/// `-[NSApplication activateIgnoringOtherApps:]`, and `-[NSApplication
/// activate]` where it exists (macOS 14+). The combination brought the window
/// to the front every time it was tried.
#[cfg(target_os = "macos")]
pub fn activate_app() {
    use objc2::{ msg_send, sel };
    use objc2::runtime::{ AnyClass, AnyObject, Bool };

    /// NSApplicationActivateAllWindows | NSApplicationActivateIgnoringOtherApps
    const ACTIVATE_ALL_WINDOWS_IGNORING_OTHER_APPS: usize = 1 | 2;

    // SAFETY: plain Objective-C messages with no unusual ownership, sent on the
    // main thread (this runs in the UI event loop). Every receiver is checked
    // for null before use.
    unsafe {
        if let Some(class) = AnyClass::get(c"NSRunningApplication") {
            let current: *mut AnyObject = msg_send![class, currentApplication];
            if !current.is_null() {
                let _: Bool = msg_send![
                    current,
                    activateWithOptions: ACTIVATE_ALL_WINDOWS_IGNORING_OTHER_APPS
                ];
            }
        }

        let Some(class) = AnyClass::get(c"NSApplication") else {
            return;
        };
        let app: *mut AnyObject = msg_send![class, sharedApplication];
        if app.is_null() {
            return;
        }
        let _: () = msg_send![app, activateIgnoringOtherApps: Bool::YES];
        let has_activate: Bool = msg_send![app, respondsToSelector: sel!(activate)];
        if has_activate.as_bool() {
            let _: () = msg_send![app, activate];
        }
    }
}
