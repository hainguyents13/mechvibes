const set_name = 'CherryMX Blue - ABS keycaps';
const set_tags = {
  switches: 'cherrymx',
  type: 'blue',
  feedback: 'tactile,clicky',
  is_lubed: false,
  keycap: 'abs',
};
const sound_file = 'sound.ogg';
const keychars = {
  // row 1
  escape: [1754, 184], // escape
  f1: [2222, 186], // f1
  f2: [2617, 180], // f2
  f3: [3028, 189], // f3
  f4: [3385, 223], // f4
  f5: [3792, 193], // f5
  f6: [4136, 212], // f6
  f7: [4540, 188], // f7
  f8: [4903, 193], // f8
  f9: [5296, 193], // f9
  f10: [5666, 183], // f10
  f11: [6054, 180], // f11
  f12: [6425, 182], // f12
  printscreen: [6818, 167], // 'print screen'
  scrolllock: [7187, 183], // 'scroll lock'
  pause: [7583, 193], // pause/break

  // row 2
  backquote: [9749, 195], // 'back quote'
  '1': [10135, 199], // '1'
  '2': [10562, 185], // '2'
  '3': [10966, 189], // '3'
  '4': [11329, 199], // '4'
  '5': [11706, 196], // '5'
  '6': [12094, 180], // '6'
  '7': [12467, 184], // '7'
  '8': [12863, 190], // '8'
  '9': [13248, 195], // '9'
  '0': [13633, 170], // '0'
  minus: [13988, 186], // minus
  equals: [14372, 180], // equals
  backspace: [14748, 212], // backspace
  insert: [15156, 180], // insert
  home: [15526, 204], // home
  pageup: [15893, 157], // 'page up'

  // row 3
  tab: [16940, 179], // tab
  q: [17316, 199], // q
  w: [17700, 172], // w
  e: [18054, 187], // e
  r: [18400, 184], // r
  t: [18761, 176], // t
  y: [19116, 188], // y
  u: [19495, 186], // u
  i: [19876, 174], // i
  o: [20238, 170], // o
  p: [20605, 158], // p
  openbracket: [20976, 164], // 'open bracket'
  closebracket: [21348, 158], // 'close bracket'
  backslash: [21707, 182], // 'back slash'
  delete: [22116, 179], // delete
  end: [22513, 173], // end
  pagedown: [22862, 158], // 'page down'

  // row 4
  capslock: [23925, 207], // 'caps lock'
  a: [24330, 196], // a
  s: [24700, 202], // s
  d: [25071, 194], // d
  f: [25444, 206], // f
  g: [25803, 188], // g
  h: [26159, 185], // h
  j: [26534, 168], // j
  k: [26928, 190], // k
  l: [27347, 180], // l
  semicolon: [27733, 183], // semicolon
  quote: [28157, 176], // quote
  enter: [28558, 161], // enter

  // row 5
  leftshift: [29603, 226], // 'left shift'
  z: [30046, 175], // z
  x: [30385, 177], // x
  c: [30761, 189], // c
  v: [31123, 191], // v
  b: [31475, 196], // b
  n: [31891, 169], // n
  m: [32333, 175], // m
  comma: [33011, 186], // comma
  period: [33438, 172], // period
  slash: [33828, 178], // slash
  rightshift: [34215, 180], // 'right shift'
  up: [34704, 159], // up

  // row 6
  leftcontrol: [35733, 190], // 'left control'
  leftmeta: [36115, 205], // 'left meta'
  leftalt: [36465, 214], // 'left alt'
  space: [36804, 240], // space
  rightalt: [37730, 184], // 'right alt'
  rightmeta: [38116, 184], // menu key // 'right meta'
  /* blank */
  rightcontrol: [38821, 188], // 'right control'
  left: [39220, 169], // left
  down: [39589, 179], // down
  right: [39954, 183], // right
};

module.exports = { set_name, set_tags, keychars, sound_file };
