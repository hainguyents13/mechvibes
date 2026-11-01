use crate::state::paths;
use crate::state::soundpack::SoundpackMetadata;
use crate::state::soundpack_library::SoundpackSort;
use crate::state::{ app::use_state_trigger };
use crate::utils::config::use_config;
use crate::utils::path::{ open_path, directory_exists };
use dioxus::document::eval;
use dioxus::prelude::*;
use lucide_dioxus::{ ArrowDownAZ, Clock, FolderOpen, Plus, RefreshCw, Trash };
use std::sync::Arc;

use super::{ ConfirmDeleteModal, PackCover };

/// Open a soundpack folder in the system file manager
/// Opens the specific soundpack folder
fn open_soundpack_folder(soundpack_id: &str) -> Result<(), String> {
    use std::path::PathBuf;

    let soundpack_path = paths::soundpacks::soundpack_dir(soundpack_id);

    // Normalize path separators for Windows
    let normalized_path = PathBuf::from(&soundpack_path);
    let normalized_str = normalized_path.to_string_lossy().to_string();

    crate::always_print!("🔍 Opening soundpack folder:");
    crate::always_print!("   Soundpack ID: {}", soundpack_id);
    crate::always_print!("   Resolved path: {}", soundpack_path);
    crate::always_print!("   Normalized path: {}", normalized_str);

    // Check if path exists
    if !normalized_path.exists() {
        return Err(format!("Soundpack folder does not exist: {}", normalized_str));
    }

    open_path(&normalized_str).map_err(|e| format!("Failed to open soundpack folder: {}", e))
}

/// Delete a soundpack directory and all its contents
fn delete_soundpack(soundpack_id: &str) -> Result<(), String> {
    let soundpack_path = paths::soundpacks::soundpack_dir(soundpack_id);

    // Check if the directory exists
    if !directory_exists(&soundpack_path) {
        return Err(format!("Soundpack directory not found: {}", soundpack_path));
    }

    // Remove the entire directory
    std::fs
        ::remove_dir_all(&soundpack_path)
        .map_err(|e| format!("Failed to delete soundpack directory: {}", e))?;

    crate::always_print!("🗑️ Successfully deleted soundpack: {}", soundpack_id);
    Ok(())
}

#[component]
pub fn SoundpackTable(
    soundpacks: Vec<SoundpackMetadata>,
    soundpack_type: &'static str,
    on_add_click: Option<EventHandler<MouseEvent>>
) -> Element {
    // Search state
    let mut search_query = use_signal(String::new);
    let mut sort_order = use_signal(|| SoundpackSort::RecentlyAdded);

    // Refresh state
    let refreshing_soundpacks = use_signal(|| false);
    let state_trigger = use_state_trigger();
    let audio_ctx: Arc<crate::libs::audio::AudioContext> = use_context();
    let (config, _update_config) = use_config();

    // Filter soundpacks based on search query - computed every render to be reactive to props changes
    let query = search_query().to_lowercase();
    let mut filtered_soundpacks: Vec<SoundpackMetadata> = if query.is_empty() {
        soundpacks.clone()
    } else {
        soundpacks
            .iter()
            .filter(|pack| {
                pack.name.to_lowercase().contains(&query) ||
                    pack.id.to_lowercase().contains(&query) ||
                    pack.author
                        .as_ref()
                        .map_or(false, |author| author.to_lowercase().contains(&query)) ||
                    pack.tags.iter().any(|tag| tag.to_lowercase().contains(&query))
            })
            .cloned()
            .collect()
    };

    let seen_soundpacks = config().soundpacks_seen.clone();
    match sort_order() {
        SoundpackSort::Name => {
            filtered_soundpacks.sort_by(|a, b|
                a.name.to_lowercase().cmp(&b.name.to_lowercase())
            );
        }
        SoundpackSort::RecentlyAdded => {
            // `soundpack_added_at` rather than `last_modified`, which is the
            // folder's mtime and moves whenever a file inside the pack is
            // edited. Packs with no timestamp sort last, then by name so the
            // order is stable rather than arbitrary.
            let added_at = config().soundpack_added_at;
            filtered_soundpacks.sort_by(|a, b| {
                let a_added = added_at.get(&a.folder_path).copied().unwrap_or(0);
                let b_added = added_at.get(&b.folder_path).copied().unwrap_or(0);
                b_added
                    .cmp(&a_added)
                    .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            });
        }
    }

    // Refresh handler
    let refresh_soundpacks_cache = {
        let audio_ctx_refresh = audio_ctx.clone();
        let refreshing_soundpacks = refreshing_soundpacks.clone();
        let state_trigger_clone = state_trigger.clone();
        Callback::new(move |_| {
            // Prevent multiple concurrent refreshes
            if refreshing_soundpacks() {
                crate::always_print!("🔄 Refresh already in progress, skipping...");
                return;
            }

            let audio_ctx = audio_ctx_refresh.clone();
            let mut refreshing_soundpacks = refreshing_soundpacks.clone();
            let state_trigger = state_trigger_clone.clone();

            spawn(async move {
                refreshing_soundpacks.set(true);
                crate::always_print!("🔄 Refreshing soundpack cache...");

                // Reload soundpacks in audio context
                crate::state::app::reload_current_soundpacks(&audio_ctx);

                // Trigger state update to refresh UI
                state_trigger.call(());

                crate::always_print!("✅ Soundpack cache refreshed");
                refreshing_soundpacks.set(false);
            });
        })
    };

    rsx! {
      div { class: "space-y-4",
        // Search field
        div { class: "flex items-center px-3 gap-2",
          input {
            class: "input input-sm w-full",
            placeholder: "Search {soundpack_type.to_lowercase()} sound packs...",
            value: "{search_query}",
            oninput: move |evt| search_query.set(evt.value()),
          }
          button {
            class: "btn btn-sm btn-ghost",
            onclick: move |_| {
                sort_order
                    .set(match sort_order() {
                        SoundpackSort::RecentlyAdded => SoundpackSort::Name,
                        SoundpackSort::Name => SoundpackSort::RecentlyAdded,
                    });
            },
            title: "Sorted by {sort_order().label().to_lowercase()} - click to change",
            // The icon carries the current order on its own: a clock for
            // recency, A-Z for alphabetical.
            match sort_order() {
                SoundpackSort::RecentlyAdded => rsx! {
                  Clock { class: "w-4 h-4" }
                },
                SoundpackSort::Name => rsx! {
                  ArrowDownAZ { class: "w-4 h-4" }
                },
            }
          }
          button {
            class: "btn btn-sm btn-ghost",
            disabled: refreshing_soundpacks(),
            onclick: refresh_soundpacks_cache,
            title: "Refresh sound pack list",
            if refreshing_soundpacks() {
              span { class: "loading loading-spinner loading-xs" }
            } else {
              RefreshCw { class: "w-4 h-4" }
            }
          }
          if let Some(add_handler) = on_add_click {
            button {
              class: "btn btn-sm btn-neutral",
              onclick: move |evt| add_handler.call(evt),
              Plus { class: "w-4 h-4 mr-2" }
              "Add"
            }
          }
        }
        if soundpacks.is_empty() {
          div { class: "p-4 text-center text-sm text-base-content/70",
            "No {soundpack_type} sound pack found. You can add new sound packs by clicking the 'Add' button above."
          }
        } else {
          // Table
          div { class: "overflow-x-auto overflow-y-auto max-h-[calc(100vh-400px)] -mb-1",
            if filtered_soundpacks.is_empty() {
              div { class: "p-4 text-center text-sm text-base-content/70",
                "No result match your search!"
              }
            } else {
              table { class: "table table-sm w-full",
                tbody {
                  for pack in filtered_soundpacks {
                    SoundpackTableRow {
                      is_new: crate::state::soundpack_library::is_new(
                          &seen_soundpacks,
                          &pack.folder_path,
                      ),
                      soundpack: pack,
                    }
                  }
                }
              }
            }
          }
        }
      }
    }
}

#[component]
pub fn SoundpackTableRow(soundpack: SoundpackMetadata, is_new: bool) -> Element {
    let state_trigger = use_state_trigger();

    // Handlers for button clicks
    let on_open_folder = {
        let folder_path = soundpack.folder_path.clone();
        let soundpack_id = soundpack.id.clone();
        let soundpack_name = soundpack.name.clone();
        move |_| {
            let folder_path = folder_path.clone();
            let soundpack_id = soundpack_id.clone();
            let soundpack_name = soundpack_name.clone();
            spawn(async move {
                crate::always_print!("🔍 Soundpack info:");
                crate::always_print!("   Name: {}", soundpack_name);
                crate::always_print!("   ID: {}", soundpack_id);
                crate::always_print!("   Folder path: {}", folder_path);

                // Use folder_path if not empty, otherwise fall back to id
                let path_to_use = if !folder_path.is_empty() {
                    folder_path
                } else {
                    soundpack_id.clone()
                };

                match open_soundpack_folder(&path_to_use) {
                    Ok(_) =>
                        crate::always_print!("✅ Successfully opened folder for soundpack: {}", soundpack_name),
                    Err(e) =>
                        crate::always_eprint!("❌ Failed to open folder for soundpack {}: {}", soundpack_name, e),
                }
            });
        }
    };

    // Handler for delete button click
    let on_confirm_delete = {
        let soundpack_id = soundpack.id.clone();
        let trigger = state_trigger.clone();
        move |_| {
            let soundpack_id = soundpack_id.clone();
            let trigger = trigger.clone();
            spawn(async move {
                match delete_soundpack(&soundpack_id) {
                    Ok(_) => {
                        crate::always_print!("✅ Successfully deleted soundpack: {}", soundpack_id);
                        // The modal will close automatically due to form method="dialog"
                        // Trigger state refresh to update the UI
                        trigger.call(());
                    }
                    Err(e) => {
                        crate::always_eprint!("❌ Failed to delete soundpack {}: {}", soundpack_id, e);
                        // Could show an error modal here if needed
                    }
                }
            });
        }
    };
    rsx! {
      tr { class: "hover:bg-base-100",
        td { class: "flex items-center gap-4",
          // Icon
          div { class: "flex items-center justify-center",
            if let Some(icon) = &soundpack.icon {
              if !icon.is_empty() {
                div { class: "w-8 h-8 rounded-lg overflow-hidden",
                  img {
                    class: "w-full h-full object-cover",
                    src: "{icon}",
                    alt: "{soundpack.name}",
                  }
                }
              } else {
                PackCover { name: soundpack.name.clone(), size: 32 }
              }
            } else {
              PackCover { name: soundpack.name.clone(), size: 32 }
            }
          }
          // Name
          div {
            div { class: "flex items-center gap-2",
              div { class: "font-medium text-sm text-base-content line-clamp-1",
                "{soundpack.name}"
              }
              if is_new {
                span { class: "badge badge-primary badge-xs shrink-0 ml-0.5", "New" }
              }
            }
            if let Some(author) = &soundpack.author {
              div { class: "text-xs text-base-content/50", "by {author}" }
            }
          }
        }
        // Actions
        td {
          div { class: "flex items-center justify-end gap-1",
            button {
              class: "btn btn-soft btn-xs",
              title: "Open soundpack folder",
              onclick: on_open_folder,
              FolderOpen { class: "w-4 h-4" }
            }
            button {
              class: "btn btn-soft btn-error btn-xs",
              title: "Delete this soundpack",
              onclick: move |_| {
                  eval(
                      &format!(
                          "document.getElementById(\"confirm_delete_modal_{}\").showModal()",
                          soundpack.id,
                      ),
                  );
              },
              Trash { class: "w-4 h-4" }
            }
          }
        }
      }
      // Delete confirmation modal
      ConfirmDeleteModal {
        modal_id: format!("confirm_delete_modal_{}", soundpack.id),
        soundpack_name: soundpack.name.clone(),
        on_confirm: on_confirm_delete,
      }
    }
}
