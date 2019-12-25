const set_name = 'CherryMX Brown - ABS keycaps';
const set_tags = {
  switches: 'cherrymx',
  type: 'brown',
  feedback: 'tactile',
  is_lubed: false,
  keycap: 'abs',
  group: 'cherrymx',
};
const sound_file = 'sound.ogg';
const keychars = {
  // row 1
  escape: [1402, 170], // escape
  f1: [1852, 162], // f1
  f2: [2275, 155], // f2
  f3: [2668, 167], // f3
  f4: [3054, 162], // f4
  f5: [3467, 161], // f5
  f6: [3877, 151], // f6
  f7: [4501, 120], // f7
  f8: [4951, 141], // f8
  f9: [5381, 133], // f9
  f10: [5779, 145], // f10
  f11: [6207, 139], // f11
  f12: [6637, 127], // f12
  printscreen: [7023, 161], // 'print screen'
  scrolllock: [7448, 142], // 'scroll lock'
  pause: [7874, 148], // pause/break

  // row 2
  backquote: [8365, 166], // 'back quote'
  '1': [8760, 173], // '1'
  '2': [9149, 167], // '2'
  '3': [9550, 165], // '3'
  '4': [9942, 150], // '4'
  '5': [10370, 145], // '5'
  '6': [10791, 140], // '6'
  '7': [11205, 150], // '7'
  '8': [11603, 136], // '8'
  '9': [12012, 148], // '9'
  '0': [12440, 140], // '0'
  minus: [12844, 131], // minus
  equals: [13231, 132], // equals
  backspace: [13667, 134], // backspace
  insert: [14084, 125], // insert
  home: [14500, 142], // home
  pageup: [14877, 146], // 'page up'

  // row 3
  tab: [15286, 161], // tab
  q: [15665, 180], // q
  w: [16052, 171], // w
  e: [16411, 174], // e
  r: [16772, 158], // r
  t: [17124, 178], // t
  y: [17478, 142], // y
  u: [17843, 146], // u
  i: [18212, 139], // i
  o: [18611, 148], // o
  p: [19036, 134], // p
  openbracket: [19440, 127], // 'open bracket'
  closebracket: [19854, 192], // 'close bracket'
  backslash: [20262, 143], // 'back slash'
  delete: [20713, 139], // delete
  end: [21121, 151], // end
  pagedown: [21527, 142], // 'page down'

  // row 4
  capslock: [21920, 176], // 'caps lock'
  a: [22324, 167], // a
  s: [22698, 167], // s
  d: [23072, 163], // d
  f: [23408, 178], // f
  g: [23770, 183], // g
  h: [24140, 145], // h
  j: [24525, 154], // j
  k: [24919, 138], // k
  l: [25314, 142], // l
  semicolon: [25711, 129], // semicolon
  quote: [26121, 123], // quote
  enter: [26548, 119], // enter

  // row 5
  leftshift: [27208, 158], // 'left shift'
  z: [27589, 164], // z
  x: [27978, 160], // x
  c: [28321, 160], // c
  v: [28694, 162], // v
  b: [29071, 152], // b
  n: [29485, 159], // n
  m: [29873, 137], // m
  comma: [30269, 139], // comma
  period: [30664, 132], // period
  slash: [31026, 138], // slash
  rightshift: [31455, 139], // 'right shift'
  up: [32054, 167], // up

  // row 6
  leftcontrol: [32761, 168], // 'left control'
  leftmeta: [33157, 170], // 'left meta'
  leftalt: [33530, 170], // 'left alt'
  space: [37338, 200], // space
  rightalt: [34354, 157], // 'right alt'
  rightmeta: [34762, 147], // menu key // 'right meta'
  /* blank */
  rightcontrol: [35572, 139], // 'right control'
  left: [35966, 143], // left
  down: [36356, 147], // down
  right: [36787, 170], // right
};

module.exports = { set_name, set_tags, keychars, sound_file };
