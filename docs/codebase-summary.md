# Codebase Summary — Mechvibes

**Last Updated:** 2026-08-03 · **Language:** Rust (1.88+) + Dioxus 0.7.10  
**Build:** `cargo build --release` (Windows); CI via GitHub Actions  
**Platform targets:** Windows (primary), Linux (X11/Wayland), macOS  
**Current version:** v0.7.0 (device watchdog removed, input injection filter added, trace facility enabled)

## Directory Tree & Module Organization

```
mechvibes/
├── src/
│   ├── main.rs                 # App entry, engine spawn, worker mode check
│   ├── libs/                   # Platform & subsystem libraries
│   │   ├── mod.rs              # Module exports
│   │   ├── audio/              # Audio engine & soundpack loading
│   │   │   ├── mod.rs
│   │   │   ├── engine.rs       # Audio engine thread (Phase 3)
│   │   │   ├── audio_context.rs # Facade over engine
│   │   │   ├── soundpack_loader.rs # Decode & resample audio
│   │   │   └── resampler.rs    # Sinc resampling (rubato)
│   │   ├── input_listener.rs   # rdev + device_query capture
│   │   ├── evdev_input_listener.rs # Linux evdev direct access
│   │   ├── focused_input_listener.rs # device_query polling on focus
│   │   ├── input_manager.rs    # Channel init, focus state
│   │   ├── device_manager.rs   # Output device tracking & enumeration
│   │   ├── input_device_manager.rs # Raw Input device ID lookups
│   │   ├── input_worker.rs     # Worker process entry (Windows)
│   │   ├── input_worker_host.rs # Worker supervisor & filtering (Windows)
│   │   ├── rawinput_listener.rs # Raw Input loop (Windows worker only)
│   │   ├── single_instance.rs  # Single-instance guard (Windows)
│   │   ├── ui.rs               # Dioxus render loop (minimal now)
│   │   ├── tray.rs             # System tray integration
│   │   ├── tray_service.rs     # Tray icon management
│   │   ├── window_manager.rs   # Window control (minimize, restore)
│   │   ├── theme.rs            # Theme definitions
│   │   ├── routes.rs           # Dioxus routing
│   │   ├── protocol.rs         # IPC protocol (if any)
│   │   ├── file_server.rs      # Asset serving
│   │   └── (platform-specific)
│   │       └── [target_os = "linux"] evdev_input_listener.rs
│   │       └── [target_os = "windows"] {input_worker, input_worker_host, etc.}
│   ├── state/                  # Persistent & runtime state
│   │   ├── mod.rs
│   │   ├── config.rs           # AppConfig (JSON schema)
│   │   ├── soundpack.rs        # SoundPack & SoundpackMetadata
│   │   ├── app.rs              # App-wide state (singleton)
│   │   ├── keyboard.rs         # Keyboard state tracking
│   │   ├── ambiance.rs         # Ambiance player state
│   │   ├── paths.rs            # Config/data directory paths
│   │   ├── manifest.rs         # Soundpack manifest
│   │   ├── themes.rs           # Theme state
│   │   └── rodio_music.rs      # [DEAD CODE] Music player (removed)
│   ├── components/             # Dioxus UI components
│   │   ├── mod.rs
│   │   ├── app_info.rs         # App info modal
│   │   ├── header.rs           # Top bar
│   │   ├── dock.rs             # Bottom controls
│   │   ├── logo.rs             # Animated logo
│   │   ├── soundpack_selector.rs # Pack selection dropdown
│   │   ├── device_selector.rs  # Audio device selector + filtering UI
│   │   ├── window_controller.rs # Min/max/close buttons
│   │   ├── theme_toggler.rs    # Dark/light theme switch
│   │   ├── volume_slider.rs    # Volume control
│   │   ├── ui/                 # Reusable UI components
│   │   │   ├── mod.rs
│   │   │   ├── modal.rs        # Base modal
│   │   │   ├── color_picker.rs # Color selector
│   │   │   ├── soundpack_table.rs # Soundpack list table
│   │   │   ├── soundpack_import_modal.rs # Drag-drop importer
│   │   │   ├── confirm_delete_modal.rs # Delete confirmation
│   │   │   ├── popover_dropdown.rs # Generic dropdown
│   │   │   ├── toggler.rs      # Toggle switch
│   │   │   ├── collapse.rs     # Collapsible section
│   │   │   └── progress_step.rs # Onboarding step indicator
│   │   └── pages/              # Page-level components (routing)
│   │       ├── mod.rs
│   │       ├── home.rs         # Main page (logo + settings)
│   │       ├── soundpacks.rs   # Soundpack management
│   │       ├── customize.rs    # UI customization (logo, background)
│   │       ├── settings.rs     # Audio, device, I/O settings
│   │       └── mood.rs         # [DEPRECATED] Music/ambiance player UI
│   └── utils/                  # Utilities & helpers
│       ├── mod.rs
│       ├── constants.rs        # App name, version, resource IDs
│       ├── config.rs           # Config I/O (JSON, registry on Windows)
│       ├── path.rs             # Path utilities
│       ├── soundpack.rs        # Soundpack file operations
│       ├── soundpack_validator.rs # Pack format validation
│       ├── soundpack_installer.rs # Pack drag-drop install
│       ├── platform.rs         # Platform detection
│       ├── theme.rs            # Theme color helpers
│       ├── time.rs             # Time formatting
│       ├── delay.rs            # Delay/sleep utilities
│       ├── spacing.rs          # UI spacing constants
│       ├── logger.rs           # Logging setup
│       ├── auto_startup.rs     # Windows auto-start registry
│       ├── auto_updater.rs     # Auto-update checker
│       ├── config_converter.rs # Config format migrations
│       └── data.rs             # Data serialization helpers
├── Cargo.toml              # Dependencies, version (0.6.0)
├── Cargo.lock              # Locked dependency versions
├── build.rs                # Pre-build script (Windows icon setup)
├── .github/
│   ├── actions/linux-system-deps/ # Shared Linux apt dependency list (used by ci.yml)
│   └── workflows/
│       └── ci.yml              # Build + test + package matrix (Windows, Linux, macOS); drafts a release if a tag points at the built commit
├── scripts/
│   ├── bump-version.ps1        # Increment version (PowerShell)
│   ├── extract-changelog.ps1   # Extract CHANGELOG.md section (PowerShell)
│   ├── build-windows-installer.ps1 # Inno Setup builder (Windows)
│   ├── build-macos-app.sh      # .app + DMG assembler (macOS)
│   ├── build-linux-appimage.sh # .deb + AppImage assembler (Linux)
│   ├── ci-check.sh             # toolchain check + cargo check + cargo test (CI and local)
│   └── lib/common.sh           # shared bash helpers (toolchain check, asset-name check)
├── installer/
│   ├── windows/
│   │   └── mechvibes-setup.iss # Inno Setup config
│   └── README.md                # Installer docs
├── assets/
│   ├── icon.ico             # Windows taskbar icon
│   └── icon.png             # Linux/macOS app icon
├── soundpacks/              # Bundled default soundpacks
│   └── (pack directories)
├── docs/
│   ├── system-architecture.md # [NEW] Architecture overview
│   ├── codebase-summary.md    # [NEW] This file
│   ├── code-standards.md      # [TBD] Code patterns & guidelines
│   ├── project-overview-pdr.md # [TBD] PDR & project vision
│   └── 260731-input-audio-refactor-plan.md # Detailed plan log
├── plans/
│   └── 260731-input-audio-refactor/
│       ├── plan.md          # Master plan with validation log
│       └── phase-*.md       # Per-phase implementation details
├── CHANGELOG.md             # Release notes
├── README.md                # User-facing guide
└── .gitignore

```

## Core Entry Points & Flow

### Startup (`src/main.rs`)

1. **Early exit for worker mode (Windows):** Check `std::env::args_os()` for `--input-worker`.
   - If present: `libs::input_worker::run()` and exit (never returns).

2. **Single-instance guard (Windows):** Acquire mutex keyed by exe path hash.
   - If already held: print error and exit.

3. **Initialize trace facility (Phase 7):** `libs::trace::init()` (enabled by `MECHVIBES_TRACE=1`).
   - Zero-overhead when disabled; writer thread spawned when enabled.

4. **Initialize logging:** `utils::logger::setup()`.

5. **Spawn input listeners:** Platform-specific.
   - Windows: `input_worker_host.rs` (spawns worker + reads stdout).
   - Linux: `evdev_input_listener.rs` on X11 and Wayland whenever `/dev/input` is readable, with hot-plug; otherwise `input_listener.rs` (rdev) + `focused_input_listener.rs` (device_query) on X11, or rdev for the mouse only on Wayland.
   - macOS: `input_listener.rs` (rdev).

6. **Spawn audio engine:** `libs::audio::engine::spawn_engine()` before Dioxus.
   - Initializes `OutputStream` on main thread (cpal opens device).
   - Engine thread starts, enters `select!` loop (purely event-driven, no polling).

7. **Ambiance player init:** Separate `OutputStream`, separate thread.

8. **Launch Dioxus:** `dioxus::launch(root)` hands control to webview event loop.

### UI Render Loop (`src/libs/ui.rs`)

**Post-Phase 3 simplification:** No longer polls input events. Engine handles that.

**Now does:**
1. Single `use_future` that polls engine's `UiEvent` receiver (non-blocking).
2. Updates `KeyboardState` for visual feedback.
3. Listens for device-switch results and pack-load results.
4. Sends UI commands (`AudioCommand`) to engine.

**Does NOT do:**
- Poll input channels (engine owns that).
- Manage audio context directly (facade only).

### Soundpack Loading Flow

1. **User action:** Select soundpack in UI or drag-drop folder.

2. **Validation:** `utils::soundpack_validator::validate_soundpack()` checks:
   - `config.json` exists and is valid JSON.
   - Required audio files exist.
   - `definitions` has at least one entry.

3. **Load command:** UI sends `AudioCommand::LoadKeyboardPack { soundpack_id, update_cache_on_error }`.

4. **Engine receives:** Calls `soundpack_loader::load_keyboard_soundpack()`.

5. **Decode:** `symphonia` opens audio file (supports WAV, OGG, MP3, AAC, ISOMP4).
   - Output: `Vec<f32>` (interleaved samples if stereo).

6. **Resample (if needed):** `resampler.rs` uses `rubato` sinc interpolation.
   - Input sample rate ≠ device sample rate → resample.
   - Otherwise → use decoded as-is (but cloned; could optimize).

7. **Store:** Samples wrapped in `Arc<Vec<f32>>`, stored in `EngineState`.

8. **Report:** Engine sends `UiEvent::PackLoaded { is_keyboard: true, result: Ok(name) }` or `Err(error)`.

### Device Switching Flow

1. **User selects device** in "Settings" → `device_selector.rs`.

2. **Write config:** `AppConfig::save()` writes selected device name to disk/registry.

3. **Send command:** `AudioCommand::SwitchDevice(Some(device_name))`.

4. **Engine receives:**
   - Calls `DeviceManager::open_stream(device_name)`.
   - If success: closes old `OutputStream`, opens new one.
   - If failure: keeps old stream, returns `Err(...)`.
   - Emits `UiEvent::DeviceSwitched(result)`.

**Watchdog (removed Phase 7):**
- The 1-second timer that polled device presence is gone.
- On device unplug: app goes silent, config retains selection, user manually reselects.
- No automatic fallback at runtime (startup fallback to default still applies if saved device unavailable at launch).

## Key Data Structures

### `AppConfig` (Persistent, `src/state/config.rs`)

```rust
pub struct AppConfig {
    // Audio
    pub keyboard_soundpack: String,
    pub mouse_soundpack: String,
    pub volume: f32,              // 0–100
    pub mouse_volume: f32,
    pub enable_sound: bool,
    pub enable_keyboard_sound: bool,
    pub enable_mouse_sound: bool,
    pub selected_audio_device: Option<String>, // Device name or None
    
    // Input filtering (Windows)
    pub enabled_keyboards: Vec<String>,  // Device IDs
    pub enabled_mice: Vec<String>,       // Device IDs
    
    // UI customization
    pub theme: Theme,
    pub logo_customization: LogoCustomization,
    pub background_customization: BackgroundCustomization,
    
    // Ambiance
    pub ambiance_active_sounds: HashMap<String, f32>, // sound_id → volume
    pub ambiance_global_volume: f32,
    pub ambiance_is_muted: bool,
    
    // System
    pub auto_start: bool,
    pub start_minimized: bool,
    pub landscape_mode: bool,
    pub auto_update: AutoUpdateConfig,
}
```

**Storage:**
- Linux/macOS: `~/.local/share/mechvibes/config.json`
- Windows: `HKEY_CURRENT_USER\Software\Mechvibes` (registry) + JSON fallback

### `SoundPack` (Soundpack Definition, `src/state/soundpack.rs`)

```rust
pub struct SoundPack {
    pub id: String,
    pub name: String,
    pub definition_method: String,  // "single" or "multi"
    pub audio_file: Option<String>, // For "single" method
    pub definitions: HashMap<String, KeyDefinition>, // "KeyA" → {timing, audio_file}
    pub options: SoundpackOptions,
    pub soundpack_type: SoundpackType, // Keyboard or Mouse
    // ... plus metadata fields (author, version, license, etc.)
}

pub struct KeyDefinition {
    pub timing: Vec<[f32; 2]>,      // [[start_ms, end_ms], ...]
    pub audio_file: Option<String>, // For "multi" method
}
```

**File format:** Loaded from `config.json` in soundpack directory (validated by `soundpack_validator.rs`).

### `EngineState` (Runtime Audio, `src/libs/audio/engine.rs`)

```rust
pub(super) struct EngineState {
    stream: OutputStream,
    stream_handle: OutputStreamHandle,
    keyboard_sinks: Vec<Sink>,     // Voice pool
    mouse_sinks: Vec<Sink>,
    keyboard_samples: Option<DecodedAudio>,
    mouse_samples: Option<DecodedAudio>,
    device_manager: DeviceManager,
    current_device_id: Option<String>,
    device_rate: Option<u32>,
    // ... plus fields for key/mouse press state, volume, sound enabled flags
}
```

**Invariant:** Only touched by engine thread. No `Arc<Mutex<>>` here. Purely event-driven—no timer or polling.

### `InputChannels` (Engine's Input Sources, `src/libs/input_manager.rs`)

```rust
pub struct InputChannels {
    pub keyboard_rx: Receiver<String>,  // "KeyA", "UP:KeyA"
    pub mouse_rx: Receiver<String>,     // "MouseLeft", "UP:MouseLeft"
    pub hotkey_rx: Receiver<String>,    // Filtered hotkey events
}
```

**Receiver types:** `crossbeam_channel::Receiver` (supports `select!`).

## Common Development Tasks

### Adding a New Soundpack Type / Sound Definition

1. **Edit `src/state/soundpack.rs`:**
   - Extend `SoundpackType` enum if needed.
   - Extend `KeyDefinition` struct if new timing model.

2. **Update `soundpack_validator.rs`:**
   - Add validation rules for new type.

3. **Update `soundpack_loader.rs`:**
   - Add handling in `load_keyboard_soundpack()` or new function.

4. **Update UI (`src/components/ui/soundpack_table.rs`):**
   - Display new type in pack list.

5. **Test:**
   - Create a test pack in the target directory.
   - Drag-drop into UI or select from list.
   - Verify audio plays correctly.

### Adding a New Audio Setting / Configuration Option

1. **Edit `src/state/config.rs`:**
   - Add field to `AppConfig` struct.
   - Implement `Default` if needed.

2. **Update `utils/config.rs`:**
   - Add serialization if not automatic (serde).

3. **Add UI component** (if user-facing):
   - Create component in `src/components/pages/settings.rs` or new file.
   - Wire up `use_context::<AppState>()` and send `AudioCommand` to engine.

4. **Update engine** (if audio-affecting):
   - Add variant to `AudioCommand` enum in `src/libs/audio/engine.rs`.
   - Handle in `run_engine()` match statement.

### Adding Per-Device Input Filtering

1. **Windows only** (implemented):
   - Device ID is hash of device name + properties.
   - Stored in `AppConfig.enabled_keyboards` / `enabled_mice` as strings.
   - `input_worker_host.rs` filters before sending to engine.

2. **Linux/macOS:**
   - Currently not supported (no per-device tracking in rdev).
   - Future: Could use evdev on Linux directly (higher complexity).

### Porting to a New Platform

1. **Check `#[cfg(target_os = "...")]`** in:
   - `src/main.rs` (worker arg check is Windows-only).
   - `src/libs/mod.rs` (conditional module exports).
   - `src/libs/input_listener.rs` (platform-specific).

2. **Implement input capture:**
   - Create `src/libs/{platform}_input_listener.rs`.
   - Must emit same string format: `"KeyA"`, `"UP:KeyA"`, etc.
   - Send to same crossbeam receiver channels.

3. **Handle device-specific UI:**
   - Device filtering only on Windows; others can skip.
   - Check `#[cfg(...)]` in `device_selector.rs`.

4. **Test:**
   - Ensure `cargo build --target ...` compiles.
   - Manual audio playback test (type a key → hear sound).

## Input Filtering & Injection Detection (Phase 7)

**Windows injected-input filter (`src/libs/input_worker.rs:89–119`):**

Keyboard events with NULL source device (`hDevice`) are dropped before transmission to the host. This silences software-injected keys without blocking per-device filtering or hotkey detection.

**Use cases:**
- Vietnamese IME Telex: typing `dd` triggers automatic corrections (backspace + ð character), all injected by `SendInput` with no device handle. Without filtering, each correction plays as machine-gun sounds.
- Same mechanism applies to other IMEs, AutoHotkey scripts, and remote-desktop clients.

**Known trade-off:** Legitimate synthetic clicks that users expect to hear (on-screen keyboards) are also silent. A per-user setting can be added if needed.

**Mouse events:** Unaffected; they pass through regardless of device presence.

## Telemetry (v0.7.1)

`src/utils/telemetry.rs` sends a single anonymous `app_started` event per launch to Aptabase (`POST {region-host}/api/v0/events`, region derived from the app-key prefix). Payload: OS name/version, app version, locale. No keystrokes, no personal data, no persistent identifiers (session id is generated per launch, never stored).

- Gated by `AppConfig::enable_telemetry` (default `true`; toggle in Settings, Privacy section). Off means zero outbound requests.
- Fire-and-forget on a background thread with a short timeout; network errors are swallowed. Never touches the engine loop, input worker, or UI poll loop.
- A placeholder app key makes the module inert, so forks without their own Aptabase app send nothing.
- Config-race warning: the sender must never hold a loaded `AppConfig` across an await and save it back (see Common Pitfalls).

## Dependency Overview

**Key crates (from `Cargo.toml`):**

| Crate | Version | Purpose |
|-------|---------|---------|
| `dioxus` | 0.7.10 | UI framework (webview) |
| `rodio` | 0.20.1 | Audio playback (sink abstraction) |
| `cpal` | 0.15 | Low-level audio I/O (used by rodio) |
| `rubato` | 0.15 | Sinc resampling |
| `symphonia` | 0.5 | Audio decoding (WAV, OGG, MP3, AAC, ISOMP4) |
| `rdev` | 0.5.3 | Global input hooking (macOS/Linux) |
| `device_query` | 4.0.1 | Physical device enumeration |
| `evdev` | 0.13 | Direct event file reading (Linux) |
| `crossbeam-channel` | 0.5 | Multi-producer channels (engine input) |
| `tokio` | 1.0 | Async runtime (some UI tasks) |
| `serde_json` | 1.0 | Config serialization |
| `winapi` | 0.3 | Windows FFI (Raw Input, registry) |
| `tray-icon` | 0.14 | System tray icon |
| `zip` | 2.2.0 | Soundpack unpacking |
| `image` | 0.24 | Icon/image handling |

**Notable absence:** No database; config is JSON/registry only. No ORM. Minimal async (tokio present but mostly unused, inherited from dioxus).

## Testing

**Unit tests location:** Inline in modules, marked with `#[cfg(test)]`.

**Key test modules:**
- `src/libs/audio/resampler.rs::{test_resample_*}` — resampling correctness
- `src/libs/audio/engine.rs::{test_voice_pool, test_the_engine_loop_never_polls_the_audio_device, test_no_device_presence_polling_remains_anywhere_in_the_engine, test_manual_device_switching_is_still_supported}` — engine logic (Phase 7: no polling tests)
- `src/libs/input_worker.rs::{test_injected_keystrokes_are_dropped, test_real_keystrokes_still_pass, test_mouse_events_are_never_dropped}` — injection filter (Phase 7)
- `src/libs/rawinput_listener.rs::{test_*}` — key code mapping (Windows)
- `src/libs/input_worker_host.rs::{test_restart_backoff, test_device_filter}` — worker supervision

**Run tests:**
```bash
cargo test
cargo test --release  # For audio tests (opt=2 in dev profile)
```

**No integration tests:** Manual testing via `dx serve` or release build + real audio device.

## Release & Deployment

**Version management:**
- Single source of truth: `Cargo.toml` version field.
- Bumped via `scripts/bump-version.ps1` (PowerShell, Windows).
- CHANGELOG.md updated manually before tagging.

**Build and release workflow (`.github/workflows/ci.yml`):** one workflow for every build and for releases. Triggers: pull requests into `main` (verification only, nothing uploaded), pushes to `main` and manual dispatch (packages kept as artifacts of the run). A tag push is not a trigger; a tag is noticed by the run that built its commit. So there is one run per commit: no second run for the tag, no duplicate build, and the packages and the draft release live in the same run. Only `main` saves the cargo cache.
1. `build` job (every run): a matrix (Windows, Linux, macOS in parallel); each leg reads the version from `Cargo.toml`, runs `scripts/ci-check.sh --release`, then its packaging script (`build-windows-installer.ps1`, `build-linux-appimage.sh`, `build-macos-app.sh`), which check that Rust is at least `rust-version` from `Cargo.toml`, build with `--locked` and check their own output, and uploads `windows-release`, `linux-release` and `macos-release` (except for pull requests).
2. `tag` job (not on PRs): after the builds, asks the remote (`resolve_release_tag` in `scripts/lib/common.sh`) whether `v<Cargo.toml version>` points at the built commit. No tag means an ordinary build; a `v*` tag that does not match `Cargo.toml` fails.
3. `release` job (only if `tag` found one): extracts the changelog section (`scripts/extract-changelog.ps1`), downloads this run's packages, generates `SHA256SUMS.txt`, refuses a tag that already has a release, and creates one draft GitHub release with every asset. If the tag arrives after the run finished, re-run the `tag` job; the packages are kept with the run.

**Platforms:**
- Windows: Inno Setup EXE installer (interim; waiting for dioxus bundle fix DioxusLabs/dioxus#5723).
- Linux: DEB package (Ubuntu/Debian) + AppImage (any distro), both built in CI from one binary.
- macOS: hand-assembled .app in a DMG (experimental; `dx bundle` unusable, see DioxusLabs/dioxus#5723).

**Current release:** v0.7.0 (Phase 3 + Phase 6 + Phase 7 on main).

## Build & Development

**Prerequisites:**
- Rust 1.88.0 or later (`rust-version` in `Cargo.toml`).
- Dioxus CLI 0.7.10 (`cargo install dioxus-cli --version 0.7.10`).
- Platform-specific:
  - Windows: Visual Studio Build Tools (MSVC).
  - Linux: libasound2-dev, libevdev-dev, pkg-config, etc. (see README.md).
  - macOS: Xcode command line tools.

**Development:**
```bash
dx serve                    # Dev mode with auto-reload
cargo run                   # Direct run (no hot-reload)
cargo build --release      # Optimized build (~5 min)
cargo test                 # Run all tests
cargo clippy               # Lint (baseline ~180 warnings pre-existing)
```

**Windows-specific:**
```powershell
.\scripts\build-windows-installer.ps1  # Build installer
.\scripts\bump-version.ps1 -Version 0.6.1  # Bump version
```

## Common Pitfalls

| Issue | Cause | Solution |
|-------|-------|----------|
| No sound when app focused (Windows) | rdev hook conflicts with webview | Use input worker (Phase 6); built-in now. |
| Click/chop on fast typing | Per-key sink drops replaced old sink | Voice pool with fade-out; built-in. |
| Device switch crashes | UI thread directly manipulated `OutputStream` | Engine thread owns it; send command via channel. |
| Slow pack load | Resampling happens per-keystroke | Resampled at load-time (Phase 2); built-in. |
| Worker spawns but no input | Raw Input registration conflicts in same process | Worker is separate process; built-in. |
| Sound stops after unplug | No automatic device polling (Phase 7 design) | User manually selects device in Settings or restarts. |
| IME corrections trigger sounds | Injected keystrokes (Phase 7) | Filter drops NULL-device events (Vietnamese Telex, etc.); built-in. |
| Config not persisting | JSON write fails silently | Check `utils/config.rs` error handling; add logging. |
| Settings silently revert | A writer held a loaded `AppConfig` across an await (or a long gap) and saved the whole struct back, clobbering concurrent edits | Never hold a config across an await: re-read after the await and mutate only your own fields (fixed in `auto_updater.rs`, v0.7.1). UI writers must publish through `update_config` immediately; debounce only the disk write. |
| Linux: no input detected | User not in `input` group | Instructions in README.md + app prompts user. |

## Future Enhancements

1. **Ambiance unified stream:** Merge with keyboard engine stream (reduce thread count).
2. **Config migration system:** Remove dead `MusicPlayerConfig` field.
3. **Injected input opt-in:** Make the keyboard injection filter configurable for users who want on-screen keyboards or remote-desktop input to trigger sounds.
4. **Per-device filtering (Linux):** Direct evdev device selection.
5. **macOS focus tracking:** Determine if needed; implement via CGEventTap if yes.
6. **Toast notifications:** Centralized system (currently only stderr).
7. **Soundpack preview:** Play pack sound without saving config.
8. **Customizable key bindings:** Remap hotkey from Ctrl+Alt+M.
9. **Cloud soundpacks:** Download/share packs online (infrastructure).

---

**For detailed architecture rationale, see:** `system-architecture.md`  
**For development standards, see:** `code-standards.md` (pending)  
**For historical plan context, see:** `plans/260731-input-audio-refactor/plan.md`
