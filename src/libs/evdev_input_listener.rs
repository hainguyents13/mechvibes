//! Linux input capture straight from the kernel's evdev nodes (`/dev/input/event*`).
//!
//! The Linux input path whenever the event nodes are readable, which takes
//! membership in the `input` group. It needs no X server and does not depend
//! on which window has focus, so it behaves the same on X11, Wayland and a
//! bare VT, and one reader covers keys, pointer buttons, the wheel and the
//! Ctrl+Alt+M hotkey. Without access, `bootstrap` falls back to the rdev and
//! device_query listeners, which on X11 still work.
//!
//! The thread opens every readable event node that reports keys or buttons,
//! polls them without blocking, and rescans `/dev/input` every few seconds so
//! devices plugged in after startup are picked up and unplugged ones dropped.

#[cfg(target_os = "linux")]
use crossbeam_channel::Sender;
#[cfg(target_os = "linux")]
use std::collections::HashSet;
#[cfg(target_os = "linux")]
use std::path::PathBuf;
#[cfg(target_os = "linux")]
use std::thread;
#[cfg(target_os = "linux")]
use std::time::{ Duration, Instant };

/// Where the kernel exposes input devices.
#[cfg(target_os = "linux")]
const INPUT_DIR: &str = "/dev/input";

/// How often `/dev/input` is rescanned for devices that appeared or vanished.
#[cfg(target_os = "linux")]
const RESCAN_INTERVAL: Duration = Duration::from_secs(2);

/// Idle sleep between polling passes. Devices are non-blocking, so this is the
/// worst-case added latency before a keystroke is noticed.
#[cfg(target_os = "linux")]
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Turns raw evdev `KEY` events into the strings the audio engine expects, and
/// owns the cross-device hotkey state (Ctrl+Alt+M).
///
/// Kept separate from the device plumbing so the routing rules can be tested
/// without any hardware.
#[cfg(target_os = "linux")]
struct EventRouter {
    keyboard_tx: Sender<String>,
    mouse_tx: Sender<String>,
    hotkey_tx: Sender<String>,
    ctrl_pressed: bool,
    alt_pressed: bool,
}

#[cfg(target_os = "linux")]
impl EventRouter {
    fn new(keyboard_tx: Sender<String>, mouse_tx: Sender<String>, hotkey_tx: Sender<String>) -> Self {
        Self {
            keyboard_tx,
            mouse_tx,
            hotkey_tx,
            ctrl_pressed: false,
            alt_pressed: false,
        }
    }

    /// Routes one `REL_WHEEL` delta. Scrolls have no release counterpart, and
    /// the engine rate-limits them, because it is the last gate every source
    /// passes through; a second limit here would use its own timestamp and
    /// drop a scroll the engine would have played.
    fn handle_wheel_event(&mut self, value: i32) {
        if let Some(wheel_code) = map_wheel_delta(value) {
            let _ = self.mouse_tx.send(wheel_code.to_string());
        }
    }

    /// Forgets the held modifiers. Called when a device goes away, since its
    /// releases will never arrive: a Ctrl still counted as held would turn the
    /// next Alt+M into a sound toggle.
    fn release_modifiers(&mut self) {
        self.ctrl_pressed = false;
        self.alt_pressed = false;
    }

    /// Routes one `EventType::KEY` event. `value` is 1 for press, 0 for
    /// release and 2 for auto-repeat, which is ignored.
    fn handle_key_event(&mut self, code: u16, value: i32) {
        // Mouse buttons arrive as KEY events carrying BTN_* codes, on the same
        // devices and path as keystrokes. They are translated first so they
        // never also travel the keyboard table.
        if let Some(button_code) = map_evdev_mouse_button(evdev::KeyCode(code)) {
            match value {
                1 => {
                    let _ = self.mouse_tx.send(button_code.to_string());
                }
                0 => {
                    let _ = self.mouse_tx.send(format!("UP:{}", button_code));
                }
                _ => {}
            }
            return;
        }

        let key_code = map_evdev_keycode(evdev::KeyCode(code));
        if key_code.is_empty() {
            return;
        }

        match value {
            1 => {
                match key_code {
                    "ControlLeft" | "ControlRight" => {
                        self.ctrl_pressed = true;
                    }
                    "AltLeft" | "AltRight" => {
                        self.alt_pressed = true;
                    }
                    "KeyM" if self.ctrl_pressed && self.alt_pressed => {
                        crate::always_print!("🔥 [evdev] Hotkey detected: Ctrl+Alt+M - Toggling global sound");
                        let _ = self.hotkey_tx.send("TOGGLE_SOUND".to_string());
                        // Not a regular key event.
                        return;
                    }
                    _ => {}
                }
                let _ = self.keyboard_tx.send(key_code.to_string());
            }
            0 => {
                match key_code {
                    "ControlLeft" | "ControlRight" => {
                        self.ctrl_pressed = false;
                    }
                    "AltLeft" | "AltRight" => {
                        self.alt_pressed = false;
                    }
                    _ => {}
                }
                let _ = self.keyboard_tx.send(format!("UP:{}", key_code));
            }
            // Auto-repeat (value == 2) is not a new keystroke.
            _ => {}
        }
    }
}

/// An opened, non-blocking evdev node.
#[cfg(target_os = "linux")]
struct OpenDevice {
    path: PathBuf,
    device: evdev::Device,
}

/// Which `/dev/input/event*` nodes have been dealt with, so a rescan only
/// touches new ones.
#[cfg(target_os = "linux")]
#[derive(Default)]
struct DeviceScanState {
    /// Nodes that were opened, or opened and found to carry no keys/buttons.
    seen: HashSet<PathBuf>,
    /// Nodes whose open failed and was already reported, to log each once.
    warned: HashSet<PathBuf>,
    /// Set once an unreadable `/dev/input` has been reported, cleared when it
    /// becomes readable again.
    dir_error_logged: bool,
}

/// Lists the `event*` nodes currently under `/dev/input`.
#[cfg(target_os = "linux")]
fn list_event_nodes() -> std::io::Result<HashSet<PathBuf>> {
    let mut nodes = HashSet::new();
    for entry in std::fs::read_dir(INPUT_DIR)? {
        let entry = entry?;
        let is_event_node = entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with("event"));
        if is_event_node {
            nodes.insert(entry.path());
        }
    }
    Ok(nodes)
}

/// Opens every node not seen before and appends the ones that report keys or
/// buttons to `devices`. Returns how many were added.
///
/// A node that fails to open is not marked seen, so it is retried on the next
/// scan: udev creates a node before it applies the `input` group permissions,
/// so a hot-plugged keyboard is usually unreadable for a moment.
#[cfg(target_os = "linux")]
fn scan_devices(state: &mut DeviceScanState, devices: &mut Vec<OpenDevice>) -> usize {
    let nodes = match list_event_nodes() {
        Ok(nodes) => nodes,
        Err(error) => {
            if !state.dir_error_logged {
                state.dir_error_logged = true;
                crate::always_eprint!("❌ [evdev] Cannot read {}: {}", INPUT_DIR, error);
            }
            return 0;
        }
    };
    state.dir_error_logged = false;

    // Forget nodes that went away, so the same path is picked up again if the
    // device is plugged back in.
    state.seen.retain(|path| nodes.contains(path));
    state.warned.retain(|path| nodes.contains(path));

    let mut added = 0;
    let mut fresh: Vec<PathBuf> = nodes.into_iter().filter(|path| !state.seen.contains(path)).collect();
    fresh.sort();

    for path in fresh {
        match evdev::Device::open(&path) {
            Ok(mut device) => {
                state.warned.remove(&path);

                // Keyboards and mice both report keys/buttons; anything else
                // (accelerometers, lid switches) has nothing to play a sound for.
                if device.supported_keys().is_none() {
                    state.seen.insert(path);
                    continue;
                }

                // Non-blocking so one idle device never stalls the others.
                // Not marked seen on failure, so the next scan tries again.
                if let Err(error) = device.set_nonblocking(true) {
                    crate::always_eprint!("⚠️ [evdev] Failed to set non-blocking mode for {:?}: {}", path.display(), error);
                    continue;
                }
                state.seen.insert(path.clone());

                crate::always_print!(
                    "🔍 [evdev] Opened input device {:?} - {}",
                    path.display(),
                    device.name().unwrap_or("Unknown")
                );
                devices.push(OpenDevice { path, device });
                added += 1;
            }
            Err(error) => {
                if state.warned.insert(path.clone()) {
                    crate::always_eprint!("⚠️ [evdev] Cannot open {:?}: {}", path.display(), error);
                }
            }
        }
    }

    added
}

/// Whether at least one keyboard or pointer under `/dev/input` can be opened.
///
/// `bootstrap` asks this once, at startup, to pick the one input source the
/// session uses: evdev when it can read something, the older listeners when it
/// cannot, never both, so no key is played twice.
#[cfg(target_os = "linux")]
pub fn has_readable_input_device() -> bool {
    let Ok(nodes) = list_event_nodes() else {
        return false;
    };
    nodes
        .iter()
        .filter_map(|path| evdev::Device::open(path).ok())
        .any(|device| device.supported_keys().is_some())
}

/// Starts reading keyboard and pointer-button events from `/dev/input`.
///
/// Returns immediately; the reader runs on its own thread for the life of the
/// process. Needs read access to the event nodes, i.e. membership in the
/// `input` group.
#[cfg(target_os = "linux")]
pub fn start_evdev_input_listener(
    keyboard_tx: Sender<String>,
    mouse_tx: Sender<String>,
    hotkey_tx: Sender<String>
) {
    crate::always_print!("🔍 [evdev] start_evdev_input_listener() called - spawning thread");
    thread::spawn(move || {
        crate::always_print!("🔍 [evdev] Starting Linux input listener (reads /dev/input directly, X11 and Wayland)");

        let mut router = EventRouter::new(keyboard_tx, mouse_tx, hotkey_tx);
        let mut state = DeviceScanState::default();
        let mut devices: Vec<OpenDevice> = Vec::new();

        scan_devices(&mut state, &mut devices);
        if devices.is_empty() {
            crate::always_eprint!("❌ [evdev] No readable keyboard devices found under {}", INPUT_DIR);
            crate::always_eprint!("💡 [evdev] Troubleshooting steps:");
            crate::always_eprint!("   1. Check if you're in the 'input' group: groups $USER");
            crate::always_eprint!("   2. Add yourself to input group: sudo usermod -a -G input $USER");
            crate::always_eprint!("   3. Log out and log back in for group changes to take effect");
            crate::always_eprint!("   4. Check /dev/input permissions: ls -la /dev/input/event*");
            crate::always_eprint!("   Still watching for devices that appear later.");
        } else {
            crate::always_print!("✅ [evdev] Successfully initialized {} input device(s)", devices.len());
        }

        let mut first_event_logged = false;
        let mut last_scan = Instant::now();

        loop {
            if last_scan.elapsed() >= RESCAN_INTERVAL {
                last_scan = Instant::now();
                let added = scan_devices(&mut state, &mut devices);
                if added > 0 {
                    crate::always_print!("🔍 [evdev] {} new input device(s), {} total", added, devices.len());
                }
            }

            // Indices of devices that went away during this pass.
            let mut gone: Vec<usize> = Vec::new();

            for (index, open) in devices.iter_mut().enumerate() {
                match open.device.fetch_events() {
                    Ok(events) => {
                        for event in events {
                            if event.event_type() == evdev::EventType::KEY {
                                if !first_event_logged {
                                    crate::always_print!("✅ [evdev] First keyboard event detected!");
                                    first_event_logged = true;
                                }
                                router.handle_key_event(event.code(), event.value());
                            } else if
                                event.event_type() == evdev::EventType::RELATIVE &&
                                event.code() == evdev::RelativeAxisCode::REL_WHEEL.0
                            {
                                router.handle_wheel_event(event.value());
                            }
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        // No events available, this is normal
                    }
                    Err(error) if error.raw_os_error() == Some(libc::ENODEV) => {
                        // Unplugged.
                        gone.push(index);
                    }
                    Err(error) => {
                        crate::always_eprint!("⚠️ [evdev] Error fetching events from {:?}: {}", open.path.display(), error);
                        gone.push(index);
                    }
                }
            }

            if !gone.is_empty() {
                router.release_modifiers();
            }
            // Highest index first so swap_remove leaves the rest in place.
            for index in gone.into_iter().rev() {
                let removed = devices.swap_remove(index);
                crate::always_print!("🔍 [evdev] Input device {:?} went away", removed.path.display());
                // Let the next scan reopen it if the node still exists.
                state.seen.remove(&removed.path);
            }

            // Small sleep to prevent busy-waiting
            thread::sleep(POLL_INTERVAL);
        }
    });
}

/// Translates a `REL_WHEEL` axis delta into a wheel button code.
///
/// Positive is up (away from the user) and negative is down, per the kernel's
/// `REL_WHEEL` convention; a zero delta carries no direction and is ignored.
/// The strings are the ones the soundpacks use for these two scrolls, so a
/// different spelling here would leave them silent until every pack was updated.
#[cfg(target_os = "linux")]
fn map_wheel_delta(value: i32) -> Option<&'static str> {
    if value > 0 {
        Some("MouseWheelUp")
    } else if value < 0 {
        Some("MouseWheelDown")
    } else {
        None
    }
}

/// Translates the pointer buttons that arrive as `EventType::KEY` events with
/// `BTN_*` codes.
///
/// Deliberately separate from `map_evdev_keycode`: these are not keys. The
/// strings returned follow the names the Windows Raw Input listener produces
/// for the buttons it can report, which is what the soundpack configs were
/// written against. rdev on X11 is not the reference: there the side buttons
/// arrive as unknown codes, so Back comes through as `Mouse8` and Forward is
/// dropped. Mice with more buttons than Raw Input can report keep going in the
/// same series.
#[cfg(target_os = "linux")]
fn map_evdev_mouse_button(key: evdev::KeyCode) -> Option<&'static str> {
    use evdev::KeyCode;

    match key {
        KeyCode::BTN_LEFT => Some("MouseLeft"),
        KeyCode::BTN_RIGHT => Some("MouseRight"),
        KeyCode::BTN_MIDDLE => Some("MouseMiddle"),
        KeyCode::BTN_SIDE => Some("Mouse4"),
        KeyCode::BTN_EXTRA => Some("Mouse5"),
        KeyCode::BTN_FORWARD => Some("Mouse6"),
        KeyCode::BTN_BACK => Some("Mouse7"),
        KeyCode::BTN_TASK => Some("Mouse8"),
        // BTN_TOUCH is finger contact on a touchpad, not a click, and every
        // other BTN_* code stays unmapped.
        _ => None,
    }
}

#[cfg(target_os = "linux")]
fn map_evdev_keycode(key: evdev::KeyCode) -> &'static str {
    use evdev::KeyCode;

    match key {
        // Letters
        KeyCode::KEY_A => "KeyA", KeyCode::KEY_B => "KeyB", KeyCode::KEY_C => "KeyC", KeyCode::KEY_D => "KeyD",
        KeyCode::KEY_E => "KeyE", KeyCode::KEY_F => "KeyF", KeyCode::KEY_G => "KeyG", KeyCode::KEY_H => "KeyH",
        KeyCode::KEY_I => "KeyI", KeyCode::KEY_J => "KeyJ", KeyCode::KEY_K => "KeyK", KeyCode::KEY_L => "KeyL",
        KeyCode::KEY_M => "KeyM", KeyCode::KEY_N => "KeyN", KeyCode::KEY_O => "KeyO", KeyCode::KEY_P => "KeyP",
        KeyCode::KEY_Q => "KeyQ", KeyCode::KEY_R => "KeyR", KeyCode::KEY_S => "KeyS", KeyCode::KEY_T => "KeyT",
        KeyCode::KEY_U => "KeyU", KeyCode::KEY_V => "KeyV", KeyCode::KEY_W => "KeyW", KeyCode::KEY_X => "KeyX",
        KeyCode::KEY_Y => "KeyY", KeyCode::KEY_Z => "KeyZ",

        // Numbers
        KeyCode::KEY_1 => "Digit1", KeyCode::KEY_2 => "Digit2", KeyCode::KEY_3 => "Digit3", KeyCode::KEY_4 => "Digit4",
        KeyCode::KEY_5 => "Digit5", KeyCode::KEY_6 => "Digit6", KeyCode::KEY_7 => "Digit7", KeyCode::KEY_8 => "Digit8",
        KeyCode::KEY_9 => "Digit9", KeyCode::KEY_0 => "Digit0",

        // Function keys
        KeyCode::KEY_F1 => "F1", KeyCode::KEY_F2 => "F2", KeyCode::KEY_F3 => "F3", KeyCode::KEY_F4 => "F4",
        KeyCode::KEY_F5 => "F5", KeyCode::KEY_F6 => "F6", KeyCode::KEY_F7 => "F7", KeyCode::KEY_F8 => "F8",
        KeyCode::KEY_F9 => "F9", KeyCode::KEY_F10 => "F10", KeyCode::KEY_F11 => "F11", KeyCode::KEY_F12 => "F12",

        // Special keys
        KeyCode::KEY_SPACE => "Space",
        KeyCode::KEY_ENTER => "Enter",
        KeyCode::KEY_BACKSPACE => "Backspace",
        KeyCode::KEY_TAB => "Tab",
        KeyCode::KEY_ESC => "Escape",
        KeyCode::KEY_CAPSLOCK => "CapsLock",
        KeyCode::KEY_LEFTSHIFT => "ShiftLeft",
        KeyCode::KEY_RIGHTSHIFT => "ShiftRight",
        KeyCode::KEY_LEFTCTRL => "ControlLeft",
        KeyCode::KEY_RIGHTCTRL => "ControlRight",
        KeyCode::KEY_LEFTALT => "AltLeft",
        KeyCode::KEY_RIGHTALT => "AltRight",
        KeyCode::KEY_LEFTMETA => "MetaLeft",
        KeyCode::KEY_RIGHTMETA => "MetaRight",

        // Arrow keys
        KeyCode::KEY_UP => "ArrowUp",
        KeyCode::KEY_DOWN => "ArrowDown",
        KeyCode::KEY_LEFT => "ArrowLeft",
        KeyCode::KEY_RIGHT => "ArrowRight",

        // Editing keys
        KeyCode::KEY_INSERT => "Insert",
        KeyCode::KEY_DELETE => "Delete",
        KeyCode::KEY_HOME => "Home",
        KeyCode::KEY_END => "End",
        KeyCode::KEY_PAGEUP => "PageUp",
        KeyCode::KEY_PAGEDOWN => "PageDown",

        // Punctuation
        KeyCode::KEY_MINUS => "Minus",
        KeyCode::KEY_EQUAL => "Equal",
        KeyCode::KEY_LEFTBRACE => "BracketLeft",
        KeyCode::KEY_RIGHTBRACE => "BracketRight",
        KeyCode::KEY_BACKSLASH => "Backslash",
        KeyCode::KEY_SEMICOLON => "Semicolon",
        KeyCode::KEY_APOSTROPHE => "Quote",
        KeyCode::KEY_GRAVE => "Backquote",
        KeyCode::KEY_COMMA => "Comma",
        KeyCode::KEY_DOT => "Period",
        KeyCode::KEY_SLASH => "Slash",
        
        // Numpad and the system/edition keys around it.
        //
        // The strings are copied from `map_key_to_code` in `input_listener.rs`
        // deliberately: the soundpacks and the engine match on these exact
        // names, so a new spelling here would leave the key silent until every
        // pack was updated. The X11 path emits `Numpad0`, `NumpadEnter`,
        // `NumLock`, `PrintScreen` and so on, and this table has to agree with
        // it.
        KeyCode::KEY_KP0 => "Numpad0",
        KeyCode::KEY_KP1 => "Numpad1",
        KeyCode::KEY_KP2 => "Numpad2",
        KeyCode::KEY_KP3 => "Numpad3",
        KeyCode::KEY_KP4 => "Numpad4",
        KeyCode::KEY_KP5 => "Numpad5",
        KeyCode::KEY_KP6 => "Numpad6",
        KeyCode::KEY_KP7 => "Numpad7",
        KeyCode::KEY_KP8 => "Numpad8",
        KeyCode::KEY_KP9 => "Numpad9",
        KeyCode::KEY_KPENTER => "NumpadEnter",
        KeyCode::KEY_KPPLUS => "NumpadAdd",
        KeyCode::KEY_KPMINUS => "NumpadSubtract",
        KeyCode::KEY_KPASTERISK => "NumpadMultiply",
        KeyCode::KEY_KPSLASH => "NumpadDivide",
        KeyCode::KEY_KPDOT => "NumpadDecimal",
        KeyCode::KEY_NUMLOCK => "NumLock",
        KeyCode::KEY_SCROLLLOCK => "ScrollLock",
        KeyCode::KEY_SYSRQ => "PrintScreen",
        KeyCode::KEY_PAUSE => "Pause",
        KeyCode::KEY_102ND => "IntlBackslash",

        _ => "",
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use crossbeam_channel::{ unbounded, Receiver };
    use evdev::KeyCode;

    /// A router wired to three channels the test can read back.
    fn router() -> (EventRouter, Receiver<String>, Receiver<String>, Receiver<String>) {
        let (keyboard_tx, keyboard_rx) = unbounded();
        let (mouse_tx, mouse_rx) = unbounded();
        let (hotkey_tx, hotkey_rx) = unbounded();
        (EventRouter::new(keyboard_tx, mouse_tx, hotkey_tx), keyboard_rx, mouse_rx, hotkey_rx)
    }

    fn drain(rx: &Receiver<String>) -> Vec<String> {
        rx.try_iter().collect()
    }

    /// Press and release must both reach the engine, with the `UP:` prefix on
    /// the release, and auto-repeat must not count as a new keystroke.
    #[test]
    fn key_press_release_and_repeat_are_routed() {
        let (mut router, keyboard_rx, mouse_rx, hotkey_rx) = router();

        router.handle_key_event(KeyCode::KEY_A.0, 1);
        router.handle_key_event(KeyCode::KEY_A.0, 2);
        router.handle_key_event(KeyCode::KEY_A.0, 0);

        assert_eq!(drain(&keyboard_rx), vec!["KeyA", "UP:KeyA"]);
        assert!(drain(&mouse_rx).is_empty());
        assert!(drain(&hotkey_rx).is_empty());
    }

    /// Clicks travel the mouse channel and never the keyboard one.
    #[test]
    fn mouse_buttons_go_to_the_mouse_channel() {
        let (mut router, keyboard_rx, mouse_rx, _hotkey_rx) = router();

        router.handle_key_event(KeyCode::BTN_LEFT.0, 1);
        router.handle_key_event(KeyCode::BTN_LEFT.0, 0);

        assert_eq!(drain(&mouse_rx), vec!["MouseLeft", "UP:MouseLeft"]);
        assert!(drain(&keyboard_rx).is_empty());
    }

    /// Codes with no table entry are dropped instead of being sent as "".
    #[test]
    fn unmapped_keys_are_dropped() {
        let (mut router, keyboard_rx, mouse_rx, _hotkey_rx) = router();

        router.handle_key_event(KeyCode::KEY_MUTE.0, 1);
        router.handle_key_event(KeyCode::BTN_TOUCH.0, 1);

        assert!(drain(&keyboard_rx).is_empty());
        assert!(drain(&mouse_rx).is_empty());
    }

    /// Ctrl+Alt+M toggles sound and is swallowed, even when the modifiers come
    /// from different devices (the router state is shared across them).
    #[test]
    fn ctrl_alt_m_fires_the_hotkey_and_is_not_played() {
        let (mut router, keyboard_rx, _mouse_rx, hotkey_rx) = router();

        router.handle_key_event(KeyCode::KEY_LEFTCTRL.0, 1);
        router.handle_key_event(KeyCode::KEY_RIGHTALT.0, 1);
        router.handle_key_event(KeyCode::KEY_M.0, 1);

        assert_eq!(drain(&hotkey_rx), vec!["TOGGLE_SOUND"]);
        assert_eq!(drain(&keyboard_rx), vec!["ControlLeft", "AltRight"]);
    }

    /// M alone, or with only one modifier held, is an ordinary key; releasing a
    /// modifier disarms the hotkey.
    #[test]
    fn m_without_both_modifiers_is_a_plain_key() {
        let (mut router, keyboard_rx, _mouse_rx, hotkey_rx) = router();

        router.handle_key_event(KeyCode::KEY_M.0, 1);
        router.handle_key_event(KeyCode::KEY_LEFTCTRL.0, 1);
        router.handle_key_event(KeyCode::KEY_LEFTALT.0, 1);
        router.handle_key_event(KeyCode::KEY_LEFTALT.0, 0);
        router.handle_key_event(KeyCode::KEY_M.0, 1);

        assert!(drain(&hotkey_rx).is_empty());
        let keys = drain(&keyboard_rx);
        assert_eq!(keys.iter().filter(|key| key.as_str() == "KeyM").count(), 2);
    }

    /// Scrolls travel the mouse channel as a press with no release, and a zero
    /// delta plays nothing.
    #[test]
    fn wheel_deltas_go_to_the_mouse_channel() {
        let (mut router, keyboard_rx, mouse_rx, _hotkey_rx) = router();

        router.handle_wheel_event(1);
        router.handle_wheel_event(-2);
        router.handle_wheel_event(0);

        assert_eq!(drain(&mouse_rx), vec!["MouseWheelUp", "MouseWheelDown"]);
        assert!(drain(&keyboard_rx).is_empty());
    }

    /// A Ctrl and Alt held on a keyboard that was then unplugged never send
    /// their release, so they must not stay armed for the next M.
    #[test]
    fn released_modifiers_do_not_arm_the_hotkey() {
        let (mut router, keyboard_rx, _mouse_rx, hotkey_rx) = router();

        router.handle_key_event(KeyCode::KEY_LEFTCTRL.0, 1);
        router.handle_key_event(KeyCode::KEY_LEFTALT.0, 1);
        router.release_modifiers();
        router.handle_key_event(KeyCode::KEY_M.0, 1);

        assert!(drain(&hotkey_rx).is_empty());
        assert!(drain(&keyboard_rx).contains(&"KeyM".to_string()));
    }

    /// REL_WHEEL deltas must land on the wheel codes the soundpacks define;
    /// positive is up, negative is down, and zero is a no-op, never a sound.
    #[test]
    fn relative_wheel_deltas_map_to_the_wheel_codes() {
        assert_eq!(map_wheel_delta(1), Some("MouseWheelUp"));
        assert_eq!(map_wheel_delta(3), Some("MouseWheelUp"));
        assert_eq!(map_wheel_delta(-1), Some("MouseWheelDown"));
        assert_eq!(map_wheel_delta(-3), Some("MouseWheelDown"));
        assert_eq!(map_wheel_delta(0), None);
    }

    /// Pointer buttons ride the keyboard event path, so their translation has
    /// to land on the names the soundpack configs key off. The first five are
    /// the ones the Windows listener publishes; the rest continue that series
    /// for mice with more buttons than it can report.
    #[test]
    fn pointer_buttons_map_to_the_names_the_soundpacks_use() {
        let cases = [
            (KeyCode::BTN_LEFT, "MouseLeft"),
            (KeyCode::BTN_RIGHT, "MouseRight"),
            (KeyCode::BTN_MIDDLE, "MouseMiddle"),
            (KeyCode::BTN_SIDE, "Mouse4"),
            (KeyCode::BTN_EXTRA, "Mouse5"),
            (KeyCode::BTN_FORWARD, "Mouse6"),
            (KeyCode::BTN_BACK, "Mouse7"),
            (KeyCode::BTN_TASK, "Mouse8"),
        ];

        for (key, expected) in cases {
            assert_eq!(
                map_evdev_mouse_button(key),
                Some(expected),
                "{:?} must map to {}",
                key,
                expected
            );
        }
    }

    /// The numpad group reaches the keyboard path as KEY events carrying
    /// `KEY_KP*` codes, and this table used to drop all of them, which made
    /// every numpad key silent on Wayland while X11 worked. The names have to
    /// match what `map_key_to_code` emits, because the soundpacks key off them.
    #[test]
    fn numpad_keys_map_to_the_same_codes_x11_emits() {
        let cases = [
            (KeyCode::KEY_KP0, "Numpad0"),
            (KeyCode::KEY_KP5, "Numpad5"),
            (KeyCode::KEY_KP9, "Numpad9"),
            (KeyCode::KEY_KPENTER, "NumpadEnter"),
            (KeyCode::KEY_KPPLUS, "NumpadAdd"),
            (KeyCode::KEY_KPMINUS, "NumpadSubtract"),
            (KeyCode::KEY_KPASTERISK, "NumpadMultiply"),
            (KeyCode::KEY_KPSLASH, "NumpadDivide"),
            (KeyCode::KEY_KPDOT, "NumpadDecimal"),
            (KeyCode::KEY_NUMLOCK, "NumLock"),
            (KeyCode::KEY_SCROLLLOCK, "ScrollLock"),
            (KeyCode::KEY_SYSRQ, "PrintScreen"),
            (KeyCode::KEY_PAUSE, "Pause"),
            (KeyCode::KEY_102ND, "IntlBackslash"),
        ];

        for (key, expected) in cases {
            assert_eq!(
                map_evdev_keycode(key),
                expected,
                "{:?} must map to {}",
                key,
                expected
            );
        }
    }

    /// Only real buttons count. A touchpad reporting finger contact, or any
    /// keyboard key, must keep travelling the keyboard path instead.
    #[test]
    fn touchpad_contact_and_keyboard_keys_are_not_clicks() {
        assert_eq!(
            map_evdev_mouse_button(KeyCode::BTN_TOUCH),
            None,
            "BTN_TOUCH is finger contact on a touchpad, not a click"
        );
        assert_eq!(
            map_evdev_mouse_button(KeyCode::KEY_A),
            None,
            "keyboard keys must keep travelling the keyboard path"
        );
    }

    /// The numpad group must not swallow the mouse buttons: these are two
    /// different groups that happen to share the KEY event type.
    #[test]
    fn the_numpad_group_did_not_swallow_the_mouse_buttons() {
        assert_eq!(map_evdev_keycode(KeyCode::BTN_LEFT), "");
    }
}
