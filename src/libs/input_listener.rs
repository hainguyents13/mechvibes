use crossbeam_channel::Sender;
use rdev::{ listen, Button, Event, EventType, Key };
use std::collections::HashSet;
use std::sync::{ Arc, Mutex };
use std::thread;
use std::time::{ Duration, Instant };

// Maps a keyboard key to its standardized code
fn map_key_to_code(key: Key) -> &'static str {
    match key {
        // Common keys across all platforms
        Key::Space => "Space",
        Key::Backspace => "Backspace",
        Key::CapsLock => "CapsLock",
        Key::Tab => "Tab",
        Key::Return => "Enter",
        Key::Escape => "Escape",
        Key::Delete => "Delete",

        // Modifier keys with left/right variants
        Key::Alt => "AltLeft",
        Key::AltGr => "AltRight",
        Key::ShiftLeft => "ShiftLeft",
        Key::ShiftRight => "ShiftRight",
        Key::ControlLeft => "ControlLeft",
        Key::ControlRight => "ControlRight",
        Key::MetaLeft => "MetaLeft",
        Key::MetaRight => "MetaRight",

        // Arrow keys
        Key::UpArrow => "ArrowUp",
        Key::DownArrow => "ArrowDown",
        Key::LeftArrow => "ArrowLeft",
        Key::RightArrow => "ArrowRight",

        // Navigation keys
        Key::Home => "Home",
        Key::End => "End",
        Key::PageUp => "PageUp",
        Key::PageDown => "PageDown",
        Key::Insert => "Insert", // Function keys F1-F12 (rdev 0.5.3 only supports F1-F12)
        Key::F1 => "F1",
        Key::F2 => "F2",
        Key::F3 => "F3",
        Key::F4 => "F4",
        Key::F5 => "F5",
        Key::F6 => "F6",
        Key::F7 => "F7",
        Key::F8 => "F8",
        Key::F9 => "F9",
        Key::F10 => "F10",
        Key::F11 => "F11",
        Key::F12 => "F12",

        // Alpha keys A-Z
        Key::KeyA => "KeyA",
        Key::KeyB => "KeyB",
        Key::KeyC => "KeyC",
        Key::KeyD => "KeyD",
        Key::KeyE => "KeyE",
        Key::KeyF => "KeyF",
        Key::KeyG => "KeyG",
        Key::KeyH => "KeyH",
        Key::KeyI => "KeyI",
        Key::KeyJ => "KeyJ",
        Key::KeyK => "KeyK",
        Key::KeyL => "KeyL",
        Key::KeyM => "KeyM",
        Key::KeyN => "KeyN",
        Key::KeyO => "KeyO",
        Key::KeyP => "KeyP",
        Key::KeyQ => "KeyQ",
        Key::KeyR => "KeyR",
        Key::KeyS => "KeyS",
        Key::KeyT => "KeyT",
        Key::KeyU => "KeyU",
        Key::KeyV => "KeyV",
        Key::KeyW => "KeyW",
        Key::KeyX => "KeyX",
        Key::KeyY => "KeyY",
        Key::KeyZ => "KeyZ",

        // Number keys 0-9
        Key::Num0 => "Digit0",
        Key::Num1 => "Digit1",
        Key::Num2 => "Digit2",
        Key::Num3 => "Digit3",
        Key::Num4 => "Digit4",
        Key::Num5 => "Digit5",
        Key::Num6 => "Digit6",
        Key::Num7 => "Digit7",
        Key::Num8 => "Digit8",
        Key::Num9 => "Digit9",

        // Punctuation and symbols
        Key::Minus => "Minus", // -
        Key::Equal => "Equal", // =
        Key::Comma => "Comma", // ,
        Key::Dot => "Period", // .
        Key::Quote => "Quote", // '
        Key::BackQuote => "Backquote", // `
        Key::Slash => "Slash", // /
        Key::LeftBracket => "BracketLeft", // [
        Key::RightBracket => "BracketRight", // ]
        Key::BackSlash => "Backslash", // \
        Key::SemiColon => "Semicolon", // ;
        Key::IntlBackslash => "IntlBackslash", // Additional backslash key on some keyboards

        // Numpad keys
        Key::KpReturn => "NumpadEnter",
        Key::KpMinus => "NumpadSubtract",
        Key::KpPlus => "NumpadAdd",
        Key::KpMultiply => "NumpadMultiply",
        Key::KpDivide => "NumpadDivide",
        Key::Kp0 => "Numpad0",
        Key::Kp1 => "Numpad1",
        Key::Kp2 => "Numpad2",
        Key::Kp3 => "Numpad3",
        Key::Kp4 => "Numpad4",
        Key::Kp5 => "Numpad5",
        Key::Kp6 => "Numpad6",
        Key::Kp7 => "Numpad7",
        Key::Kp8 => "Numpad8",
        Key::Kp9 => "Numpad9",
        Key::KpDelete => "NumpadDecimal",

        // Additional system keys
        Key::NumLock => "NumLock",
        Key::ScrollLock => "ScrollLock",
        Key::PrintScreen => "PrintScreen",
        Key::Pause => "Pause",
        Key::Function => "Fn", // Special function key on some keyboards

        // Unknown or unmapped keys
        Key::Unknown(_) => "", // Handle unknown keys gracefully
    }
}

// Maps a mouse button to its standardized code
fn map_button_to_code(button: Button) -> &'static str {
    match button {
        Button::Left => "MouseLeft",
        Button::Right => "MouseRight",
        Button::Middle => "MouseMiddle",
        Button::Unknown(code) => {
            // Handle additional mouse buttons (side buttons, etc.)
            match code {
                4 => "Mouse4", // Back/Previous
                5 => "Mouse5", // Forward/Next
                6 => "Mouse6", // Extra button 1
                7 => "Mouse7", // Extra button 2
                8 => "Mouse8", // Extra button 3
                _ => "MouseUnknown",
            }
        }
    }
}

/// Start a unified input listener that handles both keyboard and mouse events
/// This solves the issue where rdev can only have one global listener at a time
///
/// When is_focused is provided, keyboard events are only sent when the window is UNFOCUSED
/// to avoid duplicate events with the focused_input_listener
pub fn start_unified_input_listener(
    keyboard_tx: Sender<String>,
    mouse_tx: Sender<String>,
    hotkey_tx: Sender<String>,
    is_focused: Option<Arc<Mutex<bool>>>,
) {
    crate::always_print!("🎮 Starting unified input listener (keyboard + mouse + hotkeys)...");

    thread::spawn(move || {
        crate::always_print!("🎮 Unified input listener thread started");

        // Separate state tracking for keyboard and mouse
        let keyboard_last_press = Arc::new(Mutex::new(Instant::now()));
        let mouse_last_press = Arc::new(Mutex::new(Instant::now()));
        let pressed_keys = Arc::new(Mutex::new(HashSet::<String>::new()));
        let pressed_buttons = Arc::new(Mutex::new(HashSet::<String>::new()));

        // Track pressed modifier keys for hotkey detection
        let mut ctrl_pressed = false;
        let mut alt_pressed = false;

        crate::always_print!("🎮 Starting rdev::listen() - listening to keyboard/mouse events");
        let result = listen(move |event: Event| {
            match event.event_type {
                // ===== KEYBOARD EVENTS =====
                EventType::KeyPress(key) => {
                    let key_code = map_key_to_code(key);
                    if !key_code.is_empty() {
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
                                    crate::always_print!(
                                        "🔥 Hotkey detected: Ctrl+Alt+M - Toggling global sound"
                                    );
                                    let _ = hotkey_tx.send("TOGGLE_SOUND".to_string());
                                    return; // Don't process this as a regular key event
                                }
                            }
                            _ => {}
                        }

                        // If focus state is provided, only send keyboard events when UNFOCUSED
                        // This prevents duplicate events with the focused_input_listener
                        if let Some(ref focus_state) = is_focused {
                            if *focus_state.lock().unwrap() {
                                return; // Window is focused, skip keyboard event (focused_input_listener handles it)
                            }
                        }

                        // Check if key is already pressed
                        let mut pressed = pressed_keys.lock().unwrap();
                        if pressed.contains(&key_code.to_string()) {
                            return; // Key already pressed, ignore
                        }
                        pressed.insert(key_code.to_string());
                        drop(pressed); // Apply debounce and detect rapid key events
                        let now = Instant::now();
                        let mut last = keyboard_last_press.lock().unwrap();
                        let time_since_last = now.duration_since(*last);

                        // Special handling for Backspace key - skip if too rapid (< 10ms)
                        if key_code == "Backspace" && time_since_last < Duration::from_millis(10) {
                            return; // Skip this Backspace event entirely
                        }

                        if time_since_last > Duration::from_millis(1) {
                            *last = now;
                            let _ = keyboard_tx.send(key_code.to_string());
                        }
                    }
                }
                EventType::KeyRelease(key) => {
                    let key_code = map_key_to_code(key);
                    if !key_code.is_empty() {
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

                        // If focus state is provided, only send keyboard events when UNFOCUSED
                        if let Some(ref focus_state) = is_focused {
                            if *focus_state.lock().unwrap() {
                                return; // Window is focused, skip keyboard event
                            }
                        }

                        // Remove key from pressed set
                        let mut pressed = pressed_keys.lock().unwrap();
                        pressed.remove(&key_code.to_string());
                        drop(pressed);

                        let _ = keyboard_tx.send(format!("UP:{}", key_code));
                    }
                }

                // ===== MOUSE EVENTS =====
                EventType::ButtonPress(button) => {
                    let button_code = map_button_to_code(button);
                    if !button_code.is_empty() && button_code != "MouseUnknown" {
                        // crate::always_print!("🖱️ Mouse Button Pressed: {}", button_code);
                        // crate::always_print!("🔍 DEBUG: Mouse event detected: {}", button_code);

                        // Check if button is already pressed
                        let mut pressed = pressed_buttons.lock().unwrap();
                        if pressed.contains(&button_code.to_string()) {
                            return; // Button already pressed, ignore
                        }
                        pressed.insert(button_code.to_string());
                        drop(pressed); // Apply debounce and detect rapid mouse events
                        let now = Instant::now();
                        let mut last = mouse_last_press.lock().unwrap();
                        let time_since_last = now.duration_since(*last);

                        // General rapid event detection (< 60ms) - log but still process
                        if
                            time_since_last < Duration::from_millis(60) &&
                            time_since_last > Duration::from_millis(1)
                        {
                            crate::always_print!(
                                "⚡ RAPID MOUSE EVENT detected: '{}' fired {:.1}ms after previous mouse event",
                                button_code,
                                time_since_last.as_millis()
                            );
                        }

                        if time_since_last > Duration::from_millis(1) {
                            *last = now;
                            let _ = mouse_tx.send(button_code.to_string());
                        }
                    }
                }
                EventType::ButtonRelease(button) => {
                    let button_code = map_button_to_code(button);
                    if !button_code.is_empty() && button_code != "MouseUnknown" {
                        // crate::always_print!("🖱️ Mouse Button Released: {}", button_code);

                        // Remove button from pressed set
                        let mut pressed = pressed_buttons.lock().unwrap();
                        pressed.remove(&button_code.to_string());
                        drop(pressed);

                        let _ = mouse_tx.send(format!("UP:{}", button_code));
                    }
                }
                // Skip mouse wheel events for now
                EventType::Wheel { delta_x: _, delta_y: _ } => {
                    // let wheel_event = if delta_y > 0 {
                    //     "MouseWheelUp"
                    // } else if delta_y < 0 {
                    //     "MouseWheelDown"
                    // } else {
                    //     return; // No vertical scroll, ignore
                    // };

                    // crate::always_print!("🖱️ Mouse Wheel: {}", wheel_event);

                    // // Apply longer debounce for wheel events
                    // let now = Instant::now();
                    // let mut last = mouse_last_press.lock().unwrap();
                    // if now.duration_since(*last) > Duration::from_millis(50) {
                    //     *last = now;
                    //     let _ = mouse_tx.send(wheel_event.to_string());
                    // }
                }
                EventType::MouseMove { x: _, y: _ } => {
                    // Mouse move events are too noisy, ignore them
                    // crate::always_print!("🖱️ Mouse Move: ({}, {})", x, y);
                }
            }
        });

        if let Err(error) = result {
            crate::always_eprint!("❌ Unified input listener error: {:?}", error);
        }
    });
}
