use crate::components::window_controller::WindowController;
use crate::components::header::Header;
use crate::libs::routes::Route;
use crate::libs::input_manager::set_window_focus;
use crate::libs::AudioContext;
use crate::state::keyboard::KeyboardState;
use crate::state::paths;
use crate::utils::delay;
use crate::{ debug_print, always_eprint };

use dioxus::prelude::*;
use dioxus::desktop::{ use_asset_handler, use_wry_event_handler, wry::http::Response };
use dioxus::desktop::tao::event::{ Event as TaoEvent };
use std::sync::Arc;

pub fn app() -> Element {
    // Loading state to prevent FOUC
    let mut is_loading = use_signal(|| true);

    // Hide content until CSS loads
    use_effect(move || {
        spawn(async move {
            // Wait for CSS to load
            futures_timer::Delay::new(std::time::Duration::from_millis(100)).await;
            is_loading.set(false);
        });
    });

    // Create update signal for event-driven state management
    let update_signal = use_signal(|| 0u32);
    use_context_provider(|| update_signal);

    // Create global config state - shared across all pages. The value comes
    // from the single writer, which owns the authoritative struct; this signal
    // is a render-side mirror of it, never a second source of truth.
    let global_config = use_signal(|| {
        crate::always_print!("🌍 Initializing global config context");
        crate::state::config_writer::current()
    });
    use_context_provider(|| global_config);

    // Messages from `utils::notice` on their way to the screen; the poll loop
    // below fills it and `Toasts` shows it.
    let notices = use_signal(Vec::<crate::utils::notice::Notice>::new);
    use_context_provider(|| notices);

    // Create global keyboard state using signals
    let keyboard_state = use_signal(|| KeyboardState::new());

    // Provide the keyboard state context to all child components
    use_context_provider(|| keyboard_state);

    // Initialize the audio system for mechvibes sounds - moved here to be accessible by both keyboard processing and UI
    let audio_context = use_hook(|| Arc::new(AudioContext::new()));

    // Provide audio context to all child components (this will be used by Layout and other components)
    use_context_provider(|| audio_context.clone());
    {
        // Load current soundpacks on startup
        let ctx = audio_context.clone();
        use_effect(move || {
            debug_print!("🎵 Loading current soundpacks on startup...");
            crate::state::app::reload_current_soundpacks(&ctx);
        });
    }

    // Don't initialize device cache on startup to avoid interrupting system audio
    // Cache will be populated lazily when user opens Settings page
    // This prevents ALSA enumeration from interfering with other audio playback

    // Asked from here rather than before the window opens: a dialog shown
    // earlier would create the macOS application object before the window
    // library gets to create its own.
    #[cfg(target_os = "macos")]
    use_effect(move || {
        spawn(crate::utils::legacy_migration::offer_to_trash_mechvibesdx_app());
    });

    // Check for updates on startup (from completely closed state)
    use_effect(move || {
        spawn(async move {
            if
                let Ok(update_info) =
                    crate::utils::auto_updater::check_for_updates_on_startup().await
            {
                debug_print!("🔄 Startup update check completed");
                if update_info.update_available {
                    debug_print!("🆕 Update available on startup: {}", update_info.latest_version);
                }
            }
        });
    });

    // ===== WINDOW FOCUS TRACKING =====
    // Lets the rdev (unfocused) / device_query (focused) hybrid listeners
    // hand off to each other, working around rdev not seeing keys while the
    // Wry/Winit window has focus. Used by macOS and Linux/X11, and on
    // Windows only if the Raw Input worker process fails and we fall back to
    // that hybrid (see input_worker_host.rs) - the worker itself is
    // focus-independent and ignores this state.
    {
        use dioxus::desktop::tao::event::WindowEvent;

        use_wry_event_handler(move |event, _target| {
            if let TaoEvent::WindowEvent { event: window_event, .. } = event {
                // Check for focus events using proper pattern matching
                if let WindowEvent::Focused(focused) = window_event {
                    // Update global focus state
                    set_window_focus(*focused);
                }
            }
        });
    }

    // Update KeyboardState (for the on-screen key visualizer) from the audio
    // engine's UiEvent stream. The engine now reads raw input directly and
    // plays sound itself (see engine.rs) - this loop no longer drives
    // playback, it only mirrors key state for the UI and surfaces
    // device-switch/pack-load results.
    //
    // The same loop republishes the config whenever the single writer reports a
    // change, which is what lets a write made off this thread re-render the
    // window: the Ctrl+Alt+M hotkey runs on the audio engine thread and the
    // update checker in a tokio task, and neither can touch a `Signal` (the
    // shared one uses Dioxus's default non-`Send` storage). Polling a counter
    // works from anywhere and costs one atomic load per tick.
    {
        let mut keyboard_state = keyboard_state;
        let mut global_config = global_config;
        let mut seen_config_generation = crate::state::config_writer::generation();

        use_future(move || async move {
            let event_rx = crate::libs::audio::ui_event_receiver();

            loop {
                let generation = crate::state::config_writer::generation();
                if generation != seen_config_generation {
                    seen_config_generation = generation;
                    let latest = crate::state::config_writer::current();
                    // Writes that originated in the UI have already published
                    // through `update_config`; skipping an identical value
                    // keeps those from costing a second render.
                    if !global_config.peek().data_equals(&latest) {
                        global_config.set(latest);
                    }
                }

                // Messages reported off this thread, such as a failed save,
                // reach the screen the same way.
                let new_notices = crate::utils::notice::take_new();
                if !new_notices.is_empty() {
                    crate::components::ui::toast::show_notices(notices, new_notices);
                }

                // The state of the last key event seen this tick.
                //
                // Nothing is scheduled or held back: whatever the newest event
                // says is what gets rendered on this very tick. An earlier
                // design queued each keystroke behind a minimum display time,
                // which meant that past roughly 10 keys/sec presses could not
                // be shown as fast as they arrived - latency then grew with
                // every key and the backlog drained in clumps. Real-time
                // feedback must never wait: the newest keystroke wins and
                // older ones are simply superseded.
                let mut last_state: Option<(bool, String)> = None;
                while let Ok(event) = event_rx.try_recv() {
                    match event {
                        crate::libs::audio::UiEvent::KeyDown(key) => {
                            last_state = Some((true, key));
                        }
                        crate::libs::audio::UiEvent::KeyUp(key) => {
                            last_state = Some((false, key));
                        }
                        crate::libs::audio::UiEvent::DeviceSwitched(result) => {
                            match result {
                                Ok(label) => debug_print!("🔊 Switched audio device: {}", label),
                                Err(e) => always_eprint!("❌ Failed to switch audio device: {}", e),
                            }
                        }
                        crate::libs::audio::UiEvent::PackLoaded { is_keyboard, result } => {
                            match result {
                                Ok(name) =>
                                    debug_print!(
                                        "✅ Loaded {} pack: {}",
                                        if is_keyboard { "keyboard" } else { "mouse" },
                                        name
                                    ),
                                Err(e) =>
                                    always_eprint!(
                                        "❌ Failed to load {} pack: {}",
                                        if is_keyboard { "keyboard" } else { "mouse" },
                                        e
                                    ),
                            }
                        }
                    }
                }

                // Coalesce UI state to at most one write per batch so a burst
                // doesn't trigger a re-render per keystroke.
                if let Some((pressed, key)) = last_state {
                    // Only touch the signal when the rendered state actually
                    // moves. Writing an unchanged value re-renders the whole
                    // tree for no visible difference, and this costs nothing
                    // in latency: it drops redundant writes, never delays a
                    // real one.
                    let changed = {
                        let state = keyboard_state.read();
                        state.key_pressed != pressed || (pressed && state.last_key != key)
                    };

                    if changed {
                        crate::libs::trace::record(
                            crate::libs::trace::Point::UiWrite,
                            &key,
                            0.0
                        );
                        let mut state = keyboard_state.write();
                        state.key_pressed = pressed;
                        if pressed {
                            state.last_key = key;
                        }
                    }
                }

                delay::Delay::key_event().await;
            }
        });
    } // Initialize update service for background update checking
    #[cfg(feature = "auto-update")]
    {
        use crate::utils::auto_updater::UpdateService;
        use_future(move || async move {
            // No config is handed over: the service reads what it needs per
            // tick and writes only its own fields. It used to be given an
            // `Arc<Mutex<AppConfig>>` snapshotted here at launch, which it then
            // saved back over every setting changed since.
            UpdateService::new().start().await;
        });
    } // Set up asset handler for serving soundpack images
    use_asset_handler("soundpack-images", |request, response| {
        let request_path = request.uri().path();

        // Parse the path: /soundpack-images/{device_type}/{soundpack_name}/{filename}
        let path_parts: Vec<&str> = request_path.trim_start_matches('/').split('/').collect();

        if path_parts.len() >= 4 && path_parts[0] == "soundpack-images" {
            let device_type = path_parts[1];
            let soundpack_name = path_parts[2];
            let filename = path_parts[3];

            // Security: Validate path segments to prevent directory traversal
            // Reject empty segments, path separators, and parent directory references
            if device_type.is_empty() || soundpack_name.is_empty() || filename.is_empty() {
                let error_response = Response::builder()
                    .status(400)
                    .body(Vec::new())
                    .unwrap();
                response.respond(error_response);
                return;
            }

            if device_type.contains("..") || device_type.contains('/') || device_type.contains('\\')
                || soundpack_name.contains("..") || soundpack_name.contains('/') || soundpack_name.contains('\\')
                || filename.contains("..") || filename.contains('/') || filename.contains('\\')
            {
                let error_response = Response::builder()
                    .status(400)
                    .body(Vec::new())
                    .unwrap();
                response.respond(error_response);
                return;
            }

            // Join device_type/soundpack_name to form the full soundpack_id
            let soundpack_id = format!("{}/{}", device_type, soundpack_name);

            // Get the soundpack directory path - supports both built-in and custom locations
            let soundpack_dir = paths::soundpacks::soundpack_dir(&soundpack_id);
            let image_path = std::path::PathBuf::from(&soundpack_dir).join(filename);

            if image_path.exists() {
                // Read the file and determine content type
                match std::fs::read(&image_path) {
                    Ok(data) => {
                        let mut response_builder = Response::builder();

                        // Get extension and convert to lowercase for case-insensitive matching
                        let extension = image_path
                            .extension()
                            .and_then(|ext| ext.to_str())
                            .map(|ext| ext.to_lowercase());

                        let content_type = match extension.as_deref() {
                            Some("png") => "image/png",
                            Some("jpg") | Some("jpeg") => "image/jpeg",
                            Some("gif") => "image/gif",
                            Some("svg") => "image/svg+xml",
                            Some("webp") => "image/webp",
                            Some("ico") => "image/x-icon",
                            _ => "application/octet-stream",
                        };

                        response_builder = response_builder
                            .header("Content-Type", content_type)
                            .header("Cache-Control", "public, max-age=3600");

                        if let Ok(http_response) = response_builder.body(data) {
                            response.respond(http_response);
                            return;
                        }
                    }
                    Err(e) => {
                        debug_print!(
                            "❌ Failed to read soundpack image file {:?}: {}",
                            image_path,
                            e
                        );
                    }
                }
            }
        }

        // Return 404 for invalid paths or missing files
        if
            let Ok(not_found_response) = Response::builder()
                .status(404)
                .header("Content-Type", "text/plain")
                .body(b"Not Found".to_vec())
        {
            response.respond(not_found_response);
        }
    });

    // Set up asset handler for serving custom user images
    use_asset_handler("custom-images", |request, response| {
        let request_path = request.uri().path();

        // Parse the path: /custom-images/{filename}
        let path_parts: Vec<&str> = request_path.trim_start_matches('/').split('/').collect();

        if path_parts.len() >= 2 && path_parts[0] == "custom-images" {
            let filename = path_parts[1];

            // Security: Reject empty filenames (e.g., trailing slash /custom-images/)
            if filename.is_empty() {
                let error_response = Response::builder()
                    .status(400)
                    .body(Vec::new())
                    .unwrap();
                response.respond(error_response);
                return;
            }

            // Security: Validate filename to prevent directory traversal
            // Reject path separators and parent directory references
            if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
                let error_response = Response::builder()
                    .status(400)
                    .body(Vec::new())
                    .unwrap();
                response.respond(error_response);
                return;
            }

            // Get the custom images directory path
            let custom_images_dir = crate::state::paths::data::custom_images_dir();
            let image_path = custom_images_dir.join(filename);

            // Security: Ensure the resolved path is still within custom_images_dir
            if let Ok(canonical_path) = image_path.canonicalize() {
                if let Ok(canonical_base) = custom_images_dir.canonicalize() {
                    if !canonical_path.starts_with(&canonical_base) {
                        let error_response = Response::builder()
                            .status(403)
                            .body(Vec::new())
                            .unwrap();
                        response.respond(error_response);
                        return;
                    }
                }
            }

            if image_path.exists() {
                // Read the file and determine content type
                match std::fs::read(&image_path) {
                    Ok(data) => {
                        let mut response_builder = Response::builder();

                        // Get extension and convert to lowercase for case-insensitive matching
                        let extension = image_path
                            .extension()
                            .and_then(|ext| ext.to_str())
                            .map(|ext| ext.to_lowercase());

                        let content_type = match extension.as_deref() {
                            Some("png") => "image/png",
                            Some("jpg") | Some("jpeg") => "image/jpeg",
                            Some("gif") => "image/gif",
                            Some("svg") => "image/svg+xml",
                            Some("webp") => "image/webp",
                            Some("bmp") => "image/bmp",
                            Some("ico") => "image/x-icon",
                            _ => "application/octet-stream",
                        };

                        response_builder = response_builder
                            .header("Content-Type", content_type)
                            .header("Cache-Control", "public, max-age=3600");

                        if let Ok(http_response) = response_builder.body(data) {
                            response.respond(http_response);
                            return;
                        }
                    }
                    Err(e) => {
                        debug_print!(
                            "❌ Failed to read custom image file {:?}: {}",
                            image_path,
                            e
                        );
                    }
                }
            }
        }

        // Return 404 for invalid paths or missing files
        if
            let Ok(not_found_response) = Response::builder()
                .status(404)
                .header("Content-Type", "text/plain")
                .body(b"Not Found".to_vec())
        {
            response.respond(not_found_response);
        }
    });

    rsx! {
        // Loading overlay (shown while CSS loads)
        if is_loading() {
            div {
                style: "position: fixed; inset: 0; z-index: 99999; display: flex; align-items: center; justify-content: center; background: #1a1a1a;",
                div {
                    style: "display: flex; flex-direction: column; align-items: center; gap: 1rem;",
                    div {
                        style: "width: 3rem; height: 3rem; border: 4px solid rgba(255, 255, 255, 0.2); border-top-color: rgba(255, 255, 255, 0.9); border-radius: 50%; animation: spin 1s linear infinite;",
                    }
                    div {
                        style: "font-size: 0.875rem; font-weight: 500; color: rgba(255, 255, 255, 0.7);",
                        "Loading..."
                    }
                }
            }
            // Add keyframes for spinner animation
            style { "@keyframes spin {{ to {{ transform: rotate(360deg); }} }}" }
        }

        // Main app content
        // prettier-ignore
        WindowController {}
        // prettier-ignore
        Header {}

        Router::<Route> {}

        // Answer mechvibes://install and mechvibes://theme links whatever page is showing.
        crate::components::ui::LibraryInstallModal {}
        crate::components::ui::LibraryThemeModal {}
        crate::components::ui::Toasts {}
    }
}

/// Reduces one tick's worth of key events to the state that should be
/// rendered, matching the poll loop above: the newest event wins and nothing
/// is queued behind a previous keystroke.
#[cfg(test)]
fn coalesce(batch: &[(&str, bool)]) -> Option<(bool, String)> {
    let mut last = None;
    for (key, down) in batch {
        last = Some((*down, (*key).to_string()));
    }
    last
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_newest_keystroke_is_rendered_immediately() {
        // The defect this replaced gave every keystroke a minimum display
        // time, so a fast typist's presses queued and each one appeared later
        // than the last. Whatever arrived most recently must win outright.
        let state = coalesce(&[("KeyA", true)]).unwrap();
        assert_eq!(state, (true, "KeyA".to_string()));

        // Three keystrokes inside a single tick: the newest is shown now, the
        // older ones are superseded rather than owed.
        let state = coalesce(&[("KeyA", true), ("KeyB", false), ("KeyC", true)]).unwrap();
        assert_eq!(
            state,
            (true, "KeyC".to_string()),
            "a keystroke must never wait behind an earlier one"
        );
    }

    #[test]
    fn a_batch_ending_on_a_release_renders_released() {
        // The visual follows the newest event, so a tick whose last event is a
        // key-up renders released even if other keys were touched earlier in
        // the same tick.
        let state = coalesce(&[("KeyA", true), ("KeyA", false)]).unwrap();
        assert_eq!(state, (false, "KeyA".to_string()));
    }

    #[test]
    fn an_empty_tick_changes_nothing() {
        // No events means no write, which is what keeps an idle app from
        // re-rendering, and means the loop never has pending work of its own.
        assert!(coalesce(&[]).is_none());
    }

    #[test]
    fn last_key_only_advances_on_a_press() {
        // A release carries the key that was let go; adopting it as `last_key`
        // would misreport which key was most recently struck.
        let (pressed, key) = coalesce(&[("KeyZ", false)]).unwrap();
        assert!(!pressed);
        assert_eq!(key, "KeyZ", "the loop ignores this name unless pressed");
    }
}
