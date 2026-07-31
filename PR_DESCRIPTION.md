## Problem

Mechvibes cannot be built or run properly on Apple Silicon (M1/M2/M3/M4) Macs:

1. **Rosetta-only builds.** The electron-builder config in `package.json` targets
   only `x64`, so on Apple Silicon the app ships as an Intel binary running under
   Rosetta 2. macOS shows the warning *"This version of mechvibes includes a
   component that will not work with a future release of macOS"*.

2. **Crash + no click sounds on arm64.** Even when an arm64 package is produced,
   the app dies at startup with:

   ```
   Uncaught Exception:
   Error: Cannot find module '.../app.asar/node_modules/iohook/builds/
   electron-v87-darwin-arm64/build/Release/iohook.node'
   ```

   Click sounds are driven by the **iohook** native keyboard-hook module.
   iohook 0.9.3 (2020) only publishes prebuilt binaries for `linux/win32/darwin`
   **x64** — there is no `darwin-arm64` prebuild, so the packaged arm64 app has
   no keyboard hook at all and no click sounds.

3. **Cannot be rebuilt from `node_modules`.** The iohook npm tarball omits the
   `libuiohook` source (it is a git submodule of the iohook repo), so a plain
   `node-gyp rebuild` inside `node_modules/iohook` fails — the source is simply
   not there.

## Changes

- **`package.json`** — macOS build now targets both `x64` and `arm64`, and
  `build:mac` runs `node scripts/build-iohook.js` before electron-builder.
- **`scripts/build-iohook.js`** (new) — compiles the iohook native module from
  source when no prebuilt binary exists for the current platform/arch:
  - clones the tagged `wilix-team/iohook` source **with** its `libuiohook`
    submodule into a temp dir,
  - downloads the matching Electron headers (`artifacts.electronjs.org`;
    the old `atom.io/download/electron` mirror is dead),
  - patches Electron's `common.gypi` to drop the `openssl_fips` condition
    (newer gyp-next cannot evaluate it and fails the configure step; FIPS is
    irrelevant for this addon),
  - runs `node-gyp` with the correct V8 flags for the ABI (pointer compression
    off for arm64, on for x64),
  - installs `iohook.node` into `node_modules/iohook/builds/electron-v87-<platform>-<arch>/...`
    where iohook's loader and electron-builder expect it.
  - Skips instantly when the binary is already present (e.g. x64 prebuilds).
- **`README.md`** — documents the new macOS build behavior.
- **`yarn.lock`** — adds `node-abi` (devDependency) used to compute the Electron
  ABI string for the target path.

## Result

`yarn build:mac` on any macOS machine now produces:
- `dist/Mechvibes-2.3.5.dmg` (Intel / x64)
- `dist/Mechvibes-2.3.5-arm64.dmg` (Apple Silicon / arm64)

The arm64 build runs natively (no Rosetta warning) and click sounds work.

## Verification

Tested on macOS (Apple Silicon, arm64):
- `yarn build:mac` completes; both DMGs produced.
- `dist/mac-arm64/Mechvibes.app/Contents/MacOS/Mechvibes` is `Mach-O 64-bit
  executable arm64`.
- `app.asar` inside the arm64 app contains
  `node_modules/iohook/builds/electron-v87-darwin-arm64/build/Release/iohook.node`
  (verified via `asar list`).
- The compiled `iohook.node` loads, starts, and stops cleanly inside
  Electron 12.2.3.
- App launches and stays running with no uncaught exceptions.
