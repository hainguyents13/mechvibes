#!/usr/bin/env bash
#
# Assemble AppDir/ and package it as an AppImage.
#
# Usage: ./scripts/build-linux-appimage.sh <version> [--skip-build] [--no-deb]
#   e.g. ./scripts/build-linux-appimage.sh 0.8.0
#
# Builds target/release/mechvibes (cargo build --release --locked), then
# produces BOTH Linux packages from that one binary:
#   dist/mechvibes_<version>_amd64.deb         (needs cargo-deb installed)
#   dist/mechvibes-<version>-x86_64.AppImage
# Each is checked after it is built.
#
#   --skip-build  reuse an existing binary instead of building one.
#   --no-deb      skip the .deb, for a machine without cargo-deb.
#
# The AppDir deliberately mirrors the .deb's filesystem layout - usr/bin,
# usr/share/mechvibes/soundpacks - so that one binary serves both packages
# and src/state/paths.rs needs only a single extra branch rather than a
# separate AppImage code path. Two placements are NOT free choices:
#
#   usr/lib/mechvibes/assets   dioxus-asset-resolver's Linux branch looks
#                                 for <exe>/../../lib/<dir>/assets and takes
#                                 the first entry containing an assets/ dir.
#                                 Put the fonts anywhere else and every
#                                 asset!() font silently 404s at runtime.
#
#   AppDir root .desktop + icon   appimagetool requires both at the top level;
#                                 the copies under usr/share are what a
#                                 desktop-integration helper installs later.

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib/common.sh"

SKIP_BUILD=0
BUILD_DEB=1
VERSION=""
for arg in "$@"; do
  case "$arg" in
    --skip-build) SKIP_BUILD=1 ;;
    --no-deb) BUILD_DEB=0 ;;
    -*) echo "unknown option: $arg" >&2; exit 2 ;;
    *) VERSION="$arg" ;;
  esac
done
if [ -z "$VERSION" ]; then
  echo "usage: build-linux-appimage.sh <version> [--skip-build] [--no-deb]" >&2
  exit 2
fi

APP_NAME="mechvibes"
ARCH="x86_64"
BINARY="target/release/${APP_NAME}"
APPDIR="build/AppDir"
OUTPUT="dist/${APP_NAME}-${VERSION}-${ARCH}.AppImage"

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
  stale=$({ find src assets patches packaging Cargo.toml Cargo.lock build.rs -type f -newer "$BINARY" 2>/dev/null || true; } | head -5)
  if [ -n "$stale" ]; then
    echo "::error::$BINARY is older than these files - rebuild it, or run without --skip-build:"
    echo "$stale"
    exit 1
  fi
fi

# --- .deb -------------------------------------------------------------------
# Built from the same binary as the AppImage (--no-build: cargo-deb's own build
# would not reuse it, and the file list lives in [package.metadata.deb] in
# Cargo.toml). The package deliberately has no maintainer scripts, so it does
# NOT add the user to the `input` group - the release notes say so. cargo-deb
# leaves the binary unstripped and byte-identical, so the same BuildID ends up
# in the AppImage below.
if [ "$BUILD_DEB" -eq 1 ]; then
  if ! command -v cargo-deb >/dev/null 2>&1; then
    echo "::error::cargo-deb not found - install it ('cargo install cargo-deb') or pass --no-deb"
    exit 1
  fi
  echo "=== Building .deb ==="
  cargo deb --no-build
  DEB_SOURCE=$(ls target/debian/"${APP_NAME}"_"${VERSION}"*.deb 2>/dev/null | head -1 || true)
  if [ -z "$DEB_SOURCE" ] || [ ! -f "$DEB_SOURCE" ]; then
    echo "::error::No .deb produced for version ${VERSION} in target/debian/"
    exit 1
  fi
  mkdir -p dist
  cp "$DEB_SOURCE" "dist/${APP_NAME}_${VERSION}_amd64.deb"
  echo "=== Built dist/${APP_NAME}_${VERSION}_amd64.deb ==="
fi

echo "=== Assembling AppDir for ${APP_NAME} ${VERSION} ==="

rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin"
mkdir -p "$APPDIR/usr/lib/${APP_NAME}"
mkdir -p "$APPDIR/usr/share/${APP_NAME}"
mkdir -p "$APPDIR/usr/share/applications"
mkdir -p "$APPDIR/usr/share/icons/hicolor/512x512/apps"

# --- binary -----------------------------------------------------------------
install -m 755 "$BINARY" "$APPDIR/usr/bin/${APP_NAME}"

# --- read-only resources ----------------------------------------------------
cp -r soundpacks "$APPDIR/usr/share/${APP_NAME}/soundpacks"
cp -r assets "$APPDIR/usr/lib/${APP_NAME}/assets"

# --- desktop entry ----------------------------------------------------------
# Reuses the same file the .deb ships, so the two packages cannot drift apart.
# It is already appimagetool-safe: `Categories=AudioVideo;` is a single main
# category, which is what a prior attempt (commit 5222274) had to fix. Verified
# rather than assumed - the assertion below fails the build if that regresses.
if ! grep -q '^Categories=AudioVideo;$' mechvibes.desktop; then
  echo "::error::mechvibes.desktop must keep a single main category (Categories=AudioVideo;)"
  echo "  found: $(grep '^Categories=' mechvibes.desktop || echo '<none>')"
  exit 1
fi
# The entry opens .mvtheme theme files as well as mechvibes:// links.
if ! grep -q '^MimeType=x-scheme-handler/mechvibes;application/x-mechvibes-theme;$' mechvibes.desktop; then
  echo "::error::mechvibes.desktop must declare mechvibes:// links and theme files in MimeType="
  echo "  found: $(grep '^MimeType=' mechvibes.desktop || echo '<none>')"
  exit 1
fi
cp mechvibes.desktop "$APPDIR/usr/share/applications/${APP_NAME}.desktop"
cp mechvibes.desktop "$APPDIR/${APP_NAME}.desktop"

# The .mvtheme file type, for desktop-integration helpers. The app registers
# it for the user itself when run as an AppImage (src/libs/protocol.rs).
mkdir -p "$APPDIR/usr/share/mime/packages"
cp packaging/linux/mechvibes.xml "$APPDIR/usr/share/mime/packages/${APP_NAME}.xml"

cp assets/icon.png "$APPDIR/usr/share/icons/hicolor/512x512/apps/${APP_NAME}.png"
cp assets/icon.png "$APPDIR/${APP_NAME}.png"
# .DirIcon is what file managers read for the image's own thumbnail.
cp assets/icon.png "$APPDIR/.DirIcon"

# --- AppRun -----------------------------------------------------------------
# Kept minimal on purpose. It does NOT bundle or preload system libraries:
# webkit2gtk, GTK and libasound come from the host, which is the standard
# trade-off for a webview app (a bundled webkit conflicts with the host's
# GStreamer and portal stack more often than it helps).
#
# APPDIR is exported because the AppImage runtime does so and tooling expects
# it, but the app itself does not read it - paths.rs derives the AppDir from
# the executable's own location, which an inherited stale env var cannot spoof.
cat > "$APPDIR/AppRun" <<'APPRUN'
#!/usr/bin/env bash
set -eu
HERE="$(dirname "$(readlink -f "${0}")")"
export APPDIR="${HERE}"
export PATH="${HERE}/usr/bin:${PATH}"
export XDG_DATA_DIRS="${HERE}/usr/share:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"
exec "${HERE}/usr/bin/mechvibes" "$@"
APPRUN
chmod 755 "$APPDIR/AppRun"

# --- guard: resources must match the source tree ----------------------------
# A non-zero check is not enough. The four mouse packs are .mp3 while the
# keyboard packs are .ogg, so an .ogg-only count would pass while silently
# dropping every mouse pack - this exact hole was found during the macOS round.
count_audio() {
  find "$1" -type f \( -name '*.ogg' -o -name '*.mp3' -o -name '*.wav' \) | wc -l
}
count_configs() {
  find "$1" -type f -name 'config.json' | wc -l
}

SRC_AUDIO=$(count_audio soundpacks)
APP_AUDIO=$(count_audio "$APPDIR/usr/share/${APP_NAME}/soundpacks")
SRC_CONFIGS=$(count_configs soundpacks)
APP_CONFIGS=$(count_configs "$APPDIR/usr/share/${APP_NAME}/soundpacks")
SRC_FONTS=$(find assets/fonts -type f -name '*.ttf' | wc -l)
APP_FONTS=$(find "$APPDIR/usr/lib/${APP_NAME}/assets/fonts" -type f -name '*.ttf' | wc -l)

if [ "$SRC_AUDIO" -ne "$APP_AUDIO" ] || [ "$SRC_AUDIO" -eq 0 ]; then
  echo "::error::soundpack audio mismatch: AppDir has $APP_AUDIO, source tree has $SRC_AUDIO"
  exit 1
fi
if [ "$SRC_CONFIGS" -ne "$APP_CONFIGS" ] || [ "$SRC_CONFIGS" -eq 0 ]; then
  echo "::error::soundpack config.json mismatch: AppDir has $APP_CONFIGS, source tree has $SRC_CONFIGS"
  exit 1
fi
if [ "$SRC_FONTS" -ne "$APP_FONTS" ] || [ "$SRC_FONTS" -eq 0 ]; then
  echo "::error::font mismatch: AppDir has $APP_FONTS, source tree has $SRC_FONTS"
  exit 1
fi

echo "Bundled $APP_AUDIO soundpack audio files (matches source tree)"
echo "Bundled $APP_CONFIGS soundpack config.json files (matches source tree)"
echo "Bundled $APP_FONTS fonts (matches source tree)"

# --- appimagetool -----------------------------------------------------------
# GitHub runners have no FUSE, so appimagetool cannot mount *itself* to run.
# APPIMAGE_EXTRACT_AND_RUN=1 makes it unpack to a temp dir and exec from there
# instead, which is the supported workaround and needs no privileges.
TOOL="build/appimagetool-${ARCH}.AppImage"
if [ ! -f "$TOOL" ]; then
  echo "=== Downloading appimagetool ==="
  mkdir -p build
  curl -fsSL -o "$TOOL" \
    "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-${ARCH}.AppImage"
  chmod +x "$TOOL"
fi

mkdir -p dist
echo "=== Packaging ${OUTPUT} ==="
APPIMAGE_EXTRACT_AND_RUN=1 ARCH="$ARCH" "./$TOOL" --no-appstream "$APPDIR" "$OUTPUT"

chmod +x "$OUTPUT"
echo "=== Built $OUTPUT ($(du -h "$OUTPUT" | cut -f1)) ==="

# --- verify the finished image ----------------------------------------------
# Unpack it and walk it, so the build log is itself the evidence that the
# shipped artifact has the right shape. A green exit from the steps above only
# proves the commands ran.
echo "=== Verifying $OUTPUT ==="
VERIFY_DIR="build/verify"
rm -rf "$VERIFY_DIR"
mkdir -p "$VERIFY_DIR"
OUTPUT_ABS="$(cd "$(dirname "$OUTPUT")" && pwd)/$(basename "$OUTPUT")"
(cd "$VERIFY_DIR" && "$OUTPUT_ABS" --appimage-extract >/dev/null)

IMG="$VERIFY_DIR/squashfs-root"
echo "--- AppDir tree (depth 4) ---"
find "$IMG" -maxdepth 4 | sort

IMG_AUDIO=$(count_audio "$IMG/usr/share/${APP_NAME}/soundpacks")
IMG_CFG=$(count_configs "$IMG/usr/share/${APP_NAME}/soundpacks")
IMG_FONTS=$(find "$IMG/usr/lib/${APP_NAME}/assets/fonts" -type f -name '*.ttf' | wc -l)
echo "soundpack audio: image=$IMG_AUDIO source=$SRC_AUDIO"
echo "soundpack config.json: image=$IMG_CFG source=$SRC_CONFIGS"
echo "fonts: image=$IMG_FONTS source=$SRC_FONTS"
[ "$IMG_AUDIO" -eq "$SRC_AUDIO" ] && [ "$IMG_AUDIO" -gt 0 ] \
  || { echo "::error::AppImage soundpack audio does not match the source tree"; exit 1; }
[ "$IMG_CFG" -eq "$SRC_CONFIGS" ] && [ "$IMG_CFG" -gt 0 ] \
  || { echo "::error::AppImage soundpack config.json does not match the source tree"; exit 1; }
[ "$IMG_FONTS" -eq "$SRC_FONTS" ] && [ "$IMG_FONTS" -gt 0 ] \
  || { echo "::error::AppImage fonts do not match the source tree"; exit 1; }

# ELF magic checked byte-wise rather than trusting `file`'s wording.
[ "$(head -c 4 "$IMG/usr/bin/${APP_NAME}" | od -An -tx1 | tr -d ' \n')" = "7f454c46" ] \
  || { echo "::error::bundled executable is not an ELF binary"; exit 1; }
echo "ELF magic OK"
rm -rf "$VERIFY_DIR"

assert_no_updater_collision dist
