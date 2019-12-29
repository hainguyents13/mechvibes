const standard = {
  1: 'Escape',

  59: 'F1',
  60: 'F2',
  61: 'F3',
  62: 'F4',
  63: 'F5',
  64: 'F6',
  65: 'F7',
  66: 'F8',
  67: 'F9',
  68: 'F10',
  87: 'F11',
  88: 'F12',

  41: 'Backquote',

  2: '1',
  3: '2',
  4: '3',
  5: '4',
  6: '5',
  7: '6',
  8: '7',
  9: '8',
  10: '9',
  11: '0',

  12: '-',
  13: '=',
  14: 'Backspace',

  15: 'Tab',
  58: 'Caps Lock',

  30: 'A',
  48: 'B',
  46: 'C',
  32: 'D',
  18: 'E',
  33: 'F',
  34: 'G',
  35: 'H',
  23: 'I',
  36: 'J',
  37: 'K',
  38: 'L',
  50: 'M',
  49: 'N',
  24: 'O',
  25: 'P',
  16: 'Q',
  19: 'R',
  31: 'S',
  20: 'T',
  22: 'U',
  47: 'V',
  17: 'W',
  45: 'X',
  21: 'Y',
  44: 'Z',

  26: '[',
  27: ']',
  43: 'Back Slash',

  39: ';',
  40: 'Quote',
  28: 'Enter',

  51: ',',
  52: '.',
  53: '/',

  57: 'Space',

  3639: 'Printscreen',
  70: 'Scroll Lock',
  3653: 'Pause',

  3666: 'Insert',
  3667: 'Delete',
  3655: 'Home',
  3663: 'End',
  3657: 'Page Up',
  3665: 'Page Down',

  57416: 'Up',
  57419: 'Left',
  57421: 'Right',
  57424: 'Down',

  42: 'Left Shift',
  54: 'Right Shift',
  29: 'Left Ctrl',
  3613: 'Right Ctrl',
  56: 'Left Alt',
  3640: 'Right Alt',
  3675: 'Meta',
  3676: 'Meta',

  // Numpad
  69: 'Numlock',
  3637: 'Devide (Numpad)',
  55: 'Multiply (Numpad)',
  74: 'Subtract (Numpad)',
  3597: 'Equals (Numpad)',
  78: 'Add (Numpad)',
  3612: 'Enter (Numpad)',
  83: 'Separator (Numpad)',

  79: '1 (Numpad)',
  80: '2 (Numpad)',
  81: '3 (Numpad)',
  75: '4 (Numpad)',
  76: '5 (Numpad)',
  77: '6 (Numpad)',
  71: '7 (Numpad)',
  72: '8 (Numpad)',
  73: '9 (Numpad)',
  82: '0 (Numpad)',

  // Error
  0: 'KeyCode Unknown',
  65535: 'CharCode Unknown',
};

const darwin = Object.assign({}, standard, {
  28: 'Return',
  56: 'Left Option',
  69: 'Clear',
  3640: 'Right Option',
  3666: 'Fn',
  3675: 'Left Command',
  3676: 'Right Command',
});

const win32 = Object.assign({}, standard, {
  3675: 'Left Windows',
  3676: 'Right Windows',
  61010: 'Insert',
  61011: 'Delete',
  60999: 'Home',
  61007: 'End',
  61001: 'Page up',
  61009: 'Page down',
  61000: 'Up',
  61003: 'Left',
  61005: 'Right',
  61008: 'Down',
});

const linux = Object.assign({}, standard);

module.exports = { darwin, win32, linux };
