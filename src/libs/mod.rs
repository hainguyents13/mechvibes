pub mod audio;
pub mod bootstrap;
pub mod cli_args;
pub mod device_manager;
pub mod focused_input_listener;
pub mod headless;
pub mod input_device_manager;
pub mod input_listener;
pub mod input_manager;
pub mod protocol;
pub mod routes;
pub mod theme;
pub mod trace;
pub mod tray;
pub mod tray_service;
pub mod ui;
pub mod window_bounds;
pub mod window_manager;

#[cfg(target_os = "linux")]
pub mod evdev_input_listener;

#[cfg(target_os = "macos")]
pub mod native_window;

// Windows input worker: Raw Input capture runs in a separate process
// (`input_worker`, driven by `rawinput_listener`) because tao/wry claims the
// process-wide Raw Input registration in the UI process. The UI side spawns
// and reads it via `input_worker_host`.
#[cfg(target_os = "windows")]
pub mod console_attach;
#[cfg(target_os = "windows")]
pub mod input_worker;
#[cfg(target_os = "windows")]
pub mod input_worker_host;
#[cfg(target_os = "windows")]
pub mod rawinput_listener;
#[cfg(target_os = "windows")]
pub mod single_instance;

// Re-export main audio types
pub use audio::AudioContext;
