const set_name = 'CherryMX Red - PBT keycaps';
const set_tags = {
  switches: 'cherrymx',
  type: 'red',
  feedback: 'linear',
  is_lubed: false,
  keycap: 'pbt',
};
const sound_file = 'sound.ogg';
const keychars = {
  // row 1
  escape: [3339, 187], // escape
  f1: [3878, 151], // f1
  f2: [4347, 169], // f2
  f3: [4825, 166], // f3
  f4: [5272, 151], // f4
  f5: [5737, 157], // f5
  f6: [6174, 1194], // f6
  f7: [6635, 198], // f7
  f8: [7088, 186], // f8
  f9: [7578, 195], // f9
  f10: [7989, 222], // f10
  f11: [8467, 205], // f11
  f12: [8958, 190], // f12
  printscreen: [9455, 168], // 'print screen'
  scrolllock: [9935, 145], // 'scroll lock'
  pause: [10319, 214], // pause/break

  // row 2
  backquote: [11265, 179], // 'back quote'
  '1': [11673, 137], // '1'
  '2': [12175, 143], // '2'
  '3': [12601, 185], // '3'
  '4': [13056, 152], // '4'
  '5': [13512, 150], // '5'
  '6': [13919, 160], // '6'
  '7': [14411, 147], // '7'
  '8': [14853, 182], // '8'
  '9': [15330, 162], // '9'
  '0': [15771, 182], // '0'
  minus: [16215, 158], // minus
  equals: [16662, 170], // equals
  backspace: [17067, 207], // backspace
  insert: [17568, 158], // insert
  home: [18000, 195], // home
  pageup: [18413, 166], // 'page up'

  // row 3
  tab: [19327, 196], // tab
  q: [19717, 153], // q
  w: [20144, 159], // w
  e: [20556, 150], // e
  r: [20966, 158], // r
  t: [21371, 193], // t
  y: [21836, 186], // y
  u: [22303, 163], // u
  i: [22724, 198], // i
  o: [23198, 170], // o
  p: [23643, 168], // p
  openbracket: [24072, 179], // 'open bracket'
  closebracket: [24527, 175], // 'close bracket'
  backslash: [24965, 196], // 'back slash'
  delete: [25461, 177], // delete
  end: [25888, 193], // end
  pagedown: [26353, 210], // 'page down'

  // row 4
  capslock: [27372, 190], // 'caps lock'
  a: [27803, 158], // a
  s: [28294, 151], // s
  d: [28716, 178], // d
  f: [29158, 182], // f
  g: [29563, 156], // g
  h: [30053, 155], // h
  j: [30477, 148], // j
  k: [30945, 155], // k
  l: [31372, 172], // l
  semicolon: [31812, 188], // semicolon
  quote: [32290, 200], // quote
  enter: [32734, 230], // enter

  // row 5
  leftshift: [33773, 225], // 'left shift'
  z: [34211, 139], // z
  x: [34623, 154], // x
  c: [34981, 174], // c
  v: [35448, 156], // v
  b: [35870, 188], // b
  n: [36310, 170], // n
  m: [36715, 188], // m
  comma: [37150, 165], // comma
  period: [37577, 177], // period
  slash: [38025, 173], // slash
  rightshift: [38477, 180], // 'right shift'
  up: [39044, 186], // up

  // row 6
  leftcontrol: [40191, 178], // 'left control'
  leftmeta: [40588, 184], // 'left meta'
  leftalt: [40985, 175], // 'left alt'
  space: [41553, 280], // space
  rightalt: [42367, 194], // 'right alt'
  rightmeta: [43196, 203], // menu key // 'right meta'
  /* blank */
  rightcontrol: [43623, 175], // 'right control'
  left: [44074, 175], // left
  down: [44511, 178], // down
  right: [44989, 179], // right
};

module.exports = { set_name, set_tags, keychars, sound_file };
