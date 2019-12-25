const set_name = 'CherryMX Red - ABS keycaps';
const set_tags = {
  switches: 'cherrymx',
  type: 'red',
  feedback: 'linear',
  is_lubed: false,
  keycap: 'abs',
  group: 'cherrymx',
};
const sound_file = 'sound.ogg';
const keychars = {
  // row 1
  escape: [2464, 170], // escape
  f1: [3016, 182], // f1
  f2: [3534, 177], // f2
  f3: [4003, 180], // f3
  f4: [3924, 200], // f4
  f5: [4472, 188], // f5
  f6: [4988, 179], // f6
  f7: [5476, 168], // f7
  f8: [5956, 183], // f8
  f9: [6419, 208], // f9
  f10: [6927, 202], // f10
  f11: [7434, 188], // f11
  f12: [7908, 182], // f12
  printscreen: [8367, 180], // 'print screen'
  scrolllock: [9329, 184], // 'scroll lock'
  pause: [9720, 182], // pause/break

  // row 2
  backquote: [10927, 181], // 'back quote'
  '1': [11371, 178], // '1'
  '2': [11846, 169], // '2'
  '3': [12302, 194], // '3'
  '4': [12746, 214], // '4'
  '5': [13183, 174], // '5'
  '6': [13619, 183], // '6'
  '7': [14064, 177], // '7'
  '8': [14504, 188], // '8'
  '9': [14956, 166], // '9'
  '0': [15408, 211], // '0'
  minus: [15823, 192], // minus
  equals: [16291, 181], // equals
  backspace: [16727, 227], // backspace
  insert: [17343, 177], // insert
  home: [17757, 181], // home
  pageup: [18135, 163], // 'page up'

  // row 3
  tab: [19161, 185], // tab
  q: [19606, 168], // q
  w: [20024, 194], // w
  e: [20478, 175], // e
  r: [20892, 171], // r
  t: [21250, 177], // t
  y: [21670, 186], // y
  u: [22098, 197], // u
  i: [22601, 174], // i
  o: [23102, 181], // o
  p: [23594, 186], // p
  openbracket: [24062, 150], // 'open bracket'
  closebracket: [24507, 192], // 'close bracket'
  backslash: [24956, 155], // 'back slash'
  delete: [25449, 166], // delete
  end: [25889, 175], // end
  pagedown: [26298, 141], // 'page down'

  // row 4
  capslock: [27462, 179], // 'caps lock'
  a: [27942, 151], // a
  s: [28366, 177], // s
  d: [28771, 168], // d
  f: [29171, 201], // f
  g: [29577, 175], // g
  h: [29986, 189], // h
  j: [30413, 183], // j
  k: [30821, 196], // k
  l: [31527, 243], // l
  semicolon: [32024, 192], // semicolon
  quote: [32459, 193], // quote
  enter: [32833, 190], // enter

  // row 5
  leftshift: [34277, 212], // 'left shift'
  z: [34773, 192], // z
  x: [35220, 199], // x
  c: [35671, 155], // c
  v: [36089, 158], // v
  b: [36453, 180], // b
  n: [36825, 190], // n
  m: [37239, 189], // m
  comma: [37706, 168], // comma
  period: [38150, 177], // period
  slash: [38597, 191], // slash
  rightshift: [39015, 200], // 'right shift'
  up: [39528, 180], // up

  // row 6
  leftcontrol: [40730, 163], // 'left control'
  leftmeta: [41166, 216], // 'left meta'
  leftalt: [41606, 182], // 'left alt'
  space: [42071, 230], // space
  rightalt: [43033, 175], // 'right alt'
  rightmeta: [43497, 203], // menu key // 'right meta'
  /* blank */
  rightcontrol: [44374, 197], // 'right control'
  left: [44811, 202], // left
  down: [45226, 203], // down
  right: [45618, 197], // right
};

module.exports = { set_name, set_tags, keychars, sound_file };
