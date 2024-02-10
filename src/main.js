// Modules to control application life and create native browser window
const { app, BrowserWindow, Tray, Menu, shell, ipcMain } = require('electron');
const path = require('path');
const os = require("os");
const fs = require('fs-extra');
const log = require("electron-log");
const Store = require("electron-store");
const store = new Store();
const iohook = require('iohook');
const fetch = require("./utils/fetch");

const StartupHandler = require('./utils/startup_handler');
const ListenHandler = require('./utils/listen_handler');

const SYSTRAY_ICON = path.join(__dirname, '/assets/system-tray-icon.png');
const home_dir = app.getPath('home');
const user_dir = app.getPath("userData");
const custom_dir = path.join(home_dir, '/mechvibes_custom');
const current_pack_store_id = 'mechvibes-pack';

// Remote debugging defaults
let debug = {
  enabled: false, // the user must enable remote debugging via the debug options window
  identifier: undefined, // the ipc server should be configured to provide unique identifiers for live debugging sessions
  remoteUrl: "https://beta.mechvibes.com/debug/ipc/",
  level: false, // a level must be chosen by debugger
  enable() {
    this.enabled = true;
    if(this.identifier === undefined){
      // TODO: fetch identifier from ipc server
      fetch(debug.remoteUrl, {
        method: "POST",
        body: {
          type: "identify",
          data: "client",
          userInfo: {
            hostname: os.hostname(), // Lunas-Macbook-Pro.local
            username: os.userInfo().username, // lunaalfien
            platform: os.platform(), // darwin
            version: app.getVersion() // v2.3.5
          }
        },
        headers: {
          "Content-Type": "application/json"
        }
      }).then((response) => {
        return response.json();
      }).then((json) => {
        this.identifier = json.identifier;
      }).catch((e) => {
        log.error(e);
      });
    }
    // TODO: fetch debug options from ipc server
    log.transports.remote.level = this.level;
  },
  disable() {
    this.enabled = false;
    this.identifier = undefined; // clear identifier, for user privacy
    // send a request to the ipc server to remove the user's information immediately.
    fetch(debug.remoteUrl, {method: "DELETE"}).then(() => {

    }).catch((e) => {
      // NOTE: if the ipc server fails to process the delete request, user logs might not be removed,
      // depending on ipc server implementation. For this reason, users should only use the official ipc server,
      // which is bound by the debug data retention policy.
      // https://beta.mechvibes.com/blog/debug-data-retention-policy/
      log.error(e);
    });
    log.transports.remote.level = false;
  }
}

// fix so we can detect transport type from within transport hook (see log.hooks.push(...))
for (const transportName in log.transports) {
  log.transports[transportName].transportName = transportName;
}

// We set the URL now, so that we don't need to set it later.
// electron-log won't send the logs to the server until the level is set though.
log.transports.remote.url = debug.remoteUrl;
// NOTE: The IPC server requires an identifier to be set here, otherwise logs will be rejected with a 403 error.
log.transports.remote.client = {name: "Mechvibes"};
log.transports.remote.onError = (e) => {
  // TODO: decide if a log should be deferred or if we should disable logging to the remote server.
  console.log(e);
}

// parse debugging options
const debugConfigFile = path.join(user_dir, "/remote-debug.json");
if(fs.existsSync(debugConfigFile)){
  const json = require(debugConfigFile);
  console.log(json);
  // TODO: fetch debug options from ipc server
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

// TODO: Move iohook handling here
// ... maybe not? I couldn't find a reliable sound player which can be run from main.js
// const iohook = require('iohook');

// Keep a global reference of the window object, if you don't, the window will
// be closed automatically when the JavaScript object is garbage collected.
var win;
var tray = null;
global.app_version = app.getVersion();
global.custom_dir = custom_dir;
global.current_pack_store_id = current_pack_store_id;
global.debug_config_path = debugConfigFile;
// create custom sound folder if not exists
fs.ensureDirSync(custom_dir);

function createWindow(show = true) {
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
    const options = {...debug, path: debugConfigFile};
    debugWindow.webContents.send("debug-options", options);
  })

  ipcMain.on("fetch-debug-options", () => {
    const options = {...debug, path: debugConfigFile};
    debugWindow.webContents.send("debug-options", options);
  })

  ipcMain.on("set-debug-options", (event, json) => {
    console.log(json);
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
    const listen_handler = new ListenHandler(app);

    if(!listen_handler.is_muted){
      iohook.start();
    }

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
          label: 'Custom Folder',
          click: function () {
            shell.openItem(custom_dir);
          },
        },
        {
          label: 'Mute',
          type: 'checkbox',
          checked: listen_handler.is_muted,
          click: function () {
            listen_handler.toggle();
            if(!listen_handler.is_muted){
              iohook.start();
            }else{
              iohook.stop();
            }
          },
        },
        {
          label: 'Enable at Startup',
          type: 'checkbox',
          checked: startup_handler.is_enabled,
          click: function () {
            startup_handler.toggle();
          },
        },
        {
          label: 'Quit',
          click: function () {
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
    createWindow();
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
app.on("before-quit", () => {
  log.silly("Shutting down...");
  app.removeAsDefaultProtocolClient("mechvibes");
})

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
