const keycodes = [
  /* Begin Virtual Key Codes */
  { keycode: 1, keycode_hex: 0x0001, key: 'KC_ESCAPE', info: 'Escape' },

  // Begin Function Keys
  { keycode: 59, keycode_hex: 0x003b, key: 'KC_F1', info: 'F1' },
  { keycode: 60, keycode_hex: 0x003c, key: 'KC_F2', info: 'F2' },
  { keycode: 61, keycode_hex: 0x003d, key: 'KC_F3', info: 'F3' },
  { keycode: 62, keycode_hex: 0x003e, key: 'KC_F4', info: 'F4' },
  { keycode: 63, keycode_hex: 0x003f, key: 'KC_F5', info: 'F5' },
  { keycode: 64, keycode_hex: 0x0040, key: 'KC_F6', info: 'F6' },
  { keycode: 65, keycode_hex: 0x0041, key: 'KC_F7', info: 'F7' },
  { keycode: 66, keycode_hex: 0x0042, key: 'KC_F8', info: 'F8' },
  { keycode: 67, keycode_hex: 0x0043, key: 'KC_F9', info: 'F9' },
  { keycode: 68, keycode_hex: 0x0044, key: 'KC_F10', info: 'F10' },
  { keycode: 87, keycode_hex: 0x0057, key: 'KC_F11', info: 'F11' },
  { keycode: 88, keycode_hex: 0x0058, key: 'KC_F12', info: 'F12' },

  { keycode: 91, keycode_hex: 0x005b, key: 'KC_F13', info: 'F13' },
  { keycode: 92, keycode_hex: 0x005c, key: 'KC_F14', info: 'F14' },
  { keycode: 93, keycode_hex: 0x005d, key: 'KC_F15', info: 'F15' },
  { keycode: 99, keycode_hex: 0x0063, key: 'KC_F16', info: 'F16' },
  { keycode: 100, keycode_hex: 0x0064, key: 'KC_F17', info: 'F17' },
  { keycode: 101, keycode_hex: 0x0065, key: 'KC_F18', info: 'F18' },
  { keycode: 102, keycode_hex: 0x0066, key: 'KC_F19', info: 'F19' },
  { keycode: 103, keycode_hex: 0x0067, key: 'KC_F20', info: 'F20' },
  { keycode: 104, keycode_hex: 0x0068, key: 'KC_F21', info: 'F21' },
  { keycode: 105, keycode_hex: 0x0069, key: 'KC_F22', info: 'F22' },
  { keycode: 106, keycode_hex: 0x006a, key: 'KC_F23', info: 'F23' },
  { keycode: 107, keycode_hex: 0x006b, key: 'KC_F24', info: 'F24' },
  // End Function Keys

  // Begin Alphanumeric Zone
  { keycode: 41, keycode_hex: 0x0029, key: 'KC_BACKQUOTE', info: 'Backquote' },

  { keycode: 2, keycode_hex: 0x0002, key: 'KC_1', info: '1' },
  { keycode: 3, keycode_hex: 0x0003, key: 'KC_2', info: '2' },
  { keycode: 4, keycode_hex: 0x0004, key: 'KC_3', info: '3' },
  { keycode: 5, keycode_hex: 0x0005, key: 'KC_4', info: '4' },
  { keycode: 6, keycode_hex: 0x0006, key: 'KC_5', info: '5' },
  { keycode: 7, keycode_hex: 0x0007, key: 'KC_6', info: '6' },
  { keycode: 8, keycode_hex: 0x0008, key: 'KC_7', info: '7' },
  { keycode: 9, keycode_hex: 0x0009, key: 'KC_8', info: '8' },
  { keycode: 10, keycode_hex: 0x000a, key: 'KC_9', info: '9' },
  { keycode: 11, keycode_hex: 0x000b, key: 'KC_0', info: '0' },

  { keycode: 12, keycode_hex: 0x000c, key: 'KC_MINUS', info: '-' }, // '-'
  { keycode: 13, keycode_hex: 0x000d, key: 'KC_EQUALS', info: '=' }, // '='
  { keycode: 14, keycode_hex: 0x000e, key: 'KC_BACKSPACE', info: 'Backspace' },

  { keycode: 15, keycode_hex: 0x000f, key: 'KC_TAB', info: 'Tab' },
  { keycode: 58, keycode_hex: 0x003a, key: 'KC_CAPS_LOCK', info: 'Caps lock' },

  { keycode: 30, keycode_hex: 0x001e, key: 'KC_A', info: 'A' },
  { keycode: 48, keycode_hex: 0x0030, key: 'KC_B', info: 'B' },
  { keycode: 46, keycode_hex: 0x002e, key: 'KC_C', info: 'C' },
  { keycode: 32, keycode_hex: 0x0020, key: 'KC_D', info: 'D' },
  { keycode: 18, keycode_hex: 0x0012, key: 'KC_E', info: 'E' },
  { keycode: 33, keycode_hex: 0x0021, key: 'KC_F', info: 'F' },
  { keycode: 34, keycode_hex: 0x0022, key: 'KC_G', info: 'G' },
  { keycode: 35, keycode_hex: 0x0023, key: 'KC_H', info: 'H' },
  { keycode: 23, keycode_hex: 0x0017, key: 'KC_I', info: 'I' },
  { keycode: 36, keycode_hex: 0x0024, key: 'KC_J', info: 'J' },
  { keycode: 37, keycode_hex: 0x0025, key: 'KC_K', info: 'K' },
  { keycode: 38, keycode_hex: 0x0026, key: 'KC_L', info: 'L' },
  { keycode: 50, keycode_hex: 0x0032, key: 'KC_M', info: 'M' },
  { keycode: 49, keycode_hex: 0x0031, key: 'KC_N', info: 'N' },
  { keycode: 24, keycode_hex: 0x0018, key: 'KC_O', info: 'O' },
  { keycode: 25, keycode_hex: 0x0019, key: 'KC_P', info: 'P' },
  { keycode: 16, keycode_hex: 0x0010, key: 'KC_Q', info: 'Q' },
  { keycode: 19, keycode_hex: 0x0013, key: 'KC_R', info: 'R' },
  { keycode: 31, keycode_hex: 0x001f, key: 'KC_S', info: 'S' },
  { keycode: 20, keycode_hex: 0x0014, key: 'KC_T', info: 'T' },
  { keycode: 22, keycode_hex: 0x0016, key: 'KC_U', info: 'U' },
  { keycode: 47, keycode_hex: 0x002f, key: 'KC_V', info: 'V' },
  { keycode: 17, keycode_hex: 0x0011, key: 'KC_W', info: 'W' },
  { keycode: 45, keycode_hex: 0x002d, key: 'KC_X', info: 'X' },
  { keycode: 21, keycode_hex: 0x0015, key: 'KC_Y', info: 'Y' },
  { keycode: 44, keycode_hex: 0x002c, key: 'KC_Z', info: 'Z' },

  { keycode: 26, keycode_hex: 0x001a, key: 'KC_OPEN_BRACKET', info: '[' }, // '['
  { keycode: 27, keycode_hex: 0x001b, key: 'KC_CLOSE_BRACKET', info: ']' }, // ']'
  { keycode: 43, keycode_hex: 0x002b, key: 'KC_BACK_SLASH', info: 'Back slash' }, // ''

  { keycode: 39, keycode_hex: 0x0027, key: 'KC_SEMICOLON', info: ';' }, // ';'
  { keycode: 40, keycode_hex: 0x0028, key: 'KC_QUOTE', info: 'Quote' },
  { keycode: 28, keycode_hex: 0x001c, key: 'KC_ENTER', info: 'Enter' },

  { keycode: 51, keycode_hex: 0x0033, key: 'KC_COMMA', info: ',' }, // ','
  { keycode: 52, keycode_hex: 0x0034, key: 'KC_PERIOD', info: '.' }, // '.'
  { keycode: 53, keycode_hex: 0x0035, key: 'KC_SLASH', info: '/' }, // '/'

  { keycode: 57, keycode_hex: 0x0039, key: 'KC_SPACE', info: 'Space' },
  // End Alphanumeric Zone

  { keycode: 3639, keycode_hex: 0x0e37, key: 'KC_PRINTSCREEN', info: 'Printscreen' },
  { keycode: 70, keycode_hex: 0x0046, key: 'KC_SCROLL_LOCK', info: 'Scroll lock' },
  { keycode: 3653, keycode_hex: 0x0e45, key: 'KC_PAUSE', info: 'Pause' },

  // Begin Edit Key Zone
  { keycode: 3666, keycode_hex: 0x0e52, key: 'KC_INSERT', info: 'Insert' },
  { keycode: 3667, keycode_hex: 0x0e53, key: 'KC_DELETE', info: 'Delete' },
  { keycode: 3655, keycode_hex: 0x0e47, key: 'KC_HOME', info: 'Home' },
  { keycode: 3663, keycode_hex: 0x0e4f, key: 'KC_END', info: 'End' },
  { keycode: 3657, keycode_hex: 0x0e49, key: 'KC_PAGE_UP', info: 'Page up' },
  { keycode: 3665, keycode_hex: 0x0e51, key: 'KC_PAGE_DOWN', info: 'Page down' },
  // End Edit Key Zone

  // Begin Cursor Key Zone
  { keycode: 57416, keycode_hex: 0xe048, key: 'KC_UP', info: 'Up' },
  { keycode: 57419, keycode_hex: 0xe04b, key: 'KC_LEFT', info: 'Left' },
  { keycode: 57420, keycode_hex: 0xe04c, key: 'KC_CLEAR', info: 'Clear' },
  { keycode: 57421, keycode_hex: 0xe04d, key: 'KC_RIGHT', info: 'Right' },
  { keycode: 57424, keycode_hex: 0xe050, key: 'KC_DOWN', info: 'Down' },
  // End Cursor Key Zone

  // Begin Numeric Zone
  { keycode: 69, keycode_hex: 0x0045, key: 'KC_NUM_LOCK', info: 'Numlock' },
  { keycode: 3637, keycode_hex: 0x0e35, key: 'KC_KP_DIVIDE', info: 'Devide' },
  { keycode: 55, keycode_hex: 0x0037, key: 'KC_KP_MULTIPLY', info: 'Multiply' },
  { keycode: 74, keycode_hex: 0x004a, key: 'KC_KP_SUBTRACT', info: 'Subtract' },
  { keycode: 3597, keycode_hex: 0x0e0d, key: 'KC_KP_EQUALS', info: 'Equals' },
  { keycode: 78, keycode_hex: 0x004e, key: 'KC_KP_ADD', info: 'Add' },
  { keycode: 3612, keycode_hex: 0x0e1c, key: 'KC_KP_ENTER', info: 'Enter' },
  { keycode: 83, keycode_hex: 0x0053, key: 'KC_KP_SEPARATOR', info: 'Separator' },

  { keycode: 79, keycode_hex: 0x004f, key: 'KC_KP_1', info: '1' },
  { keycode: 80, keycode_hex: 0x0050, key: 'KC_KP_2', info: '2' },
  { keycode: 81, keycode_hex: 0x0051, key: 'KC_KP_3', info: '3' },
  { keycode: 75, keycode_hex: 0x004b, key: 'KC_KP_4', info: '4' },
  { keycode: 76, keycode_hex: 0x004c, key: 'KC_KP_5', info: '5' },
  { keycode: 77, keycode_hex: 0x004d, key: 'KC_KP_6', info: '6' },
  { keycode: 71, keycode_hex: 0x0047, key: 'KC_KP_7', info: '7' },
  { keycode: 72, keycode_hex: 0x0048, key: 'KC_KP_8', info: '8' },
  { keycode: 73, keycode_hex: 0x0049, key: 'KC_KP_9', info: '9' },
  { keycode: 82, keycode_hex: 0x0052, key: 'KC_KP_0', info: '0' },

  { keycode: 79, keycode_hex: 0x004f, key: 'KC_KP_END', info: 'End' },
  { keycode: 80, keycode_hex: 0x0050, key: 'KC_KP_DOWN', info: 'Down' },
  { keycode: 81, keycode_hex: 0x0051, key: 'KC_KP_PAGE_DOWN', info: 'Page down' },
  { keycode: 75, keycode_hex: 0x004b, key: 'KC_KP_LEFT', info: 'Left' },
  { keycode: 76, keycode_hex: 0x004c, key: 'KC_KP_CLEAR', info: 'Clear' },
  { keycode: 77, keycode_hex: 0x004d, key: 'KC_KP_RIGHT', info: 'Right' },
  { keycode: 71, keycode_hex: 0x0047, key: 'KC_KP_HOME', info: 'Home' },
  { keycode: 72, keycode_hex: 0x0048, key: 'KC_KP_UP', info: 'Up' },
  { keycode: 73, keycode_hex: 0x0049, key: 'KC_KP_PAGE_UP', info: 'Page up' },
  { keycode: 82, keycode_hex: 0x0052, key: 'KC_KP_INSERT', info: 'Insert' },
  { keycode: 83, keycode_hex: 0x0053, key: 'KC_KP_DELETE', info: 'Delete' },
  // End Numeric Zone

  // Begin Modifier and Control Keys
  { keycode: 42, keycode_hex: 0x002a, key: 'KC_SHIFT_L', info: 'Left shift' },
  { keycode: 54, keycode_hex: 0x0036, key: 'KC_SHIFT_R', info: 'Right shift' },
  { keycode: 29, keycode_hex: 0x001d, key: 'KC_CONTROL_L', info: 'Left ctl' },
  { keycode: 3613, keycode_hex: 0x0e1d, key: 'KC_CONTROL_R', info: 'Right ctl' },
  { keycode: 56, keycode_hex: 0x0038, key: 'KC_ALT_L', info: 'Option or Alt Key' }, // Option or Alt Key
  { keycode: 3640, keycode_hex: 0x0e38, key: 'KC_ALT_R', info: 'Option or Alt Key' }, // Option or Alt Key
  { keycode: 3675, keycode_hex: 0x0e5b, key: 'KC_META_L', info: 'Windows or Command Key' }, // Windows or Command Key
  { keycode: 3676, keycode_hex: 0x0e5c, key: 'KC_META_R', info: 'Windows or Command Key' }, // Windows or Command Key
  { keycode: 3677, keycode_hex: 0x0e5d, key: 'KC_CONTEXT_MENU', info: 'Context menu' },
  // End Modifier and Control Keys

  // Begin Media Control Keys
  { keycode: 57438, keycode_hex: 0xe05e, key: 'KC_POWER', info: 'Power' },
  { keycode: 57439, keycode_hex: 0xe05f, key: 'KC_SLEEP', info: 'Sleep' },
  { keycode: 57443, keycode_hex: 0xe063, key: 'KC_WAKE', info: 'Wake' },

  { keycode: 57378, keycode_hex: 0xe022, key: 'KC_MEDIA_PLAY', info: 'Media play' },
  { keycode: 57380, keycode_hex: 0xe024, key: 'KC_MEDIA_STOP', info: 'Media stop' },
  { keycode: 57360, keycode_hex: 0xe010, key: 'KC_MEDIA_PREVIOUS', info: 'Media previous' },
  { keycode: 57369, keycode_hex: 0xe019, key: 'KC_MEDIA_NEXT', info: 'Media next' },
  { keycode: 57453, keycode_hex: 0xe06d, key: 'KC_MEDIA_SELECT', info: 'Media select' },
  { keycode: 57388, keycode_hex: 0xe02c, key: 'KC_MEDIA_EJECT', info: 'Media eject' },

  { keycode: 57376, keycode_hex: 0xe020, key: 'KC_VOLUME_MUTE', info: 'Volume mute' },
  { keycode: 57392, keycode_hex: 0xe030, key: 'KC_VOLUME_UP', info: 'Volume up' },
  { keycode: 57390, keycode_hex: 0xe02e, key: 'KC_VOLUME_DOWN', info: 'Volume down' },

  { keycode: 57452, keycode_hex: 0xe06c, key: 'KC_APP_MAIL', info: 'App mail' },
  { keycode: 57377, keycode_hex: 0xe021, key: 'KC_APP_CALCULATOR', info: 'App calculator' },
  { keycode: 57404, keycode_hex: 0xe03c, key: 'KC_APP_MUSIC', info: 'App music' },
  { keycode: 57444, keycode_hex: 0xe064, key: 'KC_APP_PICTURES', info: 'App pictures' },

  { keycode: 57445, keycode_hex: 0xe065, key: 'KC_BROWSER_SEARCH', info: 'Browser search' },
  { keycode: 57394, keycode_hex: 0xe032, key: 'KC_BROWSER_HOME', info: 'Browser home' },
  { keycode: 57450, keycode_hex: 0xe06a, key: 'KC_BROWSER_BACK', info: 'Browser back' },
  { keycode: 57449, keycode_hex: 0xe069, key: 'KC_BROWSER_FORWARD', info: 'Browser forward' },
  { keycode: 57448, keycode_hex: 0xe068, key: 'KC_BROWSER_STOP', info: 'Browser stop' },
  { keycode: 57447, keycode_hex: 0xe067, key: 'KC_BROWSER_REFRESH', info: 'Browser refresh' },
  { keycode: 57446, keycode_hex: 0xe066, key: 'KC_BROWSER_FAVORITES', info: 'Browser favorites' },
  // End Media Control Keys

  { keycode: 0, keycode_hex: 0x0000, key: 'KC_UNDEFINED', info: 'KeyCode Unknown' }, // KeyCode Unknown
  { keycode: 65535, keycode_hex: 0xffff, key: 'CHAR_UNDEFINED', info: 'CharCode Unknown' }, // CharCode Unknown
  /* End Virtual Key Codes */
];
module.exports = keycodes;
