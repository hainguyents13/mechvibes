const assert = require('assert');
const { buildMuteAccelerator, registerMuteShortcut } = require('../src/utils/mute_shortcut');

assert.deepStrictEqual(
  buildMuteAccelerator('m', { ctrlKey: true, shiftKey: true }, 'win32'),
  { success: true, accelerator: 'Control+Shift+M' },
);
assert.deepStrictEqual(
  buildMuteAccelerator('F8', { altKey: true }, 'linux'),
  { success: true, accelerator: 'Alt+F8' },
);
assert.deepStrictEqual(
  buildMuteAccelerator('x', { metaKey: true }, 'darwin'),
  { success: true, accelerator: 'Command+X' },
);
assert.deepStrictEqual(
  buildMuteAccelerator('m', {}, 'win32'),
  { success: false, reason: 'modifier-required' },
);
assert.deepStrictEqual(
  buildMuteAccelerator('Unidentified', { ctrlKey: true }, 'win32'),
  { success: false, reason: 'unsupported-key' },
);

const calls = [];
const shortcuts = {
  register(accelerator, callback) {
    calls.push(['register', accelerator, callback]);
    return accelerator !== 'Alt+X';
  },
  unregister(accelerator) {
    calls.push(['unregister', accelerator]);
  },
};
const callback = () => {};
assert.deepStrictEqual(
  registerMuteShortcut(shortcuts, 'Control+M', 'Control+K', callback),
  { success: true },
);
assert.deepStrictEqual(calls, [
  ['register', 'Control+M', callback],
  ['unregister', 'Control+K'],
]);

calls.length = 0;
assert.deepStrictEqual(
  registerMuteShortcut(shortcuts, 'M', 'Control+K', callback),
  { success: false, reason: 'invalid' },
);
assert.deepStrictEqual(calls, []);

calls.length = 0;
assert.deepStrictEqual(
  registerMuteShortcut(shortcuts, 'Alt+X', 'Control+K', callback),
  { success: false, reason: 'unavailable' },
);
assert.deepStrictEqual(calls, [['register', 'Alt+X', callback]]);

calls.length = 0;
assert.deepStrictEqual(
  registerMuteShortcut(shortcuts, 'Control+K', 'Control+K', callback),
  { success: true },
);
assert.deepStrictEqual(calls, []);

assert.deepStrictEqual(
  registerMuteShortcut(shortcuts, '', 'Control+K', callback),
  { success: true },
);
assert.deepStrictEqual(calls, [['unregister', 'Control+K']]);

const invalidShortcuts = {
  register() {
    throw new Error('Invalid accelerator');
  },
  unregister() {
    assert.fail('An invalid replacement must not unregister the current shortcut.');
  },
};
const invalidResult = registerMuteShortcut(invalidShortcuts, 'Control+M', 'Control+K', callback);
assert.strictEqual(invalidResult.success, false);
assert.strictEqual(invalidResult.reason, 'invalid');
assert.strictEqual(invalidResult.error.message, 'Invalid accelerator');

console.log('Mute shortcut tests passed.');
