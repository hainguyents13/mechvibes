#!/usr/bin/env node
/**
 * Builds the iohook native module from source for the current platform/arch
 * when no prebuilt binary is available.
 *
 * Why this exists: iohook 0.9.3 (2020) only ships prebuilt binaries for
 * linux/win32/darwin x64 (and win32 ia32). On Apple Silicon (darwin-arm64)
 * there is no prebuilt `iohook.node`, so the packaged app crashes at startup
 * with "Cannot find module .../electron-v87-darwin-arm64/.../iohook.node"
 * and keyboard click sounds never work.
 *
 * The npm tarball also does NOT include the libuiohook source (it is a git
 * submodule of the iohook repository), so node-gyp cannot build directly from
 * node_modules/iohook. This script clones the tagged iohook source WITH
 * submodules, compiles iohook.node against the project's Electron headers,
 * and drops the binary into node_modules/iohook/builds/ so the subsequent
 * electron-builder pass packages it into the app.
 *
 * Usage: node scripts/build-iohook.js   (runs automatically via build:mac)
 */

const { execFileSync } = require('child_process');
const fs = require('fs');
const os = require('os');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const ELECTRON_VERSION = require(path.join(ROOT, 'node_modules', 'electron', 'package.json')).version;
const IOHOOK_VERSION = '0.9.3';
const IOHOOK_GIT = 'https://github.com/wilix-team/iohook.git';
const HEADERS_BASE = 'https://artifacts.electronjs.org/headers/dist';
const ARCH = process.arch === 'arm64' ? 'arm64' : process.arch;

const nodeAbi = require('node-abi');
const ABI = nodeAbi.getAbi(ELECTRON_VERSION, 'electron'); // e.g. 87 for Electron 12

const essential = `electron-v${ABI}-${process.platform}-${ARCH}`;
const outDir = path.join(ROOT, 'node_modules', 'iohook', 'builds', essential, 'build', 'Release');
const outFile = path.join(outDir, 'iohook.node');
const outDylib = path.join(outDir, 'uiohook.dylib');

function run(cmd, args, opts = {}) {
  console.log(`$ ${cmd} ${args.join(' ')}`);
  return execFileSync(cmd, args, { stdio: 'inherit', ...opts });
}

function alreadyBuilt() {
  return fs.existsSync(outFile) && fs.existsSync(outDylib);
}

function main() {
  if (alreadyBuilt()) {
    console.log(`iohook binary already present for ${essential}, skipping.`);
    process.exit(0);
  }

  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'iohook-src-'));
  console.log(`Building iohook ${IOHOOK_VERSION} for ${essential} in ${tmp} ...`);

  try {
    // 1. iohook source WITH libuiohook submodule (absent from the npm tarball)
    run('git', ['clone', '--recursive', '--depth', '1', '--branch', `v${IOHOOK_VERSION}`, IOHOOK_GIT, tmp]);

    // 2. node-gyp + nan (nan ships in iohook's devDeps; install without running
    //    iohook's install script, which would try to download a prebuild)
    run('npm', ['install', '--no-save', '--ignore-scripts', 'node-gyp@10.0.1', 'nan'], { cwd: tmp });

    // 3. Electron headers for the exact runtime/ABI (atom.io/download/electron
    //    is dead; artifacts.electronjs.org is the working mirror). The tarball
    //    extracts a top-level `node_headers/` directory, so unpack into tmp.
    const headersTar = path.join(tmp, 'node-headers.tar.gz');
    const headersDir = path.join(tmp, 'node_headers');
    const headersUrl = `${HEADERS_BASE}/v${ELECTRON_VERSION}/node-v${ELECTRON_VERSION}-headers.tar.gz`;
    run('curl', ['-sL', '-o', headersTar, headersUrl]);
    run('tar', ['-xzf', headersTar, '-C', tmp]);

    // 4. Stage the darwin binding.gyp files (build.js copies these at build
    //    time; they are not present at the package root in the npm tarball)
    fs.copyFileSync(path.join(tmp, 'build_def', process.platform, 'binding.gyp'), path.join(tmp, 'binding.gyp'));
    fs.copyFileSync(path.join(tmp, 'build_def', process.platform, 'uiohook.gyp'), path.join(tmp, 'uiohook.gyp'));

    // 5. Patch Electron's common.gypi: newer gyp-next cannot evaluate the
    //    `openssl_fips != ""` condition (name 'openssl_fips' is not defined).
    //    FIPS is irrelevant for this addon, so drop the condition block.
    const commonGypi = path.join(headersDir, 'include', 'node', 'common.gypi');
    const common = fs.readFileSync(commonGypi, 'utf8');
    const fipsCond = `      ['openssl_fips != ""', {
        'openssl_product': '<(STATIC_LIB_PREFIX)crypto<(STATIC_LIB_SUFFIX)',
      }, {
        'openssl_product': '<(STATIC_LIB_PREFIX)openssl<(STATIC_LIB_SUFFIX)',
      }],`;
    if (common.includes(fipsCond)) {
      fs.writeFileSync(commonGypi, common.replace(fipsCond, ''));
      console.log('patched common.gypi: removed openssl_fips condition');
    } else {
      console.log('common.gypi: openssl_fips condition not found (already patched?), continuing');
    }

    // 6. Build. Flags mirror iohook's build.js for abi >= 80.
    const gyp = path.join(tmp, 'node_modules', '.bin', 'node-gyp');
    const args = [
      'rebuild',
      `--target=${ELECTRON_VERSION}`,
      `--arch=${ARCH}`,
      `--nodedir=${headersDir}`,
      '--build_v8_with_gn=false',
      '--enable_lto=false',
    ];
    if (ARCH === 'x64') {
      args.push('--v8_enable_pointer_compression=1');
    } else {
      args.push('--v8_enable_pointer_compression=0');
      args.push('--v8_enable_31bit_smis_on_64bit_arch=1');
    }
    run(gyp, args, { cwd: tmp });

    // 7. Install the binary where iohook's index.js and electron-builder expect
    //    it. iohook.node dynamically links @rpath/uiohook.dylib (uiohook is
    //    built as a shared library), so the dylib must ship alongside it —
    //    exactly like iohook's official x64 prebuild layout. electron-builder
    //    unpacks .dylib files from node_modules into app.asar.unpacked, which
    //    is how the packaged app finds it at runtime.
    fs.mkdirSync(outDir, { recursive: true });
    fs.copyFileSync(path.join(tmp, 'build', 'Release', 'iohook.node'), outFile);
    fs.copyFileSync(path.join(tmp, 'build', 'Release', 'uiohook.dylib'), path.join(outDir, 'uiohook.dylib'));
    console.log(`Installed iohook.node + uiohook.dylib -> ${outDir}`);
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
}

main();
