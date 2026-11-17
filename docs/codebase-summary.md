# Codebase Summary — Mechvibes

**Last Updated:** 2026-10-31 · **Language:** Rust (1.88+) + Dioxus 0.7.10  
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
│   │   ├── ui.rs               # Dioxus render loop; theme file drop area, root dialogs and toasts
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
│   │   ├── saved_themes.rs     # Saved themes list (saved_themes.json): damaged entries kept, read-only when it cannot be read
│   │   └── rodio_music.rs      # [DEAD CODE] Music player (removed)
│   ├── i18n/                   # Every string the user reads (see Internationalization)
│   │   ├── mod.rs              # Language, resolve, system_language, use_language, use_strings, fill, plural
│   │   ├── detect.rs           # System locale: LC_ALL/LC_MESSAGES/LANG, then sys-locale
│   │   ├── strings.rs          # Strings (one field per zone), one const per language, for_language
│   │   └── catalog/            # One file per zone: struct + EN/ZH/ES + ENTRIES
│   │       ├── mod.rs          # zone! macro, ZONES, IDENTICAL_IS_INTENTIONAL, the invariants
│   │       ├── chrome.rs       # Title bar, header, volume slider, app info
│   │       ├── common.rs       # Buttons and words several screens use
│   │       ├── customize.rs    # Customize page and its tabs
│   │       ├── devices.rs      # Device selectors
│   │       ├── errors.rs       # Notices and messages from outside the component tree
│   │       ├── home.rs         # Home page, update button, app info
│   │       ├── mood.rs         # Mood page + ambience_text (the ten built-in ambiences)
│   │       ├── nav.rs          # Dock labels
│   │       ├── settings.rs     # Settings page
│   │       ├── soundpacks.rs   # Sound packs page, selector, table, import modals
│   │       └── tray.rs         # Tray menu
│   ├── components/             # Dioxus UI components
│   │   ├── mod.rs
│   │   ├── app_info.rs         # App info modal
│   │   ├── header.rs           # Top bar
│   │   ├── dock.rs             # Bottom controls
│   │   ├── logo.rs             # Animated logo
│   │   ├── soundpack_selector.rs # Pack selection dropdown
│   │   ├── device_selector.rs  # Audio device selector + filtering UI
│   │   ├── window_controller.rs # Min/max/close buttons
│   │   ├── theme_share.rs      # Share a card or the look to a .mvtheme file, pick one to import, put_on (installs a card) and undo_look
│   │   ├── customize/          # The Customize page's pieces
│   │   │   ├── current_theme_row.rs  # Swatch + name of the theme in use; Share / Import / Find more
│   │   │   ├── customize_tabs.rs     # Themes / Colors / Logo / Background tabs (tab kept for the session)
│   │   │   ├── themes_tab.rs         # Gallery: Ready-made, My themes, Installed, Find more; card menus
│   │   │   ├── theme_card.rs         # One theme card: preview, name, color dots, menu
│   │   │   ├── rename_modal.rs       # Name field for Rename
│   │   │   ├── look_editor.rs        # use_look_editor: edits saved on their own (300 ms), apply_card
│   │   │   ├── colors_tab.rs         # Ready-made colors, My colors, New colors, delete with confirm (not the colors in use)
│   │   │   ├── colors_editor_modal.rs # New / edit colors; saves through safe_theme_css
│   │   │   ├── part_panel.rs         # Shared Logo / Background shell: switch, first edit turns the part on, Start over
│   │   │   ├── logo_tab.rs           # Logo fields and Sound on / Sound off preview
│   │   │   └── background_tab.rs     # Background fields
│   │   ├── volume_slider.rs    # Volume control
│   │   ├── ui/                 # Reusable UI components
│   │   │   ├── mod.rs
│   │   │   ├── modal.rs        # Base modal
│   │   │   ├── color_picker.rs # Color selector; THEME_COLOR_OPTIONS; a typed color saves only once complete
│   │   │   ├── fill_field.rs   # Color or Image switch for one fill
│   │   │   ├── image_picker.rs # Picks an image and copies it into the images folder
│   │   │   ├── confirm_modal.rs # Yes / no dialog
│   │   │   ├── theme_import_modal.rs # The one "Use this theme?" dialog (root): files, drops, library links, checklist, Undo
│   │   │   ├── toast.rs        # Toasts (root) for notices, with Show file / Undo
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
│   │       ├── customize.rs    # Customize page: current theme row + tabs (see components/customize/)
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
│       ├── theme_share.rs      # .mvtheme read/write and checks, Look, ImportParts (see theme-file-format.md)
│       ├── theme_share/
│       │   ├── wire_v1.rs      # Frozen format 1 layout of theme.json
│       │   └── fixtures/v1/    # Files written by v3.3.0, pinned by tests
│       ├── custom_images.rs    # Saves picked and imported images as img-<content hash>.<ext>; owned-images.json; clears unused ones
│       ├── theme_library.rs    # Saved themes logic: sync_action / run_sync, my copy names, installing cards
│       ├── starter_themes.rs   # The ready-made themes built in from assets/starter-themes/
│       ├── look_undo.rs        # One-step Undo after a theme or the Default look goes on (memory only, pointer included)
│       ├── notice.rs           # Thread-safe messages for the user, shown as toasts
│       ├── deep_link.rs        # mechvibes:// links and .mvtheme files from the command line or a second launch
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
│   ├── icon.png             # Linux/macOS app icon
│   ├── starter-themes/      # Ready-made .mvtheme files built into the app, CREDITS.md
│   └── flags/               # Language flags for Settings > General > Language (us, cn, es)
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
    pub language: Option<Language>, // None = follow the system language
    pub theme: Theme,
    pub logo_customization: LogoCustomization,
    pub background_customization: BackgroundCustomization,
    pub active_theme_id: Option<String>, // Saved theme id or "starter:<slug>"; None = Default or not saved yet
    
    // Ambiance
    pub ambiance_active_sounds: HashMap<String, f32>, // sound_id → volume
    pub ambiance_global_volume: f32,
    pub ambiance_is_muted: bool,
    pub ambiance_custom_sounds: Vec<CustomMoodSound>, // user-added sounds; files in data/mood_sounds
    
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

## Internationalization (i18n)

Every string the user reads comes from a typed catalog in `src/i18n/`. Nothing is read from disk at runtime and no i18n crate is used: the only dependency this feature added is `sys-locale = "0.3"`, for one platform query.

### The model

`Language` (`src/i18n/mod.rs`) is `En | Zh | Es` and serializes as its primary subtag (`"en"`, `"zh"`, `"es"`), so the identifier in `config.json` is stable across reordering the enum. `native_label()` returns `English` / `简体中文` / `Español` and is **never translated**: a language is named in its own language, or the one reader who needs it cannot find it. `from_bcp47` keeps only the primary subtag (`es-AR` → `Es`, `zh_TW.UTF-8` → `Zh`) and answers `None` for a language the app does not speak. Chinese has one Simplified catalog for now, so a Traditional locale lands there instead of falling back to English.

`AppConfig::language` is `Option<Language>` (`src/state/config.rs`). **`None` means "follow the system"**, which is why there is no fourth `System` variant: it is the absence of a preference, not a language. `resolve(preferred)` is `preferred.unwrap_or_else(system_language)`. A config written before the field existed carries no `language` key and loads as `None` (an `Option` field may be absent), so an upgrade keeps the OS language instead of being pinned to English. The field is listed in `data_equals`, which is what `config_writer::apply` uses to decide whether to persist, so choosing a language is saved like any other setting; a damaged value falls back to "follow the system" through `parse_lenient`.

### Detection (two layers)

`src/i18n/detect.rs`, in this order:

1. **The POSIX environment** — `LC_ALL`, then `LC_MESSAGES`, then `LANG`. The first value that is not empty, `C` or `POSIX` wins. `env_locale_from` takes the lookup as a closure so the precedence is testable: `std::env::set_var` is `unsafe` in edition 2024 and races between parallel tests. This is the precedence the repo already had in telemetry.
2. **`sys_locale::get_locale()`** — the preferred UI language on Windows, `NSLocale` on macOS, `setlocale` on Unix.

The second layer is not redundancy. An app started from the Windows desktop or from Finder inherits no `LANG` at all, so the environment alone would answer "unknown" on exactly the platforms where most people launch it from an icon, and the app would open in English regardless of the system language. `system_locale` tries layer 1 and falls back to layer 2; `system_language` runs that once and caches it in a `OnceLock`, so every render does not hit the platform API and the language cannot change halfway through a screen.

Telemetry keeps its own private copy of layer 1 in `src/utils/telemetry.rs`, on purpose: it feeds an anonymous launch metric whose continuity nobody asked to change. The duplication is documented in both places; unifying it is a metrics decision, not a refactor.

### The catalog

`src/i18n/catalog/` holds one file per zone. A zone is a group of screens edited together (`settings`, `soundpacks`, `tray`…), so two people can translate different parts at the same time without touching the same file. Each file has one `zone!` block with one line per key and one column per language:

```rust
zone! {
    NavStrings {
        home: "Home", "首页", "Inicio",
    }
}
```

The `zone!` macro (`src/i18n/catalog/mod.rs`) generates the struct, the three `const`s (`EN`, `ZH`, `ES`) and the test-only `ENTRIES` table from that one list:

- **A typo does not compile.** `strings.settings.page_title` is a field access; there is no lookup by name, so a misspelled key is `no field … on type …`.
- **A key cannot be missing from one language.** All three `const`s come from the same list, so a key without its three columns is a macro arity error.
- **A new zone must be registered.** A new module does not compile until its struct is a field of `Strings` (`src/i18n/strings.rs`) and appears in all three language `const`s. Adding it to `ZONES` (next to the `zone!` block) is what puts it under the invariants — the compiler does not ask for that one.

`Strings` is the composed struct, one field per zone. `src/i18n/strings.rs` builds one `const Strings` per language out of the zone `const`s and `for_language` matches on the variant; `Language::strings()` hands out a `&'static Strings`, so nothing is allocated per render.

**Invariants** — `src/i18n/catalog/mod.rs` walks every zone's `ENTRIES`:

| Test | Catches |
|------|---------|
| `no_translation_is_blank` | a language left empty |
| `no_translation_has_surrounding_whitespace` | spacing baked into a value, which an editor trims and no review sees — the separator belongs at the call site |
| `every_translation_keeps_the_english_placeholders` | a translation that drops or repeats `{}`, which `fill` would then misplace silently |
| `every_translation_differs_from_the_base_language` | a column copied from the English |
| `the_allowlist_has_no_stale_entries` | an allowlist entry naming a key that no longer exists, or with no reason |
| `every_language_resolves_to_a_catalog` | a language wired to an empty catalog — the type system cannot tell |

**The allowlist.** `IDENTICAL_IS_INTENTIONAL` is the reviewed list of entries whose translation legitimately equals the English one, each with its reason on the same line. It is not a way to silence a failure: `Error` and `Color` are the correct Spanish words, and `General` names the section identically. Such a word still belongs in the catalog rather than as a literal at the call site — Spanish happens to spell three of these the same, German or French would not, so the key is what keeps the word translatable for the next language even when this one coincides. A hardcoded literal cannot be translated at all.

Two zones guard what the compiler cannot. `nav.rs` pins the English dock labels to the text users already had, so the migration re-worded nothing on screen. `mood.rs` carries `every_built_in_ambience_has_its_text_in_every_language`, because the ten built-in ambiences are looked up by `id` through `ambience_text` and nothing makes a new one arrive with its two keys.

### `fill` and `plural`

- `fill(template, &[args])` replaces `{}` in order. `format!` requires a literal format string, so `format!(strings.some_key, n)` does not compile and every count-dependent sentence would have to be split around its number at the call site. A placeholder with no matching argument stays visible as `{}` instead of panicking: a stray `{}` is a bug report, a render panic is a dead window.
- `plural(one, other, count)` picks between two catalog keys with the `_one` / `_other` suffix. Count-dependent wording belongs in the data, not in a `match` at the call site, and a language that does not inflect for number (Chinese) writes the same text in both keys.

### Hooks and the `use_memo` rule

- `use_language()` reads the config signal through `use_config()` and resolves it with `resolve`.
- `use_strings()` is `use_language().strings()`, so a component that calls it is subscribed to the config signal and re-renders when the language changes. There is no global language variable and nothing is invalidated by hand.

**No translated value goes inside a `use_memo`.** A memo recomputes only when a signal read *inside* its closure changes, so a value captured outside keeps the language it was computed with until the component unmounts. Call `use_strings()` in the component body; `use_memo` stays for config values that are not text.

### Adding a Language

1. Add the variant to `Language` (`src/i18n/mod.rs`) and its arms in `tag` and `native_label`; add it to `Language::ALL` and bump the count in `the_selector_lists_every_language`. The compiler names the first two; `ALL` is the one place it does not check.
2. Widen `zone!` (`src/i18n/catalog/mod.rs`) by one column: one more `literal` in the matcher, one more `const` it emits, one more column in `ENTRIES`. The invariant tests destructure `(key, en, zh, es)`, so they change shape with it.
3. Fill the new column in every zone file — eleven today. A file left behind fails with a macro error on the offending line: it points at the line, not at the key.
4. `src/i18n/strings.rs`: one more `const Strings` and an arm in `for_language`, whose match is exhaustive.
5. A flag in `assets/flags/` and its arm in `flag_for` (`src/components/pages/settings.rs`).
6. `cargo test`.

**What the compiler catches:** a missing arm in `tag`, `native_label`, `for_language` or `flag_for`; a zone file with the wrong number of columns; a misspelled key at a call site.
**What it cannot:** whether a column is a real translation or a copy of the English, which is `every_translation_differs_from_the_base_language` plus `IDENTICAL_IS_INTENTIONAL`; an ambience added without its text, which is `mood.rs`'s test; a forgotten entry in `Language::ALL`; and a zone left out of `ZONES`, whose entries then escape the invariants entirely.

### What is not in the catalog

- **The wordmark.** `logo.rs` draws "Mechvibes" as part of the mark (`src/components/logo.rs`), and `APP_NAME` (`src/utils/constants.rs`) is what `fill` splices into a sentence. A brand is not a translation: giving every translator the name to retype invites them to translate it.
- **DaisyUI theme identifiers.** The `data-theme` names DaisyUI switches on (`light`, `synthwave`, …) are identifiers in `src/libs/theme.rs`, not text. The user-facing label of a mode (`Light`, `Dark`, `System`) is in the catalog.
- **URLs and file paths.** A link is a URL where the code opens it (`LIBRARY_BASE_URL` in `src/utils/constants.rs`, the release and social links in `src/components/pages/settings.rs` and `src/components/window_controller.rs`), not a key; a path is an argument to a catalog sentence, never a key.
- **Key combinations.** `Ctrl+Alt+M` is written inside the sentence that mentions it, so the translator moves the words around a shortcut nobody translates.
- **Spacing and punctuation around a value.** The space after `Volume`, the `%` after a number, the `❌` before an error: layout stays at the call site, because an editor trimming trailing whitespace must not change what the user sees.

### The layout constraint

The dock is a bar of five buttons where each label gets a fixed, equal slice of the window width. A translation that does not fit does not only look wrong: it wraps to a second line, pushes the icon row up and misaligns the whole bar (it did happen, with "Paquetes de sonido"). Translations are shortened until they fit — `Paquetes` — and the dock is not restructured around the longest label.

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

`src/utils/telemetry.rs` sends a single anonymous launch ping per app start to the website (`POST {LIBRARY_BASE_URL}/api/app/launch`, one JSON object: `os`, `osVersion`, `appVersion`, `locale`, `debug`). The site adds it to a per-day counter (country from Cloudflare's `CF-IPCountry`), keeps no IP or single launches, and skips `debug: true` pings. No keystrokes, no personal data, no identifiers of any kind.

- Gated by `AppConfig::enable_telemetry` (default `true`; toggle in Settings, Privacy section). Off means zero outbound requests.
- Fire-and-forget on a background thread with a short timeout; network errors are swallowed. Never touches the engine loop, input worker, or UI poll loop.
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
1. `build` job (every run): a matrix (Windows, Linux, macOS on Apple Silicon and on Intel, in parallel); each leg reads the version from `Cargo.toml`, runs `scripts/ci-check.sh --release`, then its packaging script (`build-windows-installer.ps1`, `build-linux-appimage.sh`, `build-macos-app.sh`), which check that Rust is at least `rust-version` from `Cargo.toml`, build with `--locked` and check their own output, and uploads `windows-release`, `linux-release`, `macos-arm64-release` and `macos-x86_64-release` (except for pull requests).
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
6. **Soundpack preview:** Play pack sound without saving config.
7. **Customizable key bindings:** Remap hotkey from Ctrl+Alt+M.
8. **Cloud soundpacks:** Download/share packs online (infrastructure).

---

**For detailed architecture rationale, see:** `system-architecture.md`  
**For development standards, see:** `code-standards.md` (pending)  
**For historical plan context, see:** `plans/260731-input-audio-refactor/plan.md`
