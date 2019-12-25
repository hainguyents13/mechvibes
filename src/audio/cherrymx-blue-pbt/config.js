const set_name = 'CherryMX Blue - PBT keycaps';
const set_tags = {
  switches: 'cherrymx',
  type: 'blue',
  feedback: 'tactile,clicky',
  is_lubed: false,
  keycap: 'abs',
  group: 'cherrymx',
};
const sound_file = 'sound.ogg';
const keychars = {
  // row 1
  escape: [1203, 192], // escape
  f1: [1783, 185], // f1
  f2: [2336, 204], // f2
  f3: [2884, 182], // f3
  f4: [3404, 188], // f4
  f5: [3940, 168], // f5
  f6: [4487, 165], // f6
  f7: [5013, 178], // f7
  f8: [5510, 187], // f8
  f9: [6048, 207], // f9
  f10: [6609, 174], // f10
  f11: [7119, 175], // f11
  f12: [7650, 174], // f12
  printscreen: [8159, 221], // 'print screen'
  scrolllock: [8668, 222], // 'scroll lock'
  pause: [9180, 193], // pause/break

  // row 2
  backquote: [10318, 183], // 'back quote'
  '1': [10794, 197], // '1'
  '2': [11315, 196], // '2'
  '3': [11814, 198], // '3'
  '4': [12334, 185], // '4'
  '5': [12845, 196], // '5'
  '6': [13305, 188], // '6'
  '7': [13846, 187], // '7'
  '8': [14362, 197], // '8'
  '9': [14881, 188], // '9'
  '0': [15408, 180], // '0'
  minus: [15901, 200], // minus
  equals: [16410, 182], // equals
  backspace: [16906, 230], // backspace
  insert: [17483, 175], // insert
  home: [17955, 189], // home
  pageup: [18419, 199], // 'page up'

  // row 3
  tab: [19473, 191], // tab
  q: [19944, 207], // q
  w: [20490, 183], // w
  e: [20995, 192], // e
  r: [21500, 186], // r
  t: [21993, 198], // t
  y: [22495, 185], // y
  u: [22970, 194], // u
  i: [23492, 179], // i
  o: [23972, 212], // o
  p: [24468, 181], // p
  openbracket: [24955, 187], // 'open bracket'
  closebracket: [25463, 172], // 'close bracket'
  backslash: [25952, 184], // 'back slash'
  delete: [26567, 177], // delete
  end: [27069, 190], // end
  pagedown: [27541, 169], // 'page down'

  // row 4
  capslock: [28470, 191], // 'caps lock'
  a: [28961, 164], // a
  s: [29448, 171], // s
  d: [29968, 163], // d
  f: [30454, 208], // f
  g: [30944, 172], // g
  h: [31395, 186], // h
  j: [31884, 174], // j
  k: [32358, 172], // k
  l: [32358, 172], // l
  semicolon: [33331, 191], // semicolon
  quote: [33824, 166], // quote
  enter: [34274, 204], // enter

  // row 5
  leftshift: [35598, 197], // 'left shift'
  z: [36143, 204], // z
  x: [36645, 165], // x
  c: [37120, 164], // c
  v: [37606, 181], // v
  b: [38062, 179], // b
  n: [38563, 163], // n
  m: [39025, 171], // m
  comma: [39505, 172], // comma
  period: [39982, 152], // period
  slash: [40437, 170], // slash
  rightshift: [40939, 176], // 'right shift'
  up: [41501, 174], // up

  // row 6
  leftcontrol: [42603, 172], // 'left control'
  leftmeta: [43435, 155], // 'left meta'
  leftalt: [43991, 192], // 'left alt'
  space: [49628, 230], // space
  rightalt: [46069, 202], // 'right alt'
  rightmeta: [46544, 174], // menu key // 'right meta'
  /* blank */
  rightcontrol: [47002, 188], // 'right control'
  left: [47492, 178], // left
  down: [47981, 201], // down
  right: [48412, 201], // right
};

module.exports = { set_name, set_tags, keychars, sound_file };
