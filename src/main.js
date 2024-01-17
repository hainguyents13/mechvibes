// Modules to control application life and create native browser window
const { app, BrowserWindow, Tray, Menu, shell, ipcMain } = require('electron');
const path = require('path');
const fs = require('fs-extra');
const Store = require("electron-store");
const store = new Store();

const StartupHandler = require('./utils/startup_handler');
const ListenHandler = require('./utils/listen_handler');
const StartMinimizedHandler = require('./utils/start_minimized_handler');

const SYSTRAY_ICON = path.join(__dirname, '/assets/system-tray-icon.png');
const home_dir = app.getPath('home');
const user_dir = app.getPath("userData");
const custom_dir = path.join(home_dir, '/mechvibes_custom');
const current_pack_store_id = 'mechvibes-pack';

let start_minimized = store.get('mechvibes-start-minimized') || false;

const os = require("os");
const log = require("electron-log");
if(fs.existsSync(path.join(user_dir, "/remote-logging-opt-in.txt"))){
  // Remote logging
  // **************************************************************************
  // Remote logging is opt-in only. To opt-in, create...
  // on Windows: %appData%/Mechvibes/remote-logging-opt-in.txt
  // on macOS: /Users/lunaalfien/Library/Application Support/Mechvibes/remote-logging-opt-in.txt
  // on linux: tbd
  // **************************************************************************
  // see below for examples of the personal data which is collected by remote logging
  log.transports.remote.client = {
    name: "Mechvibes",
    hostname: os.hostname(), // Lunas-Macbook-Pro.local
    username: os.userInfo().username, // lunaalfien
    platform: os.platform() // darwin
  };
  // the url the logs will be sent to.
  log.transports.remote.url = "https://www.lunarwebsite.ca/mechvibes/ipc/";
  log.transports.remote.level = "info";
  // end remote logging

}
log.transports.file.fileName = "mechvibes.log";
log.transports.file.level = "info";
log.transports.file.resolvePath = (variables) => {
  // ~/mechvibes.log
  // eg. /Users/lunaalfien/mechvibes.log
  return path.join(variables.home, variables.fileName);
}
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
// create custom sound folder if not exists
fs.ensureDirSync(custom_dir);

function createWindow(show = false) {
  // Create the browser window.
  win = new BrowserWindow({
    width: 400,
    height: 600,
    webSecurity: false,
    // resizable: false,
    // fullscreenable: false,
    webPreferences: {
      preload: path.join(__dirname, 'app.js'),
      contextIsolation: false,
      nodeIntegration: true,
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
  if (start_minimized) {
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
      console.log("Submitting request to install...");
      console.log(packId);
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
    app.setAsDefaultProtocolClient('mechvibes');

    win = createWindow();

    function createTrayIcon(){
      // prevent dupe tray icons
      if(tray !== null) return;

      // start tray icon
      tray = new Tray(SYSTRAY_ICON);

      // tray icon tooltip
      tray.setToolTip('Mechvibes');

      const startup_handler = new StartupHandler(app);
      const listen_handler = new ListenHandler(app);
      const start_minimized_handler = new StartMinimizedHandler(app);

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
            win.webContents.send('muted', listen_handler.is_muted);
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
          label: 'Start Minimized',
          type: 'checkbox',
          checked: start_minimized_handler.is_enabled,
          click: function () {
            start_minimized_handler.toggle();
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

    ipcMain.on("log", (event, message) => {
      log.info(message);
    })

    // allow the installer to set its size using the height of the body so that when content changes,
    // the installer can only be as big or as small as it needs to be.
    ipcMain.on("resize-installer", (event, size) => {
      const diff = installer.getSize()[1] - installer.getContentSize()[1];
      installer.setSize(300, size + diff, true);
    })
    ipcMain.on("installed", (event, packFolder) => {
      store.set(current_pack_store_id, "custom-" + packFolder);
      win.reload();
      installer.close();
      installer = null;
    })

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
  if (process.platform !== 'darwin') app.quit();
});

app.on('activate', function () {
  // On macOS it's common to re-create a window in the app when the
  // dock icon is clicked and there are no other windows open.
  if (win === null){
    createWindow(true);
  }else{
    // on macOS clicking the app icon in the launcher or in finder, triggers activate instead of second-instance for some reason
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
  app.removeAsDefaultProtocolClient("mechvibes");
})

// always be sure that your application handles the 'quit' event in your main process
app.on('quit', () => {
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
