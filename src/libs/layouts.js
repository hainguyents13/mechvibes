'use strict';

const standard = {
  // standard
  main: [
    [1, 0, 59, 60, 61, 62, 0, 63, 64, 65, 66, 67, 68, 87, 88],
    [41, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14],
    [15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 43],
    [58, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 28],
    [42, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54],
    [29, 3675, 56, 57, 3640, 3676, 3677, 3613],
  ],
  // edit
  edit: [
    [3639, 70, 3653], //
    [3666, 3655, 3657], //
    [3667, 3663, 3665], //
    [],
    [0, 57416, 0], //
    [57419, 57424, 57421], //
  ],
  // numpad
  numpad: [
    [], //
    [69, 3637, 55, 74],
    [71, 72, 73, 78],
    [75, 76, 77],
    [79, 80, 81, 3612],
    [82, 83],
  ],
};

function remap(to = 'win32') {
  const layout = JSON.parse(JSON.stringify(standard));
  switch (to) {
    case 'linux':
      break;

    case 'win32': {
      layout.edit[1] = [61010, 60999, 61001];
      layout.edit[2] = [61011, 61007, 61009];
      layout.edit[4] = [0, 61000, 0];
      layout.edit[5] = [61003, 61008, 61005];
      break;
    }

    case 'darwin': {
      layout.main[5][1] = 56;
      layout.main[5][2] = 3675;
      layout.main[5][4] = 3675;
      layout.main[5][5] = 56;
      layout.main[5][6] = 29;

      layout.edit[0][0] = [91, 91, 92];

      layout.numpad[0] = [69, 3597, 3637, 55];
      layout.numpad[1][3] = 74;
      layout.numpad[2][3] = 78;
      break;
    }
  }

  return layout;
}

const sizes = {
  15: 'key-15u',
  58: 'key-175u',
  42: 'key-225u',
  29: 'key-125u',
  3675: 'key-125u',
  56: 'key-125u',
  57: 'key-625u',
  3640: 'key-125u',
  3676: 'key-125u',
  3677: 'key-125u',
  3613: 'key-125u',
  54: 'key-275u',
  28: 'key-225u',
  43: 'key-15u',
  14: 'key-2u',

  82: 'key-2u',
  78: 'key-height-2u',
  3612: 'key-height-2u',
};

module.exports = { win32: remap('win32'), darwin: remap('darwin'), linux: standard, sizes };
