// Smoke test: load iohook from node_modules, verify the dylib resolves, start/stop hook.
const iohook = require('iohook');
console.log('iohook version:', iohook.version);
console.log('LOADED OK - dylib resolved');
iohook.start();
setTimeout(() => {
  iohook.stop();
  console.log('hook started and stopped cleanly');
  process.exit(0);
}, 500);
