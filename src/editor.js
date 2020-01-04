'use strict';

// const remote = require('remote'); // Load remote component that contains the dialog dependency
// const dialog = remote.require('dialog'); // Load the dialogs component of the OS
// const fs = require('fs'); // Load the File System to execute our common tasks (CRUD)

const remapper = require('./utils/remapper');
const layouts = require('./libs/layouts');
const keycodes = require('./libs/keycodes');
const $ = require('./assets/jquery');

const layout = layouts[process.platform];
const { sizes } = layouts;
const os_keycode = keycodes[process.platform];

const pack_keycodes = JSON.parse(JSON.stringify(os_keycode));
Object.keys(pack_keycodes).map(kc => {
  pack_keycodes[kc] = null;
});

let selected_keycode = null;
let current_edit_mode = 'visual';
let current_key_define_mode = 'single';

let pack_data = {
  id: `custom-sound-pack-${Date.now()}`,
  name: 'New pack',
  includes_numpad: false,
  defines: pack_keycodes,
};

(function(document) {
  $(document).ready(() => {
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
            const _key = $(`
              <div id="key-${key}" class="key ${sizes[key] ? sizes[key] : ''} ${key ? '' : 'key-blank'}" data-keycode="${key}">
                <div class="letter">${os_keycode[key] || ''}</div> <div class="key-rk">${r}-${k++}</div>
              </div>
           `);
            _key.appendTo(_row);
            _genPopover(_key, key);
            // manual edit
            if (key) {
              const manual_key = $(`
                <div id="manual-key-${key}" style="border-bottom: 1px solid #eee; margin-bottom: 10px;padding-bottom: 10px; display: flex; justify-content: space-between; align-items: center" class="manual-key">
                  <div style="font-weight: bold; margin-right: 10px">${os_keycode[key]}</div>
                  <div class="define-mode define-mode-single">
                    <div style="display: flex;">
                      <input type="number" placeholder="Start..." style="margin-right: 5px; width: 100px;" class="key-define sound-start" data-keycode="${key}"/>
                      <input type="number" placeholder="Length..." style=" width: 100px;" class="key-define sound-length" data-keycode="${key}"/>
                    </div>
                  </div>
                  <div class="define-mode define-mode-multi">
                    <input type="text" placeholder="File name..." style="margin-right: 5px; width: 100%;" class="key-define sound-file" data-keycode="${key}"/>
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
      if (!$(e.target).hasClass('.close')) {
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
          pack_keycodes[keycode] = [Number(start[0].value), Number(length[0].value)];
        } else {
          const file_name = $(e.currentTarget).find('.sound-name');
          pack_keycodes[keycode] = file_name[0].value;
        }
        _checkIfHasSound();
      });
    });

    $('.edit-mode-manual').on('change', '.key-define', e => {
      const keycode = $(e.currentTarget).data('keycode');
      if (!current_key_define_mode || current_key_define_mode == 'single') {
        let start = 0;
        let length = 0;
        if (e.currentTarget.hasClass('sound-start')) {
          start = $(e.currentTarget);
        } else {
          length = $(e.currentTarget);
        }
        pack_keycodes[keycode] = [Number(start[0].value), Number(length[0].value)];
      } else {
        const file_name = $(e.currentTarget);
        pack_keycodes[keycode] = file_name[0].value;
      }
      _checkIfHasSound();
    });

    _checkIfHasSound();

    $('.edit-mode-manual').hide();
    $('#edit-mode').on('change', e => {
      current_edit_mode = e.target.value;
      $('.edit-mode').hide();
      $(`.edit-mode-${e.target.value}`).show();
      if (e.target.value == 'manual') {
        for (let kc in pack_keycodes) {
          if (pack_keycodes[kc]) {
            if (typeof pack_keycodes[kc] == 'string') {
              $(`.sound-file[data-keycode="${kc}"]`).value = pack_keycodes[kc];
            } else {
              $(`.sound-start[data-keycode="${kc}"]`)[0].value = pack_keycodes[kc][0];
              $(`.sound-length[data-keycode="${kc}"]`)[0].value = pack_keycodes[kc][1];
            }
          }
        }
      }
    });

    $('.define-mode-multi').hide();
    $('#key-define-mode').on('change', e => {
      current_key_define_mode = e.target.value;
      $('.define-mode').hide();
      $(`.define-mode-${e.target.value}`).show();
      $('.key-define').each((index, el) => {
        el.value = '';
      });
    });
  });

  function _checkIfHasSound() {
    Object.keys(pack_keycodes).map(kc => {
      if (pack_keycodes[kc] != null) {
        $(`.key[data-keycode=${kc}]`).addClass('key-has-sound');
      }
    });
    genResults();
  }

  function _genPopover(target, keycode) {
    const popover = $(`
      <div class="popover" style="min-width: 250px">
        <div class="define-mode define-mode-single" style="margin-bottom: 10px">
          <div style="margin-bottom: 5px">Set start and length (ms)</div>
          <div style="display: flex; ">
            <input type="number" placeholder="Start..." style="margin-right: 10px; width: 100px;" class="key-define sound-start"/>
            <input type="number" placeholder="Length..." style="width: 100px" class="key-define sound-length"/>
          </div>
        </div>

        <div class="define-mode define-mode-multi" style="margin-bottom: 20px">
          <div style="margin-bottom: 5px">Enter audio file name:</div>
          <input type="text" placeholder="Sound file name..."  class="key-define sound-name"/>
        </div>

        <div class="divider"></div>
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
})(document);
