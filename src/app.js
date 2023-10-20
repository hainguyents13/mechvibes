'use strict';

// All of the Node.js APIs are available in the preload process.
// It has the same sandbox as a Chrome extension.
// const gkm = require('gkm');
const Store = require('electron-store');
const store = new Store();
const { Howl } = require('howler');
const { shell, remote, ipcRenderer } = require('electron');
const fs = require('fs');
const glob = require('glob');
// TODO: move iohook and audio playback to main.js so that if the configurator dies the audio doesn't.
const iohook = require("iohook");
const path = require('path');
const { platform } = process;
const remapper = require('./utils/remapper');

const MV_PACK_LSID = 'mechvibes-pack';
const MV_VOL_LSID = 'mechvibes-volume';
const MV_TRAY_LSID = 'mechvibes-hidden';

const CUSTOM_PACKS_DIR = remote.getGlobal('custom_dir');
const OFFICIAL_PACKS_DIR = path.join(__dirname, 'audio');
const APP_VERSION = remote.getGlobal('app_version');

let current_pack = null;
let current_key_down = null;
let is_muted = store.get('mechvibes-muted') || false;
const packs = [];
const all_sound_files = {};

function loadPack(packId = null){
  if(packId === null){
    Object.keys(packs).map((pid) => {
      const _pack = packs[pid];
      if(_pack.id == current_pack.id){
        packId = pid;
      }
    })
  }
  return new Promise((resolve, reject) => {
    if(packs[packId] !== undefined){
      unloadAllPacks(); // unload all loaded packs before attempting to load a new pack.
      const pack = packs[packId];
      if(pack.key_define_type == 'single'){
        const sound_data = packs[packId].sound_data;
        
        const audio = new Howl(sound_data);
        audio.once('load', function () {
          packs[packId].sound = audio;
          resolve();
        });
      }else{
        let loaded_sounds = {};
        let check = () => {
          let unloaded_exists = false;
          Object.keys(loaded_sounds).map((key) => {
            if (typeof loaded_sounds[key] !== 'object') {
              unloaded_exists = true;
            }
          });
          if(!unloaded_exists){
            packs[packId].sound = loaded_sounds;
            resolve();
          }
        }
        Object.keys(pack.sound_data).map((kc) => {
          const audio = new Howl(pack.sound_data[kc]);
          loaded_sounds[kc] = false;
          audio.once('load', function(){
            loaded_sounds[kc] = audio;
            check();
          })
        })
      }
    }else{
      reject("That packID doesn't exist");
    }
  })
}

function unloadPack(packId){
  if(packs[packId] !== undefined){
    if(packs[packId].sound !== undefined){
      if(packs[packId].key_define_type == 'single'){
        packs[packId].sound.unload();
        delete packs[packId].sound;
      }else{
        Object.keys(packs[packId].sound).map((kc) => {
          packs[packId].sound[kc].unload();
        })
        delete packs[packId].sound;
      }
      return [true];
    }else{
      return [false, "pack is unloaded already"];
    }
  }else{
    return [false, "pack doesn't exist"];
  }
}

function unloadAllPacks(){
  Object.keys(packs).map((packId) => {
    if(packs[packId].sound !== undefined){
      unloadPack(packId);
    }
  })
}

window.packs = packs;
window.loadPack = loadPack;
window.unloadPack = unloadPack;

// ==================================================
// load all pack
async function loadPacks() {
  // get all audio folders
  const official_packs = await glob.sync(OFFICIAL_PACKS_DIR + '/*/');
  const custom_packs = await glob.sync(CUSTOM_PACKS_DIR + '/*/');
  const folders = [...official_packs, ...custom_packs];

  // get pack data
  folders.map((folder) => {
    // define group by types
    const is_custom = folder.indexOf('mechvibes_custom') > -1 ? true : false;

    // get folder name
    const splited = folder.split('/');
    const folder_name = splited[splited.length - 2];

    // define config file path
    const config_file = `${folder.replace(/\/$/, '')}/config.json`;

    // get pack info and defines data
    if(fs.existsSync(config_file)){
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
        const sound_data = { src: [sound_path], sprite: keycodesRemap(defines) };
        Object.assign(pack_data, { sound_data: sound_data });
      } else {
        const sound_data = {};
        Object.keys(defines).map((kc) => {
          if (defines[kc]) {
            // define sound path
            const sound_path = `${folder}${defines[kc]}`;
            sound_data[kc] = { src: [sound_path] };
          }
        });
        if (Object.keys(sound_data).length) {
          Object.assign(pack_data, { sound_data: keycodesRemap(sound_data) });
        }
      }
  
      // push pack data to pack list
      packs.push(pack_data);
    }
  });

  // end load
  return;
}

// ==================================================
// remap keycodes from standard to os based keycodes
function keycodesRemap(defines) {
  const sprite = remapper('standard', platform, defines);
  Object.keys(sprite).map((kc) => {
    sprite[`keycode-${kc}`] = sprite[kc];
    delete sprite[kc];
  });
  return sprite;
}

function getPack(pack_id){
  return packs.find((pack) => pack.pack_id == pack_id);
}

function getSavedPack() {
  if (store.get(MV_PACK_LSID)) {
    const pack_id = store.get(MV_PACK_LSID);
    const pack = getPack(pack_id);
    if (!pack) {
      return packs[0];
    }else{
      return pack;
    }
  } else {
    return packs[0];
  }
}

// set pack by its index in the packs array
function setPack(pack_id){
  let index = 0;
  Object.keys(packs).map((packId) => {
    if(packs[packId].pack_id == pack_id){
      index = packId;
    }
  })
  loadPack(index);
  current_pack = packs[index];
  store.set(MV_PACK_LSID, current_pack.pack_id);
}

window.store = store;

// set pack by its string id
function setPackByIndex(index){
  loadPack(index);
  current_pack = packs[index];
  store.set(MV_PACK_LSID, current_pack.pack_id);
}

// ==================================================
// transform pack to select option list
function packsToOptions(packs, pack_list) {
  // get saved pack id
  const selected_pack_id = store.get(MV_PACK_LSID);
  const groups = [];
  packs.map((pack) => {
    const exists = groups.find((group) => group.id == pack.group);
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
        setPack(pack.pack_id);
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
  pack_list.addEventListener('change', (e) => {
    const selected_id = e.target.options[e.target.selectedIndex].value;
    setPack(selected_id);
  });
}

// ==================================================
// main
(function (window, document) {
  window.addEventListener('DOMContentLoaded', async () => {
    const version = document.getElementById('app-version');
    const update_available = document.getElementById('update-available');
    const new_version = document.getElementById('new-version');
    const app_logo = document.getElementById('logo');
    const app_body = document.getElementById('app-body');
    const pack_list = document.getElementById('pack-list');
    const random_button = document.getElementById('random-button');
    const volume_value = document.getElementById('volume-value-display');
    const volume = document.getElementById('volume');
    const tray_icon_toggle = document.getElementById("tray_icon_toggle");
    const tray_icon_toggle_group = document.getElementById("tray_icon_toggle_group");

    // init
    app_logo.innerHTML = 'Loading...';

    // set app version
    version.innerHTML = APP_VERSION;

    // load all packs
    await loadPacks(app_logo, app_body);

    // transform packs to options list
    packsToOptions(packs, pack_list);

    // check for new version
    fetch('https://api.github.com/repos/hainguyents13/mechvibes/releases/latest')
      .then((res) => res.json())
      .then((json) => {
        if (json.tag_name > APP_VERSION) {
          new_version.innerHTML = json.tag_name;
          update_available.classList.remove('hidden');
        }
      });

    // a little hack for open link in browser
    Array.from(document.getElementsByClassName('open-in-browser')).forEach((elem) => {
      elem.addEventListener('click', (e) => {
        e.preventDefault();
        shell.openExternal(e.target.href);
      });
    });

    // get last selected pack
    current_pack = getSavedPack();
    loadPack().then(() => {
      app_logo.innerHTML = 'Mechvibes';
      app_body.classList.remove('loading');
    });

    // handle tray hiding
    console.log(store.get(MV_TRAY_LSID));
    if (store.get(MV_TRAY_LSID) !== undefined){
      tray_icon_toggle.checked = store.get(MV_TRAY_LSID);
    }
    tray_icon_toggle_group.onclick = function(e) {
      e.preventDefault();
      e.stopPropagation();
      // toggle checkbox
      tray_icon_toggle.checked = !tray_icon_toggle.checked;
      ipcRenderer.send("show_tray_icon", tray_icon_toggle.checked);
      store.set(MV_TRAY_LSID, tray_icon_toggle.checked);
    }

    // ensure tray icon is reflected
    let initTray = () => {
      ipcRenderer.send("show_tray_icon", tray_icon_toggle.checked);
    }
    initTray();

    // display volume value
    if (store.get(MV_VOL_LSID)) {
      volume.value = store.get(MV_VOL_LSID);
    }else{
      // TODO: set default
    }
    volume_value.innerHTML = volume.value;
    volume.oninput = function (e) {
      volume_value.innerHTML = this.value;
      store.set(MV_VOL_LSID, this.value);
    };

    if (!is_muted) {
      iohook.start();
    }

    // listen to key press
    ipcRenderer.on('muted', function (_event, _is_muted) {
      is_muted = _is_muted;
      if (is_muted) {
        iohook.stop();
      } else {
        iohook.start();
      }
    });

    // if key released, clear current key
    iohook.on('keyup', () => {
      current_key_down = null;
      app_logo.classList.remove('pressed');
    });

    // key pressed, pack current key and play sound
    iohook.on('keydown', ({ keycode }) => {
      // if hold down a key, not repeat the sound
      // On macOS this doesn't seem to be an issue?
      if (current_key_down != null && current_key_down == keycode) {
        return;
      }

      // display current pressed key
      // app_logo.innerHTML = keycode;
      app_logo.classList.add('pressed');

      // pack current pressed key
      current_key_down = keycode;

      // pack sprite id
      const sound_id = `keycode-${current_key_down}`;

      // get loaded audio object
      // if object valid, pack volume and play sound
      if (current_pack) {
        playSound(sound_id, volume.value);
      }
    });

    // on random button click
    // set random sound
    random_button.addEventListener('click', (e) => {
      e.preventDefault();
      let getRandomPackId = () => {
        let randomId = Math.floor(Math.random() * packs.length);
        if (packs[randomId].pack_id === current_pack.pack_id) {
          return getRandomPackId();
        }
        return randomId;
      }
      const packId = getRandomPackId();
      pack_list.selectedIndex = packId;
      setPackByIndex(packId);
    });
  });
})(window, document);

// ==================================================
// universal play function
function playSound(sound_id, volume) {
  if(current_pack.sound === undefined){
    // sound for this pack hasn't been loaded
    return;
  }
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
