use crate::{
    components::ui::{ ImportStep, ProgressStep },
    state::app::use_state_trigger,
    utils::delay,
    utils::soundpack_installer::{
        check_soundpack_conflict,
        extract_and_install_soundpack_with_type,
        get_soundpack_id_from_zip,
        SoundpackConflict,
    },
    utils::soundpack_validator::{ validate_soundpack_structure, validate_zip_file },
};
use dioxus::prelude::*;
use lucide_dioxus::FolderArchive;
use std::sync::Arc;

#[component]
pub fn SoundpackImportModal(
    modal_id: String,
    audio_ctx: Arc<crate::libs::audio::AudioContext>,
    target_soundpack_type: Option<crate::state::soundpack::SoundpackType>,
    on_import_success: EventHandler<()>
) -> Element {
    // Loading
    let is_loading = use_signal(|| false);

    // Import step tracking
    let current_step = use_signal(|| ImportStep::Idle);
    let error_step = use_signal(|| ImportStep::Idle);
    let error_message = use_signal(|| String::new());
    let success_step = use_signal(|| ImportStep::Idle);
    let success_message = use_signal(|| String::new());
    // Success messages for each step
    let file_selected_message = use_signal(|| String::new());
    let installation_success_message = use_signal(|| String::new());
    let finalization_success_message = use_signal(|| String::new());
    let state_trigger = use_state_trigger();

    // Reset function to clear all states
    let reset_modal = {
        let mut is_loading = is_loading.clone();
        let mut current_step = current_step.clone();
        let mut error_step = error_step.clone();
        let mut error_message = error_message.clone();
        let mut success_step = success_step.clone();
        let mut success_message = success_message.clone();
        let mut file_selected_message = file_selected_message.clone();
        let mut installation_success_message = installation_success_message.clone();
        let mut finalization_success_message = finalization_success_message.clone();

        Callback::new(move |_| {
            is_loading.set(false);
            current_step.set(ImportStep::Idle);
            error_step.set(ImportStep::Idle);
            error_message.set(String::new());
            success_step.set(ImportStep::Idle);
            success_message.set(String::new());
            file_selected_message.set(String::new());
            installation_success_message.set(String::new());
            finalization_success_message.set(String::new());
        })
    };

    // Close button handler with reset
    let handle_close = {
        let reset_modal = reset_modal.clone();
        move |_| {
            reset_modal.call(());
        }
    }; // File import handler
    let handle_import_click = {
        let audio_ctx = audio_ctx.clone();
        let state_trigger = state_trigger.clone();
        let reset_modal = reset_modal.clone();
        let target_soundpack_type = target_soundpack_type.clone(); // Clone for closure
        let error_step = error_step.clone();
        let error_message = error_message.clone();
        let success_step = success_step.clone();
        let success_message = success_message.clone();
        let current_step = current_step.clone();
        let file_selected_message = file_selected_message.clone();
        let is_loading = is_loading.clone();
        move |_| {
            let audio_ctx = audio_ctx.clone();
            let on_import_success = on_import_success.clone();
            let state_trigger = state_trigger.clone();
            let reset_modal = reset_modal.clone();
            let mut error_step = error_step.clone();
            let mut error_message = error_message.clone();
            let mut success_step = success_step.clone();
            let mut success_message = success_message.clone();
            let mut current_step = current_step.clone();
            let mut file_selected_message = file_selected_message.clone();
            let mut is_loading = is_loading.clone();

            spawn(async move {
                // Reset modal state before starting import
                reset_modal.call(());

                // Set loading state
                is_loading.set(true);

                // Reset all steps and messages
                current_step.set(ImportStep::FileSelecting);

                // ===========================================
                // Step 1: Open file dialog and select file
                // ===========================================
                // Open file dialog to select ZIP file
                let file_dialog = rfd::AsyncFileDialog
                    ::new()
                    .add_filter("ZIP Files", &["zip"])
                    .set_title("Select Sound Pack ZIP File")
                    .pick_file().await;

                let file_handle = match file_dialog {
                    Some(handle) => handle,
                    None => {
                        // User cancelled the dialog
                        current_step.set(ImportStep::Idle);
                        is_loading.set(false);
                        return;
                    }
                };

                // File selected successfully
                let file_name = file_handle.file_name();
                file_selected_message.set(file_name);
                delay::Delay::ms(500).await;
                let file_path = file_handle.path().to_string_lossy().to_string();

                // ============================================
                // Step 2: Validating file
                // ============================================
                current_step.set(ImportStep::Validating);
                delay::Delay::ms(500).await;

                // First validate ZIP file structure
                if let Err(e) = validate_zip_file(&file_path).await {
                    error_step.set(ImportStep::Validating);
                    error_message.set(format!("Invalid file: {}", e));
                    is_loading.set(false);
                    return;
                }

                // Then validate soundpack structure and configuration
                match validate_soundpack_structure(&file_path).await {
                    Ok((_, _)) => {
                        // Continue to next step if validation succeeds
                    }
                    Err(e) => {
                        error_step.set(ImportStep::Validating);
                        error_message.set(e);
                        is_loading.set(false);
                        return; // Stop import process on validation error
                    }
                }

                // =============================================
                // Step 3: Checking for conflicts
                // =============================================
                // Now get the soundpack ID for conflict checking
                let soundpack_id = match get_soundpack_id_from_zip(&file_path) {
                    Ok(id) => id,
                    Err(e) => {
                        error_step.set(ImportStep::Validating);
                        error_message.set(format!("Failed to read soundpack ID: {}", e));
                        is_loading.set(false);
                        return; // Stop import process on ID reading error
                    }
                };

                current_step.set(ImportStep::CheckingConflicts);
                delay::Delay::ms(500).await;

                // Resolve the exact directory the install would write to and
                // refuse the import if a pack already lives there. Previously
                // this compared a raw ID against prefixed ones and never fired.
                // A built-in pack is a conflict too, even though its folder is
                // free: the built-in one would hide the imported pack.
                let soundpacks_dir =
                    crate::state::paths::soundpacks::get_custom_soundpacks_dir();
                match
                    check_soundpack_conflict(
                        &file_path,
                        target_soundpack_type,
                        &soundpacks_dir
                    )
                {
                    Ok(Some(SoundpackConflict::AlreadyInstalled(_))) => {
                        error_step.set(ImportStep::CheckingConflicts);
                        error_message.set(
                            format!("A sound pack with ID '{}' already exists.\nPlease remove the existing sound pack and try again.", soundpack_id)
                        );
                        is_loading.set(false);
                        return; // Stop import process on conflict
                    }
                    Ok(Some(SoundpackConflict::ShadowsBuiltin { folder_path })) => {
                        // Pointing at the existing pack would be a dead end here:
                        // a built-in pack cannot be removed by the user.
                        error_step.set(ImportStep::CheckingConflicts);
                        error_message.set(
                            format!("'{}' is the ID of a built-in sound pack, so the built-in one would hide the pack you are importing.\nEdit the pack's config.json to use a different ID and try again.", folder_path)
                        );
                        is_loading.set(false);
                        return; // Stop import process on conflict
                    }
                    Ok(None) => {}
                    Err(e) => {
                        error_step.set(ImportStep::Validating);
                        error_message.set(format!("Failed to read soundpack ID: {}", e));
                        is_loading.set(false);
                        return; // Stop import process on ID reading error
                    }
                }

                // ==============================================
                // Step 4: Installing soundpack
                // ==============================================
                current_step.set(ImportStep::Installing);
                delay::Delay::ms(500).await;

                crate::always_print!("⚒️ Installing soundpack ...");

                let soundpack_info = match
                    extract_and_install_soundpack_with_type(&file_path, target_soundpack_type)
                {
                    Ok(info) => info,
                    Err(e) => {
                        error_step.set(ImportStep::Installing);
                        error_message.set(e);
                        is_loading.set(false);
                        return; // Stop import process on installation error
                    }
                };

                // =============================================
                // Step 5: Finalizing installation
                // =============================================
                current_step.set(ImportStep::Finalizing);
                delay::Delay::ms(500).await;

                // Reload soundpacks in audio context
                crate::state::app::reload_current_soundpacks(&audio_ctx);

                // =============================================
                // Step 6: Refreshing soundpack list
                // =============================================
                current_step.set(ImportStep::Refreshing);
                delay::Delay::ms(500).await;

                // Refresh the soundpack cache to show the new soundpack in the UI
                crate::always_print!("🔄 Triggering soundpack cache refresh after import...");
                state_trigger.call(());

                // Notify parent component (this will trigger UI update)
                on_import_success.call(());

                // ============================================
                // Step 7: Completed
                // ============================================
                current_step.set(ImportStep::Completed);
                success_step.set(ImportStep::Completed);
                success_message.set(format!("Successfully installed: {}", soundpack_info.name));

                // Reset after showing success for a while
                delay::Delay::ms(2000).await;
                reset_modal.call(());
            });
        }
    };

    // Render the modal
    rsx! {
      dialog { class: "modal", id: "{modal_id}",
        div { class: "modal-box max-w-2xl",
          form { method: "dialog",
            button {
              disabled: *is_loading.read(),
              class: "btn btn-sm btn-circle btn-ghost absolute right-2 top-2",
              "✕"
            }
          }
          h3 { class: "font-bold text-lg mb-2", "Import sound pack" }

          if *current_step.read() == ImportStep::Idle {
            div { class: "card border border-base-300 bg-base-200 text-sm p-4 space-y-4",
              div {
                "To import a sound pack, select a ZIP file containing the sound pack structure as shown below:"
              }
              div { class: "bg-base-100 p-2 px-3 rounded-box font-mono text-base-content/70 text-xs space-x-1",
                div { "soundpack-name.zip" }
                div { class: "", "├── config.json" }
                div { class: "", "├── sound.ogg (for \"single\" def type)" }
                div { class: "", "├── key_a.ogg (for \"multi\" def type)" }
                div { class: "", "├── key_b.ogg (for \"multi\" def type)" }
                div { class: "", "├── key_c.ogg (for \"multi\" def type)" }
              }
              div {
                div { class: "text-sm", "Notes:" }
                ul { class: "list list-disc text-xs ml-4 text-base-content/70 space-y-1",
                  li {
                    span { class: "kbd kbd-xs bg-base-100", "single" }
                    " Use a single sound file to play for the entire sound pack."
                  }
                  li {
                    span { class: "kbd kbd-xs bg-base-100", "multi" }
                    " Use multiple sound files for different keys."
                  }
                  li { "The config.json file must be in the root of the ZIP file" }
                }
              }
            }
          } else {

            div { class: "space-y-4",
              // Progress Steps
              div { class: "space-y-2",
                ProgressStep {
                  step_number: 1,
                  title: "Select file (zip)".to_string(),
                  current_step: *current_step.read(),
                  error_message: if *error_step.read() == ImportStep::FileSelecting { error_message.read().clone() } else { String::new() },
                  success_message: file_selected_message.read().clone(),
                }
                ProgressStep {
                  step_number: 2,
                  title: "Validating".to_string(),
                  current_step: *current_step.read(),
                  error_message: if *error_step.read() == ImportStep::Validating { error_message.read().clone() } else { String::new() },
                  success_message: String::new(),
                }
                ProgressStep {
                  step_number: 3,
                  title: "Checking conflicts".to_string(),
                  current_step: *current_step.read(),
                  error_message: if *error_step.read() == ImportStep::CheckingConflicts { error_message.read().clone() } else { String::new() },
                  success_message: String::new(),
                }
                ProgressStep {
                  step_number: 4,
                  title: "Installing sound pack".to_string(),
                  current_step: *current_step.read(),
                  error_message: if *error_step.read() == ImportStep::Installing { error_message.read().clone() } else { String::new() },
                  success_message: installation_success_message.read().clone(),
                }
                ProgressStep {
                  step_number: 5,
                  title: "Finalizing".to_string(),
                  current_step: *current_step.read(),
                  error_message: if *error_step.read() == ImportStep::Finalizing { error_message.read().clone() } else { String::new() },
                  success_message: finalization_success_message.read().clone(),
                }
                ProgressStep {
                  step_number: 6,
                  title: "Refreshing sound pack list".to_string(),
                  current_step: *current_step.read(),
                  error_message: if *error_step.read() == ImportStep::Refreshing { error_message.read().clone() } else { String::new() },
                  success_message: String::new(),
                }
              }

              // Success message display
              if !success_message.read().is_empty() {
                div { class: "alert alert-success alert-soft",
                  "{success_message.read()}"
                }
              }
            }
          }

          // Modal Actions
          div { class: "modal-action mt-6",
            form { method: "dialog",
              button {
                class: "btn btn-sm btn-ghost",
                disabled: *is_loading.read(),
                onclick: handle_close,
                "Close"
              }
            }
            button {
              class: "btn btn-sm btn-neutral",
              disabled: *is_loading.read(),
              onclick: handle_import_click,
              if *is_loading.read() == false {
                FolderArchive { class: "w-4 h-4 mr-2" }
                "Select file"
              } else {
                span { class: "loading loading-spinner loading-sm mr-2" }
                "Importing..."
              }
            }
          }
        }
        form { method: "dialog", class: "modal-backdrop",
          button { disabled: *is_loading.read(), onclick: handle_close, "close" }
        }
      }
    }
}
