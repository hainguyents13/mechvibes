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
/// `window_focused` is the shared focus flag the macOS hybrid listeners, and
/// the X11 fallback used when evdev cannot read any device, consult to decide
/// which of the two is authoritative. Headless has no
/// window, so it passes a flag that is permanently false - "never focused" is
/// exactly right there: it is the state in which the unfocused listener, the
/// one that works without a window, is the one that reports.
#[cfg_attr(
    target_os = "windows",
    allow(unused_variables, reason = "the Raw Input worker is focus-independent")
)]
pub fn start_input_capture_with_focus(
    keyboard_tx: Sender<String>,
    mouse_tx: Sender<String>,
    hotkey_tx: Sender<String>,
    window_focused: std::sync::Arc<std::sync::Mutex<bool>>
) {
    #[cfg(target_os = "linux")]
    {
        use crate::libs::input_listener::start_unified_input_listener;
        use crate::libs::focused_input_listener::start_focused_keyboard_listener;

        let display_server = std::env
            ::var("XDG_SESSION_TYPE")
            .unwrap_or_else(|_| "x11".to_string());
        crate::debug_print!("🔍 Detected display server: {}", display_server);

        // One input source per session, picked once here, so no key or click
        // is ever reported twice.
        if crate::libs::evdev_input_listener::has_readable_input_device() {
            // evdev reads the device nodes directly: no X server and no
            // dependence on focus, the same on X11 and Wayland, and devices
            // plugged in later are picked up. It covers keys, pointer buttons,
            // the wheel and the hotkey, so nothing else runs beside it. On
            // Wayland it is also the only way to hear clicks over
            // Wayland-native windows: rdev's Linux listen() is X11 XRecord,
            // which XWayland only feeds while one of its windows is under the
            // pointer.
            crate::debug_print!("🎮 Starting evdev input listener...");
            crate::libs::evdev_input_listener::start_evdev_input_listener(
                keyboard_tx,
                mouse_tx,
                hotkey_tx
            );
        } else if display_server == "wayland" {
            // No readable device, so keys stay silent on Wayland until the
            // user joins the `input` group. rdev still hears clicks over
            // XWayland windows, so it covers the mouse, with a permanently
            // focused flag that keeps its keyboard branch quiet.
            crate::always_eprint!(
                "⚠️ Cannot read /dev/input: keys stay silent on Wayland until you join the 'input' group (sudo usermod -a -G input $USER, then log out and back in)"
            );
            let always_focused = std::sync::Arc::new(std::sync::Mutex::new(true));
            start_unified_input_listener(
                keyboard_tx,
                mouse_tx,
                hotkey_tx,
                Some(always_focused),
                None
            );
        } else {
            // X11 without access to /dev/input: the older hybrid, which still
            // works there. rdev handles keyboard while unfocused, device_query
            // while focused.
            crate::always_eprint!(
                "ℹ️ Cannot read /dev/input, using the X11 listeners. Join the 'input' group to also pick up devices plugged in while the app runs."
            );
            crate::debug_print!("🎮 Starting unified input listener (X11 mode - unfocused)...");
            start_unified_input_listener(
                keyboard_tx.clone(),
                mouse_tx,
                hotkey_tx,
                Some(window_focused.clone()),
                None
            );

            crate::debug_print!("🎮 Starting focused keyboard listener (X11 mode - focused)...");
            start_focused_keyboard_listener(keyboard_tx, window_focused);
        }
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

    /// Linux runs one input source, picked once: evdev when a device can be
    /// read, alone, with every channel, since it carries keys, clicks, the
    /// wheel and the hotkey; the older listeners only when it cannot. Starting
    /// both would play every keystroke twice.
    ///
    /// A source assertion because the real listeners need /dev/input or an X
    /// server, neither of which a test run has.
    #[test]
    fn linux_runs_evdev_alone_or_the_older_listeners_never_both() {
        const SOURCE: &str = include_str!("bootstrap.rs");
        let runtime = SOURCE.split("#[cfg(test)]").next().expect("runtime code precedes tests");

        let evdev_branch = runtime
            .split("if crate::libs::evdev_input_listener::has_readable_input_device() {")
            .nth(1)
            .expect("Linux must ask whether evdev can read a device")
            .split("} else if display_server == \"wayland\" {")
            .next()
            .expect("the evdev branch must end before the fallbacks");

        let evdev_arguments = call_arguments(evdev_branch, "start_evdev_input_listener");
        for channel in ["keyboard_tx", "mouse_tx", "hotkey_tx"] {
            assert!(
                evdev_arguments.contains(channel),
                "evdev must be handed {channel}, got: {evdev_arguments}"
            );
        }
        assert!(
            !evdev_branch.contains("start_unified_input_listener") &&
                !evdev_branch.contains("start_focused_keyboard_listener"),
            "nothing may run beside evdev, or every key plays twice"
        );

        let wayland_fallback = runtime
            .split("} else if display_server == \"wayland\" {")
            .nth(1)
            .expect("the Wayland fallback must exist")
            .split("} else {")
            .next()
            .expect("the Wayland fallback must end before the X11 one");
        let unified_arguments = call_arguments(wayland_fallback, "start_unified_input_listener");
        assert!(
            unified_arguments.contains("Some(always_focused)"),
            "the Wayland fallback must keep rdev's keyboard branch quiet, got: {unified_arguments}"
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
    /// silence the mouse or need a listener that does not exist. The X11 branch
    /// says the same thing and is compiled here, so it needs no guard.
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
