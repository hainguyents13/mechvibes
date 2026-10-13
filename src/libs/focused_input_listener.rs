use crossbeam_channel::Sender;
use device_query::{ DeviceQuery, DeviceState, Keycode };
use std::collections::HashSet;
use std::sync::{ Arc, Mutex };
use std::thread;
use std::time::Duration;

#[cfg(target_os = "macos")]
fn macos_device_state_with_permission_checks(
    is_trusted: impl FnOnce() -> bool,
    request_trust: impl FnOnce() -> bool,
) -> Option<DeviceState> {
    if is_trusted() || request_trust() {
        // DeviceState is a stateless handle on macOS. Constructing it directly
        // avoids DeviceState::new(), which always performs another prompted AX
        // check and panics when access has not been granted yet.
        Some(DeviceState)
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
fn macos_device_state() -> Option<DeviceState> {
    use macos_accessibility_client::accessibility::{
        application_is_trusted,
        application_is_trusted_with_prompt,
    };

    macos_device_state_with_permission_checks(
        application_is_trusted,
        application_is_trusted_with_prompt,
    )
}

/// Maps device_query Keycode to our standardized key code format (same as rdev)
fn map_device_query_keycode(key: Keycode) -> &'static str {
    match key {
        // Letters
        Keycode::A => "KeyA",
        Keycode::B => "KeyB",
        Keycode::C => "KeyC",
        Keycode::D => "KeyD",
        Keycode::E => "KeyE",
        Keycode::F => "KeyF",
        Keycode::G => "KeyG",
        Keycode::H => "KeyH",
        Keycode::I => "KeyI",
        Keycode::J => "KeyJ",
        Keycode::K => "KeyK",
        Keycode::L => "KeyL",
        Keycode::M => "KeyM",
        Keycode::N => "KeyN",
        Keycode::O => "KeyO",
        Keycode::P => "KeyP",
        Keycode::Q => "KeyQ",
        Keycode::R => "KeyR",
        Keycode::S => "KeyS",
        Keycode::T => "KeyT",
        Keycode::U => "KeyU",
        Keycode::V => "KeyV",
        Keycode::W => "KeyW",
        Keycode::X => "KeyX",
        Keycode::Y => "KeyY",
        Keycode::Z => "KeyZ",

        // Numbers
        Keycode::Key0 => "Digit0",
        Keycode::Key1 => "Digit1",
        Keycode::Key2 => "Digit2",
        Keycode::Key3 => "Digit3",
        Keycode::Key4 => "Digit4",
        Keycode::Key5 => "Digit5",
        Keycode::Key6 => "Digit6",
        Keycode::Key7 => "Digit7",
        Keycode::Key8 => "Digit8",
        Keycode::Key9 => "Digit9",

        // Special keys
        Keycode::Space => "Space",
        Keycode::Backspace => "Backspace",
        Keycode::Enter => "Enter",
        Keycode::Tab => "Tab",
        Keycode::Escape => "Escape",
        Keycode::Delete => "Delete",
        Keycode::Insert => "Insert",

        // Modifiers
        Keycode::LShift => "ShiftLeft",
        Keycode::RShift => "ShiftRight",
        Keycode::LControl => "ControlLeft",
        Keycode::RControl => "ControlRight",
        Keycode::LAlt => "AltLeft",
        Keycode::RAlt => "AltRight",
        Keycode::LMeta => "MetaLeft",
        Keycode::RMeta => "MetaRight",

        // Arrow keys
        Keycode::Up => "ArrowUp",
        Keycode::Down => "ArrowDown",
        Keycode::Left => "ArrowLeft",
        Keycode::Right => "ArrowRight",

        // Navigation
        Keycode::Home => "Home",
        Keycode::End => "End",
        Keycode::PageUp => "PageUp",
        Keycode::PageDown => "PageDown",

        // Function keys
        Keycode::F1 => "F1",
        Keycode::F2 => "F2",
        Keycode::F3 => "F3",
        Keycode::F4 => "F4",
        Keycode::F5 => "F5",
        Keycode::F6 => "F6",
        Keycode::F7 => "F7",
        Keycode::F8 => "F8",
        Keycode::F9 => "F9",
        Keycode::F10 => "F10",
        Keycode::F11 => "F11",
        Keycode::F12 => "F12",

        // Punctuation
        Keycode::Minus => "Minus",
        Keycode::Equal => "Equal",
        Keycode::LeftBracket => "BracketLeft",
        Keycode::RightBracket => "BracketRight",
        Keycode::BackSlash => "Backslash",
        Keycode::Semicolon => "Semicolon",
        Keycode::Apostrophe => "Quote",
        Keycode::Grave => "Backquote",
        Keycode::Comma => "Comma",
        Keycode::Dot => "Period",
        Keycode::Slash => "Slash",

        // Numpad
        Keycode::Numpad0 => "Numpad0",
        Keycode::Numpad1 => "Numpad1",
        Keycode::Numpad2 => "Numpad2",
        Keycode::Numpad3 => "Numpad3",
        Keycode::Numpad4 => "Numpad4",
        Keycode::Numpad5 => "Numpad5",
        Keycode::Numpad6 => "Numpad6",
        Keycode::Numpad7 => "Numpad7",
        Keycode::Numpad8 => "Numpad8",
        Keycode::Numpad9 => "Numpad9",

        _ => "",
    }
}

/// Start the focused keyboard listener (uses device_query polling)
/// This listener is ONLY active when the window is focused
pub fn start_focused_keyboard_listener(
    keyboard_tx: Sender<String>,
    is_focused: Arc<Mutex<bool>>,
) {
    thread::spawn(move || {
        crate::always_print!("🎮 Starting focused keyboard listener (device_query polling)...");
        
        // Without Accessibility there is nothing to poll yet. Wait for it instead
        // of ending the thread, so granting it in System Settings starts the
        // listener without a restart.
        #[cfg(target_os = "macos")]
        let device_state = match macos_device_state() {
            Some(device_state) => device_state,
            None => {
                crate::always_print!(
                    "🔐 Accessibility is not granted: focused keyboard polling starts when it is"
                );
                crate::libs::permissions::wait_for_accessibility();
                crate::always_print!("🔐 Accessibility granted: starting focused keyboard polling");
                // A stateless handle on macOS, as in `macos_device_state`.
                DeviceState
            }
        };

        #[cfg(not(target_os = "macos"))]
        let device_state = DeviceState::new();
        let mut prev_keys: HashSet<Keycode> = HashSet::new();

        let mut last_focus_log = std::time::Instant::now();

        loop {
            // Check if window is focused
            let focused = *is_focused.lock().unwrap();

            // Log focus state every 5 seconds for debugging
            if last_focus_log.elapsed().as_secs() >= 5 {
                crate::always_print!("🔍 [device_query] Focus state: {}, polling active: {}",
                    if focused { "FOCUSED" } else { "UNFOCUSED" },
                    focused);
                last_focus_log = std::time::Instant::now();
            }

            if focused {
                // Poll keyboard state
                let keys = device_state.get_keys();
                let current_keys: HashSet<Keycode> = keys.into_iter().collect();

                // Detect newly pressed keys
                for key in current_keys.difference(&prev_keys) {
                    let key_code = map_device_query_keycode(*key);
                    if !key_code.is_empty() {
                        // Send key event without logging sensitive keystrokes
                        let _ = keyboard_tx.send(key_code.to_string());
                    }
                }

                // Detect released keys
                for key in prev_keys.difference(&current_keys) {
                    let key_code = map_device_query_keycode(*key);
                    if !key_code.is_empty() {
                        let _ = keyboard_tx.send(format!("UP:{}", key_code));
                    }
                }

                prev_keys = current_keys;
            } else {
                // Window not focused - clear state and sleep longer
                prev_keys.clear();
                thread::sleep(Duration::from_millis(100));
                continue;
            }

            // Poll at ~100Hz when focused (10ms interval)
            thread::sleep(Duration::from_millis(10));
        }
    });
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::macos_device_state_with_permission_checks;
    use std::cell::Cell;

    #[test]
    fn trusted_launch_does_not_request_accessibility_again() {
        let requested = Cell::new(false);

        let state = macos_device_state_with_permission_checks(
            || true,
            || {
                requested.set(true);
                false
            },
        );

        assert!(state.is_some());
        assert!(!requested.get());
    }

    #[test]
    fn missing_permission_requests_once_without_panicking() {
        let request_count = Cell::new(0);

        let state = macos_device_state_with_permission_checks(
            || false,
            || {
                request_count.set(request_count.get() + 1);
                false
            },
        );

        assert!(state.is_none());
        assert_eq!(request_count.get(), 1);
    }
}
