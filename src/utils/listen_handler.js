const Store = require('electron-store');
const store = new Store();

class startupHandler {
  constructor(app) {
    this.app = app;
    this.MV_MUTED_LSID = 'mechvibes-muted';
  }

  get is_muted() {
    return store.get(this.MV_MUTED_LSID);
  }

  enable() {
    store.set(this.MV_MUTED_LSID, true);
  }

  disable() {
    store.set(this.MV_MUTED_LSID, false);
  }

  toggle() {
    if (this.is_muted) {
      this.disable();
    } else {
      this.enable();
    }
  }
}

module.exports = startupHandler;
