// All of the Node.js APIs are available in the preload process.
// It has the same sandbox as a Chrome extension.
// const gkm = require('gkm');
const { Howl } = require('howler');
const { shell } = require('electron');
const glob = require('glob');
const iohook = require('iohook');

const os_keycodes = require('./os-keycodes');

const MV_SET_LS_ID = 'mechvibes-saved-set';
const MV_VOL_LS_ID = 'mechvibes-saved-volume';
const { platform } = process;

let current_set = null;
let sets = [];
let enabled = true;
let current_key_down = null;

// ==================================================
// ==================================================
// ==================================================
// load all set
async function loadSets(status_display_elem) {
  status_display_elem.innerHTML = 'Loading...';
  sets = [];
  const folders = await glob.sync(__dirname + '/audio/*/');
  const _sets = folders.map(async folder => {
    const splited = folder.split('/');
    const folder_name = splited[splited.length - 2];
    const config_file = `./audio/${folder_name}/config`;
    const { set_name, keychars, sound_file } = require(config_file);
    const sound_path = `./audio/${folder_name}/${sound_file}`;
    sound_data = new Howl({ src: [sound_path], sprite: keycharsToOsBasedKeycodes(keychars) });

    const set_data = {
      set_id: folder_name,
      set_name,
      sound: sound_data,
    };

    sound_data.once('load', function() {
      set_data._loaded = true;
      if (isAllSetsLoaded()) {
        status_display_elem.innerHTML = 'Mechvibes';
      }
    });

    sets.push(set_data);
  });

  return await Promise.all(_sets);
}

// ==================================================
// ==================================================
// ==================================================
// check if all sets loaded
function isAllSetsLoaded() {
  return sets.every(set => set._loaded);
}

// ==================================================
// ==================================================
// ==================================================
function keycharsToOsBasedKeycodes(keychars) {
  const sprite = {};
  const keycodes = os_keycodes[platform];
  Object.keys(keycodes).forEach(char => {
    const keycode = keycodes[char];
    sprite[keycode] = keychars[char];
  });
  return sprite;
}

// ==================================================
// ==================================================
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
// ==================================================
// ==================================================
// transform set to select option list
function setsToOptions(sets, set_list, onselect) {
  // get saved set id
  const selected_set_id = localStorage.getItem(MV_SET_LS_ID);
  for (let set of sets) {
    // check if selected
    const is_selected = selected_set_id == set.set_id;
    if (is_selected) {
      // set current set to saved set
      current_set = set;
    }
    // add set to set list
    const opt = document.createElement('option');
    opt.text = set.set_name;
    opt.value = set.set_id;
    opt.selected = is_selected ? 'selected' : false;
    set_list.add(opt);
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
// ==================================================
// ==================================================
// main
(function(window, document) {
  'use strict';

  window.addEventListener('DOMContentLoaded', async () => {
    const keycode_display = document.getElementById('keycode-display');
    const set_list = document.getElementById('set-list');
    const enable_btn = document.getElementById('enable');
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
    enable_btn.addEventListener('click', () => {
      enabled = !enabled;
      enable_btn.innerHTML = enabled ? 'Pause' : 'Start';
    });

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
      keycode_display.classList.remove('pressed');
    });

    // key pressed, set current key and play sound
    iohook.on('keydown', ({ rawcode }) => {
      // if turned off, play no sound
      if (!enabled) {
        return;
      }
      if (current_key_down == rawcode) {
        return;
      }
      // display current pressed key
      // keycode_display.innerHTML = rawcode;
      keycode_display.classList.add('pressed');
      // set current pressed key
      current_key_down = rawcode;
      // get loaded audio object
      // if object valid, set volume and play sound
      if (current_set) {
        current_set.sound.volume(Number(volume.value / 100));
        current_set.sound.play(current_key_down.toString());
      }
    });
  });
})(window, document);
