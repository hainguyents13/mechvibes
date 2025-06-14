'use strict';

const { contextBridge, shell } = require('electron');
const fs = require('fs');
const path = require('path');
const remapper = require('./utils/remapper');
const layouts = require('./libs/layouts');
const keycodes = require('./libs/keycodes');

// Don't require jQuery here - it will be loaded in the browser

// Expose required APIs and modules to window
contextBridge.exposeInMainWorld('nodeAPI', {
  fs: fs,
  path: path,
  electron: require('electron'),
  shell: shell,
  remapper: remapper,
  layouts: layouts,
  keycodes: keycodes,
  __dirname: __dirname,
  platform: process.platform
});
