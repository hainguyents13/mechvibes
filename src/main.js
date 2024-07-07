// Modules to control application life and create native browser window
const { app, BrowserWindow, Tray, Menu, shell, ipcMain } = require('electron');
const { getVolume, getMute } = require('easy-volume');
const path = require('path');
const os = require("os");
const fs = require('fs-extra');
// NOTE: Do not update electron-log, as we have a custom transport override which may not be compatible with newer versions.
const log = require("electron-log");
const Store = require("electron-store");
const store = new Store();
const iohook = require('iohook');

const StartupHandler = require('./utils/startup_handler');
const StoreToggle = require('./utils/store_toggle');

const SYSTRAY_ICON = path.join(__dirname, '/assets/system-tray-icon.png');
const home_dir = app.getPath('home');
const user_dir = app.getPath("userData");
const custom_dir = path.join(home_dir, '/mechvibes_custom');
const current_pack_store_id = 'mechvibes-pack';

const mute = new StoreToggle("mechvibes-muted", false);
const start_minimized = new StoreToggle("mechvibes-start-minimized", false);
const active_volume = new StoreToggle("mechvibes-active-volume", true);

// Remote debugging defaults
const IpcServer = require("./utils/ipc");
let debug = {
  enabled: false, // the user must enable remote debugging via the debug options window
  identifier: undefined, // the ipc server should be configured to provide unique identifiers for live debugging sessions
  remoteUrl: "https://beta.mechvibes.com/debug/ipc/",
  async enable() {
    this.enabled = true;
    const userInfo = {
      hostname: os.hostname(), // Lunas-Macbook-Pro.local
      username: os.userInfo().username, // lunaalfien
      platform: os.platform(), // darwin
      version: app.getVersion() // v2.3.5
    };

    if(this.identifier === undefined){
      const json = await IpcServer.identify(userInfo);
      if(json.success){
        this.identifier = json.identifier;
        fs.writeJsonSync(debugConfigFile, {enabled: true, identifier: json.identifier});
        log.transports.remote.client.identifier = this.identifier;
        // TODO: set the level based on what the debugger wants
        // We're going to set the level to silly for now, because we don't have a way to live-update the level,
        // when the debugger changes the level, so we'll just set it to the most verbose level.
        // But this should absolutely be changed, and soon because it is an unnecessary load on the server.
        log.transports.remote.level = "silly";
        // NOTE: Remote debugging will include a websocket connection in the future, but it wasn't implemented
        // yet due to weird issues with the version of electron we use, and the version of node it uses,
        // causing an SSL error saying that the certificate was expired when it wasn't.
        // TODO: Check if the electron update fixed the above mentioned issue.
        const options = {
          enabled: debug.enabled,
          level: log.transports.remote.level,
          identifier: debug.identifier
        };
        if(debugWindow !== null){
          debugWindow.webContents.send("debug-update", options);
        }
      }else{
        this.enabled = false;
        console.log(json);
      }
    }else{
      // TODO: set the level based on what the debugger wants
      console.log("enabling early");
      log.transports.remote.client.identifier = this.identifier;
      log.transports.remote.level = "silly";
      const json = await IpcServer.validate(this.identifier, userInfo);
      if(!json.success){
        console.log("Failed validation");
        log.transports.remote.level = false;
        this.enabled = false;
        this.identifier = undefined;
        fs.unlinkSync(debugConfigFile);
      }
    }
    if(win !== null){
      win.webContents.send("debug-in-use", true);
    }
  },
  disable() {
    this.enabled = false;
    this.identifier = undefined; // clear identifier, for user privacy
    log.transports.remote.level = false;
    log.transports.remote.client.identifier = undefined;
    fs.unlinkSync(debugConfigFile);
    // send a request to the ipc server to remove the user's information immediately.
    // NOTE: if the ipc server fails to process the delete request, user logs might not be removed,
    // depending on ipc server implementation. For this reason, users should only use the official ipc server,
    // which is bound by the debug data retention policy.
    // https://beta.mechvibes.com/blog/debug-data-retention-policy/
    // transport.clear();

    if(win !== null){
      win.webContents.send("debug-in-use", false);
    }
  }
}
IpcServer.setRemoteUrl(debug.remoteUrl);

// Override the default remote logger, to use our own implementation.
// TODO: you know what, just move everything inside this tbh.
log.transports.remote = require("./libs/electron-log/transports/remote")(log, debug.remoteUrl);

// fix so we can detect transport type from within transport hook (see log.hooks.push(...))
for (const transportName in log.transports) {
  log.transports[transportName].transportName = transportName;
}

// parse debugging options
const debugConfigFile = path.join(user_dir, "/remote-debug.json");
if(fs.existsSync(debugConfigFile)){
  const json = require(debugConfigFile);
  console.log(json);
  if(json.identifier){
    debug.identifier = json.identifier;
    if(json.enabled){
      debug.enable();
      console.log("enabled?");
    }
  }else{
    fs.unlinkSync(debugConfigFile);
  }
  // log.transports.remote.level = debug.level;
}
log.transports.file.fileName = "mechvibes.log";
log.transports.file.level = "info";
log.transports.file.resolvePath = (variables) => {
  // ~/mechvibes.log
  // eg. /Users/lunaalfien/mechvibes.log
  return path.join(variables.home, variables.fileName);
}
log.variables.sender = "main";
// console.log(log.transports.console.format); // uncomment to see default formats in console
// console.log(log.transports.file.format); // uncomment to see default formats in console
log.transports.console.format = "%c{h}:{i}:{s}.{ms}%c {sender} › {text}"
log.transports.file.format = "[{y}-{m}-{d} {h}:{i}:{s}.{ms}] [{level}]({sender}) {text}"

const LogTransportMap = { error: 'red', warn: 'yellow', info: 'cyan', debug: 'magenta', silly: 'green', default: 'unset' };
log.hooks.push((msg, {transportName}) => {
  if (transportName === 'console') {
    // apply color, only to console transport
    return {
      ...msg,
      data: [`color: ${LogTransportMap[msg.level]}`, 'color: unset', ...msg.data]
    };
  }
  return msg;
});

// const custom_dir = path.join(user_dir, "/custom");

// Keep a global reference of the window object, if you don't, the window will
// be closed automatically when the JavaScript object is garbage collected.
var win = null;
var tray = null;
global.app_version = app.getVersion();
global.custom_dir = custom_dir;
global.current_pack_store_id = current_pack_store_id;
global.debug_config_path = debugConfigFile;
// create custom sound folder if not exists
fs.ensureDirSync(custom_dir);

function createWindow(show = false) {
  // Create the browser window.
  win = new BrowserWindow({
    name: "app", // used by logger to differentiate messages sent by different windows.
    width: 400,
    height: 600,
    webSecurity: false,
    // resizable: false,
    // fullscreenable: false,
    webPreferences: {
      preload: path.join(__dirname, 'app.js'),
      contextIsolation: false,
      nodeIntegration: true,
      enableRemoteModule: true,
    },
    show,
  });

  // remove menu bar
  win.removeMenu();

  // and load the index.html of the app.
  win.loadFile('./src/app.html');

  // Open the DevTools.
  // win.openDevTools();
  // win.webContents.openDevTools();

  win.webContents.on("did-finish-load", () => {
    if(debug.enabled){
      win.webContents.send("debug-in-use", true);
    }
    win.webContents.send("ava-toggle", active_volume.is_enabled);
    win.webContents.send("mechvibes-mute-status", mute.is_enabled);
  })

  // Emitted when the window is closed.
  win.on('closed', function () {
    // Dereference the window object, usually you would store windows
    // in an array if your app supports multi windows, this is the time
    // when you should delete the corresponding element.
    win = null;
  });

  win.on('close', function (event) {
    if (!app.isQuiting) {
      if (process.platform === 'darwin') {
        app.dock.hide();
      }
      event.preventDefault();
      win.hide();
    }
    return false;
  });

  win.on("unresponsive", () => {
    log.warn("Window has entered unresponsive state");
    console.log("unresponsive");
  })

  // condition for start_minimized
  if (start_minimized.is_enabled) {
    win.close();
  } else {
    win.show();
  }

  return win;
}

let installer = null;
function openInstallWindow(packId){
  // Create the browser window.
  installer = new BrowserWindow({
    width: 300,
    height: 200,
    useContentSize: false,
    webSecurity: false,
    // resizable: false,
    // fullscreenable: false,
    webPreferences: {
      preload: path.join(__dirname, 'install.js'),
      contextIsolation: false,
      nodeIntegration: true,
    },
    show: false,
    parent: win,
  });

  // remove menu bar
  installer.removeMenu();

  // and load the index.html of the app.
  installer.loadFile('./src/install.html');

  installer.webContents.on("did-finish-load", () => {
    installer.webContents.send("install-pack", packId);
  })

  installer.on("ready-to-show", () => {
    installer.show();
  })

  // Emitted when the window is closed.
  installer.on('closed', function () {
    // Dereference the window object, usually you would store windows
    // in an array if your app supports multi windows, this is the time
    // when you should delete the corresponding element.
    installer = null;
  });
}

let debugWindow = null;
function createDebugWindow(){
  // Create the browser window.
  debugWindow = new BrowserWindow({
    width: 350,
    height: 500,
    useContentSize: false,
    webSecurity: false,
    // resizable: false,
    // fullscreenable: false,
    webPreferences: {
      preload: path.join(__dirname, 'debug.js'),
      contextIsolation: false,
      nodeIntegration: true,
    },
    show: false,
    parent: win,
  });

  // remove menu bar
  debugWindow.removeMenu();

  // and load the index.html of the app.
  debugWindow.loadFile('./src/debug.html');

  debugWindow.webContents.on("did-finish-load", () => {
    const options = {
      enabled: debug.enabled,
      level: log.transports.remote.level,
      identifier: debug.identifier
    };
    debugWindow.webContents.send("debug-options", options);
  })

  ipcMain.on("fetch-debug-options", () => {
    const options = {...debug, path: debugConfigFile};
    debugWindow.webContents.send("debug-options", options);
  })

  debugWindow.on("ready-to-show", () => {
    debugWindow.show();
  })

  // Emitted when the window is closed.
  debugWindow.on('closed', function () {
    // Dereference the window object, usually you would store windows
    // in an array if your app supports multi windows, this is the time
    // when you should delete the corresponding element.
    debugWindow = null;
  });
}

const gotTheLock = app.requestSingleInstanceLock();
app.on('second-instance', () => {
  // Someone tried to run a second instance, we should focus our window.
  if (win) {
    if (process.platform === 'darwin') {
      app.dock.show();
    }
    win.show();
    win.focus();
  }
});

const protocolCommands = {
  install(packId){
    if(installer === null){
      log.debug(`Processing request to install ${packId}...`);
      openInstallWindow(packId);
    }else{
      installer.focus();
      installer.webContents.send("install-pack", packId);
    }
  }
}
function callProtocolCommand(command, ...args){
  protocolCommands[command](...args);
}

if (!gotTheLock) {
  app.quit();
} else {
  app.on('second-instance', (event, commandLine, workingDirectory) => {
    // Someone tried to run a second instance, we should focus our window.
    if (win) {
      if (process.platform === 'darwin') {
        app.dock.show();
      }else{
        // when we reach this code, we're hitting open-url on win or linux
        // Note, this doesn't occur on macos, we have to use open-url below.
        const url = commandLine.pop();
        const command = decodeURI(url.slice("mechvibes://".length)).split(" ");
        if(protocolCommands[command[0]]){
          callProtocolCommand(...command);
        }
      }
      if (win.isMinimized()) {
        win.restore();
      }
      win.show();
      win.focus();
    }
  });

  app.on("open-url", (event, url) => {
    const command = decodeURI(url.slice("mechvibes://".length)).split(" ");
    if(protocolCommands[command[0]]){
      callProtocolCommand(...command);
    }
  })

  // This method will be called when Electron has finished
  // initialization and is ready to create browser windows.
  // Some APIs can only be used after this event occurs.
  // Don't show the window and create a tray instead
  // create and get window instance
  app.on('ready', () => {
    log.silly("Ready event has fired.");
    app.setAsDefaultProtocolClient('mechvibes');

    log.silly("Creating main window for the first time...");
    win = createWindow(true);

    const startup_handler = new StartupHandler(app);
    if(!mute.is_enabled){
      iohook.start();
    }

    let volume = -1; // set to an out-of-bound value to force an update on first run
    let system_mute = false;
    let system_volume_error = false;
    let sys_check_interval = setInterval(() => {
      if(!mute.is_enabled){
        getVolume().then((v) => {
          if(v !== volume){
            volume = v;
            win.webContents.send("system-volume-update", volume);
          }
        }).catch((err) => {
          clearInterval(sys_check_interval);
          if(err == "" && !system_volume_error){
            // this condition appears to only be hit when using ctrl+c to kill the app during development or on windows
            system_volume_error = true;
          }
          log.error(`Volume Error: ${err}`);
        });

        getMute().then((m) => {
          if(m !== system_mute){
            system_mute = m;
            win.webContents.send("system-mute-status", system_mute);
          }
        }).catch((err) => {
          clearInterval(sys_check_interval);
          if(err == "" && !system_volume_error){
            // this condition appears to only be hit when using ctrl+c to kill the app during development.
            system_volume_error = true;
            OnBeforeQuit();
            app.exit(1);
          }
          log.error(`Mute Error: ${err}`);
        });
      }
    }, 3000);
    // NOTE: we could go lower than 3 seconds, but the problem is, the system volume check is slow,
    // so it's not a good idea to spam the system with requests.

    iohook.on('keydown', (event) => {
      win.webContents.send("keydown", event);
    });

    iohook.on('keyup', (event) => {
      win.webContents.send("keyup", event);
    });

    function createTrayIcon(){
      // prevent dupe tray icons
      if(tray !== null) return;

      // start tray icon
      tray = new Tray(SYSTRAY_ICON);

      // tray icon tooltip
      tray.setToolTip('Mechvibes');

      // context menu when hover on tray icon
      const contextMenu = Menu.buildFromTemplate([
        {
          label: 'Mechvibes',
          click: function () {
            // show app on click
            if (process.platform === 'darwin') {
              app.dock.show();
            }
            win.show();
            win.focus();
          },
        },
        {
          label: 'Editor',
          click: function () {
            openEditorWindow();
          },
        },
        {
          label: 'Folders',
          submenu: [
            {
              label: 'Custom Soundpacks',
              click: function () {
                shell.openPath(custom_dir).then((err) => {
                  if(err){
                    log.error(err);
                  }
                });
              },
            },
            {
              label: 'Application Data',
              click: function () {
                shell.openPath(user_dir).then((err) => {
                  if(err){
                    log.error(err);
                  }
                });
              },
            },
          ],
        },
        {
          label: 'Mute',
          type: 'checkbox',
          checked: mute.is_enabled,
          click: function () {
            mute.toggle();
            if(!mute.is_enabled){
              iohook.start();
            }else{
              iohook.stop();
            }
            win.webContents.send("mechvibes-mute-status", mute.is_enabled);
          },
        },
        {
          label: 'Extras',
          submenu: [
            {
              label: 'Enable at Startup',
              type: 'checkbox',
              checked: startup_handler.is_enabled,
              click: function () {
                startup_handler.toggle();
              },
            },
            {
              label: 'Start Minimized',
              type: 'checkbox',
              checked: start_minimized.is_enabled,
              click: function () {
                start_minimized.toggle();
              },
            },
            {
              label: 'Active Volume Adjustment',
              type: 'checkbox',
              checked: active_volume.is_enabled,
              click: function () {
                active_volume.toggle();
                win.webContents.send("ava-toggle", active_volume.is_enabled);
              },
            },
          ],
        },
        {
          label: 'Quit',
          click: function () {
            // stop system check interval, because it's an external program, and
            // it doesn't know how to handle shutdowns.
            clearInterval(sys_check_interval);
            // quit
            app.isQuiting = true;
            app.quit();
          },
        },
      ]);

      // On macOS double click doesn't work if we use tray.setContextMenu(), so we'll do it manually.
      if(process.platform == "darwin"){
        // click on tray icon, show context menu
        tray.on('click', () => {
          tray.popUpContextMenu(contextMenu);
        });

        // right click on tray icon, show the app
        tray.on("right-click", () => {
          app.dock.show();
          win.show();
          win.focus();
        })
      }else{
        tray.setContextMenu(contextMenu);
        // double click on tray icon, show the app
        tray.on("double-click", () => {
          win.show();
          win.focus();
        })
      }
    }

    ipcMain.on("show_tray_icon", (event, show) => {
      if(show && tray === null){
        createTrayIcon();
      }else if(!show && tray !== null){
        tray.destroy()
        tray = null;
      }else if(!show && tray === null){
        createTrayIcon();
      }
    })

    ipcMain.on("electron-log", (event, message, level) => {
      const window_options = event.sender.browserWindowOptions;
      if(window_options.name !== undefined && typeof window_options.name == "string"){
        log.variables.sender = window_options.name
      }else{
        log.variables.sender = "u/w"; // unknown window
      }
      log[level](message);
      log.variables.sender = "main"; // reset sender
    })

    ipcMain.on("open-debug-options", (event) => {
      createDebugWindow();
    })

    ipcMain.on("set-debug-options", (event, json) => {
      if(json.enabled && !debug.enabled){
        debug.enable();
      }else if(!json.enabled && debug.enabled){
        debug.disable();
      }
    })

    // allow the installer to set its size using the height of the body so that when content changes,
    // the installer can only be as big or as small as it needs to be.
    ipcMain.on("resize-installer", (event, size) => {
      const diff = installer.getSize()[1] - installer.getContentSize()[1];
      log.silly(`Installer requested ${size}, offset is ${diff}, so size is ${(size + diff)}`);
      installer.setSize(300, size + diff, true);
    })
    ipcMain.on("installed", (event, packFolder) => {
      log.silly(`Installed ${packFolder}`);
      store.set(current_pack_store_id, "custom-" + packFolder);
      win.reload();
      installer.close();
      installer = null;
    })

    log.debug(`Platform: ${process.platform}`);
    log.info("App is ready and has been initialized");

    // prevent Electron app from interrupting macOS system shutdown
    if (process.platform == 'darwin') {
      const { powerMonitor } = require('electron');
      powerMonitor.on('shutdown', () => {
        app.quit();
      });
    }
  });
}

app.commandLine.appendSwitch('autoplay-policy', 'no-user-gesture-required');

// Quit when all windows are closed.
app.on('window-all-closed', function () {
  // On macOS it is common for applications and their menu bar
  // to stay active until the user quits explicitly with Cmd + Q
  log.silly("All windows were closed.");
  if (process.platform !== 'darwin') app.quit();
});

app.on('activate', function () {
  // On macOS it's common to re-create a window in the app when the
  // dock icon is clicked and there are no other windows open.
  log.silly("App has been activated")
  if (win === null){
    createWindow(true);
  }else{
    // on macOS clicking the app icon in the launcher or in finder, triggers activate instead of second-instance for some reason.
    if (process.platform === 'darwin') {
      app.dock.show();
    }
    if (win.isMinimized()) {
      win.restore();
    }
    win.show();
    win.focus();
  }
});

// ensure app gets unregistered
function OnBeforeQuit(){
  log.silly("Shutting down...");
  app.removeAsDefaultProtocolClient("mechvibes");
}
app.on("before-quit", OnBeforeQuit);

// always be sure that your application handles the 'quit' event in your main process
app.on('quit', () => {
  log.silly("Goodbye.");
  app.quit();
});

var editor_window = null;

function openEditorWindow() {
  if (editor_window) {
    editor_window.focus();
    return;
  }

  editor_window = new BrowserWindow({
    width: 1200,
    height: 600,
    // resizable: false,
    // minimizable: false,
    // fullscreenable: false,
    // modal: true,
    // parent: win,
    webPreferences: {
      // preload: path.join(__dirname, 'editor.js'),
      nodeIntegration: true,
    },
  });

  // editor_window.openDevTools();

  editor_window.loadFile('./src/editor.html');

  editor_window.on('closed', function () {
    editor_window = null;
  });
}
