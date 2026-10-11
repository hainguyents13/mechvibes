![image](https://github.com/user-attachments/assets/5aa36739-76c8-4a34-9a9b-7e9272927f22)

# MechvibesDX

Play rich keyboard and mouse sounds with every keystroke and click. A polyphonic, low-latency mechanical keyboard sound simulator for Windows, macOS, and Linux. Successor of Mechvibes, now with resample-on-load audio, dedicated audio engine thread, and one-click verified updates.

## Download

| Platform | Installer | Notes |
|----------|-----------|-------|
| **Windows** | [`MechvibesDX-*-Setup-x64.exe`](https://github.com/hainguyents13/mechvibes-dx/releases/latest) | One-click installer with automatic in-app updates (SHA-256 verified). |
| **Linux (Debian/Ubuntu)** | [`mechvibes-dx_*_amd64.deb`](https://github.com/hainguyents13/mechvibes-dx/releases/latest) | `sudo dpkg -i`. Requires: `sudo usermod -a -G input $USER` + re-log. |
| **Linux (any distro)** | [`mechvibes-dx-*-x86_64.AppImage`](https://github.com/hainguyents13/mechvibes-dx/releases/latest) | Portable (no install). Requires: `chmod +x` + same input group setup. |
| **macOS** | [`mechvibes-dx-*-macos-arm64.dmg`](https://github.com/hainguyents13/mechvibes-dx/releases/latest) | Experimental, unsigned. Right-click the app, then Open to bypass Gatekeeper. |

All links point to the [latest release](https://github.com/hainguyents13/mechvibes-dx/releases/latest); every release also ships a `SHA256SUMS.txt` covering all assets.

## Features

- **Polyphonic low-latency audio**: Sounds play within ~15ms of keystroke. Resampled at load time (not realtime) for high quality. 32-voice pool with soft eviction prevents clicks and clipping on rapid keystrokes.
- **Works while app is focused**: Dedicated Raw Input worker process on Windows; in-process listeners on Linux/macOS. Keyboard and mouse capture work the same whether MechvibesDX window is active or not.
- **Per-device input filtering** (Windows): Disable specific keyboards or mice in Settings, effective immediately.
- **Runtime output device switching**: Change audio output (headphones, speakers, virtual devices) in Settings without restarting.
- **Soundpack support**: Import or create custom soundpacks. Drag and drop folders into the app. Supports OGG, WAV, MP3, FLAC. Auto-converts classic Mechvibes packs (V1) to V2 format.
- **Ambiance sounds**: Long-playing background audio (rain, coffee shop) that moves with your selected output device.
- **Themes and customization**: Light/dark themes, custom logo, background images.
- **Tray icon and global hotkey**: `Ctrl+Alt+M` to mute/unmute. Tray icon shows mute state at a glance.
- **One-click verified updates** (Windows): When new versions are available, download and install with SHA-256 verification. Choose "Later" to defer, but the verified file stays ready.
- **Settings save correctly**: Writable state lives in `%APPDATA%\Mechvibes` (Windows), `~/.local/share/mechvibes` (Linux), or `~/Library/Application Support/Mechvibes` (macOS). Settings persist across restarts, reinstalls, and updates.
- **Debug section**: Live log viewer in Settings with export button for bug reports. Optional verbose mode adds per-keystroke timing for diagnosing latency.

## Soundpacks

Soundpacks define the sounds that play for each key and mouse button. The app ships with a curated collection (Classic Mechvibes, Cherry MX Black, etc.). You can add your own.

### Import a soundpack

1. Download or create a soundpack folder.
2. In the app, Settings > Soundpacks > "Import Soundpack" (or drag the folder in).
3. Choose which soundpack to load for keyboard and mouse sounds.

### Create a soundpack

A soundpack is a folder containing:
- **config.json** (required): metadata and key mappings
- **Audio files** (OGG, WAV, MP3, FLAC): the sounds to play
- **icon.png** (optional): a display icon

**Minimal example:**
```
my-keyboard-pack/
├── config.json
└── typewriter-click.ogg
```

**config.json (V2 format):**
```json
{
  "id": "typewriter-minimal",
  "name": "Typewriter",
  "author": "You",
  "config_version": "2",
  "definition_method": "single",
  "audio_file": "typewriter-click.ogg",
  "definitions": {
    "KeyA": { "timing": [[0, 50]] },
    "KeyB": { "timing": [[50, 100]] }
  },
  "options": { "recommended_volume": 0.8, "random_pitch": false },
  "soundpack_type": "Keyboard"
}
```

Timing values are `[start_ms, end_ms]` within the audio file. For "single" method, all keys reference the same audio file with different timing windows. For "multi" method, each key can use a different file.

For complete documentation on config.json fields, key naming, templates, and troubleshooting, see [Soundpack Config.json V2 Format](docs/soundpack-config-v2.md).

### Classic Mechvibes packs

Old V1 soundpacks (e.g., from the original Mechvibes) are auto-detected on import. The app automatically converts them to V2 format with no loss of sound quality. Your original files stay intact; a backup is kept if re-imported.

## Platform Notes

### Windows
- SmartScreen may warn on first run (unsigned installer). Click "More info" then "Run anyway" to proceed.
- "Start at login" in Settings uses Task Scheduler (admin install required for per-user startup).
- Single-instance mutex prevents multiple app windows running at once.

### Linux
- **Input group requirement**: The app reads raw keyboard/mouse events from `/dev/input/event*`, which requires membership in the `input` group:
  ```bash
  sudo usermod -a -G input $USER
  # Log out and log back in for the group change to take effect
  ```
- **Wayland**: rdev (the input listener) supports Wayland as of recent versions; X11 is also fully supported.
- **AppImage notes**: The `.AppImage` mounts read-only; writable state goes to `~/.local/share/mechvibes`. No FUSE required to run.

### macOS
- **Experimental**: Not tested on real hardware. Ad-hoc signed, not notarized.
- **First launch**: Right-click the app, choose "Open" to bypass Gatekeeper's "unidentified developer" block.
- **Permissions**: global key capture needs **both** of these in System Settings > Privacy & Security, enabled for `MechvibesDX` (or for your terminal, if you launch the binary from there):
  - **Accessibility**: used while the MechvibesDX window is focused.
  - **Input Monitoring**: used while the window is minimized or another app is focused. Without it the app runs normally but is silent unless its window has focus.
- **After installing a new build**: the app is ad-hoc signed, so macOS ties its permissions to that exact binary. A rebuilt or reinstalled app keeps its toggles on but they stop working. Remove `MechvibesDX` from both lists with the `-` button, add `/Applications/MechvibesDX.app` back, and relaunch. Alternatively run `tccutil reset ListenEvent com.hainguyents13.mechvibesdx` and `tccutil reset Accessibility com.hainguyents13.mechvibesdx`, and macOS prompts again on next launch.
- **Start at login**: registers MechvibesDX as a login item (System Settings > General > Login Items), which needs macOS 13 or later and the app installed in `/Applications`. If macOS asks, allow it there. "Start minimized" is read from your saved settings, so it applies whenever the app launches.
- Arm64 (Apple Silicon) only in current builds. Intel builds available on request.

## Privacy and Telemetry

MechvibesDX sends one anonymous ping per launch to track rough usage. Nothing else leaves your device.

**What is sent (once per launch):**
- OS name and version
- App version
- Your system language (locale)
- A random session ID (generated fresh each launch, never stored)

**What is never sent:**
- Keystrokes, mouse clicks, or any input data
- Soundpack names or usage
- File paths or personal data
- IP-derived identity or persistent identifiers

Telemetry is handled by [Aptabase](https://aptabase.com), an open-source privacy-first service. **Opt out** anytime in Settings > Privacy > "Share anonymous usage stats".

Debug logs stay local until you export them via the Debug button. Key names in logs are always masked (e.g., "KEY_A" instead of actual keystroke data).

## Building from Source

### Prerequisites

**All platforms:**
- [Rust](https://rustup.rs/) 1.88.0 or later, declared as `rust-version` in `Cargo.toml`. The latest stable works.
- [Dioxus CLI](https://dioxuslabs.com/learn/0.7/getting_started) 0.7.10: `cargo install dioxus-cli --version 0.7.10`. If you use [mise](https://mise.jdx.dev/), `mise install` installs the pinned `dioxus-cli` from `.mise.toml`.

**Windows:**
- Visual Studio Build Tools (C++ support)
- [Inno Setup 6](https://jrsoftware.org/isinfo.php) (for building installers)

**Linux (Ubuntu/Debian):**
```bash
sudo apt-get update
sudo apt-get install -y \
    libasound2-dev \
    pkg-config \
    libwebkit2gtk-4.1-dev \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    libevdev-dev \
    libxdo-dev \
    libxi-dev \
    libxtst-dev \
    libssl-dev
```

**Linux (Fedora/RHEL):**
```bash
sudo dnf install -y \
    alsa-lib-devel \
    pkg-config \
    webkit2gtk4.1-devel \
    gtk3-devel \
    libappindicator-gtk3-devel \
    librsvg2-devel \
    libevdev-devel \
    xdotool-devel \
    libXi-devel \
    libXtst-devel \
    openssl-devel
```

**macOS:**
```bash
xcode-select --install
```

### Build

**Development (all platforms):**
```bash
git clone https://github.com/hainguyents13/mechvibes-dx.git
cd mechvibes-dx
dx serve
```

**Run the release binary directly (macOS/Linux):**

The app looks for `soundpacks/` next to the executable. `cargo build` copies the bundled packs to `target/release/soundpacks/` for you (see `build.rs`), so the binary runs as is:
```bash
cargo build --release
./target/release/mechvibes-dx
```
To capture a log for a bug report, run it as `RUST_BACKTRACE=1 MECHVIBES_TRACE=1 ./target/release/mechvibes-dx 2>&1 | tee /tmp/mv.log`.

On macOS the permissions apply to the app that launched it. When run from a terminal, grant that terminal app **Accessibility** and **Input Monitoring** in System Settings > Privacy & Security, then restart the terminal.

**Release (Windows):**
```bash
.\scripts\build-windows-installer.ps1   # builds first; add -SkipBuild to reuse target\release
# Output: dist/MechvibesDX-0.8.1-Setup-x64.exe
```

**Release (Linux, AppImage and DEB):**
```bash
cargo install cargo-deb                   # once; or pass --no-deb to skip the .deb
./scripts/build-linux-appimage.sh 0.8.1   # builds first; add --skip-build to reuse target/release
chmod +x dist/mechvibes-dx-0.8.1-x86_64.AppImage
sudo dpkg -i dist/mechvibes-dx_0.8.1_amd64.deb
# Remember: run 'sudo usermod -a -G input $USER' and re-log before first use
```

**Release (macOS):**
```bash
./scripts/build-macos-app.sh 0.8.1   # builds first; add --skip-build to reuse target/release
# Output: dist/mechvibes-dx-0.8.1-macos-arm64.dmg
```

To run the same compile-and-test gate that CI runs (it also checks that Rust is at least the `rust-version` in `Cargo.toml`): `./scripts/ci-check.sh --release`. CI builds and packages every pull request and every push to `main` this way; when a `v*` tag points at a commit it built, the same run drafts the release from those packages.

For architecture details, see [docs/system-architecture.md](docs/system-architecture.md). For release procedures and deployment, see [docs/deployment-guide.md](docs/deployment-guide.md).

## Troubleshooting

**No sound playing?**
- Check if muted (tray icon or `Ctrl+Alt+M`) and that **Allow all sounds** is on in Settings. The setting is saved, so it stays off across restarts.
- Verify a soundpack is selected in Settings > Soundpacks.
- Check system volume.

**macOS: sounds only play while the app window is focused?**
- Input Monitoring is missing or stale. See the macOS permissions notes above: remove and re-add `MechvibesDX` under both Accessibility and Input Monitoring, then relaunch.

**Hotkey (`Ctrl+Alt+M`) not working?**
- Windows: Run as administrator (required for global hotkey).
- Verify no other app has claimed the same hotkey.

**Linux: No keyboard input detected?**
- Verify user is in `input` group: `groups $USER` should list `input`.
- Re-log in for group changes to take effect.
- Check device permissions: `ls -la /dev/input/event*` (should show `crw-rw---- root input`).

**Soundpack won't import?**
- Ensure `config.json` is valid JSON (use a JSON validator).
- Check that `definitions` (or `defs`) contains valid key mappings.
- Verify audio files (OGG, WAV, MP3, FLAC) exist and are readable.

**Settings reset after restart?**
- Settings are stored at: `%APPDATA%\Mechvibes` (Windows), `~/.local/share/mechvibes` (Linux), or `~/Library/Application Support/Mechvibes` (macOS). Verify these folders exist and are writable.

**macOS: Right-click Open not working?**
- Try `codesign --deep -s - /Applications/MechvibesDX.app` if you trust the build. Otherwise, await an official notarized release.

## Credits

MechvibesDX is built in Rust using [Dioxus](https://dioxuslabs.com/) (0.7), [rodio](https://github.com/RustAudio/rodio) (audio), and [rdev](https://github.com/enigo-rs/rdev) (input capture).

Based on the original [Mechvibes](https://github.com/hainguyents13/mechvibes) by hainguyents13. Distributed under the MIT License.

## License

MIT License. See [LICENSE](LICENSE) for details.
