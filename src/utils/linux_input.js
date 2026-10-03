'use strict';

// Reads key events straight from the kernel (/dev/input/event*) instead of hooking X11.
// This is what lets Mechvibes hear typing on Wayland, where iohook only sees XWayland windows.
// It needs read access to the keyboard devices, which normally means being in the "input" group.
// Exposes the same start()/stop()/on('keydown'|'keyup') interface as iohook.

const { EventEmitter } = require('events');
const { spawn } = require('child_process');
const fs = require('fs');
const log = require('electron-log');

const DEVICES_LIST = '/proc/bus/input/devices';
const INPUT_DIR = '/dev/input';

const LONG_BITS = ['x64', 'arm64', 'ppc64', 's390x'].includes(process.arch) ? 64 : 32;
const KEY_F12 = 88; // evdev and iohook share every code up to here
const LETTER_KEYS = [16, 30, 44]; // KEY_Q, KEY_A, KEY_Z: tells real keyboards apart from power buttons, hotkeys etc.
const MAX_KEYBOARDS = 16; // each open keyboard holds one reader thread

// evdev codes that differ from the iohook (libuiohook) codes soundpacks are defined with
const EVDEV_TO_STANDARD = {
  96: 3612, // KP Enter
  97: 3613, // Right Ctrl
  98: 3637, // KP /
  99: 3639, // PrtSc
  100: 3640, // Right Alt
  102: 3655, // Home
  103: 57416, // Up
  104: 3657, // PgUp
  105: 57419, // Left
  106: 57421, // Right
  107: 3663, // End
  108: 57424, // Down
  109: 3665, // PgDn
  110: 3666, // Ins
  111: 3667, // Del
  117: 3597, // KP =
  119: 3653, // Pause
  125: 3675, // Left Meta
  126: 3676, // Right Meta
  127: 3677, // Menu
  183: 91, // F13
  184: 92, // F14
  185: 93, // F15
};

function toStandardKeycode(code) {
  if (EVDEV_TO_STANDARD[code] !== undefined) return EVDEV_TO_STANDARD[code];
  if (code <= KEY_F12) return code;
  return 0; // iohook's VC_UNDEFINED, for keys it doesn't know either
}

// /proc lists the supported keys as a bitmap of hex longs, most significant first
function hasKey(keyBitmap, code) {
  const words = keyBitmap.trim().split(/\s+/).reverse();
  const word = words[Math.floor(code / LONG_BITS)];
  if (!word) return false;
  return ((BigInt(`0x${word}`) >> BigInt(code % LONG_BITS)) & 1n) === 1n;
}

function findKeyboards() {
  let devices;
  try {
    devices = fs.readFileSync(DEVICES_LIST, 'utf8');
  } catch (err) {
    log.warn(`Could not read ${DEVICES_LIST}: ${err.message}`);
    return [];
  }

  const keyboards = [];
  for (const block of devices.split(/\n\s*\n/)) {
    const name = (block.match(/^N: Name="(.*)"$/m) || [])[1];
    const handlers = (block.match(/^H: Handlers=(.*)$/m) || [])[1] || '';
    const keyBitmap = (block.match(/^B: KEY=(.*)$/m) || [])[1];
    const event = handlers.split(/\s+/).find((handler) => /^event\d+$/.test(handler));
    if (!event || !keyBitmap || !LETTER_KEYS.every((code) => hasKey(keyBitmap, code))) continue;
    keyboards.push({ name, path: `${INPUT_DIR}/${event}` });
  }
  return keyboards;
}

function isReadable(path) {
  try {
    fs.accessSync(path, fs.constants.R_OK);
    return true;
  } catch (err) {
    return false;
  }
}

// Runs inside the reader process, serialized with toString(), so it can only use its arguments.
// Takes device paths on stdin, writes "<code> <value>" per key press/release and "closed <path>" to stdout.
function readerMain(long_bits) {
  const fs = require('fs');
  // struct input_event: struct timeval, __u16 type, __u16 code, __s32 value
  const EVENT_SIZE = long_bits / 8 * 2 + 8;
  const EV_KEY = 0x01;
  const BTN_MISC = 0x100; // codes from here on are mouse/joystick buttons, not keys

  function open(path) {
    let pending = Buffer.alloc(0);
    fs.createReadStream(path, { highWaterMark: EVENT_SIZE * 64 })
      .on('data', (chunk) => {
        const buffer = pending.length ? Buffer.concat([pending, chunk]) : chunk;
        let offset = 0;
        let lines = '';
        for (; offset + EVENT_SIZE <= buffer.length; offset += EVENT_SIZE) {
          const type = buffer.readUInt16LE(offset + EVENT_SIZE - 8);
          const code = buffer.readUInt16LE(offset + EVENT_SIZE - 6);
          const value = buffer.readInt32LE(offset + EVENT_SIZE - 4);
          // autorepeat (value 2) is dropped, the app only plays a sound for the first press
          if (type === EV_KEY && code < BTN_MISC && (value === 0 || value === 1)) lines += `${code} ${value}\n`;
        }
        pending = buffer.subarray(offset);
        if (lines) process.stdout.write(lines);
      })
      .on('error', () => {})
      .on('close', () => process.stdout.write(`closed ${path}\n`));
  }

  let input = '';
  process.stdin.setEncoding('utf8');
  process.stdin.on('data', (chunk) => {
    input += chunk;
    const paths = input.split('\n');
    input = paths.pop();
    paths.filter(Boolean).forEach(open);
  });
  // exiting normally would wait for the blocked reads, so die at once when Mechvibes goes away
  process.stdin.on('end', () => process.kill(process.pid, 'SIGKILL'));
}

class LinuxInput extends EventEmitter {
  // true when at least one keyboard can be read, otherwise iohook is the better choice
  static isAvailable() {
    return findKeyboards().some((keyboard) => isReadable(keyboard.path));
  }

  constructor() {
    super();
    this.running = false;
    this.reader = null;
    this.open_paths = new Set();
    this.watcher = null;
    this.rescan_timeout = null;
  }

  start() {
    if (this.running) return;
    this.running = true;
    this.startReader();
    this.openKeyboards();

    // pick up keyboards plugged in later; udev sets their permissions shortly after the node appears
    try {
      this.watcher = fs.watch(INPUT_DIR, (_, filename) => {
        if (!/^event\d+$/.test(`${filename}`)) return;
        clearTimeout(this.rescan_timeout);
        this.rescan_timeout = setTimeout(() => this.openKeyboards(), 1000);
      });
    } catch (err) {
      log.warn(`Not watching ${INPUT_DIR} for new keyboards: ${err.message}`);
    }
  }

  stop() {
    this.running = false;
    if (this.watcher) this.watcher.close();
    this.watcher = null;
    clearTimeout(this.rescan_timeout);
    if (this.reader) this.reader.kill('SIGKILL');
    this.reader = null;
    this.open_paths.clear();
  }

  // Reads block until a key is pressed, and each one holds a libuv threadpool thread meanwhile.
  // Doing them in the app would starve its own file access, so they get a separate process
  // (Electron running as plain Node) with a threadpool sized for them.
  startReader() {
    const reader = spawn(process.execPath, ['-e', `(${readerMain})(${LONG_BITS})`], {
      env: { ...process.env, ELECTRON_RUN_AS_NODE: '1', UV_THREADPOOL_SIZE: `${MAX_KEYBOARDS + 4}` },
      stdio: ['pipe', 'pipe', 'inherit'],
    });
    this.reader = reader;
    reader.stdin.on('error', () => {}); // the exit handler deals with a dead reader

    let output = '';
    reader.stdout.setEncoding('utf8');
    reader.stdout.on('data', (chunk) => {
      if (this.reader !== reader) return; // muted, drop what was still in the pipe
      output += chunk;
      const lines = output.split('\n');
      output = lines.pop();
      for (const line of lines) {
        const [first, second] = line.split(' ');
        if (first === 'closed') {
          log.info(`Stopped listening to ${second}`);
          this.open_paths.delete(second);
          continue;
        }
        const type = second === '1' ? 'keydown' : 'keyup';
        const code = Number(first);
        this.emit(type, { type, keycode: toStandardKeycode(code), rawcode: code });
      }
    });

    reader.on('error', (err) => log.error(`Keyboard reader failed: ${err.message}`));
    reader.on('exit', (code, signal) => {
      if (this.reader !== reader) return; // stopped on purpose
      log.error(`Keyboard reader exited (${signal || code}), restarting`);
      this.reader = null;
      this.open_paths.clear();
      setTimeout(() => {
        if (this.running && this.reader === null) {
          this.startReader();
          this.openKeyboards();
        }
      }, 1000);
    });
  }

  openKeyboards() {
    if (!this.reader) return;
    for (const keyboard of findKeyboards()) {
      if (this.open_paths.has(keyboard.path) || this.open_paths.size >= MAX_KEYBOARDS) continue;
      if (!isReadable(keyboard.path)) continue;
      log.info(`Listening to keyboard "${keyboard.name}" (${keyboard.path})`);
      this.open_paths.add(keyboard.path);
      this.reader.stdin.write(`${keyboard.path}\n`);
    }
  }
}

LinuxInput.toStandardKeycode = toStandardKeycode;

module.exports = LinuxInput;
