class startupHandler {
  constructor(app) {
    this.app = app;
  }

  get is_enabled() {
    return this.app.getLoginItemSettings().openAtLogin;
  }

  enable() {
    this.app.setLoginItemSettings({
      openAtLogin: true,
    });
  }

  disable() {
    this.app.setLoginItemSettings({
      openAtLogin: false,
    });
  }

  toggle() {
    if (this.isEnabled) {
      this.disable();
    } else {
      this.enable();
    }
  }
}

module.exports = startupHandler;
