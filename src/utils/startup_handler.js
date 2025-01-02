class startupHandler {
  constructor(app) {
    this.app = app;
  }

  get is_enabled() {
    return this.app.getLoginItemSettings().openAtLogin;
  }

  get was_started_at_login() {
    if(process.platform == 'darwin') {
      return this.app.getLoginItemSettings().wasOpenedAtLogin;
    }else{
      return process.argv.includes('--startup');
    }
  }

  enable() {
    this.app.setLoginItemSettings({
      openAtLogin: true,
      args: ['--startup']
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
