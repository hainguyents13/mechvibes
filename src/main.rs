#![windows_subsystem = "windows"]
#![allow(non_snake_case)]

mod components;
mod libs;
mod state;
mod utils;

use dioxus::desktop::{ Config, LogicalSize, WindowBuilder };
use dioxus::prelude::*;
use utils::constants::{ APP_NAME };
use libs::ui;
use libs::window_manager::{ WindowAction, WINDOW_MANAGER };
use libs::input_manager::{ init_window_focus_state_with_value, get_window_focus_state };
use std::sync::mpsc;

// Platform-specific icon files for optimal quality
#[cfg(target_os = "windows")]
const EMBEDDED_ICON: &[u8] = include_bytes!("../assets/icon.ico");

#[cfg(not(target_os = "windows"))]
const EMBEDDED_ICON: &[u8] = include_bytes!("../assets/icon.png");

fn load_icon() -> Option<dioxus::desktop::tao::window::Icon> {
    // Platform-specific icon format and size
    // Windows: ICO format (multi-size), 32x32 for taskbar
    // Linux/macOS: PNG format, 64x64 for better X11/Wayland support

    #[cfg(target_os = "windows")]
    let format = image::ImageFormat::Ico;

    #[cfg(not(target_os = "windows"))]
    let format = image::ImageFormat::Png;

    match image::load_from_memory_with_format(EMBEDDED_ICON, format) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (width, height) = rgba.dimensions();
            debug_print!("📐 Loaded icon: {}x{}", width, height);

            // Platform-specific target sizes
            #[cfg(target_os = "windows")]
            let target_size = 32u32;

            #[cfg(target_os = "linux")]
            let target_size = 64u32;

            #[cfg(target_os = "macos")]
            let target_size = 64u32;

            let final_rgba = if width != target_size || height != target_size {
                debug_print!("🔄 Resizing icon from {}x{} to {}x{}", width, height, target_size, target_size);
                image::imageops::resize(&rgba, target_size, target_size, image::imageops::FilterType::Lanczos3)
            } else {
                debug_print!("✅ Icon already at optimal size ({}x{})", width, height);
                rgba
            };

            match dioxus::desktop::tao::window::Icon::from_rgba(final_rgba.into_raw(), target_size, target_size) {
                Ok(icon) => {
                    debug_print!("✅ Successfully created window icon ({}x{})", target_size, target_size);
                    Some(icon)
                }
                Err(e) => {
                    always_eprint!("❌ Failed to create window icon from RGBA data: {}", e);
                    None
                }
            }
        }
        Err(e) => {
            always_eprint!("❌ Failed to load embedded ICO data: {}", e);
            None
        }
    }
}

fn main() {
    // Input worker mode: this must be the very first thing main() does. The
    // worker shares this executable but must never touch app state, the
    // tray, config or the Dioxus runtime - it only registers Raw Input and
    // streams events to the parent over stdout. See input_worker.rs.
    // `args_os` rather than `args`: the latter panics on an argument that is
    // not valid Unicode, which would kill the app before any logging exists
    // to say why.
    #[cfg(target_os = "windows")]
    if std::env::args_os().any(|arg| arg == libs::input_worker::WORKER_ARG) {
        libs::input_worker::run();
        return;
    }

    // Parsed before anything else so the headless branch can borrow the
    // parent's console before the first status line is printed.
    let cli = libs::cli_args::parse(std::env::args_os());

    // Headless runs from a terminal, and this binary is built
    // `#![windows_subsystem = "windows"]`, so without this every message
    // below - including "already running" and any soundpack error - would go
    // to a console that does not exist.
    #[cfg(target_os = "windows")]
    if cli.headless {
        libs::console_attach::attach_to_parent_console();
    }

    // A theme file opened from the file manager travels the same way as a
    // library link, as text: `deliver_link` tells the two apart. A headless
    // run has no window to ask in, so it takes no theme file.
    let deep_link = utils::deep_link::link_from_args(std::env::args_os()).or_else(|| {
        (!cli.headless)
            .then(|| utils::deep_link::theme_file_from_args(std::env::args_os()))
            .flatten()
            .map(|path| path.to_string_lossy().into_owned())
    });

    // Refuse to start a second copy: two instances means two input listeners
    // and two audio engines, so every keystroke would play twice. Claimed
    // after the worker branch above on purpose - the worker is a child of an
    // instance that already holds this lock and must not be turned away.
    // Held for the whole run; Windows releases it when this process dies.
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    let _instance_guard = match libs::single_instance::acquire() {
        Some(guard) => guard,
        None => {
            if cli.headless {
                // Two capture pipelines would play every keystroke twice, so
                // a headless run refuses rather than joining in. It also does
                // not raise the other copy's window: someone asking for
                // headless mode from a terminal did not ask for a window.
                always_eprint!(
                    "⚠️ {} is already running - close it first, or use the running copy",
                    APP_NAME
                );
                return;
            }
            // Surface the window of the copy that is already running before
            // giving up. Launching the app a second time is how people ask to
            // see it again, and this process is about to exit invisibly -
            // `#![windows_subsystem = "windows"]` means the message below
            // reaches no console.
            //
            // A library link or theme file that started this launch goes to
            // the running copy, which picks it up when it wakes. Only the
            // path is passed on; the running copy reads the file.
            if let Some(link) = &deep_link {
                if let Err(e) = utils::deep_link::hand_over(link) {
                    always_eprint!("⚠️ {}", e);
                }
            }
            libs::single_instance::signal_running_instance();
            always_eprint!("⚠️ {} is already running - raising its window instead", APP_NAME);
            return;
        }
    };

    // Answer those requests for the rest of this process's life. Routed
    // through the same WindowAction::Show the tray's "Show" item uses, so a
    // second launch and the tray converge on one show/focus path. A library
    // link or theme file that launch left behind is passed on to the window.
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    libs::single_instance::listen_for_wake_requests(|| {
        WINDOW_MANAGER.request_show();
        if let Some(link) = utils::deep_link::take_handed_over() {
            utils::deep_link::deliver_link(&link);
        }
    });

    // A link or theme file that started this launch, and any left over from a
    // launch that handed one over just as the last copy was closing.
    let pending_link = deep_link.or_else(utils::deep_link::take_handed_over);
    let opened_by_link = pending_link.is_some();
    if let Some(link) = pending_link {
        utils::deep_link::deliver_link(&link);
    }

    // Initialize debug logging first
    utils::logger::init_debug_logging();

    env_logger::init();

    // Opt-in latency tracing (MECHVIBES_TRACE=1). Must run before the input
    // worker, audio engine or UI loop start, since each of those records
    // trace points and reads the enabled flag once it is live.
    libs::trace::init();

    debug_print!("🚀 Initializing {}...", APP_NAME);

    // Ahead of the first config read, which syncs `auto_start` from the
    // registry and would save "off" for an entry still under the old name.
    #[cfg(target_os = "windows")]
    utils::auto_startup::migrate_legacy_run_entry();

    // Initialize app manifest first
    let _manifest = state::manifest::AppManifest::load();

    // Ensure soundpack directories exist
    if let Err(e) = state::paths::soundpacks::ensure_soundpack_directories() {
        debug_eprint!("⚠️ Failed to create soundpack directories: {}", e);
    }

    // Headless: audio engine plus the same input capture the GUI uses, and
    // nothing else. Branches here rather than earlier so config, logging and
    // the soundpack directories are already set up, and before the GUI-only
    // work below (telemetry, ambiance, update state, the window) - none of
    // which a windowless run has any use for.
    //
    // Everything past this point is unreachable in headless mode, which is
    // what keeps the default launch path unchanged.
    if cli.headless {
        libs::headless::run(&cli);
        return;
    }

    // Check if we should start minimized (from auto-startup).
    //
    // This is the first read of the config, so it is what loads the file into
    // the writer's authority (parsing it, applying migrations and syncing
    // `auto_start` against the registry) - every later read and write in the
    // process goes to that same in-memory state rather than re-parsing.
    let startup_config = state::config_writer::current();
    // A library link or theme file always opens the window: its dialog is
    // the point of the launch, even when "start minimized" is on.
    let should_start_minimized =
        !opened_by_link &&
        (cli.minimized || (startup_config.auto_start && startup_config.start_minimized));

    // Before anything shows the look: save a look of the user's own that no
    // theme holds yet as "My theme", point at the theme the look on screen
    // belongs to, and put back colors the colors list lost.
    // A failed save has queued a notice for the window.
    let _ = utils::theme_library::run_sync(utils::theme_library::SyncCause::Check);

    // A while after start, remove images this copy made that nothing has used
    // for a week. Not while the saved themes cannot be read: an image only
    // they name would look unused.
    if state::saved_themes::current().read_only.is_none() {
        utils::custom_images::clear_unused_images_later();
    }

    // Before the app state loads the sound pack list, so the packs brought over
    // from Mechvibes 2 are in it from the first frame.
    utils::legacy_migration::import_mechvibes2_packs();

    // Windows registers mechvibes:// and .mvtheme files in the installer and
    // the .deb in its desktop and MIME files. An AppImage installs nothing,
    // so it registers itself.
    #[cfg(target_os = "linux")]
    if libs::protocol::should_self_register() {
        if let Err(e) = libs::protocol::register_protocol() {
            always_eprint!("⚠️  Could not register mechvibes:// links and theme files: {}", e);
        }
    }

    // Initialize global app state before rendering
    state::app::init_app_state();
    state::app::init_update_state();

    // Drop an update record left by an older build that has since been
    // installed - otherwise the app advertises an update to the version it
    // is already running until the next check happens to go through.
    utils::auto_updater::clear_stale_available_version();

    // Pick up an installer staged by a previous session ("Later"). Re-hashes
    // the file before trusting it, and drops it if the app has since been
    // updated some other way.
    utils::auto_updater::restore_staged_update();

    // Report this launch, if the user has not opted out. Returns immediately
    // and does its work on a detached thread, so it runs before the audio
    // engine and the input worker start and never touches either. The input
    // worker process returned long before this line, and so did a duplicate
    // instance, so neither is ever counted as a launch.
    utils::telemetry::report_app_started(state::config_writer::current().enable_telemetry);

    // Initialize ambiance player
    state::ambiance::initialize_global_ambiance_player();
    debug_print!("🎵 Ambiance player initialized");

    // Note: Update service will be initialized within the UI components
    // to ensure proper Dioxus runtime context

    // Create input event channels for communication between input listeners
    // and the audio engine (crossbeam so the engine thread can `select!` on
    // these alongside its own AudioCommand channel without polling).
    let (keyboard_tx, keyboard_rx) = crossbeam_channel::unbounded::<String>();
    let (mouse_tx, mouse_rx) = crossbeam_channel::unbounded::<String>();
    let (hotkey_tx, hotkey_rx) = crossbeam_channel::unbounded::<String>();

    // Spawn the audio engine thread before the Dioxus/webview runtime starts.
    // The engine owns rodio's OutputStream exclusively on a plain OS thread
    // (OutputStream is not Send), and drives keyboard/mouse playback via a
    // blocking select!/recv() loop instead of the UI polling it every ~1ms.
    libs::audio::spawn_engine(keyboard_rx, mouse_rx, hotkey_rx);
    debug_print!("🎧 Audio engine thread started");

    // Initialize window focus state
    // If window starts visible (not minimized), it will be focused
    let initial_focus_state = !should_start_minimized;
    init_window_focus_state_with_value(initial_focus_state);
    debug_print!("🔍 Initial window focus state: {}", if initial_focus_state { "FOCUSED" } else { "UNFOCUSED" });

    // Start platform input capture. Shared with headless mode
    // (`libs::bootstrap`) so both launch paths get the same listeners - on
    // Windows that is the Raw Input worker process, with the rdev hybrid only
    // as its fallback.
    libs::bootstrap::start_input_capture_with_focus(
        keyboard_tx,
        mouse_tx,
        hotkey_tx,
        get_window_focus_state()
    );

    // Create window action channel
    let (window_tx, _window_rx) = mpsc::channel::<WindowAction>();
    WINDOW_MANAGER.set_action_sender(window_tx);

    // Window dimensions - allow vertical resizing.
    //
    // These are a starting point, not the final geometry: the window is
    // measured against its monitor's work area and shrunk to fit once it
    // exists (see `libs::window_bounds`), because an 820pt window is taller
    // than a 1366x768 laptop's usable screen.
    let (window_width, default_height) = libs::window_bounds::DEFAULT_WINDOW_SIZE;
    let min_height = libs::window_bounds::MIN_WINDOW_SIZE.1;

    // Load icon before creating window
    let window_icon = load_icon();
    if window_icon.is_none() {
        always_eprint!("⚠️ Warning: Failed to load window icon - taskbar icon may not appear");
    }

    // Create a WindowBuilder with custom appearance and vertical resizing
    // On Linux, disable transparency to ensure proper border-radius and shadow rendering
    #[cfg(target_os = "linux")]
    let window_builder = {
        debug_print!("🐧 Linux detected: Configuring window without transparency for proper CSS rendering");
        WindowBuilder::default()
            .with_title(APP_NAME)
            .with_transparent(false) // Disable transparency on Linux for better CSS rendering
            .with_always_on_top(false)
            .with_inner_size(LogicalSize::new(window_width, default_height))
            .with_min_inner_size(LogicalSize::new(window_width, min_height))
            .with_fullscreen(None)
            .with_decorations(false)
            .with_resizable(true)
            .with_visible(!should_start_minimized)
            .with_window_icon(window_icon)
    };

    // On Windows, enable transparency for custom window styling
    #[cfg(not(target_os = "linux"))]
    let window_builder = WindowBuilder::default()
        .with_title(APP_NAME)
        .with_transparent(true) // Enable transparency for custom window styling
        .with_always_on_top(false) // Allow normal window behavior for taskbar
        .with_inner_size(LogicalSize::new(window_width, default_height))
        .with_min_inner_size(LogicalSize::new(window_width, min_height))
        .with_fullscreen(None)
        .with_decorations(false) // Use custom title bar
        .with_resizable(true) // Enable vertical resizing
        .with_visible(!should_start_minimized) // Hide window if starting minimized
        .with_window_icon(window_icon); // Set window icon for taskbar

    // Create config with our window settings and custom protocol handlers
    let config = Config::new()
        .with_window(window_builder)
        .with_menu(None);

    // Launch the app with our config
    dioxus::LaunchBuilder::desktop().with_cfg(config).launch(app_with_stylesheets)
}

fn app_with_stylesheets() -> Element {
    rsx! {
        ui::app {}
    }
}
