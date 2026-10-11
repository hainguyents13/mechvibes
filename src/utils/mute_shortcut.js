const SPECIAL_KEYS = {
  ' ': 'Space',
  ArrowDown: 'Down',
  ArrowLeft: 'Left',
  ArrowRight: 'Right',
  ArrowUp: 'Up',
  Backspace: 'Backspace',
  Delete: 'Delete',
  End: 'End',
  Enter: 'Enter',
  Escape: 'Escape',
  Home: 'Home',
  Insert: 'Insert',
  PageDown: 'PageDown',
  PageUp: 'PageUp',
  Tab: 'Tab',
  '+': 'Plus',
};
const ACCELERATOR_MODIFIERS = new Set(['Alt', 'Command', 'Control', 'Shift', 'Super']);
const ACCELERATOR_KEYS = new Set([
  'Backspace', 'Delete', 'Down', 'End', 'Enter', 'Escape', 'Home', 'Insert',
  'Left', 'PageDown', 'PageUp', 'Plus', 'Right', 'Space', 'Tab', 'Up',
]);

function buildMuteAccelerator(key, event, platform) {
  const modifiers = [];
  if (event.ctrlKey) modifiers.push('Control');
  if (event.altKey) modifiers.push('Alt');
  if (event.shiftKey) modifiers.push('Shift');
  if (event.metaKey) modifiers.push(platform === 'darwin' ? 'Command' : 'Super');

  if (modifiers.length === 0) {
    return { success: false, reason: 'modifier-required' };
  }

  let accelerator_key = SPECIAL_KEYS[key];
  if (!accelerator_key && /^[a-z0-9]$/i.test(key)) {
    accelerator_key = key.toUpperCase();
  }
  if (!accelerator_key && /^F([1-9]|1[0-9]|2[0-4])$/.test(key)) {
    accelerator_key = key;
  }
  if (!accelerator_key) {
    return { success: false, reason: 'unsupported-key' };
  }

  return { success: true, accelerator: [...modifiers, accelerator_key].join('+') };
}

function registerMuteShortcut(globalShortcut, accelerator, currentAccelerator, callback) {
  if (accelerator === currentAccelerator) {
    return { success: true };
  }

  if (accelerator === '') {
    if (currentAccelerator) {
      globalShortcut.unregister(currentAccelerator);
    }
    return { success: true };
  }

  const parts = accelerator.split('+');
  const modifiers = parts.slice(0, -1);
  const key = parts[parts.length - 1];
  const validKey = /^[A-Z0-9]$/.test(key) || /^F([1-9]|1[0-9]|2[0-4])$/.test(key) || ACCELERATOR_KEYS.has(key);
  const validModifiers = modifiers.length > 0
    && modifiers.every((modifier) => ACCELERATOR_MODIFIERS.has(modifier))
    && new Set(modifiers).size === modifiers.length;
  if (!validKey || !validModifiers) {
    return { success: false, reason: 'invalid' };
  }

  let registered;
  try {
    registered = globalShortcut.register(accelerator, callback);
  } catch (error) {
    return { success: false, reason: 'invalid', error };
  }

  if (!registered) {
    return { success: false, reason: 'unavailable' };
  }

  if (currentAccelerator) {
    globalShortcut.unregister(currentAccelerator);
  }
  return { success: true };
}

module.exports = { buildMuteAccelerator, registerMuteShortcut };
