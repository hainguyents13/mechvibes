function remapper(from = 'standard', to = 'darwin', defines = {}) {
  if (from == 'standard') {
    // from standard linux, darwin, win32
    switch (to) {
      case 'linux':
        break;
      case 'darwin': {
        defines['91'] = defines['3639']; // mac f13
        defines['92'] = defines['70']; // mac f14
        defines['93'] = defines['3653']; // mac f15
        defines['91'] = defines['57416'];
        defines['56'] = defines['3675'];
        defines['3675'] = defines['56'];
        defines['3675'] = defines['3640'];
        defines['56'] = defines['3676'];
        defines['29'] = defines['3613'];

        defines['3597'] = defines['69']; // numlock -> clear
        break;
      }
      case 'win32': {
        // row 2
        defines['61010'] = defines['3666'];
        defines['60999'] = defines['3655'];
        defines['61001'] = defines['3657'];
        // row 3
        defines['61011'] = defines['3667'];
        defines['61007'] = defines['3663'];
        defines['61009'] = defines['3665'];
        defines['3677'] = defines['3613'];
        // row 4
        defines['61000'] = defines['57416'];
        // row 5
        defines['61003'] = defines['57419'];
        defines['61008'] = defines['57424'];
        defines['61005'] = defines['57421'];
        break;
      }
    }
  } else {
    // from linux, darwin, win32 to standard
    switch (from) {
      case 'darwin': {
        defines['3639'] = defines['91'];
        defines['70'] = defines['92'];
        defines['3653'] = defines['93'];
        defines['3675'] = defines['56'];
        defines['56'] = defines['3675'];
        defines['3640'] = defines['3675'];
        defines['3676'] = defines['56'];
        defines['3613'] = defines['29'];
        break;
      }
      case 'win32': {
        // row 2
        defines['3666'] = defines['61010'];
        defines['3655'] = defines['60999'];
        defines['3657'] = defines['61001'];
        // row 3
        defines['3667'] = defines['61011'];
        defines['3663'] = defines['61007'];
        defines['3665'] = defines['61009'];
        // row 4
        defines['57416'] = defines['61000'];
        // row 5
        defines['57419'] = defines['61003'];
        defines['57424'] = defines['61008'];
        defines['57421'] = defines['61005'];
        break;
      }
    }
  }
  return defines;
}

module.exports = remapper;
