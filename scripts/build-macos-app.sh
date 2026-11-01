#!/usr/bin/env bash
#
# Assemble Mechvibes.app and package it as a compressed DMG.
#
# `dx bundle` is deliberately NOT used: DioxusLabs/dioxus#5723 makes the 0.7.x
# resource copier fail on every directory entry in Dioxus.toml's `resources`,
# and worse, it exits non-zero having already produced a .app with an EMPTY
# Contents/Resources. Hand-assembly is fully specified and verifiable, so it is
# what ships until that fix lands (confirmed present in 0.8.0-alpha.1).
#
# Usage: scripts/build-macos-app.sh <version> [--skip-build]
#   Builds target/release/mechvibes (cargo build --release --locked) first.
#   --skip-build reuses an existing binary, e.g. when CI built it already.
#   Writes dist/mechvibes-<version>-macos-<arch>.dmg
#
# macOS only (uses sips, iconutil, codesign, hdiutil).

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib/common.sh"

SKIP_BUILD=0
VERSION=""
for arg in "$@"; do
  case "$arg" in
    --skip-build) SKIP_BUILD=1 ;;
    -*) echo "unknown option: $arg" >&2; exit 2 ;;
    *) VERSION="$arg" ;;
  esac
done
if [ -z "$VERSION" ]; then
  echo "usage: build-macos-app.sh <version> [--skip-build]" >&2
  exit 2
fi
ARCH=$(uname -m)

APP_NAME="Mechvibes"
BUNDLE="dist/${APP_NAME}.app"
BINARY="target/release/mechvibes"
IDENTIFIER="com.hainguyents13.mechvibesdx"
# arm64 macOS starts at 11.0; nothing older can run an Apple Silicon build. The
# Intel build keeps the same floor: nothing older has been tried.
MIN_MACOS="11.0"

# Fail first, and with a clear message, if the Rust in use is older than the
# `rust-version` declared in Cargo.toml.
verify_rust_toolchain

# Build step. The release binary is built here by default so a wrong toolchain
# compiler older than `rust-version` in Cargo.toml, or a stale Cargo.lock,
# fails at the start, with cargo's own message, instead of surfacing later.
# `--locked` refuses to modify Cargo.lock.
#
# With --skip-build the caller has already built the binary (CI does, once, and
# shares it between packages). Nothing is compiled here, so run a plain
# `cargo check --locked` to still validate the pin and the lockfile.
if [ "$SKIP_BUILD" -eq 1 ]; then
  echo "Skipping build (--skip-build); checking the crate against the pinned toolchain..."
  if ! cargo check --locked; then
    echo "::error::'cargo check --locked' failed - check the Rust version against rust-version in Cargo.toml, or Cargo.lock"
    exit 1
  fi
else
  echo "Building release binary..."
  cargo build --release --locked
fi

if [ ! -f "$BINARY" ]; then
  echo "::error::$BINARY not found - run without --skip-build to build it"
  exit 1
fi

# --skip-build packages whatever binary is already there, which could be left
# over from an older checkout. Refuse a binary older than anything it is built
# from, rather than ship stale code under a new version number. Rebuild it, or
# run without --skip-build.
if [ "$SKIP_BUILD" -eq 1 ]; then
  stale=$({ find src assets patches Cargo.toml Cargo.lock build.rs -type f -newer "$BINARY" 2>/dev/null || true; } | head -5)
  if [ -n "$stale" ]; then
    echo "::error::$BINARY is older than these files - rebuild it, or run without --skip-build:"
    echo "$stale"
    exit 1
  fi
fi

rm -rf dist staging
mkdir -p "$BUNDLE/Contents/MacOS" "$BUNDLE/Contents/Resources"

# ---------------------------------------------------------------------------
# 1. Executable
# ---------------------------------------------------------------------------
cp "$BINARY" "$BUNDLE/Contents/MacOS/mechvibes"
chmod +x "$BUNDLE/Contents/MacOS/mechvibes"

# ---------------------------------------------------------------------------
# 2. Resources
#
# `soundpacks/` is what src/state/paths.rs resolves via ../Resources when it
# detects the bundle layout. `assets/` must ALSO be here because
# dioxus-asset-resolver hardcodes Contents/Resources as the asset root on
# macOS - without it every asset!() font and icon 404s at runtime.
# ---------------------------------------------------------------------------
cp -R soundpacks "$BUNDLE/Contents/Resources/"
cp -R assets "$BUNDLE/Contents/Resources/"
cp README-macos.txt "$BUNDLE/Contents/Resources/"

# The failure mode this job already shipped once: an archive whose Resources
# were silently empty. Assert instead of trusting the copy.
#
# Compared against the source tree rather than a hardcoded number, and counting
# every audio extension - the mouse packs are .mp3, so an .ogg-only check would
# pass while silently dropping all four of them. A count mismatch means the
# copy lost files.
count_audio() { find "$1" \( -name "*.ogg" -o -name "*.mp3" -o -name "*.wav" \) | wc -l | tr -d ' '; }
expected=$(count_audio soundpacks)
packs=$(count_audio "$BUNDLE/Contents/Resources/soundpacks")
if [ "$packs" -eq 0 ]; then
  echo "::error::No soundpacks in the bundle - refusing to ship a silent app"
  exit 1
fi
if [ "$packs" -ne "$expected" ]; then
  echo "::error::Bundled $packs soundpack audio files but the source tree has $expected"
  exit 1
fi
echo "Bundled $packs soundpack audio files (matches source tree)"

# Every built-in pack listed in src/state/paths.rs must have its config.json,
# or that pack silently fails to load at runtime.
configs=$(find "$BUNDLE/Contents/Resources/soundpacks" -name "config.json" | wc -l | tr -d ' ')
expected_configs=$(find soundpacks -name "config.json" | wc -l | tr -d ' ')
if [ "$configs" -ne "$expected_configs" ]; then
  echo "::error::Bundled $configs soundpack config.json files, expected $expected_configs"
  exit 1
fi
echo "Bundled $configs soundpack config.json files"

if [ ! -d "$BUNDLE/Contents/Resources/assets/fonts" ]; then
  echo "::error::assets/fonts missing from Resources - asset!() lookups would fail"
  exit 1
fi

# ---------------------------------------------------------------------------
# 3. Icon: generate .icns on the runner from the repo's 512x512 PNG.
# ---------------------------------------------------------------------------
ICONSET="staging/${APP_NAME}.iconset"
mkdir -p "$ICONSET"
for size in 16 32 128 256 512; do
  sips -z $size $size assets/icon.png --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z $double $double assets/icon.png --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$BUNDLE/Contents/Resources/${APP_NAME}.icns"
echo "Generated ${APP_NAME}.icns"

# ---------------------------------------------------------------------------
# 4. Info.plist
#
# LSUIElement is NOT set: the app has a real window, and a tray-only agent
# cannot be granted Accessibility permission through the normal UI flow.
# NSMicrophoneUsageDescription is absent on purpose - the app only plays audio.
# ---------------------------------------------------------------------------
cat > "$BUNDLE/Contents/Info.plist" << PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleExecutable</key>
    <string>mechvibes</string>
    <key>CFBundleIconFile</key>
    <string>${APP_NAME}</string>
    <key>CFBundleIdentifier</key>
    <string>${IDENTIFIER}</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>${APP_NAME}</string>
    <key>CFBundleDisplayName</key>
    <string>${APP_NAME}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>
    <key>CFBundleVersion</key>
    <string>${VERSION}</string>
    <key>LSMinimumSystemVersion</key>
    <string>${MIN_MACOS}</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>
</dict>
</plist>
PLIST

plutil -lint "$BUNDLE/Contents/Info.plist"

# ---------------------------------------------------------------------------
# 5. Ad-hoc signature.
#
# This is NOT notarization and NOT a Developer ID signature - it only makes the
# bundle internally consistent so macOS will run it after the user clears
# quarantine. `spctl` still rejects it, which is expected and NOT gated on.
# ---------------------------------------------------------------------------
codesign --force --deep -s - "$BUNDLE"
codesign --verify --deep --strict --verbose=2 "$BUNDLE"
echo "Ad-hoc signature verified"

echo "--- Gatekeeper assessment (expected to FAIL: unsigned/not notarized) ---"
spctl --assess --type execute --verbose=4 "$BUNDLE" || true
echo "-----------------------------------------------------------------------"

# ---------------------------------------------------------------------------
# 6. DMG with an /Applications symlink so the user can drag-install.
# ---------------------------------------------------------------------------
DMG_ROOT="staging/dmg"
mkdir -p "$DMG_ROOT"
cp -R "$BUNDLE" "$DMG_ROOT/"
ln -s /Applications "$DMG_ROOT/Applications"
cp README-macos.txt "$DMG_ROOT/README.txt"

# "arm64"/"x86_64" contain no "x64" substring and this is not a .exe, so the
# Windows auto-updater filter in src/utils/auto_updater.rs cannot pick it up.
DMG="dist/mechvibes-${VERSION}-macos-${ARCH}.dmg"
hdiutil create -volname "${APP_NAME}" -srcfolder "$DMG_ROOT" -ov -format UDZO "$DMG"
hdiutil verify "$DMG"

# ---------------------------------------------------------------------------
# 7. Mount the finished DMG and check what is actually inside it, so the build
# log is itself the evidence that the shipped artifact has the right shape. A
# green exit from the assembly steps only proves the commands ran.
# ---------------------------------------------------------------------------
echo "=== Verifying $DMG ($(du -h "$DMG" | cut -f1)) ==="
MOUNT_POINT="$(mktemp -d)"
trap 'hdiutil detach "$MOUNT_POINT" -quiet 2>/dev/null || true; rmdir "$MOUNT_POINT" 2>/dev/null || true' EXIT
hdiutil attach "$DMG" -mountpoint "$MOUNT_POINT" -nobrowse -readonly -quiet

MOUNTED_APP="$MOUNT_POINT/${APP_NAME}.app"
[ -d "$MOUNTED_APP" ] || { echo "::error::DMG has no ${APP_NAME}.app"; exit 1; }
[ -L "$MOUNT_POINT/Applications" ] || { echo "::error::DMG has no Applications symlink"; exit 1; }

echo "--- bundle tree ---"
find "$MOUNTED_APP" -maxdepth 3 | sort
echo "--- binary ---"
file "$MOUNTED_APP/Contents/MacOS/mechvibes"
file "$MOUNTED_APP/Contents/MacOS/mechvibes" | grep -q "Mach-O" \
  || { echo "::error::bundled executable is not a Mach-O binary"; exit 1; }

DMG_AUDIO=$(count_audio "$MOUNTED_APP/Contents/Resources/soundpacks")
if [ "$DMG_AUDIO" -ne "$expected" ]; then
  echo "::error::DMG holds $DMG_AUDIO soundpack audio files but the source tree has $expected"
  exit 1
fi
echo "soundpack audio in DMG: $DMG_AUDIO (matches source tree)"

# Ad-hoc signature only. spctl is deliberately NOT gated on: it fails without
# notarization, which is expected for this build.
codesign --verify --deep --strict --verbose=2 "$MOUNTED_APP"

hdiutil detach "$MOUNT_POINT" -quiet
rmdir "$MOUNT_POINT"
trap - EXIT

# Ship the bundle only inside the DMG.
rm -rf "$BUNDLE"
cp README-macos.txt "dist/README-macos-${VERSION}.txt"

assert_no_updater_collision dist

echo "--- dist/ ---"
ls -la dist/
