'use strict';

// All of the Node.js APIs are available in the preload process.
// It has the same sandbox as a Chrome extension.
// const gkm = require('gkm');
const { Howl } = require('howler');
const { shell } = require('electron');
const glob = require('glob');
const iohook = require('iohook');
const { ipcRenderer } = require('electron');
const { platform } = process;

console.log('start');

const keycodes = require('./libs/keycodes');
const layouts = require('./libs/layouts');
const remapper = require('./utils/remapper');

const MV_SET_LS_ID = 'mechvibes-saved-set';
const MV_VOL_LS_ID = 'mechvibes-saved-volume';
const KEYPRESS_TIMEOUT = 10; // ms

let current_set = null;
let sets = [];
let enabled = true;
let current_key_down = null;
let last_key_pressed = Date.now();

// ==================================================
// load all set
async function loadSets(status_display_elem) {
  // init
  status_display_elem.innerHTML = 'Loading...';
  sets = [];

  // get all audio folders
  const folders = await glob.sync(__dirname + '/audio/*/');

  // get set data
  const _sets = folders.map(async folder => {
    // get folder name
    const splited = folder.split('/');
    const folder_name = splited[splited.length - 2];

    // define config file path
    const config_file = `./audio/${folder_name}/config`;

    // get set info and timing data
    const { info, timing } = require(config_file);

    // define sound path
    const sound_path = `./audio/${folder_name}/${info.sound}`;

    // init sound data
    const sound_data = new Howl({ src: [sound_path], sprite: keycodesRemap(timing) });

    // set sound set data
    const set_data = {
      set_id: folder_name,
      info,
      sound: sound_data,
    };

    // event when sound loaded
    sound_data.once('load', function() {
      set_data._loaded = true;

      // if all set loaded
      if (isAllSetsLoaded()) {
        status_display_elem.innerHTML = 'Mechvibes';
      }
    });

    // push set data to set list
    sets.push(set_data);
  });

  // run promises
  await Promise.all(_sets);

  // end load
  return;
}

// ==================================================
// check if all sets loaded
function isAllSetsLoaded() {
  return sets.every(set => set._loaded);
}

// ==================================================
// remap keycodes from standard to os based keycodes
function keycodesRemap(timing) {
  const sprite = remapper('standard', platform, timing);
  Object.keys(sprite).map(kc => {
    sprite[`keycode-${kc}`] = sprite[kc];
    delete sprite[kc];
  });
  return sprite;
}

// ==================================================
// get set by id,
// if id is null,
// get saved set
function getSet(set_id = null) {
  if (!set_id) {
    if (localStorage.getItem(MV_SET_LS_ID)) {
      set_id = localStorage.getItem(MV_SET_LS_ID);
      if (!getSet(set_id)) {
        return sets[0];
      }
    } else {
      return sets[0];
    }
  }
  localStorage.setItem(MV_SET_LS_ID, set_id);
  return sets.find(set => set.set_id == set_id);
}

// ==================================================
// transform set to select option list
function setsToOptions(sets, set_list, onselect) {
  // get saved set id
  const selected_set_id = localStorage.getItem(MV_SET_LS_ID);
  const groups = [];
  sets.map(set => {
    const exists = groups.find(group => group.id == set.info.group);
    if (!exists) {
      const group = {
        id: set.info.group,
        name: set.info.group.toUpperCase(),
        sets: [set],
      };
      groups.push(group);
    } else {
      exists.sets.push(set);
    }
  });

  for (let group of groups) {
    const optgroup = document.createElement('optgroup');
    optgroup.label = group.name;
    for (let set of group.sets) {
      // check if selected
      const is_selected = selected_set_id == set.set_id;
      if (is_selected) {
        // set current set to saved set
        current_set = set;
      }
      // add set to set list
      const opt = document.createElement('option');
      opt.text = set.info.name;
      opt.value = set.set_id;
      opt.selected = is_selected ? 'selected' : false;
      optgroup.appendChild(opt);
    }
    set_list.appendChild(optgroup);
  }

  // on select an option
  // update saved list id
  set_list.addEventListener('change', e => {
    const selected_id = e.target.options[e.target.selectedIndex].value;
    localStorage.setItem(MV_SET_LS_ID, selected_id);
    current_set = getSet();
  });
}

// ==================================================
// main
(function(window, document) {
  'use strict';

  window.addEventListener('DOMContentLoaded', async () => {
    const version = document.getElementById('app-version');
    // request current app version
    ipcRenderer.send('app_version');
    ipcRenderer.on('app_version', (event, arg) => {
      ipcRenderer.removeAllListeners('app_version');
      version.innerText = arg.version;
    });

    // display keycode
    const keycode_display = document.getElementById('keycode-display');
    const set_list = document.getElementById('set-list');
    // const enable_btn = document.getElementById('enable');
    const volume_value = document.getElementById('volume-value-display');
    const volume = document.getElementById('volume');

    // a little hack for open link in browser
    Array.from(document.getElementsByClassName('open-in-browser')).forEach(elem => {
      elem.addEventListener('click', e => {
        e.preventDefault();
        shell.openExternal(e.target.href);
      });
    });

    // listen toggle button
    // enable_btn.addEventListener('click', () => {
    //   enabled = !enabled;
    //   enable_btn.innerHTML = enabled ? 'Pause' : 'Start';
    // });

    // load all sets
    await loadSets(keycode_display);

    // get last selected set
    current_set = getSet();

    // transform sets to options list
    setsToOptions(sets, set_list);

    // display volume value
    if (localStorage.getItem(MV_VOL_LS_ID)) {
      volume.value = localStorage.getItem(MV_VOL_LS_ID);
    }
    volume_value.innerHTML = volume.value;
    volume.oninput = function(e) {
      volume_value.innerHTML = this.value;
      localStorage.setItem(MV_VOL_LS_ID, this.value);
    };

    // listen to key press
    iohook.start();

    // if key released, clear current key
    iohook.on('keyup', () => {
      current_key_down = null;
      last_key_pressed = Date.now();
      keycode_display.classList.remove('pressed');
    });

    // key pressed, set current key and play sound
    iohook.on('keydown', ({ keycode }) => {
      // if turned off, play no sound
      // if (!enabled) {
      //   return;
      // }

      // if hold down a key, not repeat the sound
      if (current_key_down != null && current_key_down == keycode) {
        return;
      }

      // this code prevent language input tools (unikey, ibus...)
      // send multiple keys when they perform auto correct
      const threshold = Date.now() - last_key_pressed;
      if (threshold <= 30) {
        return;
      }

      // display current pressed key
      // keycode_display.innerHTML = keycode;
      keycode_display.classList.add('pressed');

      // set current pressed key
      current_key_down = keycode;

      // set sprite id
      const sprite_id = `keycode-${current_key_down}`;

      // get loaded audio object
      // if object valid, set volume and play sound
      if (current_set) {
        current_set.sound.volume(Number(volume.value / 100));
        current_set.sound.play(sprite_id);
      }
    });
  });
})(window, document);
