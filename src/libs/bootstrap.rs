//! Startup wiring shared by the GUI and headless launch paths.
//!
//! Input capture is the one part of startup both modes need to be identical.
//! Duplicating it would let the two drift - and the way it drifts is
//! predictable: whoever writes the second copy reaches for the simplest
//! listener that compiles, which on Windows is the rdev fallback, and ships a
//! mode that eats dead keys and delays clicks (phase 06). One function, called
//! from both, makes that impossible.
//!
//! Only capture lives here. The window, tray, ambiance player, telemetry and
//! update checker are GUI-only and stay in `main()`.

use crossbeam_channel::Sender;

/// Starts platform input capture, feeding the three channels the audio engine
/// reads.
///
/// Returns immediately; events begin arriving once the platform listener is
/// up. Must be called after `spawn_engine`, so nothing sends into a channel
/// with no receiver.
///
/// `window_focused` is the shared focus flag the macOS (and Windows fallback)
/// hybrid listeners consult to decide which of the two is authoritative.
/// Headless has no window, so it passes a flag that is permanently false -
/// "never focused" is exactly right there: it is the state in which the
/// unfocused listener, the one that works without a window, is the one that
/// reports.
#[cfg_attr(
    any(target_os = "windows", target_os = "linux"),
    allow(
        unused_variables,
        reason = "the Raw Input worker and the evdev listener are focus-independent"
    )
)]
#[cfg(target_os = "linux")]
fn linux_evdev_is_available() -> bool {
    let input_dir = std::path::Path::new("/dev/input");
    let Ok(entries) = std::fs::read_dir(input_dir) else {
        return false;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|file| file.to_str()) else {
            continue;
        };

        if name.starts_with("event") && std::fs::File::open(&path).is_ok() {
            return true;
        }
    }

    false
}

#[cfg(target_os = "linux")]
fn linux_on_x11() -> bool {
    std::env::var_os("DISPLAY").is_some() && std::env::var_os("WAYLAND_DISPLAY").is_none()
}

pub fn start_input_capture_with_focus(
    keyboard_tx: Sender<String>,
    mouse_tx: Sender<String>,
    hotkey_tx: Sender<String>,
    window_focused: std::sync::Arc<std::sync::Mutex<bool>>
) {
    // Linux: evdev reads /dev/input directly, so it does not depend on X or on
    // whether the app window has focus. On X11 only, if that device path cannot
    // be opened at startup (for example a permission problem), we fall back to
    // the older rdev + device_query pair and choose exactly one source so nothing
    // is played twice. Pointer buttons and the Ctrl+Alt+M hotkey come from the
    // same reader in either path.
    #[cfg(target_os = "linux")]
    {
        if linux_evdev_is_available() {
            crate::debug_print!("🎮 Starting evdev input listener (X11 and Wayland)...");
            crate::libs::evdev_input_listener::start_evdev_input_listener(
                keyboard_tx,
                mouse_tx,
                hotkey_tx
            );
            return;
        }

        if linux_on_x11() {
            use crate::libs::focused_input_listener::start_focused_keyboard_listener;
            use crate::libs::input_listener::start_unified_input_listener;

            let fallback_keyboard_tx = keyboard_tx.clone();
            let fallback_mouse_tx = mouse_tx.clone();
            let fallback_hotkey_tx = hotkey_tx.clone();
            let fallback_focus = window_focused;

            crate::debug_print!("🎮 Starting X11 fallback listener pair (rdev + device_query)...");
            start_unified_input_listener(
                fallback_keyboard_tx.clone(),
                fallback_mouse_tx,
                fallback_hotkey_tx,
                Some(fallback_focus.clone()),
                None
            );
            start_focused_keyboard_listener(fallback_keyboard_tx, fallback_focus);
            return;
        }

        crate::debug_print!("🎮 Starting evdev input listener (X11 and Wayland)...");
        crate::libs::evdev_input_listener::start_evdev_input_listener(
            keyboard_tx,
            mouse_tx,
            hotkey_tx
        );
    }

    // Windows: capture runs in a separate worker process using Raw Input, so
    // events arrive regardless of which window has focus. It cannot run in
    // the UI process - tao/wry takes the process-wide Raw Input registration
    // once the webview is built (see rawinput_listener.rs). If the worker
    // cannot be kept alive we fall back to the rdev + device_query hybrid,
    // which works while unfocused only.
    #[cfg(target_os = "windows")]
    {
        use crate::libs::input_listener::start_unified_input_listener;
        use crate::libs::focused_input_listener::start_focused_keyboard_listener;

        let fallback_keyboard_tx = keyboard_tx.clone();
        let fallback_mouse_tx = mouse_tx.clone();
        let fallback_hotkey_tx = hotkey_tx.clone();
        let fallback_focus = window_focused;

        crate::debug_print!("🎮 Starting Raw Input worker process...");
        crate::libs::input_worker_host::start_input_worker_host(
            keyboard_tx,
            mouse_tx,
            hotkey_tx,
            Box::new(move || {
                start_unified_input_listener(
                    fallback_keyboard_tx.clone(),
                    fallback_mouse_tx,
                    fallback_hotkey_tx,
                    Some(fallback_focus.clone()),
                    None
                );
                start_focused_keyboard_listener(fallback_keyboard_tx, fallback_focus);
            })
        );
    }

    // macOS: hybrid approach (rdev + device_query) - rdev handles keyboard
    // when unfocused, device_query when focused.
    #[cfg(target_os = "macos")]
    {
        use crate::libs::input_listener::start_unified_input_listener;
        use crate::libs::focused_input_listener::start_focused_keyboard_listener;

        crate::debug_print!("🎮 Starting unified input listener (unfocused)...");
        start_unified_input_listener(
            keyboard_tx.clone(),
            mouse_tx,
            hotkey_tx,
            Some(window_focused.clone()),
            None
        );

        crate::debug_print!("🎮 Starting focused keyboard listener (focused)...");
        start_focused_keyboard_listener(keyboard_tx, window_focused);
    }
}

/// Starts input capture for a run with no window.
///
/// The focus flag is pinned false: with no window there is nothing to focus,
/// and on the platforms that consult it that selects the listener which does
/// not need one.
pub fn start_input_capture(
    keyboard_tx: Sender<String>,
    mouse_tx: Sender<String>,
    hotkey_tx: Sender<String>
) {
    start_input_capture_with_focus(
        keyboard_tx,
        mouse_tx,
        hotkey_tx,
        std::sync::Arc::new(std::sync::Mutex::new(false))
    );
}

#[cfg(test)]
mod tests {
    /// Returns the text inside the parentheses of `function(...)` in `source`,
    /// paren-balanced so nested calls such as `mouse_tx.clone()` survive.
    fn call_arguments<'a>(source: &'a str, function: &str) -> &'a str {
        let open = source
            .find(&format!("{}(", function))
            .unwrap_or_else(|| panic!("{} must be called", function));
        let rest = &source[open + function.len() + 1..];
        let mut depth = 1usize;
        for (index, byte) in rest.bytes().enumerate() {
            match byte {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return &rest[..index];
                    }
                }
                _ => {}
            }
        }
        panic!("{} is not a closed call", function);
    }

    /// Windows headless must never be routed through rdev as its primary
    /// listener. rdev's low-level hooks are what swallow dead keys and delay
    /// the second click of a double-click (phase 06); the Raw Input worker is
    /// free of both, and remains the only primary path here.
    ///
    /// A source assertion because the alternative is spawning a real worker
    /// process and typing at it. What it pins is structural and worth pinning:
    /// rdev may appear in this file only inside the worker's fallback closure.
    #[test]
    fn windows_capture_starts_the_raw_input_worker_not_rdev() {
        const SOURCE: &str = include_str!("bootstrap.rs");
        let runtime = SOURCE.split("#[cfg(test)]").next().expect("runtime code precedes tests");

        let windows_block = runtime
            .split("#[cfg(target_os = \"windows\")]")
            .nth(1)
            .expect("the Windows branch must exist");

        let worker_start = windows_block
            .find("start_input_worker_host")
            .expect("Windows capture must go through the Raw Input worker");
        let fallback_start = windows_block
            .find("Box::new(move ||")
            .expect("the rdev fallback must be a closure passed to the worker host");

        assert!(
            worker_start < fallback_start,
            "the worker must be started first, with rdev only as its fallback"
        );

        let before_worker = &windows_block[..worker_start];
        assert!(
            !before_worker.contains("start_unified_input_listener("),
            "no rdev listener may be started ahead of the Raw Input worker"
        );
    }

    /// Both launch modes must reach the same wiring.
    #[test]
    fn the_headless_entry_point_delegates_to_the_shared_wiring() {
        const SOURCE: &str = include_str!("bootstrap.rs");
        let runtime = SOURCE.split("#[cfg(test)]").next().expect("runtime code precedes tests");

        let headless_fn = runtime
            .split("pub fn start_input_capture(")
            .nth(1)
            .expect("the headless entry point must exist");

        assert!(
            headless_fn.contains("start_input_capture_with_focus("),
            "headless must not carry its own copy of the platform wiring"
        );
    }

    /// Linux chooses a single input source at startup: evdev when it can open
    /// the event nodes, otherwise X11 falls back to the older rdev +
    /// device_query pair. The key is one choice only, so the same key never
    /// reaches both listeners and the audio engine never hears it twice.
    #[test]
    fn linux_capture_chooses_evdev_or_the_x11_fallback_once() {
        const SOURCE: &str = include_str!("bootstrap.rs");

        assert!(
            SOURCE.contains("linux_evdev_is_available()"),
            "Linux startup must choose evdev only after checking the event nodes"
        );
        assert!(
            SOURCE.contains("linux_on_x11()"),
            "Linux startup must detect X11 before falling back"
        );
        assert!(
            SOURCE.contains("start_unified_input_listener"),
            "Linux X11 fallback must still start the old pair when evdev is blocked"
        );
        assert!(
            SOURCE.contains("start_focused_keyboard_listener"),
            "the focused keyboard listener must stay in the Linux X11 fallback"
        );
    }

    /// Neither Windows nor macOS can be compiled by this suite: both branches
    /// sit behind `cfg(target_os)`, so on Linux they are never built. CI does
    /// cover both platforms - every leg of the packaging matrix runs
    /// `scripts/ci-check.sh` - but a leg only proves that the platform builds
    /// and that its own tests pass. It says nothing about what this branch hands
    /// to the listener, so this guard is what pins that.
    ///
    /// Both must hand the unified listener `None` for the pointer gate. Neither
    /// platform has /dev/input, so rdev is the mouse source there and there is no
    /// evdev listener to hand the gate to - passing one by accident would either
    /// silence the mouse or need a listener that does not exist.
    #[test]
    fn the_platforms_without_evdev_must_pass_no_pointer_gate() {
        const SOURCE: &str = include_str!("bootstrap.rs");
        let runtime = SOURCE.split("#[cfg(test)]").next().expect("runtime code precedes tests");

        for platform in ["windows", "macos"] {
            let block = runtime
                .split(&format!("#[cfg(target_os = \"{platform}\")]"))
                .nth(1)
                .unwrap_or_else(|| panic!("the {platform} branch must exist"));

            let arguments = call_arguments(block, "start_unified_input_listener");
            assert!(
                arguments.trim_end().ends_with("None"),
                "{platform} has no evdev listener, so it must pass None as the pointer gate, got: {arguments}"
            );
        }
    }
}
