const fs = require('fs-extra');

const current = "v2.3.10";
const new_version = "v2.3.9";

console.log(new_version.localeCompare(current, undefined, { numeric: true }));

// const package = fs.readJsonSync('package.json');
// const version = package.version.split('.');
// const patch = parseInt(version[2]) + 1;
// version[2] = patch;
// package.version = version.join('.');

// fs.writeJsonSync('package.json', package, { spaces: 2 });
