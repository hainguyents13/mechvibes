function remapper(from = 'standard', to = 'darwin', timing = []) {
  if (from == 'standard') {
    // from standard linux, darwin, win32
    switch (to) {
      case 'linux':
        break;
      case 'darwin': {
        timing[91] = timing[3639]; // mac f13
        timing[92] = timing[70]; // mac f14
        timing[93] = timing[3653]; // mac f15
        timing[91] = timing[57416];
        timing[56] = timing[3675];
        timing[3675] = timing[56];
        timing[3675] = timing[3640];
        timing[56] = timing[3676];
        timing[29] = timing[3613];

        timing[3597] = timing[69]; // numlock -> clear
        break;
      }
      case 'win32': {
        // row 2
        timing[61010] = timing[3666];
        timing[60999] = timing[3655];
        timing[61001] = timing[3657];
        // row 3
        timing[61011] = timing[3667];
        timing[61007] = timing[3663];
        timing[61009] = timing[3665];
        timing[3677] = timing[3613];
        // row 4
        timing[61000] = timing[57416];
        // row 5
        timing[61003] = timing[57419];
        timing[61008] = timing[57424];
        timing[61005] = timing[57421];
        break;
      }
    }
  } else {
    // from linux, darwin, win32 to standard
    switch (from) {
      case 'darwin': {
        timing[3639] = timing[91];
        timing[70] = timing[92];
        timing[3653] = timing[93];
        timing[57416] = timing[91];
        timing[3675] = timing[56];
        timing[56] = timing[3675];
        timing[3640] = timing[3675];
        timing[3676] = timing[56];
        timing[3613] = timing[29];
        break;
      }
      case 'win32': {
        // row 2
        timing[3666] = timing[61010];
        timing[3655] = timing[60999];
        timing[3657] = timing[61001];
        // row 3
        timing[3667] = timing[61011];
        timing[3663] = timing[61007];
        timing[3665] = timing[61009];
        // row 4
        timing[57416] = timing[61000];
        // row 5
        timing[57419] = timing[61003];
        timing[57424] = timing[61008];
        timing[57421] = timing[61005];
        break;
      }
    }
  }
  return timing;
}

module.exports = remapper;
