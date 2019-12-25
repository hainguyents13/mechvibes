const set_name = 'CherryMX Black - ABS keycaps';
const set_tags = {
  switches: 'cherrymx',
  type: 'black',
  feedback: 'linear',
  is_lubed: false,
  keycap: 'abs',
  group: 'cherrymx',
};
const sound_file = 'sound.ogg';
const keychars = {
  // row 1
  escape: [2894, 226], // escape
  f1: [3610, 195], // f1
  f2: [4210, 180], // f2
  f3: [4758, 180], // f3
  f4: [5250, 199], // f4
  f5: [5831, 209], // f5
  f6: [6396, 210], // f6
  f7: [6900, 210], // f7
  f8: [7443, 221], // f8
  f9: [7955, 181], // f9
  f10: [8504, 209], // f10
  f11: [9046, 187], // f11
  f12: [9582, 191], // f12
  printscreen: [132, 208], // 'print screen'
  scrolllock: [10763, 160], // 'scroll lock'
  pause: [11270, 200], // pause/break

  // row 2
  backquote: [12476, 200], // 'back quote'
  '1': [12946, 191], // '1'
  '2': [13470, 190], // '2'
  '3': [13963, 199], // '3'
  '4': [14481, 204], // '4'
  '5': [14994, 187], // '5'
  '6': [15505, 217], // '6'
  '7': [15990, 193], // '7'
  '8': [16529, 184], // '8'
  '9': [17012, 205], // '9'
  '0': [17550, 174], // '0'
  minus: [18052, 186], // minus
  equals: [18553, 177], // equals
  backspace: [19065, 220], // backspace
  insert: [19634, 171], // insert
  home: [20143, 202], // home
  pageup: [20647, 193], // 'page up'

  // row 3
  tab: [21734, 238], // tab
  q: [22245, 190], // q
  w: [22790, 177], // w
  e: [23317, 166], // e
  r: [23817, 184], // r
  t: [24297, 183], // t
  y: [24811, 186], // y
  u: [25313, 189], // u
  i: [25795, 182], // i
  o: [26309, 167], // o
  p: [26804, 166], // p
  openbracket: [27330, 169], // 'open bracket'
  closebracket: [27883, 197], // 'close bracket'
  backslash: [28393, 200], // 'back slash'
  delete: [28914, 212], // delete
  end: [29427, 177], // end
  pagedown: [29928, 190], // 'page down'

  // row 4
  capslock: [31011, 251], // 'caps lock'
  a: [31542, 170], // a
  s: [32031, 175], // s
  d: [32492, 169], // d
  f: [32973, 174], // f
  g: [33453, 188], // g
  h: [33986, 185], // h
  j: [34425, 176], // j
  k: [34932, 180], // k
  l: [35410, 190], // l
  semicolon: [35914, 189], // semicolon
  quote: [36428, 173], // quote
  enter: [36902, 234], // enter

  // row 5
  leftshift: [38136, 265], // 'left shift'
  z: [38694, 160], // z
  x: [39148, 151], // x
  c: [39632, 190], // c
  v: [40136, 188], // v
  b: [40621, 214], // b
  n: [41103, 180], // n
  m: [41610, 186], // m
  comma: [42110, 183], // comma
  period: [42594, 180], // period
  slash: [43105, 190], // slash
  rightshift: [43565, 273], // 'right shift'
  up: [44251, 220], // up

  // row 6
  leftcontrol: [45327, 165], // 'left control'
  leftmeta: [45750, 164], // 'left meta'
  leftalt: [46199, 199], // 'left alt'
  space: [51541, 287], // space
  rightalt: [47929, 149], // 'right alt'
  rightmeta: [48381, 168], // menu key // 'right meta'
  /* blank */
  rightcontrol: [49329, 164], // 'right control' ////////////////////// up
  left: [49837, 176], // left
  down: [50333, 179], // down
  right: [50783, 221], // right
};

module.exports = { set_name, set_tags, keychars, sound_file };
