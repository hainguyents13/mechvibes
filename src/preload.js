'use strict';

// All of the Node.js APIs are available in the preload process.
// It has the same sandbox as a Chrome extension.
// const gkm = require('gkm');
const { Howl } = require('howler');
const { shell } = require('electron');
const glob = require('glob');
const iohook = require('iohook');
const path = require('path');
const { ipcRenderer } = require('electron');
const { platform } = process;

const keycodes = require('./libs/keycodes');
const layouts = require('./libs/layouts');
const remapper = require('./utils/remapper');

const MV_PACK_LSID = 'mechvibes-saved-pack';
const MV_VOL_LSID = 'mechvibes-saved-volume';
const KEYPRESS_TIMEOUT = 10; // ms

const CUSTOM_PACKS_DIR = path.join(__dirname, '../../../custom');
const OFFICIAL_PACKS_DIR = path.join(__dirname, './audio');

let current_pack_id = null;
let current_pack = null;
let packs = [];
let loaded = false;
let current_key_down = null;
let last_key_pressed = Date.now();
const all_sound_files = {};

// ==================================================
// load all pack
async function loadPacks(status_display_elem) {
  // init
  status_display_elem.innerHTML = 'Loading...';
  packs = [];

  // get all audio folders
  const official_packs = await glob.sync(OFFICIAL_PACKS_DIR + '/*/');
  const custom_packs = await glob.sync(CUSTOM_PACKS_DIR + '/*/');
  const folders = [...official_packs, ...custom_packs];

  // get pack data
  folders.map(folder => {
    // define group by types
    const is_custom = folder.indexOf('/custom/') > -1 ? true : false;

    // get folder name
    const splited = folder.split('/');
    const folder_name = splited[splited.length - 2];

    // define config file path
    const config_file = `${folder}/config.json`;

    // get pack info and defines data
    const { name, includes_numpad, sound = '', defines, key_define_type = 'single' } = require(config_file);

    // pack sound pack data
    const pack_data = {
      pack_id: `${is_custom ? 'custom' : 'default'}-${folder_name}`,
      group: is_custom ? 'Custom' : 'Default',
      abs_path: folder,
      key_define_type,
      name,
      includes_numpad,
    };

    // init sound data
    if (key_define_type == 'single') {
      // define sound path
      const sound_path = `${folder}${sound}`;

      const sound_data = new Howl({ src: [sound_path], sprite: keycodesRemap(defines) });
      Object.assign(pack_data, { sound: sound_data });
      all_sound_files[pack_data.pack_id] = false;
      // event when sound loaded
      sound_data.once('load', function() {
        all_sound_files[pack_data.pack_id] = true;
        checkIfAllSoundLoaded(status_display_elem);
      });
    } else {
      const sound_data = {};
      Object.keys(defines).map(kc => {
        if (defines[kc]) {
          // define sound path
          console.log(defines[kc]);
          const sound_path = `${folder}${defines[kc]}`;
          sound_data[kc] = new Howl({ src: [sound_path] });
          all_sound_files[`${pack_data.pack_id}-${kc}`] = false;
          // event when sound_data loaded
          sound_data[kc].once('load', function() {
            all_sound_files[`${pack_data.pack_id}-${kc}`] = true;
            checkIfAllSoundLoaded(status_display_elem);
          });
        }
      });
      if (Object.keys(sound_data).length) {
        Object.assign(pack_data, { sound: keycodesRemap(sound_data) });
      }
    }

    // push pack data to pack list
    packs.push(pack_data);
  });

  // end load
  return;
}

// ==================================================
// universal play function
function playSound(sound_id, volume) {
  const play_type = current_pack.key_define_type ? current_pack.key_define_type : 'single';
  const sound = play_type == 'single' ? current_pack.sound : current_pack.sound[sound_id];
  if (!sound) {
    return;
  }
  sound.volume(Number(volume / 100));
  if (play_type == 'single') {
    sound.play(sound_id);
  } else {
    sound.play();
  }
}

// ==================================================
// check if all packs loaded
function checkIfAllSoundLoaded(status_display_elem) {
  Object.keys(all_sound_files).map(key => {
    if (!all_sound_files[key]) {
      loaded = false;
      return false;
    }
  });
  status_display_elem.innerHTML = 'Mechvibes';
  loaded = true;
  return true;
}

// ==================================================
// remap keycodes from standard to os based keycodes
function keycodesRemap(defines) {
  const sprite = remapper('standard', platform, defines);
  Object.keys(sprite).map(kc => {
    sprite[`keycode-${kc}`] = sprite[kc];
    delete sprite[kc];
  });
  return sprite;
}

// ==================================================
// get pack by id,
// if id is null,
// get saved pack
function getPack(pack_id = null) {
  if (!pack_id) {
    if (localStorage.getItem(MV_PACK_LSID)) {
      pack_id = localStorage.getItem(MV_PACK_LSID);
      if (!getPack(pack_id)) {
        return packs[0];
      }
    } else {
      return packs[0];
    }
  }
  localStorage.setItem(MV_PACK_LSID, pack_id);
  return packs.find(pack => pack.pack_id == pack_id);
}

// ==================================================
// transform pack to select option list
function packsToOptions(packs, pack_list, onselect) {
  // get saved pack id
  const selected_pack_id = localStorage.getItem(MV_PACK_LSID);
  const groups = [];
  packs.map(pack => {
    const exists = groups.find(group => group.id == pack.group);
    if (!exists) {
      const group = {
        id: pack.group,
        name: pack.group || 'Default',
        packs: [pack],
      };
      groups.push(group);
    } else {
      exists.packs.push(pack);
    }
  });

  for (let group of groups) {
    const optgroup = document.createElement('optgroup');
    optgroup.label = group.name;
    for (let pack of group.packs) {
      // check if selected
      const is_selected = selected_pack_id == pack.pack_id;
      if (is_selected) {
        // pack current pack to saved pack
        current_pack = pack;
      }
      // add pack to pack list
      const opt = document.createElement('option');
      opt.text = pack.name;
      opt.value = pack.pack_id;
      opt.selected = is_selected ? 'selected' : false;
      optgroup.appendChild(opt);
    }
    pack_list.appendChild(optgroup);
  }

  // on select an option
  // update saved list id
  pack_list.addEventListener('change', e => {
    const selected_id = e.target.options[e.target.selectedIndex].value;
    localStorage.setItem(MV_PACK_LSID, selected_id);
    current_pack = getPack();
    current_pack_id = current_pack.pack_id;
  });
}

// ==================================================
// main
(function(window, document) {
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
    const pack_list = document.getElementById('pack-list');
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

    // load all packs
    await loadPacks(keycode_display);

    // get last selected pack
    current_pack = getPack();

    // transform packs to options list
    packsToOptions(packs, pack_list);

    // display volume value
    if (localStorage.getItem(MV_VOL_LSID)) {
      volume.value = localStorage.getItem(MV_VOL_LSID);
    }
    volume_value.innerHTML = volume.value;
    volume.oninput = function(e) {
      volume_value.innerHTML = this.value;
      localStorage.setItem(MV_VOL_LSID, this.value);
    };

    // listen to key press
    iohook.start();

    // if key released, clear current key
    iohook.on('keyup', () => {
      current_key_down = null;
      last_key_pressed = Date.now();
      keycode_display.classList.remove('pressed');
    });

    // key pressed, pack current key and play sound
    iohook.on('keydown', ({ keycode }) => {
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

      // pack current pressed key
      current_key_down = keycode;

      // pack sprite id
      const sound_id = `keycode-${current_key_down}`;

      // get loaded audio object
      // if object valid, pack volume and play sound
      if (current_pack) {
        playSound(sound_id);
      }
    });
  });
})(window, document);
