const set_name = 'CherryMX Brown - PBT keycaps';
const set_tags = {
  switches: 'cherrymx',
  type: 'brown',
  feedback: 'tactile',
  is_lubed: false,
  keycap: 'pbt',
};
const sound_file = 'sound.ogg';
const keychars = {
  // row 1
  escape: [2280, 199], // escape
  f1: [2754, 189], // f1
  f2: [3155, 180], // f2
  f3: [3545, 187], // f3
  f4: [3913, 183], // f4
  f5: [4305, 174], // f5
  f6: [4666, 187], // f6
  f7: [5034, 200], // f7
  f8: [5433, 187], // f8
  f9: [7795, 198], // f9
  f10: [6146, 191], // f10
  f11: [6560, 196], // f11
  f12: [6932, 193], // f12
  printscreen: [7322, 177], // 'print screen'
  scrolllock: [7699, 178], // 'scroll lock'
  pause: [8036, 168], // pause/break

  // row 2
  backquote: [9069, 209], // 'back quote'
  '1': [9444, 185], // '1'
  '2': [9833, 187], // '2'
  '3': [10185, 194], // '3'
  '4': [10551, 196], // '4'
  '5': [10899, 194], // '5'
  '6': [11282, 180], // '6'
  '7': [11623, 188], // '7'
  '8': [11976, 200], // '8'
  '9': [12337, 197], // '9'
  '0': [12667, 194], // '0'
  minus: [13058, 191], // minus
  equals: [13408, 187], // equals
  backspace: [13765, 184], // backspace
  insert: [14199, 181], // insert
  home: [14522, 196], // home
  pageup: [14852, 169], // 'page up'

  // row 3
  tab: [15916, 176], // tab
  q: [16284, 150], // q
  w: [16637, 176], // w
  e: [16964, 190], // e
  r: [17275, 185], // r
  t: [17613, 196], // t
  y: [17957, 173], // y
  u: [18301, 190], // u
  i: [18643, 200], // i
  o: [18994, 178], // o
  p: [19331, 197], // p
  openbracket: [19671, 171], // 'open bracket'
  closebracket: [20020, 175], // 'close bracket'
  backslash: [20387, 176], // 'back slash'
  delete: [20766, 185], // delete
  end: [21102, 187], // end
  pagedown: [21409, 151], // 'page down'

  // row 4
  capslock: [22560, 181], // 'caps lock'
  a: [22869, 198], // a
  s: [23237, 178], // s
  d: [23586, 187], // d
  f: [23898, 179], // f
  g: [24237, 185], // g
  h: [24550, 192], // h
  j: [24917, 188], // j
  k: [25274, 185], // k
  l: [25625, 183], // l
  semicolon: [25989, 182], // semicolon
  quote: [26335, 180], // quote
  enter: [26703, 181], // enter

  // row 5
  leftshift: [28109, 180], // 'left shift'
  z: [28550, 167], // z
  x: [28855, 184], // x
  c: [29557, 204], // c
  v: [29557, 204], // v
  b: [29909, 179], // b
  n: [30252, 203], // n
  m: [30605, 184], // m
  comma: [30965, 212], // comma
  period: [31315, 176], // period
  slash: [31659, 175], // slash
  rightshift: [32015, 184], // 'right shift'
  up: [32429, 174], // up

  // row 6
  leftcontrol: [33857, 182], // 'left control'
  leftmeta: [34181, 177], // 'left meta'
  leftalt: [34551, 175], // 'left alt'
  space: [34910, 223], // space
  rightalt: [35492, 157], // 'right alt'
  rightmeta: [35878, 164], // menu key // 'right meta'
  /* blank */
  rightcontrol: [36571, 182], // 'right control'
  left: [36907, 163], // left
  down: [37267, 170], // down
  right: [37586, 160], // right
};

module.exports = { set_name, set_tags, keychars, sound_file };
