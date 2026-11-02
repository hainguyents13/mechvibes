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
        set_dock_icon_visible(true);
        activate_app();
    }
    found
}

/// `NSApplicationActivationPolicyRegular`: an ordinary app, with a Dock icon
/// and a menu bar.
const ACTIVATION_POLICY_REGULAR: isize = 0;
/// `NSApplicationActivationPolicyAccessory`: no Dock icon and no menu bar, like
/// a menu bar app. Its windows and its status item keep working.
const ACTIVATION_POLICY_ACCESSORY: isize = 1;

/// The activation policy that gives the app a Dock icon or takes it away.
fn activation_policy(dock_icon_visible: bool) -> isize {
    if dock_icon_visible { ACTIVATION_POLICY_REGULAR } else { ACTIVATION_POLICY_ACCESSORY }
}

/// Shows or hides the app's Dock icon.
///
/// Hiding the window to the menu bar hides the Dock icon too, and showing the
/// window brings it back. The app is still launched as a regular app (the
/// bundle does not set `LSUIElement`), so Accessibility can be granted to it
/// the normal way; only the running app switches policy.
///
/// Switching back to a regular app does not bring it to the front, so a show
/// path calls `activate_app()` after this. Does nothing when the app already
/// has the asked-for policy, so calling it from every show path is cheap.
///
/// Must run on the main thread, like the rest of this module.
pub fn set_dock_icon_visible(visible: bool) {
    let policy = activation_policy(visible);

    // SAFETY: plain Objective-C messages on the main thread. The receiver is
    // checked for null before use, and the policy is one of the two values
    // AppKit defines for `NSApplicationActivationPolicy`.
    unsafe {
        let Some(class) = AnyClass::get(c"NSApplication") else {
            return;
        };
        let app: *mut AnyObject = msg_send![class, sharedApplication];
        if app.is_null() {
            return;
        }
        let current: isize = msg_send![app, activationPolicy];
        if current == policy {
            return;
        }
        let changed: Bool = msg_send![app, setActivationPolicy: policy];
        if !changed.as_bool() {
            crate::always_eprint!("⚠️ Could not {} the Dock icon", if visible { "show" } else { "hide" });
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// AppKit reads the policy as a plain number, so a swapped value would
    /// hide the Dock icon when the window comes back instead of when it goes.
    #[test]
    fn the_dock_icon_follows_the_regular_and_accessory_policies() {
        assert_eq!(activation_policy(true), 0, "a visible Dock icon is the Regular policy");
        assert_eq!(activation_policy(false), 1, "a hidden Dock icon is the Accessory policy");
    }
}
