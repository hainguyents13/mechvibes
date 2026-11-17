use crate::libs::window_manager::WINDOW_MANAGER;
use crate::utils::constants::APP_NAME;
use crate::state::app::use_update_info;
use dioxus::desktop::use_window;
use dioxus::prelude::*;
use lucide_dioxus::{ Minus, X, EyeClosed, Download };

/// A custom title bar component that allows for window dragging and includes minimize/close buttons
#[component]
pub fn TitleBar() -> Element {
    let window = use_window();
    let window_drag = window.clone();
    let update_info = use_update_info();
    // Read at render time, never inside a memo: the config signal this reads is
    // what re-renders the bar when the language changes.
    let strings = crate::i18n::use_strings();

    // Get current version for display
    let current_version = crate::utils::constants::APP_VERSION;

    let start_drag = move |_| {
        window_drag.drag();
    };

    // Function to minimize window to tray
    let minimize_to_tray = move |_| {
        // Hide the window to system tray
        WINDOW_MANAGER.hide();
        crate::always_print!("🔽 Window minimized to system tray");
    };

    // Function to close application
    let window_close = window.clone();
    let close = move |_| {
        window_close.close();
    };

    // Function to minimize application
    let window_minimize = window.clone();
    let minimize = move |_| {
        window_minimize.set_minimized(true);
    };
    rsx! {
      div { class: "fixed inset-0 h-10 z-999 flex justify-between items-center select-none gap-0 bg-gradient-to-b from-base-300/70 to-transparent backdrop-blur-sm transition-all ",
      // Left side - app title and draggable area
        div {
          class: "flex items-center grow cursor-move px-3 py-2",
          onmousedown: start_drag,
          // App title
          span { class: "text-sm font-semibold text-base-content", "{APP_NAME}" }
          // Optional version badge
          span { class: "ml-2 text-xs bg-base-300 text-base-content/50 px-1.5 py-0.5 rounded",
            "v{current_version}"
          }
        }


        // Right side - window controls
        div { class: "flex items-center space-x-2 px-3 py-2 pl-0",
          // Update notification (separate from draggable area)
          if let Some(update) = update_info.clone() {
            if update.update_available {
              div {
                class: "tooltip tooltip-bottom ml-2",
                "data-tip": crate::i18n::fill(strings.chrome.new_version_available, &[&update.latest_version]),
                button {
                  class: "btn btn-success btn-xs",
                  // Notification only: this routes to Settings, where the
                  // single install control lives. It deliberately no longer
                  // opens the raw .exe in a browser - that would bypass the
                  // SHA256 verification the in-app path performs, and on
                  // Linux/macOS there is no installable asset to open at all.
                  onclick: move |_| {
                    crate::libs::routes::navigate("/settings");
                  },
                  Download { class: "w-3 h-3" }
                  "{update.latest_version}"
                }
              }
            }
          }
          // Minimize to taskbar button
          div {
            class: "tooltip tooltip-bottom",
            "data-tip": "{strings.chrome.minimize}",
            button {
              class: "p-1.5 rounded-box hover:bg-neutral/70 text-base-content/70 hover:text-neutral-content transition-colors",
              onclick: minimize,
              Minus { class: "w-4 h-4" }
            }
          }
          // Minimize to tray button
          div {
            class: "tooltip tooltip-bottom",
            "data-tip": "{strings.chrome.hide_to_tray}",
            button {
              class: "p-1.5 rounded-box hover:bg-neutral/70 text-base-content/70 hover:text-neutral-content transition-colors",
              onclick: minimize_to_tray,
              EyeClosed { class: "w-4 h-4" }
            }
          }
          // Close button
          div { class: "tooltip tooltip-bottom", "data-tip": "{strings.chrome.quit}",
            button {
              class: "p-1.5 rounded-box hover:bg-error text-base-content/70 hover:text-error-content transition-colors",
              title: "{strings.common.close}",
              onclick: close,
              X { class: "w-4 h-4" }
            }
          }
        }
      }
    }
}
