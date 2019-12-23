const set_name = 'Topre Purple Hybrid - PBT keycaps';
const set_tags = {
  switches: 'topre',
  type: 'purple',
  feedback: 'tactile',
  is_lubed: false,
  keycap: 'pbt',
  desc: 'Topre Purple Hybrid from Novatouch',
  keyboard: 'CMStorm Novatouch',
};
const sound_file = 'sound.ogg';
const keychars = {
  // row 1
  escape: [1813, 130], // escape
  f1: [3076, 149], // f1
  f2: [4300, 166], // f2
  f3: [5415, 168], // f3
  f4: [6581, 162], // f4
  f5: [7724, 184], // f5
  f6: [8860, 197], // f6
  f7: [9972, 195], // f7
  f8: [1119, 182], // f8
  f9: [12242, 178], // f9
  f10: [13401, 194], // f10
  f11: [14479, 189], // f11
  f12: [15559, 187], // f12
  printscreen: [16619, 166], // 'print screen'
  scrolllock: [17743, 200], // 'scroll lock'
  pause: [18833, 200], // pause/break

  // row 2
  backquote: [19950, 190], // 'back quote'
  '1': [20987, 180], // '1'
  '2': [22041, 183], // '2'
  '3': [23126, 194], // '3'
  '4': [24260, 191], // '4'
  '5': [25371, 185], // '5'
  '6': [26392, 189], // '6'
  '7': [27457, 180], // '7'
  '8': [28502, 175], // '8'
  '9': [29566, 192], // '9'
  '0': [30619, 200], // '0'
  minus: [31656, 191], // minus
  equals: [31712, 191], // equals
  backspace: [33746, 190], // backspace
  insert: [34738, 166], // insert
  home: [35728, 186], // home
  pageup: [36825, 175], // 'page up'

  // row 3
  tab: [37933, 184], // tab
  q: [3900, 180], // q
  w: [39991, 188], // w
  e: [40955, 200], // e
  r: [41924, 190], // r
  t: [42918, 192], // t
  y: [43909, 180], // y
  u: [44822, 185], // u
  i: [45779, 194], // i
  o: [46763, 175], // o
  p: [47787, 200], // p
  openbracket: [48805, 208], // 'open bracket'
  closebracket: [49764, 171], // 'close bracket'
  backslash: [50707, 183], // 'back slash'
  delete: [51695, 200], // delete
  end: [52666, 163], // end
  pagedown: [53744, 192], // 'page down'

  // row 4
  capslock: [55235, 205], // 'caps lock'
  a: [56440, 177], // a
  s: [57571, 198], // s
  d: [58641, 182], // d
  f: [59729, 182], // f
  g: [60765, 183], // g
  h: [61898, 171], // h
  j: [63008, 180], // j
  k: [64156, 168], // k
  l: [65199, 171], // l
  semicolon: [66281, 167], // semicolon
  quote: [67418, 175], // quote
  enter: [68501, 174], // enter

  // row 5
  leftshift: [69592, 186], // 'left shift'
  z: [70895, 174], // z
  x: [72019, 184], // x
  c: [73119, 192], // c
  v: [74276, 198], // v
  b: [75328, 193], // b
  n: [76458, 170], // n
  m: [77514, 190], // m
  comma: [78570, 180], // comma
  period: [79601, 175], // period
  slash: [80675, 160], // slash
  rightshift: [81734, 176], // 'right shift'
  up: [82838, 211], // up

  // row 6
  leftcontrol: [84014, 174], // 'left control'
  leftmeta: [85179, 173], // 'left meta'
  leftalt: [86211, 187], // 'left alt'
  space: [87213, 181], // space
  rightalt: [88293, 169], // 'right alt'
  rightmeta: [89346, 164], // menu key // 'right meta'
  /* blank */
  rightcontrol: [91445, 198], // 'right control'
  left: [92565, 173], // left
  down: [93711, 174], // down
  right: [93739, 195], // right
};

module.exports = { set_name, set_tags, keychars, sound_file };
