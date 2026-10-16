use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[cfg(target_os = "linux")]
use crossbeam_channel::Sender;

#[cfg(target_os = "linux")]
pub fn start_evdev_keyboard_listener(
    keyboard_tx: Sender<String>,
    mouse_tx: Sender<String>,
    hotkey_tx: Sender<String>,
    _is_focused: Arc<Mutex<bool>>,
    pointer_capture_active: Arc<std::sync::atomic::AtomicBool>,
) {
    crate::always_print!("🔍 [evdev] start_evdev_keyboard_listener() called - spawning thread");
    thread::spawn(move || {
        use evdev::{Device, EventType, KeyCode, RelativeAxisCode};

        crate::always_print!("🔍 [evdev] Thread started - initializing keyboard listener");
        crate::always_print!("🔍 [evdev] Current user: {:?}", std::env::var("USER"));
        crate::always_print!("🔍 [evdev] Starting Linux keyboard listener (Wayland/X11 compatible)");

        // Track modifier keys for hotkey detection
        let mut ctrl_pressed = false;
        let mut alt_pressed = false;

        // Find all keyboard devices
        let mut keyboards = Vec::new();

        crate::always_print!("🔍 [evdev] Enumerating input devices...");
        let devices: Vec<_> = evdev::enumerate().collect();
        let device_count = devices.len();
        crate::always_print!("🔍 [evdev] Found {} total input devices", device_count);

        if device_count == 0 {
            crate::always_eprint!("❌ [evdev] No devices found - cannot access /dev/input/event* devices");
            crate::always_eprint!("💡 [evdev] Troubleshooting steps:");
            crate::always_eprint!("   1. Check if you're in the 'input' group: groups $USER");
            crate::always_eprint!("   2. Add yourself to input group: sudo usermod -a -G input $USER");
            crate::always_eprint!("   3. Log out and log back in for group changes to take effect");
            crate::always_eprint!("   4. Check /dev/input permissions: ls -la /dev/input/event*");
            return;
        }

        for (path, mut device) in devices {
            // Check if device has keyboard capabilities
            if device.supported_keys().is_some() {
                crate::always_print!("🔍 [evdev] Found keyboard device: {:?} - {}", path.display(), device.name().unwrap_or("Unknown"));

                // A device advertising BTN_LEFT is where pointer clicks come
                // from, so evdev now covers the mouse and the rdev mouse path
                // must stay silent to avoid reporting the same click twice.
                if device.supported_keys().is_some_and(|keys| keys.contains(KeyCode::BTN_LEFT)) {
                    crate::always_print!("🖱️ [evdev] Pointer buttons available on {:?} - gating the rdev mouse path", path.display());
                    pointer_capture_active.store(true, std::sync::atomic::Ordering::Relaxed);
                }

                // Set device to non-blocking mode to prevent blocking on idle devices
                if let Err(e) = device.set_nonblocking(true) {
                    crate::always_eprint!("⚠️ [evdev] Failed to set non-blocking mode for {:?}: {}", path.display(), e);
                }

                keyboards.push(device);
            } else {
                crate::always_print!("🔍 [evdev] Skipping non-keyboard device: {:?}", path.display());
            }
        }

        if keyboards.is_empty() {
            crate::always_eprint!("❌ [evdev] No keyboard devices found among the {} input devices!", device_count);
            crate::always_eprint!("💡 [evdev] This might indicate a permission issue or unusual hardware setup");
            return;
        }

        crate::always_print!("✅ [evdev] Successfully initialized {} keyboard device(s)", keyboards.len());
        crate::always_print!("🔍 [evdev] Starting event monitoring loop...");

        let mut event_count = 0;
        let mut first_event_logged = false;

        // Monitor all keyboards in a loop
        loop {
            for device in &mut keyboards {
                // Fetch events (non-blocking)
                match device.fetch_events() {
                    Ok(events) => {
                        for event in events {
                            if event.event_type() == EventType::KEY {
                                event_count += 1;
                                if !first_event_logged {
                                    crate::always_print!("✅ [evdev] First keyboard event detected!");
                                    first_event_logged = true;
                                }

                                let key_value = event.value();

                                // Mouse buttons arrive as KEY events carrying
                                // BTN_* codes, on the same devices and path as
                                // keystrokes. They are translated first so they
                                // never also travel the keyboard table.
                                if let Some(button_code) = map_evdev_mouse_button(KeyCode(event.code())) {
                                    match key_value {
                                        // Button press (value == 1)
                                        1 => {
                                            if event_count <= 5 {
                                                crate::always_print!("🔍 [evdev] Sending mouse press: {}", button_code);
                                            }
                                            let _ = mouse_tx.send(button_code.to_string());
                                        }
                                        // Button release (value == 0)
                                        0 => {
                                            let _ = mouse_tx.send(format!("UP:{}", button_code));
                                        }
                                        // Ignore auto-repeat (value == 2)
                                        _ => {}
                                    }
                                    continue;
                                }

                                // Convert event code to KeyCode
                                let key = KeyCode(event.code());
                                {
                                    let key_code = map_evdev_keycode(key);
                                    if !key_code.is_empty() {
                                        // Handle key press (value == 1)
                                        if key_value == 1 {
                                            // Track modifier keys for hotkey detection
                                            match key_code {
                                                "ControlLeft" | "ControlRight" => {
                                                    ctrl_pressed = true;
                                                }
                                                "AltLeft" | "AltRight" => {
                                                    alt_pressed = true;
                                                }
                                                "KeyM" => {
                                                    // Check for Ctrl+Alt+M hotkey combination
                                                    if ctrl_pressed && alt_pressed {
                                                        crate::always_print!("🔥 [evdev] Hotkey detected: Ctrl+Alt+M - Toggling global sound");
                                                        let _ = hotkey_tx.send("TOGGLE_SOUND".to_string());
                                                        continue; // Don't process this as a regular key event
                                                    }
                                                }
                                                _ => {}
                                            }

                                            // Send key press event
                                            if event_count <= 5 {
                                                crate::always_print!("🔍 [evdev] Sending key press: {}", key_code);
                                            }
                                            let _ = keyboard_tx.send(key_code.to_string());
                                        }
                                        // Handle key release (value == 0)
                                        else if key_value == 0 {
                                            // Track modifier key releases for hotkey detection
                                            match key_code {
                                                "ControlLeft" | "ControlRight" => {
                                                    ctrl_pressed = false;
                                                }
                                                "AltLeft" | "AltRight" => {
                                                    alt_pressed = false;
                                                }
                                                _ => {}
                                            }

                                            // Send key release event
                                            let _ = keyboard_tx.send(format!("UP:{}", key_code));
                                        }
                                        // Ignore key repeat (value == 2)
                                    }
                                }
                            } else if event.event_type() == EventType::RELATIVE {
                                // Vertical scrolls arrive as REL_WHEEL axis
                                // deltas, not key events, and have no release
                                // counterpart. The engine rate-limits them,
                                // because it is the last gate every source
                                // passes through; a second limit here would use
                                // its own timestamp and drop a scroll the engine
                                // would have played.
                                if event.code() == RelativeAxisCode::REL_WHEEL.0 {
                                    if let Some(wheel_code) = map_wheel_delta(event.value()) {
                                        let _ = mouse_tx.send(wheel_code.to_string());
                                    }
                                }
                            }
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        // No events available, this is normal
                    }
                    Err(e) => {
                        crate::always_eprint!("⚠️ [evdev] Error fetching events: {}", e);
                    }
                }
            }
            
            // Small sleep to prevent busy-waiting
            thread::sleep(Duration::from_millis(10));
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
/// strings returned must stay identical to the ones `map_button_to_code`
/// produces for rdev's buttons, because soundpack configs key off them.
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
    use evdev::KeyCode;

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
    /// to land on the exact codes `map_button_to_code` emits: the soundpack
    /// configs key off those strings, not off evdev's BTN_* names.
    #[test]
    fn pointer_buttons_map_to_the_same_codes_rdev_produces() {
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
