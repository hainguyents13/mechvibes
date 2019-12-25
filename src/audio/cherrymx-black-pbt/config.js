const set_name = 'CherryMX Black - PBT keycaps';
const set_tags = {
  switches: 'cherrymx',
  type: 'black',
  feedback: 'linear',
  is_lubed: false,
  keycap: 'pbt',
  group: 'cherrymx',
};
const sound_file = 'sound.ogg';
const keychars = {
  // row 1
  escape: [2078, 176], // escape
  f1: [2511, 186], // f1
  f2: [2919, 177], // f2
  f3: [3307, 185], // f3
  f4: [3700, 176], // f4
  f5: [4164, 148], // f5
  f6: [4613, 155], // f6
  f7: [5024, 150], // f7
  f8: [5438, 147], // f8
  f9: [5862, 149], // f9
  f10: [6270, 138], // f10
  f11: [6697, 137], // f11
  f12: [7122, 136], // f12
  printscreen: [7547, 134], // 'print screen'
  scrolllock: [7957, 141], // 'scroll lock'
  pause: [8372, 140], // pause/break

  // row 2
  backquote: [8869, 181], // 'back quote'
  '1': [9291, 164], // '1'
  '2': [9702, 153], // '2'
  '3': [10097, 140], // '3'
  '4': [10459, 165], // '4'
  '5': [10859, 165], // '5'
  '6': [11250, 165], // '6'
  '7': [11648, 159], // '7'
  '8': [12014, 150], // '8'
  '9': [12414, 136], // '9'
  '0': [12826, 137], // '0'
  minus: [13227, 138], // minus
  equals: [13625, 134], // equals
  backspace: [14035, 137], // backspace
  insert: [14524, 141], // insert
  home: [14949, 119], // home
  pageup: [15311, 143], // 'page up'

  // row 3
  tab: [15707, 163], // tab
  q: [16114, 157], // q
  w: [16515, 150], // w
  e: [16889, 147], // e
  r: [17253, 161], // r
  t: [17630, 156], // t
  y: [18024, 140], // y
  u: [18416, 134], // u
  i: [18815, 140], // i
  o: [19229, 128], // o
  p: [19632, 127], // p
  openbracket: [20024, 131], // 'open bracket'
  closebracket: [20438, 131], // 'close bracket'
  backslash: [20858, 129], // 'back slash'
  delete: [21286, 113], // delete
  end: [21671, 146], // end
  pagedown: [22078, 134], // 'page down'

  // row 4
  capslock: [22462, 166], // 'caps lock'
  a: [22850, 160], // a
  s: [23243, 156], // s
  d: [23645, 155], // d
  f: [24034, 152], // f
  g: [24445, 146], // g
  h: [24824, 148], // h
  j: [25181, 164], // j
  k: [25593, 145], // k
  l: [25993, 139], // l
  semicolon: [26393, 136], // semicolon
  quote: [26801, 128], // quote
  enter: [27154, 171], // enter

  // row 5
  leftshift: [28061, 186], // 'left shift'
  z: [28466, 161], // z
  x: [28845, 151], // x
  c: [29208, 156], // c
  v: [29571, 144], // v
  b: [29926, 159], // b
  n: [30248, 139], // n
  m: [30598, 140], // m
  comma: [30981, 130], // comma
  period: [31353, 128], // period
  slash: [31740, 126], // slash
  rightshift: [32135, 169], // 'right shift'
  up: [32948, 138], // up

  // row 6
  leftcontrol: [33678, 163], // 'left control'
  leftmeta: [34093, 157], // 'left meta'
  leftalt: [34451, 164], // 'left alt'
  space: [34906, 168], // space
  rightalt: [35316, 145], // 'right alt'
  rightmeta: [35716, 136], // menu key // 'right meta'
  /* blank */
  rightcontrol: [36449, 137], // 'right control'
  left: [36831, 144], // left
  down: [37213, 144], // down
  right: [37569, 155], // right
};

module.exports = { set_name, set_tags, keychars, sound_file };
