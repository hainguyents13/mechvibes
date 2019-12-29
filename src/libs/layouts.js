const standard = {
  // standard
  main: [
    [1, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 87, 88, 3639, 70, 3653],
    [41, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 3666, 3655, 3657],
    [15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 43, 3667, 3663, 3665],
    [58, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 28],
    [42, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 57416],
    [29, 3675, 56, 57, 3640, 3676, 3613, 57419, 57424, 57421],
  ],
  // numpad
  numpad: [
    [69, 3637, 55, 74],
    [71, 72, 73, 78],
    [75, 76, 77],
    [79, 80, 81, 3612],
    [82, 83],
  ],
};

function remap(to = 'win32') {
  const layout = Object.assign({}, standard);
  switch (to) {
    default:
    case 'linux':
      break;

    case 'win32': {
      // row 2
      layout.main[1][15] = 61010;
      layout.main[1][16] = 60999;
      layout.main[1][17] = 61001;
      // row 3
      layout.main[2][15] = 61011;
      layout.main[2][16] = 61007;
      layout.main[2][17] = 61009;
      // row 4
      layout.main[3][13] = 61000;
      // row 5
      layout.main[4][13] = 61003;
      layout.main[4][13] = 61008;
      layout.main[4][13] = 61005;
      break;
    }

    case 'darwin': {
      // row 1
      layout.main[0][14] = 91;
      layout.main[0][15] = 92;
      layout.main[0][16] = 93;
      // row 5
      layout.main[4][14] = 91;
      // row 6
      layout.main[5][1] = 56;
      layout.main[5][2] = 3675;
      layout.main[5][4] = 3675;
      layout.main[5][5] = 56;
      layout.main[5][6] = 29;

      // numpad
      layout.numpad[0] = [69, 3597, 3637, 55];
      layout.numpad[1][3] = 74;
      layout.numpad[2][3] = 78;
      break;
    }
  }

  return layout;
}

module.exports = { win32: remap('win32'), darwin: remap('darwin'), linux: standard };
