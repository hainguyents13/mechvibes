// Modules to control application life and create native browser window
const { app, BrowserWindow, Tray, Menu } = require('electron');
const path = require('path');

const SYSTRAY_ICON = path.join(__dirname, '/assets/system-tray-icon.png');

// Keep a global reference of the window object, if you don't, the window will
// be closed automatically when the JavaScript object is garbage collected.
let win;
let tray = null;

function createWindow(show = true) {
  // Create the browser window.
  win = new BrowserWindow({
    width: 400,
    height: 600,
    webSecurity: false,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: false,
      nodeIntegration: true,
    },
    show,
  });

  // remove menu bar
  win.removeMenu();

  // and load the index.html of the app.
  win.loadFile('./src/index.html');

  // Open the DevTools.
  // win.openDevTools();
  // win.webContents.openDevTools();

  // Emitted when the window is closed.
  win.on('closed', function() {
    // Dereference the window object, usually you would store windows
    // in an array if your app supports multi windows, this is the time
    // when you should delete the corresponding element.
    win = null;
  });

  win.on('minimize', function(event) {
    event.preventDefault();
    win.hide();
  });

  win.on('close', function(event) {
    if (!app.isQuiting) {
      event.preventDefault();
      win.hide();
    }

    return false;
  });

  return win;
}

const gotTheLock = app.requestSingleInstanceLock();
app.on('second-instance', (event, argv, cwd) => {
  // Someone tried to run a second instance, we should focus our window.
  if (win) {
    win.show();
    win.focus();
  }
});

if (!gotTheLock) {
  app.quit();
} else {
  app.on('second-instance', (event, commandLine, workingDirectory) => {
    // Someone tried to run a second instance, we should focus our window.
    if (win) {
      if (win.isMinimized()) {
        win.restore();
      }
      win.show();
      win.focus();
    }
  });

  // This method will be called when Electron has finished
  // initialization and is ready to create browser windows.
  // Some APIs can only be used after this event occurs.
  // Don't show the window and create a tray instead
  // create and get window instance
  app.on('ready', () => {
    win = createWindow(false);

    // start tray icon
    tray = new Tray(SYSTRAY_ICON);

    // tray icon tooltip
    tray.setToolTip('Mechvibes.');

    // context menu when hover on tray icon
    const contextMenu = Menu.buildFromTemplate([
      {
        label: 'Mechvibes.',
        click: function() {
          // show app on click
          win.show();
        },
      },
      {
        label: 'Quit',
        click: function() {
          // quit
          app.isQuiting = true;
          app.quit();
        },
      },
    ]);

    // double click on tray icon, show the app
    tray.on('double-click', () => {
      win.show();
    });

    tray.setContextMenu(contextMenu);
  });
}

app.commandLine.appendSwitch('autoplay-policy', 'no-user-gesture-required');

// Quit when all windows are closed.
app.on('window-all-closed', function() {
  // On macOS it is common for applications and their menu bar
  // to stay active until the user quits explicitly with Cmd + Q
  if (process.platform !== 'darwin') app.quit();
});

app.on('activate', function() {
  // On macOS it's common to re-create a window in the app when the
  // dock icon is clicked and there are no other windows open.
  if (win === null) createWindow();
});

// In this file you can include the rest of your app's specific main process
// code. You can also put them in separate files and require them here.
