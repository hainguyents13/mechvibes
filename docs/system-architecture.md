# System Architecture — MechvibesDX

**Last Updated:** 2026-08-03 · **Version:** 0.7.0  
**Status:** Reflects actual codebase post-Phase 3 (audio engine thread), Phase 6 (Windows input worker process), and Phase 7 (device watchdog removal, input injection filter, trace facility).

## Overview

MechvibesDX is a mechanical keyboard sound simulator (Rust + Dioxus 0.7 desktop). The system captures keyboard and mouse input across three platforms (Windows, Linux, macOS), plays synthesized sounds via audio output, and provides a customizable UI with soundpack/theme management.

The architecture prioritizes:
1. **Low-latency audio**: Dedicated engine thread owns `OutputStream` (non-`Send` type) exclusively.
2. **Platform-specific input capture**: Windows uses a separate worker process to bypass Raw Input registration conflicts; Linux/macOS use in-process listeners.
3. **Clean separation of concerns**: Audio engine runs independently of UI polling; input listeners run on their own threads; configuration changes propagate through typed command channels.

## UI & UX Changes (Phase 7)

**Logo press feedback:** Last-event-wins semantics restored. CSS transition reduced to 30ms inline (from 150ms) to capture fast typists (15–20 keys/sec) — measured at 0/30 visible presses with 150ms vs 29/30 with 30ms.

**Settings Devices note:** Updated to clarify "Device changes apply immediately — no restart needed. If audio stops working (e.g., after unplugging), restarting the app can help."

## High-Level Data Flow

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         KEYBOARD / MOUSE INPUT                           │
└────────────────┬────────────────────────────┬────────────────────────────┘
                 │                            │
        ┌────────▼─────────┐        ┌────────▼─────────┐
        │  Windows         │        │ Linux/macOS      │
        │  Input Worker    │        │ In-Process       │
        │  Process         │        │ Listeners        │
        │ (Raw Input)      │        │ (rdev/evdev)     │
        └────────┬─────────┘        └────────┬─────────┘
                 │                           │
                 └───────────┬────────────────┘
                             │  String events
                             │  "KeyA" / "UP:KeyA" / "MouseLeft"
                             │
        ┌────────────────────▼────────────────────┐
        │     Audio Engine Thread                 │
        │  (src/libs/audio/engine.rs)             │
        │                                         │
        │  • Owns OutputStream (rodio)            │
        │  • Voice pool (Vec<Sink>, 32 max)      │
        │  • Fade/anti-click management          │
        │  • Receives AudioCommand via channel   │
        │  • Purely event-driven (no polling)    │
        └────────────────────┬────────────────────┘
                             │
        ┌────────────────────▼────────────────────┐
        │   Rodio Audio Output                    │
        │  (cpal → OS audio driver)               │
        └─────────────────────────────────────────┘

        ┌──────────────────────────────────────────┐
        │ UI Thread (Dioxus webview)               │
        │ • Polls UiEvent channel (non-blocking)  │
        │ • Updates state for device/config       │
        │ • Sends AudioCommand to engine          │
        └──────────────────────────────────────────┘
```

## Three Major Subsystems

### 1. Audio Engine (Dedicated Thread)

**Location:** `src/libs/audio/engine.rs`, `src/libs/audio/audio_context.rs`  
**Responsibility:** Sole owner of all audio playback; bridges input events to sound output.

#### Key Invariant
`OutputStream` from rodio is **not `Send`** — Windows native APIs and some driver implementations require it to live on exactly one thread for the lifetime of the app. The engine thread satisfies this by never sharing the stream across thread boundaries.

#### Core Components

**`AudioEngineHandle`** (`engine.rs:87–99`)
- A cheap, cloneable, `Send` handle that the UI and input listeners hold.
- Used to send `AudioCommand` across thread boundaries.
- Panics are treated as unrecoverable (engine thread exits → app can no longer make sound).

**`EngineState`** (`engine.rs:119–143`)
- All audio state owned exclusively by the engine thread.
- Key fields:
  - `stream: OutputStream` — rodio's bridge to the OS audio subsystem.
  - `stream_handle: OutputStreamHandle` — used to create `Sink` instances.
  - `keyboard_sinks: Vec<Sink>` — polyphonic voice pool (Phase 1 refactor: replaced per-key HashMap).
  - `mouse_sinks: Vec<Sink>` — separate pool for mouse events.
  - `keyboard_samples`/`mouse_samples` — decoded, resampled audio buffers (one per loaded pack).
  - `device_manager: DeviceManager` — tracks current output device and available devices.

**`run_engine()` Main Loop** (`engine.rs:200+`)
- Runs on the audio engine thread.
- Uses `crossbeam_channel::select!` to multiplex (purely event-driven, no timed arms):
  1. **Command channel** (`cmd_rx`) — UI-initiated operations (volume, pack load, device switch).
  2. **Input channels** (`keyboard_rx`, `mouse_rx`, `hotkey_rx`) — raw `"KeyA"` / `"UP:KeyA"` strings from listeners.
  3. **Hotkey channel** (`hotkey_rx`) — Ctrl+Alt+M sound toggle (handled separately on Windows; detected in-process on Linux/macOS).

#### Voice Pool & Anti-Click (Phase 1)

**Why it matters:** Per-key HashMap sinks caused click artifacts when rapid keystrokes replaced old sinks before their fade-out completed.

**Implementation:**
- `keyboard_sinks` and `mouse_sinks` are `Vec<Sink>`, not HashMap.
- Each new keystroke appends a new `Sink` (FIFO pool, max 32 voices).
- **Fade-in**: 2ms linear ramp (prevent pops on attack).
- **Eviction (soft)**: When pool exceeds `MAX_VOICES` (32), oldest sinks are ramped down over 10ms instead of dropped immediately.
- **Eviction (hard)**: Once fade-out completes, the sink is removed from the pool to free resources.

**Key functions:** `apply_fade` (line 383), `manage_active_sinks` (line 421), `play_segment` (line 332).

#### Resampling On Load (Phase 2)

**Why it matters:** Packs recorded at 44.1 kHz played on a 48 kHz device would be resampled in real-time by rodio (linear interpolation, poor quality). Offline resampling at load-time is higher-quality.

**Implementation:**
- `rubato` (Rust sinc resampler) is called during pack load, not during playback.
- Each pack's audio file is decoded → resampled to device's sample rate → stored in `EngineState`.
- If device query fails, resampling is skipped and the file's native rate is used (fallback).
- Files are stored as `Arc<Vec<f32>>` (immutable, cheaply cloned into each `Sink`).

**Key file:** `src/libs/audio/resampler.rs` (sinc interpolation), `soundpack_loader.rs` (integration).

#### Device Switching (Phase 3, Watchdog Removed in Phase 7)

**Why it matters:** Unplugging headphones or switching to a different speaker should not crash the app.

**Architecture:**
- UI's device selector sends `AudioCommand::SwitchDevice(Option<String>)`.
- Engine calls `DeviceManager::open_stream()` which closes the old `OutputStream` and opens a new one on the selected device.
- If the device doesn't exist, the command returns `Err(...)` and the old stream remains active (keep current sound, report error).

**Device Watchdog (Removed Phase 7):**
The automatic device-presence polling (1-second timer checking `has_output_device_named()`, auto-fallback on 2 strikes) was removed entirely. Reason: cpal's device enumeration costs hundreds of milliseconds per call, and running it on the engine loop (which also plays sound and emits UI events) stalled keystrokes by up to several seconds.

**New behavior on device unplug:**
- Unplugging the selected output device → app goes silent.
- Config keeps the selection; user manually reselects in Settings or restarts.
- Manual switching via `AudioCommand::SwitchDevice` is unaffected.
- At startup, the saved device selection is attempted; if unavailable, falls back to system default (no startup-only fallback applies at runtime).

**Guard test:** `the_engine_loop_never_polls_the_audio_device` in `engine.rs:661-679` enforces zero timed arms in the engine's `select!` loop. Future periodic work must never run on the engine thread.

**Key files:** `engine.rs:561-643` (main loop, purely event-driven), `engine.rs:254-288` (device switch handler).

#### Latency Tracing Facility (Phase 7)

**Location:** `src/libs/trace.rs`  
**Opt-in:** Set `MECHVIBES_TRACE=1` before launching the app.

**Why it matters:** Diagnosing latency across the keyboard→worker→engine→sound→UI path without introducing the very stall being measured.

**Design:**
- Zero overhead when disabled (atomic load that fails immediately per keystroke).
- Non-blocking: all I/O is delegated to a dedicated writer thread using an unbounded channel.
- Emits one summary console line per keystroke with per-hop durations (worker→host→engine→sound→UI).
- Marks hops ≥50ms as slow.
- Full detailed log written to `%TEMP%/mechvibes-trace-{pid}.log` for offline analysis.

**Key files:** `trace.rs:1-373` (implementation).

#### Ambiance Player Integration

- The ambiance player (`src/state/ambiance.rs`) runs on a separate thread with its own `OutputStream`.
- Shares the same device-switching mechanism: `switch_ambiance_player_device()` sends a command to the ambiance engine thread.
- On device switch, ambiance sounds currently playing are resumed (bug fix from Phase 3).

### 2. Input Capture (Platform-Specific)

#### Windows: 2-Process Architecture (Phase 6)

**Problem:** Windows allows exactly one Raw Input registration target per device class per process. `tao`/`wry` claim this slot in the UI process, preventing in-process Raw Input capture. The workaround (rdev) has a race condition when the app is focused (webview event loop interferes with hook delivery).

**Solution:** Spawn a separate worker process that owns Raw Input, feed events back to the UI via stdio lines.

**Components:**

1. **Worker Process** (`src/libs/input_worker.rs`)
   - Same executable, launched with `--input-worker` flag (checked in `main()` before any app startup).
   - Runs `rawinput_listener::run_rawinput_loop()`.
   - Never touches config, tray, or Dioxus.
   - Streams events to parent over stdout, one line per event (tab-delimited).
   - Exits when stdin reaches EOF (parent's pipe handle closes, even on crash).

2. **Raw Input Loop** (`src/libs/rawinput_listener.rs`)
   - Registers a hidden (but real) window with `RegisterRawInputDevices` + `RIDEV_INPUTSINK`.
   - Handles `WM_INPUT` messages in the window's message loop (running on its own thread).
   - Decodes `RAWINPUT` structures and extracts key codes and device handles.
   - Memoizes device-ID lookups (one `InputDeviceManager::device_id_for_handle()` per device, not per keystroke).
   - Suppresses key-repeat events (held key produces one down + one up, not multiple downs).
   - **Filters injected keystrokes** (Phase 7): Drops keyboard events with NULL `hDevice` (software-injected via `SendInput`, e.g., Vietnamese IME Telex corrections like `dd→đ`). Preserves physical keys and mouse events. Filtering is transparent per-device and does not affect hotkey detection.
   - Does **NOT** filter by device or detect hotkey — config lives in the UI process.

3. **Host (Supervisor)** (`src/libs/input_worker_host.rs`)
   - Spawns and restarts the worker process if it dies (with exponential backoff: 0.5s → 1s → 2s → 4s → 8s).
   - Reads the worker's stdout line-by-line in a dedicated thread.
   - Parses events and applies **device filtering** (configured in UI).
   - Detects **Ctrl+Alt+M hotkey** (configured in UI).
   - Forwards valid events to the same `crossbeam_channel` receivers the engine listens to.
   - Falls back to rdev + device_query if worker cannot be restarted after `MAX_RESTARTS` (5 attempts).

**Wire Format (worker → host):**
```
K<TAB>{device_id}<TAB>{code}<TAB>down|up    # keyboard (only physical devices)
M<TAB>{device_id}<TAB>{code}<TAB>down|up    # mouse
```

**Injected-input filter (Phase 7):** Keyboard events with NULL device handle (injected by software like IMEs or SendInput) never reach the wire; they are dropped in `is_physical_keyboard_event()` before serialization. This silences Vietnamese IME Telex corrections (e.g., `dd→đ`) which the user didn't type directly.

**Known trade-off (Phase 7):** On-screen keyboards, AutoHotkey, and remote-desktop client input are also injected (silent). A per-user opt-in can be added if demand exists.

**Key invariant:** Single-instance guard on startup (`single_instance.rs`) prevents multiple UI processes from running (which would spawn multiple input workers competing over Raw Input).

**Key files:** `input_worker.rs:89-119` (injection filter), `rawinput_listener.rs`, `input_worker_host.rs`, `single_instance.rs`.

#### Linux: evdev on Wayland, rdev + device_query on X11 (In-Process)

`bootstrap.rs` picks the listeners from `XDG_SESSION_TYPE`:

**Wayland:** `evdev_input_listener.rs` reads `/dev/input/event*` directly, so it needs the user in the `input` group. It reports keys, the Ctrl+Alt+M hotkey, mouse buttons (`BTN_*`) and the wheel (`REL_WHEEL`), whether or not the app window has focus. `rdev` still runs for the mouse, but stays silent once evdev has opened a device with `BTN_LEFT` (`evdev_pointer_active`), so a click is never played twice.

**X11:** `rdev` (XRecord) in `input_listener.rs` reports keys while the app is unfocused, plus mouse buttons and the hotkey; `device_query` in `focused_input_listener.rs` reports keys while it is focused. Neither opens `/dev/input`, so X11 works without the `input` group. The wheel is not reported on X11.

No package grants `/dev/input` access (see `deployment-guide.md`), and there is no in-app warning yet when the devices can't be read (#124).

**Key files:** `bootstrap.rs`, `evdev_input_listener.rs`, `input_listener.rs`, `focused_input_listener.rs`.

#### macOS: rdev + device_query (In-Process)

Uses the same code as Linux, but `rdev` on macOS does not have focus issues (unlike Windows).

**Key file:** `input_listener.rs`.

### 3. Soundpack Loading & Configuration

**Location:** `src/libs/audio/soundpack_loader.rs`, `src/state/soundpack.rs`, `src/state/config.rs`  
**Responsibility:** Parse soundpack config, decode audio, resample, and hand decoded buffers to the engine.

#### Soundpack Format

**Directory structure:**
```
my-pack/
├── config.json         # Metadata and sound definitions
├── sounds.ogg          # Single audio file (definition_method: "single")
│   or
├── sounds/
│   ├── keydown.ogg     # One file per sound (definition_method: "multi")
│   └── keyup.ogg
└── icon.png            # Optional
```

**config.json (V2 format):**
```json
{
  "id": "my-keyboard-pack",
  "name": "My Keyboard",
  "description": "A custom keyboard sound pack",
  "definition_method": "single",
  "audio_file": "sounds.ogg",
  "definitions": {
    "KeyA": { "timing": [[0, 100]] },
    "KeyB": { "timing": [[100, 200]] },
    "KeyUp": { "timing": [[200, 250]] }
  },
  "options": {
    "recommended_volume": 0.8,
    "random_pitch": false
  },
  "soundpack_type": "Keyboard"
}
```

For `definition_method: "multi"`, each sound can have its own audio file:
```json
"KeyA": {
  "timing": [[0, 100]],
  "audio_file": "sounds/keydown.ogg"
}
```

#### Loading Path

1. **User action:** Drag-and-drop folder or select from soundpacks list.
2. **Validation:** `SoundpackValidator` checks format and required files.
3. **Load command:** UI sends `AudioCommand::LoadKeyboardPack { soundpack_id, ... }`.
4. **Decode:** `soundpack_loader.rs` decodes the audio file(s) using `symphonia` (multi-codec: WAV, OGG, MP3).
5. **Resample:** If file sample rate ≠ device sample rate, `rubato` resamples offline.
6. **Store:** Decoded samples stored in `EngineState.keyboard_samples` as `Arc<Vec<f32>>`.
7. **Report:** Engine sends `UiEvent::PackLoaded` back to UI (success or error).

**Key invariant:** Audio files are loaded into memory at pack-load time, not on the first keystroke. This avoids stutter on first use.

**Key files:** `soundpack_loader.rs`, `resampler.rs`, `src/utils/soundpack_validator.rs`.

## Configuration & State

### AppConfig (Persistent)

**Location:** `src/state/config.rs`, stored at `~/.local/share/mechvibes-dx/config.json` (Linux/macOS) or registry (Windows).

**Key fields relevant to audio/input:**
- `keyboard_soundpack`, `mouse_soundpack` — currently loaded pack IDs.
- `volume` (0–100), `mouse_volume` (0–100) — master and per-input-type volumes.
- `enable_sound`, `enable_keyboard_sound`, `enable_mouse_sound` — mute controls.
- `selected_audio_device` — device name (or `None` for system default).
- `enabled_keyboards`, `enabled_mice` — device instance IDs for per-device filtering (Windows).
- `ambiance_active_sounds` — sounds currently playing in ambiance player (HashMap key = sound_id).

### Channel Contracts

#### Input Channel Formats (String-based)

All input listeners (rdev, evdev, Raw Input) emit the same string format:

**Keyboard:**
- Keydown: `"KeyA"` (string code)
- Keyup: `"UP:KeyA"` (prefixed with `"UP:"`)

**Mouse:**
- Button down: `"MouseLeft"`
- Button up: `"UP:MouseLeft"`

**Hotkey (processed by host on Windows, in-process on Linux/macOS):**
- Not emitted as a string; detected and handled internally, only broadcasts `UiEvent::KeyDown` / `UiEvent::KeyUp` on valid hotkey press.

#### Audio Command Channel

Sent by UI/device selector to engine:
```rust
pub enum AudioCommand {
    SetVolume(f32),
    SetMouseVolume(f32),
    SetSoundEnabled(bool),
    SetKeyboardSoundEnabled(bool),
    SetMouseSoundEnabled(bool),
    LoadKeyboardPack { soundpack_id: String, update_cache_on_error: bool },
    LoadMousePack { soundpack_id: String, update_cache_on_error: bool },
    SwitchDevice(Option<String>),
}
```

#### UI Event Channel

Engine → UI (for status updates):
```rust
pub enum UiEvent {
    KeyDown(String),
    KeyUp(String),
    DeviceSwitched(Result<String, String>),
    PackLoaded { is_keyboard: bool, result: Result<String, String> },
}
```
Note: `DeviceLost` was removed in Phase 7 (watchdog removal). On device unplug, the app goes silent with no event.

## Critical Design Decisions & Rationale

| Decision | Alternative Considered | Rationale |
|----------|------------------------|----|
| **Dedicated engine thread** | Keep audio in UI thread (like Phase 1) | `OutputStream` is not `Send`. UI thread is a Dioxus/wry executor, not a good fit for blocking audio operations like `Sink::sleep_until_end()`. Thread affinity eliminates `Arc<Mutex<OutputStream>>` fragility. |
| **Windows worker process** | In-process Raw Input (Phase 4) | tao/wry's Raw Input registration conflicts with ours at the process level — empirically verified. Only solution is separate process. |
| **Voice pool (Vec) not HashMap** | Keep per-key HashMap with hard eviction | HashMap `insert()` drops the old sink immediately → click/chop. Voice pool with soft fade-out over 10ms preserves tail. |
| **Offline resampling (rubato)** | Real-time resampling (rodio default) | Offline sinc (higher quality) vs. real-time linear (faster, lower quality). Audio is the main product; offline is negligible cost at load time. |
| **No device watchdog** | Previous design: 1s poll timer with auto-fallback | Polling cost hundreds of milliseconds per check, stalling keystrokes. User decision: go silent on unplug instead of polling. Manual switch (Settings) remains the only way to change devices. |
| **No device config in worker** | Worker reads config directly | Config lives in UI (persistence, multi-process coordination). Pipe remains one-directional. Host applies filtering post-capture. |
| **Ambiance on separate thread** | Ambiance sharing keyboard engine stream | Ambiance is long-lived (can play for minutes). Sharing a stream would mix device-switch logic. Separate `OutputStream` is cleaner; only downside is 2 audio threads. |

## Startup Sequence

1. **Early Windows check:** `main()` checks for `--input-worker` flag (before any app init).
   - If present, jump to `input_worker::run()` and exit process.
   - Never reached: worker doesn't know about Dioxus/tray/UI.

2. **Single-instance guard:** Windows-only.
   - Hash the current executable path + hash into a mutex name.
   - If already acquired, print error and exit (another instance is running).

3. **Input listeners start:** Spawn OS threads for rdev, device_query, evdev (platform-specific).
   - On Windows, supervisor in `input_worker_host.rs` spawns the worker process and reader thread.

4. **Audio engine spawns:** Before `dioxus::launch`.
   - Initializes rodio `OutputStream` (claims OS audio device).
   - Spawns engine thread.
   - Caches device sample rate.

5. **Dioxus webview launches:** UI runs, calls `use_context::<AudioContext>()` (facade over engine handle).

6. **Ambiance player starts:** Separate `OutputStream`, separate thread, paused initially.

## Error Handling & Fallbacks

| Failure | Behavior |
|---------|----------|
| Audio device not found at startup | Fall back to system default; report via `UiEvent::DeviceSwitched(Err(...))`. |
| Audio device not found via manual switch | Keep previous device; report error via `UiEvent::DeviceSwitched(Err(...))`. |
| Audio device unplugged at runtime | No automatic fallback; app goes silent. User manually reselects in Settings or restarts. |
| Soundpack decode error | Log error; emit `UiEvent::PackLoaded(Err(...))`. |
| Resampler fails | Skip resample, use original sample rate (fallback). |
| Windows worker crash | Restart with exponential backoff (0.5s → 1s → 2s → 4s → 8s). After 5 fails, fall back to rdev. |
| Windows worker spam on restart | Tracked by `HEALTHY_UPTIME` (30s); if worker survives 30s, backoff resets to 0.5s. |
| Linux/macOS input listener dies | App continues (rdev + device_query). Stale hotkey bindings on re-focus (acceptable tradeoff). |

## Testing Approach

**Unit tests:**
- `resampler.rs`: Impulse response, chunk truncation, tail flush.
- `engine.rs`: Voice pool FIFO, fade math, purely event-driven loop (no timed arms), no device polling.
- `rawinput_listener.rs`: Key code mapping, device ID memoization.
- `input_worker.rs`: Injected keystroke filtering (NULL device detection).
- `input_worker_host.rs`: Restart backoff, device filtering logic.

**Integration tests:**
- UI can load pack → engine receives → sound plays.
- Device switch → old stream closed, new stream opened.
- Worker restart → input recovered after crash.

**Manual testing (user responsibility):**
- Trill 15–20 keys/sec: no click/chop.
- Reverb-tail soundpack: tails not cut off.
- Switch device while typing: no crash, sound stays on new device.
- Unplug headphones mid-keystroke: auto-fallback, no stutter.

## Known Limitations & Future Work

1. **Ambiance:** Separate `OutputStream` means potentially 2 concurrent audio threads. Could be unified with careful stream-sharing logic (future).

2. **Injected input filtering (Windows):** Keyboard-only. Filters software-injected keys (e.g., IME corrections, SendInput, on-screen keyboards). A per-user opt-in can be added if demand exists.

3. **Per-device filtering (Windows):** Only keyboard/mouse. Other input types (gamepad, etc.) not supported.

4. **Focus tracking (macOS):** Not implemented. Unclear if macOS has the same focus issue as Windows; needs testing.

5. **Wayland:** rdev on Wayland is newer; edge cases possible.

6. **Config migration:** `MusicPlayerConfig` is a dead field (music player removed); kept for backward compat. Could be pruned in a major version.

## File Reference Map

| Path | Purpose |
|------|---------|
| `src/libs/trace.rs` | Opt-in latency tracing (MECHVIBES_TRACE=1) |
| `src/libs/audio/engine.rs` | Engine thread, voice pool, device switching |
| `src/libs/audio/audio_context.rs` | Facade for UI; forwards to engine |
| `src/libs/audio/soundpack_loader.rs` | Decode, resample, load packs into engine |
| `src/libs/audio/resampler.rs` | Sinc resampling (rubato) |
| `src/libs/device_manager.rs` | Track output device, enumerate, test presence |
| `src/libs/input_listener.rs` | rdev capture (macOS/Linux X11) |
| `src/libs/evdev_input_listener.rs` | Direct evdev (Linux) |
| `src/libs/focused_input_listener.rs` | device_query fallback on focus |
| `src/libs/input_manager.rs` | Channel initialization, focus state |
| `src/libs/input_worker.rs` | Worker process entry (Windows) |
| `src/libs/input_worker_host.rs` | Worker supervisor, config filter (Windows) |
| `src/libs/rawinput_listener.rs` | Raw Input loop (Windows worker) |
| `src/libs/input_device_manager.rs` | Device ID lookup from Raw Input handle |
| `src/libs/single_instance.rs` | Single-instance guard (Windows) |
| `src/state/config.rs` | Persistent configuration schema |
| `src/state/soundpack.rs` | Soundpack metadata and definitions |
| `src/state/ambiance.rs` | Ambiance player state and commands |
| `.github/workflows/ci.yml` | Build + test + package for Windows, Linux, macOS on PRs and pushes to `main`; drafts a release when a tag points at the built commit |

---

**See also:** `codebase-summary.md` for module-by-module navigation; `code-standards.md` for development guidelines.
