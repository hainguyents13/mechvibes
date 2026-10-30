use crate::state::{ app::use_app_state };
use dioxus::prelude::*;
use lucide_dioxus::{ ExternalLink, FolderOpen, RefreshCcw };
use std::sync::Arc;

#[component]
pub fn SoundpackManager(on_import_click: EventHandler<MouseEvent>) -> Element {
    let app_state = use_app_state();
    let audio_ctx: Arc<crate::libs::audio::AudioContext> = use_context();
    let state_trigger = crate::state::app::use_state_trigger(); // UI state for notification and loading
    let refreshing_soundpacks = use_signal(|| false);
    let refresh_soundpacks_cache = {
        let audio_ctx_refresh = audio_ctx.clone();
        let mut refreshing_soundpacks = refreshing_soundpacks.clone();
        let state_trigger_clone = state_trigger.clone();
        Callback::new(move |_| {
            // Prevent multiple concurrent refreshes
            if refreshing_soundpacks() {
                crate::always_print!("🔄 Refresh already in progress, skipping...");
                return;
            }

            crate::always_print!("🔄 Refresh button clicked!");
            // Set loading state to true
            refreshing_soundpacks.set(true);
            // Clone necessary variables for the async task
            let mut refreshing_signal = refreshing_soundpacks.clone();
            let audio_ctx_clone = audio_ctx_refresh.clone();
            let trigger = state_trigger_clone.clone();

            // Perform the refresh operation in a separate task to not block the UI
            spawn(async move {
                // Use async sleep instead of std::thread::sleep
                use futures_timer::Delay;
                use std::time::Duration;

                Delay::new(Duration::from_millis(100)).await;
                crate::always_print!("🔄 Starting soundpack refresh operation...");

                // Use the state trigger to refresh cache and update UI
                // This will automatically update the count as well
                crate::always_print!("🔄 Calling state trigger...");
                trigger.call(());
                crate::always_print!("🔄 State trigger called successfully");

                // Reload current soundpacks to apply any changes
                crate::always_print!("🔄 Reloading current soundpacks...");
                crate::state::app::reload_current_soundpacks(&audio_ctx_clone);

                // Add another small delay before changing the loading state back
                Delay::new(Duration::from_millis(100)).await;
                // Reset loading state
                refreshing_signal.set(false);
                crate::always_print!("✅ Soundpack refresh complete");
            });
        })
    };

    // Get current counts from cache
    let soundpack_count_keyboard = app_state.optimized_cache.count.keyboard;
    let soundpack_count_mouse = app_state.optimized_cache.count.mouse;

    rsx! {
      div { class: "space-y-4",
        div { class: "text-base-content",
          div {
            div { class: "font-medium text-sm pb-1",
              if soundpack_count_keyboard + soundpack_count_mouse == 0 {
                "Click refresh to scan for sound packs"
              } else {
                "Found {soundpack_count_keyboard + soundpack_count_mouse} sound pack(s)"
              }
            }
            if soundpack_count_keyboard + soundpack_count_mouse > 0 {
              ul { class: "list-disc pl-6",
                li { class: "text-sm text-base-content/70",
                  "Keyboard: {soundpack_count_keyboard}"
                }
                li { class: "text-sm text-base-content/70",
                  "Mouse: {soundpack_count_mouse}"
                }
              }
            }
          }
        }
        div { class: "space-y-2",
          div { class: "text-base-content/70 text-sm",
            "Refresh sound pack list to detect newly added or removed sound packs."
          }
          div { class: "flex items-center gap-4",
            button {
              class: "btn  btn-soft btn-sm",
              onclick: refresh_soundpacks_cache,
              disabled: refreshing_soundpacks(),
              if refreshing_soundpacks() {
                span { class: "loading loading-spinner loading-xs mr-2" }
                "Refreshing..."
              } else {
                RefreshCcw { class: "w-4 h-4 mr-1" }
                "Refresh"
              }
            } // Last scan info
            if app_state.optimized_cache.last_scan > 0 {
              div { class: "text-xs text-base-content/60",
                "Last scan: {crate::utils::time::format_relative_time(app_state.optimized_cache.last_scan)}"
              }
            }
          }
        }
        div { class: "divider" }
        div { class: "space-y-2",
          div { class: "text-base-content font-medium text-sm", "Built-in sound packs folder" }
          div { class: "text-sm text-base-content/70",
            "Default sound packs that ship with the app."
          }
          div { class: "flex gap-2",
            button {
              class: "btn btn-soft btn-sm",
              onclick: move |_| {
                  let builtin_keyboard_dir = crate::state::paths::soundpacks::get_builtin_soundpacks_dir().join("keyboard");
                  crate::always_print!("🔍 Opening built-in keyboard soundpacks folder: {}", builtin_keyboard_dir.display());

                  // Check if directory exists
                  if !builtin_keyboard_dir.exists() {
                      crate::always_print!("❌ Built-in keyboard soundpacks directory not found: {}", builtin_keyboard_dir.display());
                      crate::always_print!("   This might be expected on Linux installations.");
                      crate::always_print!("   Trying parent directory instead...");

                      // Try opening parent directory (soundpacks root)
                      let soundpacks_root = crate::state::paths::soundpacks::get_builtin_soundpacks_dir();
                      if soundpacks_root.exists() {
                          let _ = crate::utils::path::open_path(&soundpacks_root.to_string_lossy());
                      } else {
                          crate::always_print!("❌ Soundpacks root directory also not found: {}", soundpacks_root.display());
                      }
                  } else {
                      let _ = crate::utils::path::open_path(&builtin_keyboard_dir.to_string_lossy());
                  }
              },
              FolderOpen { class: "w-4 h-4 mr-1" }
              "Keyboard"
            }
            button {
              class: "btn btn-soft btn-sm",
              onclick: move |_| {
                  let builtin_mouse_dir = crate::state::paths::soundpacks::get_builtin_soundpacks_dir().join("mouse");
                  crate::always_print!("🔍 Opening built-in mouse soundpacks folder: {}", builtin_mouse_dir.display());

                  // Check if directory exists
                  if !builtin_mouse_dir.exists() {
                      crate::always_print!("❌ Built-in mouse soundpacks directory not found: {}", builtin_mouse_dir.display());
                      crate::always_print!("   This might be expected on Linux installations.");
                      crate::always_print!("   Trying parent directory instead...");

                      // Try opening parent directory (soundpacks root)
                      let soundpacks_root = crate::state::paths::soundpacks::get_builtin_soundpacks_dir();
                      if soundpacks_root.exists() {
                          let _ = crate::utils::path::open_path(&soundpacks_root.to_string_lossy());
                      } else {
                          crate::always_print!("❌ Soundpacks root directory also not found: {}", soundpacks_root.display());
                      }
                  } else {
                      let _ = crate::utils::path::open_path(&builtin_mouse_dir.to_string_lossy());
                  }
              },
              FolderOpen { class: "w-4 h-4 mr-1" }
              "Mouse"
            }
          }
        }
        div { class: "divider" }
        div { class: "space-y-2",
          div { class: "text-base-content font-medium text-sm", "Custom sound packs folder" }
          div { class: "text-sm text-base-content/70",
            "Add your own custom sound packs here."
          }
          div { class: "flex gap-2",
            button {
              class: "btn btn-soft btn-sm",
              onclick: move |_| {
                  let custom_keyboard_dir = crate::state::paths::soundpacks::get_custom_soundpacks_dir().join("keyboard");
                  // Create the directory if it doesn't exist
                  let _ = std::fs::create_dir_all(&custom_keyboard_dir);
                  let _ = crate::utils::path::open_path(&custom_keyboard_dir.to_string_lossy());
              },
              FolderOpen { class: "w-4 h-4 mr-1" }
              "Keyboard"
            }
            button {
              class: "btn btn-soft btn-sm",
              onclick: move |_| {
                  let custom_mouse_dir = crate::state::paths::soundpacks::get_custom_soundpacks_dir().join("mouse");
                  // Create the directory if it doesn't exist
                  let _ = std::fs::create_dir_all(&custom_mouse_dir);
                  let _ = crate::utils::path::open_path(&custom_mouse_dir.to_string_lossy());
              },
              FolderOpen { class: "w-4 h-4 mr-1" }
              "Mouse"
            }
          }
        }
        div { class: "divider" }
        div { class: "space-y-3",
          div { class: "text-base-content font-medium text-sm", "Need more sound packs?" }
          div { class: "text-sm text-base-content/70",
            "Check out the Mechvibes website to find more sound packs. You can also create your own sound packs using the Sound Pack Editor."
          }
          div { class: "flex items-center gap-2",
            a {
              class: "btn btn-soft btn-sm",
              href: "https://mechvibes.com/sound-packs?utm_source=mechvibes&utm_medium=app&utm_campaign=soundpack_manager",
              target: "_blank",
              "Browse sound packs"
              ExternalLink { class: "w-4 h-4 ml-1" }
            }
            a {
              class: "btn btn-soft btn-sm",
              href: "https://editor.mechvibes.com/?utm_source=mechvibes&utm_medium=app&utm_campaign=soundpack_manager",
              target: "_blank",
              "Open Editor"
              ExternalLink { class: "w-4 h-4 ml-1" }
            }
          }
        }
      }
    }
}
