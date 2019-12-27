// from https://github.com/kwhat/libuiohook/blob/1.1/include/uiohook.h

const keycodes = [
  /* Begin Virtual Key Codes */
  { keycode: 1, key: 'KC_ESCAPE', info: 'Escape' },

  // Begin Function Keys
  { keycode: 59, key: 'KC_F1', info: 'F1' },
  { keycode: 60, key: 'KC_F2', info: 'F2' },
  { keycode: 61, key: 'KC_F3', info: 'F3' },
  { keycode: 62, key: 'KC_F4', info: 'F4' },
  { keycode: 63, key: 'KC_F5', info: 'F5' },
  { keycode: 64, key: 'KC_F6', info: 'F6' },
  { keycode: 65, key: 'KC_F7', info: 'F7' },
  { keycode: 66, key: 'KC_F8', info: 'F8' },
  { keycode: 67, key: 'KC_F9', info: 'F9' },
  { keycode: 68, key: 'KC_F10', info: 'F10' },
  { keycode: 87, key: 'KC_F11', info: 'F11' },
  { keycode: 88, key: 'KC_F12', info: 'F12' },

  { keycode: 91, key: 'KC_F13', info: 'F13' },
  { keycode: 92, key: 'KC_F14', info: 'F14' },
  { keycode: 93, key: 'KC_F15', info: 'F15' },
  { keycode: 99, key: 'KC_F16', info: 'F16' },
  { keycode: 100, key: 'KC_F17', info: 'F17' },
  { keycode: 101, key: 'KC_F18', info: 'F18' },
  { keycode: 102, key: 'KC_F19', info: 'F19' },
  { keycode: 103, key: 'KC_F20', info: 'F20' },
  { keycode: 104, key: 'KC_F21', info: 'F21' },
  { keycode: 105, key: 'KC_F22', info: 'F22' },
  { keycode: 106, key: 'KC_F23', info: 'F23' },
  { keycode: 107, key: 'KC_F24', info: 'F24' },
  // End Function Keys

  // Begin Alphanumeric Zone
  { keycode: 41, key: 'KC_BACKQUOTE', info: 'Backquote' },

  { keycode: 2, key: 'KC_1', info: '1' },
  { keycode: 3, key: 'KC_2', info: '2' },
  { keycode: 4, key: 'KC_3', info: '3' },
  { keycode: 5, key: 'KC_4', info: '4' },
  { keycode: 6, key: 'KC_5', info: '5' },
  { keycode: 7, key: 'KC_6', info: '6' },
  { keycode: 8, key: 'KC_7', info: '7' },
  { keycode: 9, key: 'KC_8', info: '8' },
  { keycode: 10, key: 'KC_9', info: '9' },
  { keycode: 11, key: 'KC_0', info: '0' },

  { keycode: 12, key: 'KC_MINUS', info: '-' }, // '-'
  { keycode: 13, key: 'KC_EQUALS', info: '=' }, // '='
  { keycode: 14, key: 'KC_BACKSPACE', info: 'Backspace' },

  { keycode: 15, key: 'KC_TAB', info: 'Tab' },
  { keycode: 58, key: 'KC_CAPS_LOCK', info: 'Caps Lock' },

  { keycode: 30, key: 'KC_A', info: 'A' },
  { keycode: 48, key: 'KC_B', info: 'B' },
  { keycode: 46, key: 'KC_C', info: 'C' },
  { keycode: 32, key: 'KC_D', info: 'D' },
  { keycode: 18, key: 'KC_E', info: 'E' },
  { keycode: 33, key: 'KC_F', info: 'F' },
  { keycode: 34, key: 'KC_G', info: 'G' },
  { keycode: 35, key: 'KC_H', info: 'H' },
  { keycode: 23, key: 'KC_I', info: 'I' },
  { keycode: 36, key: 'KC_J', info: 'J' },
  { keycode: 37, key: 'KC_K', info: 'K' },
  { keycode: 38, key: 'KC_L', info: 'L' },
  { keycode: 50, key: 'KC_M', info: 'M' },
  { keycode: 49, key: 'KC_N', info: 'N' },
  { keycode: 24, key: 'KC_O', info: 'O' },
  { keycode: 25, key: 'KC_P', info: 'P' },
  { keycode: 16, key: 'KC_Q', info: 'Q' },
  { keycode: 19, key: 'KC_R', info: 'R' },
  { keycode: 31, key: 'KC_S', info: 'S' },
  { keycode: 20, key: 'KC_T', info: 'T' },
  { keycode: 22, key: 'KC_U', info: 'U' },
  { keycode: 47, key: 'KC_V', info: 'V' },
  { keycode: 17, key: 'KC_W', info: 'W' },
  { keycode: 45, key: 'KC_X', info: 'X' },
  { keycode: 21, key: 'KC_Y', info: 'Y' },
  { keycode: 44, key: 'KC_Z', info: 'Z' },

  { keycode: 26, key: 'KC_OPEN_BRACKET', info: '[' }, // '['
  { keycode: 27, key: 'KC_CLOSE_BRACKET', info: ']' }, // ']'
  { keycode: 43, key: 'KC_BACK_SLASH', info: 'Back Slash' }, // ''

  { keycode: 39, key: 'KC_SEMICOLON', info: ';' }, // ';'
  { keycode: 40, key: 'KC_QUOTE', info: 'Quote' },
  { keycode: 28, key: 'KC_ENTER', info: 'Enter' },

  { keycode: 51, key: 'KC_COMMA', info: ',' }, // ','
  { keycode: 52, key: 'KC_PERIOD', info: '.' }, // '.'
  { keycode: 53, key: 'KC_SLASH', info: '/' }, // '/'

  { keycode: 57, key: 'KC_SPACE', info: 'Space' },
  // End Alphanumeric Zone

  { keycode: 3639, key: 'KC_PRINTSCREEN', info: 'Printscreen' },
  { keycode: 70, key: 'KC_SCROLL_LOCK', info: 'Scroll Lock' },
  { keycode: 3653, key: 'KC_PAUSE', info: 'Pause' },

  // Begin Edit Key Zone
  // mac
  { keycode: 3666, key: 'KC_INSERT', info: 'Insert (Mac/Linux/Windows Numpad)' },
  { keycode: 3667, key: 'KC_DELETE', info: 'Delete (Mac/Linux/Windows Numpad)' },
  { keycode: 3655, key: 'KC_HOME', info: 'Home (Mac/Linux/Windows Numpad)' },
  { keycode: 3663, key: 'KC_END', info: 'End (Mac/Linux/Windows Numpad)' },
  { keycode: 3657, key: 'KC_PAGE_UP', info: 'Page Up (Mac/Linux/Windows Numpad)' },
  { keycode: 3665, key: 'KC_PAGE_DOWN', info: 'Page Down (Mac/Linux/Windows Numpad)' },
  // windows
  { keycode: 3666 + 57344, key: 'KC_INSERT', info: 'Insert (Windows)' },
  { keycode: 3667 + 57344, key: 'KC_DELETE', info: 'Delete (Windows)' },
  { keycode: 3655 + 57344, key: 'KC_HOME', info: 'Home (Windows)' },
  { keycode: 3663 + 57344, key: 'KC_END', info: 'End (Windows)' },
  { keycode: 3657 + 57344, key: 'KC_PAGE_UP', info: 'Page up (Windows)' },
  { keycode: 3665 + 57344, key: 'KC_PAGE_DOWN', info: 'Page down (Windows)' },
  // End Edit Key Zone

  // Begin Cursor Key Zone
  // Mac, Linux arrow keys
  // equals to Windows Numpad keys
  { keycode: 57416, key: 'KC_UP', info: 'Arrow Up (Mac/Linux/Windows Numpad Up)' },
  { keycode: 57419, key: 'KC_LEFT', info: 'Arrow Left (Mac/Linux/Windows Numpad Left)' },
  { keycode: 57420, key: 'KC_CLEAR', info: 'Arrow Clear (Mac/Linux/Windows Numpad Clear)' },
  { keycode: 57421, key: 'KC_RIGHT', info: 'Arrow Right (Mac/Linux/Windows Numpad Right)' },
  { keycode: 57424, key: 'KC_DOWN', info: 'Arrow Down (Mac/Linux/Windows Numpad Down)' },
  // Windows arrow keys
  { keycode: 57416 + 0xe00, key: 'KC_UP', info: 'Arrow Up (Windows)' },
  { keycode: 57419 + 0xe00, key: 'KC_LEFT', info: 'Arrow Left (Windows)' },
  { keycode: 57421 + 0xe00, key: 'KC_RIGHT', info: 'Arrow Right (Windows)' },
  { keycode: 57424 + 0xe00, key: 'KC_DOWN', info: 'Arrow Down (Windows)' },
  // End Cursor Key Zone

  // Begin Numeric Zone
  { keycode: 69, key: 'KC_NUM_LOCK', info: 'Numpad Numlock' },
  { keycode: 3637, key: 'KC_KP_DIVIDE', info: 'Numpad Devide' },
  { keycode: 55, key: 'KC_KP_MULTIPLY', info: 'Numpad Multiply' },
  { keycode: 74, key: 'KC_KP_SUBTRACT', info: 'Numpad Subtract' },
  { keycode: 3597, key: 'KC_KP_EQUALS', info: 'Numpad Equals' },
  { keycode: 78, key: 'KC_KP_ADD', info: 'Numpad Add' },
  { keycode: 3612, key: 'KC_KP_ENTER', info: 'Numpad Enter' },
  { keycode: 83, key: 'KC_KP_SEPARATOR', info: 'Numpad Separator (Delete)' },

  { keycode: 79, key: 'KC_KP_1', info: 'Numpad 1 (End)' },
  { keycode: 80, key: 'KC_KP_2', info: 'Numpad 2 (Down)' },
  { keycode: 81, key: 'KC_KP_3', info: 'Numpad 3 (Page Down)' },
  { keycode: 75, key: 'KC_KP_4', info: 'Numpad 4 (Left)' },
  { keycode: 76, key: 'KC_KP_5', info: 'Numpad 5 (Clear)' },
  { keycode: 77, key: 'KC_KP_6', info: 'Numpad 6 (right)' },
  { keycode: 71, key: 'KC_KP_7', info: 'Numpad 7 (Home)' },
  { keycode: 72, key: 'KC_KP_8', info: 'Numpad 8 (Up)' },
  { keycode: 73, key: 'KC_KP_9', info: 'Numpad 9 (Page Up)' },
  { keycode: 82, key: 'KC_KP_0', info: 'Numpad 0 (Insert)' },
  // End Numeric Zone

  // Begin Modifier and Control Keys
  { keycode: 42, key: 'KC_SHIFT_L', info: 'Left Shift' },
  { keycode: 54, key: 'KC_SHIFT_R', info: 'Right Shift' },
  { keycode: 29, key: 'KC_CONTROL_L', info: 'Left Ctrl' },
  { keycode: 3613, key: 'KC_CONTROL_R', info: 'Right Ctrl' },
  { keycode: 56, key: 'KC_ALT_L', info: 'Option or Alt Key' }, // Option or Alt Key
  { keycode: 3640, key: 'KC_ALT_R', info: 'Option or Alt Key' }, // Option or Alt Key
  { keycode: 3675, key: 'KC_META_L', info: 'Windows or Command Key' }, // Windows or Command Key
  { keycode: 3676, key: 'KC_META_R', info: 'Windows or Command Key' }, // Windows or Command Key
  { keycode: 3677, key: 'KC_CONTEXT_MENU', info: 'Context Menu' },
  // End Modifier and Control Keys

  { keycode: 0, key: 'KC_UNDEFINED', info: 'KeyCode Unknown' }, // KeyCode Unknown
  { keycode: 65535, key: 'CHAR_UNDEFINED', info: 'CharCode Unknown' }, // CharCode Unknown
  /* End Virtual Key Codes */
];
module.exports = keycodes;
