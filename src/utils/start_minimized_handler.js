const Store = require('electron-store');
const store = new Store();

class startMinimizedHandler {
  constructor(app) {
    this.app = app;
    this.MV_MIN_LSID = 'mechvibes-start-minimized';
  }

  get is_enabled() {
    return store.get(this.MV_MIN_LSID);
  }

  enable() {
    store.set(this.MV_MIN_LSID, true);
  }

  disable() {
    store.set(this.MV_MIN_LSID, false);
  }

  toggle() {
    if (this.is_enabled) {
      this.disable();
    } else {
      this.enable();
    }
  }
}

module.exports = startMinimizedHandler;
