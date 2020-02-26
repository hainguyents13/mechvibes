'use strict';

const fs = require('fs');
const path = require('path');
const { shell } = require('electron');

const remapper = require('./utils/remapper');
const layouts = require('./libs/layouts');
const keycodes = require('./libs/keycodes');
const $ = require('./assets/jquery');

const layout = layouts[process.platform];
const { sizes } = layouts;
const os_keycode = keycodes[process.platform];
const CUSTOM_PACKS_DIR = path.join(__dirname, '../../../custom');

let selected_keycode = null;
let current_edit_mode = 'visual';
let current_key_define_mode = 'single';

let pack_data = {
  id: `custom-sound-pack-${Date.now()}`,
  name: 'Untitled',
  key_define_type: 'single',
  includes_numpad: false,
  sound: 'sound.ogg',
  defines: JSON.parse(JSON.stringify(os_keycode)),
};
Object.keys(pack_data.defines).map(kc => {
  pack_data.defines[kc] = null;
});

(function(document) {
  $(document).ready(() => {
    // a little hack for open link in browser
    Array.from(document.getElementsByClassName('open-in-browser')).forEach(elem => {
      elem.addEventListener('click', e => {
        e.preventDefault();
        shell.openExternal(e.target.href);
      });
    });

    // open custom sound pack folder
    $('#open-custom-pack-folder').on('click', () => {
      shell.showItemInFolder(CUSTOM_PACKS_DIR);
    });

    $('#pack-name').val(pack_data.name);
    $('#single-sound-file').val(pack_data.sound);

    const keyboard_holder = $('#kb');
    keyboard_holder.html('');
    for (let zone of ['main', 'edit', 'numpad']) {
      const zone_wrapper = $(`<div id="zone-${zone}"></div>`);
      const manual_zone_wrapper = $(`#pack-zone-${zone}`);
      const zone_rows = layout[zone];
      let r = 0;
      for (let row of zone_rows) {
        const _row = $(`<div class="key-row ${r == 0 ? 'key-row-top' : ''}"></div>`);
        let k = 0;
        if (row.length) {
          for (let key of row) {
            // visual edit
            // <div class="key-rk">${r}-${k++}</div>
            const _key = $(`
              <div id="key-${key}" class="key ${sizes[key] ? sizes[key] : ''} ${key ? '' : 'key-blank'}" data-keycode="${key}">
                <div class="letter">${os_keycode[key] || ''}</div> 
              </div>
           `);
            _key.appendTo(_row);
            _genPopover(_key, key, r > 3, zone == 'numpad');
            // manual edit
            if (key) {
              const manual_key = $(`
                <div id="manual-key-${key}" style="border-bottom: 1px solid #eee; padding: 10px; display: flex; justify-content: space-between; align-items: center" class="manual-key">
                  <div style="font-weight: bold; margin-right: 10px">${os_keycode[key]}</div>
                  <div class="define-mode define-mode-single">
                    <div style="display: flex;">
                      <input type="number" placeholder="Start..." style="margin-right: 5px; width: 60px;" class="key-define custom-input sound-start" data-keycode="${key}"/>
                      <input type="number" placeholder="Length..." style=" width: 60px;" class="key-define custom-input sound-length" data-keycode="${key}"/>
                    </div>
                  </div>
                  <div class="define-mode define-mode-multi">
                    <input type="text" placeholder="File name..." style="margin-right: 5px; width: 100%;" class="key-define custom-input sound-file" data-keycode="${key}"/>
                  </div>
                </div>
              `);
              manual_key.appendTo(manual_zone_wrapper);
            }
          }
        }
        r++;
        _row.appendTo(zone_wrapper);
      }
      zone_wrapper.appendTo(keyboard_holder);
    }

    // ======================
    $('#kb').on('click', '.key', e => {
      const target = $(e.currentTarget);
      if (target.hasClass('key-blank')) {
        e.preventDefault();
        return false;
      }
      const keycode = target.data('keycode');
      if (!$(e.target).hasClass('close')) {
        $('.key')
          .removeClass('key-pressed key-show-popover')
          .addClass('key-hide');
        target.addClass('key-pressed key-show-popover').removeClass('key-hide');
      }
      const letter = target.find('.letter').text();
      selected_keycode = keycode;
    });

    $('#kb').on('click', '.popover .close', e => {
      setTimeout(() => {
        $('.key').removeClass('key-pressed key-show-popover key-hide');
      });
    });

    $('#kb').on('click', '.popover .save', e => {
      setTimeout(() => {
        $('.key').removeClass('key-pressed key-show-popover key-hide');
        const keycode = $(e.currentTarget).data('keycode');

        if (!current_key_define_mode || current_key_define_mode == 'single') {
          const start = $(e.currentTarget).find('.sound-start');
          const length = $(e.currentTarget).find('.sound-length');
          pack_data.defines[keycode] = [Number(start.val()), Number(length.val())];
        } else {
          const file_name = $(e.currentTarget).find('.sound-name');
          pack_data.defines[keycode] = file_name.val();
        }
        _checkIfHasSound();
      });
    });

    $('.edit-mode-manual').on('change', '.key-define', e => {
      const keycode = $(e.target).data('keycode');
      if (!current_key_define_mode || current_key_define_mode == 'single') {
        pack_data.defines[keycode] = pack_data.defines[keycode] || [0, 0];
        if ($(e.target).hasClass('sound-start')) {
          pack_data.defines[keycode][0] = Number($(e.target).val());
        } else {
          pack_data.defines[keycode][1] = Number($(e.target).val());
        }
      } else {
        pack_data.defines[keycode] = $(e.target).val();
      }
      _checkIfHasSound();
    });

    $('#single-sound-file').on('change', e => {
      pack_data.sound = $(e.target).val() || 'sound.ogg';
      genResults();
    });

    $('#pack-name').on('change', e => {
      pack_data.name = $(e.target).val() || 'Untitled';
      genResults();
    });

    $('.edit-mode-manual').hide();
    $('#edit-mode').on('change', e => {
      current_edit_mode = e.target.value;
      $('.edit-mode').hide();
      $(`.edit-mode-${e.target.value}`).show();
      if (e.target.value == 'manual') {
        for (let kc in pack_data.defines) {
          if (pack_data.defines[kc] && $(`.sound-file[data-keycode="${kc}"]`)) {
            if (typeof pack_data.defines[kc] == 'string') {
              $(`.sound-file[data-keycode="${kc}"]`).val(pack_data.defines[kc]);
            } else {
              $(`.sound-start[data-keycode="${kc}"]`).val(pack_data.defines[kc][0]);
              $(`.sound-length[data-keycode="${kc}"]`).val(pack_data.defines[kc][1]);
            }
          }
        }
      } else {
        for (let kc in pack_data.defines) {
          if (pack_data.defines[kc] && $(`.sound-file[data-keycode="${kc}"]`)) {
            if (typeof pack_data.defines[kc] == 'string') {
              $(`.key[data-keycode="${kc}"]`)
                .find('.sound-name')
                .val(pack_data.defines[kc]);
            } else {
              $(`.key[data-keycode="${kc}"]`)
                .find('.sound-start')
                .val(pack_data.defines[kc][0]);
              $(`.key[data-keycode="${kc}"]`)
                .find('.sound-length')
                .val(pack_data.defines[kc][1]);
            }
          }
        }
      }
    });

    $('.define-mode-multi').hide();
    $('#key-define-mode').on('change', e => {
      pack_data.key_define_type = e.target.value;
      current_key_define_mode = e.target.value;
      $('.define-mode').hide();
      $(`.define-mode-${e.target.value}`).show();
      $('.key-define').each((index, el) => {
        el.value = '';
      });
      Object.keys(pack_data.defines).map(kc => {
        pack_data.defines[kc] = null;
      });
      _checkIfHasSound();
    });
  });

  function _checkIfHasSound() {
    Object.keys(pack_data.defines).map(kc => {
      if (pack_data.defines[kc] != null) {
        $(`.key[data-keycode=${kc}]`).addClass('key-has-sound');
        $(`#manual-key-${kc}`).addClass('key-has-sound');
      } else {
        $(`.key[data-keycode=${kc}]`).removeClass('key-has-sound');
        $(`#manual-key-${kc}`).removeClass('key-has-sound');
      }
    });
    genResults();
  }

  function _genPopover(target, keycode, up, left) {
    const popover = $(`
      <div class="popover ${up ? 'up' : ''} ${left ? 'left' : ''}" style="min-width: 250px; position: absolute">
        <div class="define-mode define-mode-single" style="margin-bottom: 10px">
          <div style="margin-bottom: 5px">Set start and length (ms)</div>
          <div style="display: flex;">
            <input type="number" placeholder="Start..." style="margin-right: 10px; width: 50%;" class="key-define custom-input sound-start"/>
            <input type="number" placeholder="Length..." style="width: 50%" class="key-define custom-input sound-length"/>
          </div>
        </div>

        <div class="define-mode define-mode-multi" style="margin-bottom: 10px">
          <div style="margin-bottom: 5px">Enter audio file name:</div>
          <input type="text" placeholder="Sound file name..."  class="key-define custom-input sound-name" style="width: 95%; margin-right: 10px;"/>
        </div>

        <div style="display: flex; justify-content: space-between">
          <button class="save">Save</button>
          <button class="close">Close</button>
        </div>
      </div>
    `);
    popover.appendTo(target);
  }

  function genResults() {
    const container = $('#result');
    pack_data.defines = remapper(process.platform, 'standard', pack_data.defines);
    const result = JSON.stringify(pack_data, null, 2);
    container.html(result);
  }

  // ======================================
  // new pack
  $('#create').on('click', () => {
    Object.assign(pack_data, {
      id: `custom-sound-pack-${Date.now()}`,
      key_define_type: 'single',
      name: 'Untitled',
      sound: 'sound.ogg',
    });
    Object.keys(pack_data.defines).map(kc => {
      pack_data.defines[kc] = null;
    });
    $('#pack-name').val(pack_data.name);
    $('#single-sound-file').val(pack_data.sound);
    $('.key-define').val(null);
  });

  function importPack(imported) {
    Object.assign(pack_data, imported);
    $('#pack-name').val(pack_data.name);
    $('#single-sound-file').val(pack_data.sound);

    pack_data.defines = remapper('standard', process.platform, pack_data.defines);

    const key_define_type = imported.key_define_type || 'single';
    $('.define-mode').hide();
    $(`.define-mode-${key_define_type}`).show();
    $('#key-define-mode').val(key_define_type);
    $(`#key-define-mode option[value=${key_define_type}]`).attr('selected', 'selected');
    current_key_define_mode = key_define_type;

    console.log(key_define_type);

    for (let kc in pack_data.defines) {
      if (pack_data.defines[kc] && $(`.sound-file[data-keycode="${kc}"]`) && pack_data.defines[kc] != '' && pack_data.defines[kc] != [0, 0]) {
        if (current_key_define_mode == 'manual') {
          if (typeof pack_data.defines[kc] == 'string') {
            $(`.sound-file[data-keycode="${kc}"]`).val(pack_data.defines[kc]);
          } else {
            $(`.sound-start[data-keycode="${kc}"]`).val(pack_data.defines[kc][0]);
            $(`.sound-length[data-keycode="${kc}"]`).val(pack_data.defines[kc][1]);
          }
        } else {
          if (typeof pack_data.defines[kc] == 'string') {
            $(`.key[data-keycode="${kc}"]`)
              .find('.sound-name')
              .val(pack_data.defines[kc]);
          } else {
            $(`.key[data-keycode="${kc}"]`)
              .find('.sound-start')
              .val(pack_data.defines[kc][0]);
            $(`.key[data-keycode="${kc}"]`)
              .find('.sound-length')
              .val(pack_data.defines[kc][1]);
          }
        }
      }
    }
  }

  // ======================================
  // inport
  $('#import').on('click', e => {
    $('#import-input').click();
  });
  $('#import-input').on('change', e => {
    const buffer = fs.readFileSync(e.target.files[0].path);
    const imported_data = JSON.parse(buffer.toString());
    importPack(imported_data);
    _checkIfHasSound();
    genResults();
  });

  // ======================================
  // export
  $('#export').on('click', e => {
    var a = document.createElement('a');
    var file = new Blob([JSON.stringify(pack_data, null, 2)], { type: 'text/plain' });
    a.href = URL.createObjectURL(file);
    a.download = 'config.json';
    a.click();
    URL.revokeObjectURL(a.href);
  });
})(document);
