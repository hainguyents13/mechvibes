use crate::{
    components::ui::{ PageHeader, SoundpackImportModal, SoundpackManager, SoundpackTable },
    state::app::{ use_app_state, use_state_trigger },
};
use dioxus::document::eval;
use dioxus::prelude::*;
use lucide_dioxus::{ Keyboard, Mouse, Music, Settings2 };
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub enum TabType {
    Keyboard,
    Mouse,
    Manage,
}

#[component]
pub fn Soundpacks() -> Element {
    // Get app state and trigger function
    let app_state = use_app_state();
    let trigger_update = use_state_trigger(); // Track the current active tab
    let mut current_tab = use_signal(|| TabType::Keyboard);

    // Get all soundpacks (this will be reactive to app_state changes)
    let all_soundpacks = app_state.get_soundpacks();
    crate::always_print!("🔄 Soundpacks component rendering with {} soundpacks", all_soundpacks.len()); // Filter soundpacks by type (these will update when all_soundpacks changes)
    let keyboard_soundpacks: Vec<_> = all_soundpacks
        .iter()
        .filter(|pack| pack.soundpack_type == crate::state::soundpack::SoundpackType::Keyboard)
        .cloned()
        .collect();

    let mouse_soundpacks: Vec<_> = all_soundpacks
        .iter()
        .filter(|pack| pack.soundpack_type == crate::state::soundpack::SoundpackType::Mouse)
        .cloned()
        .collect();

    crate::always_print!(
        "🔄 Filtered: {} keyboard, {} mouse soundpacks",
        keyboard_soundpacks.len(),
        mouse_soundpacks.len()
    );

    // Get access to audio context for reloading soundpacks
    let audio_ctx: Arc<crate::libs::audio::AudioContext> = use_context();

    // Reads the config signal, so choosing a language in Settings re-renders this
    // page with the new text. Not memoized: a memo would keep the first language.
    let strings = crate::i18n::use_strings();

    rsx! {
      div { class: "",
        // Page header
        PageHeader {
          title: strings.soundpacks.page_title.to_string(),
          subtitle: strings.soundpacks.page_subtitle.to_string(),
          icon: Some(rsx! {
            Music { class: "w-8 h-8 mx-auto" }
          }),
        }        // Tabs for soundpack types
        div { class: "tabs tabs-lift",          // Keyboard tab
          label { class: "tab [--tab-border-color:var(--color-base-300)] [--tab-bg:var(--color-base-200)]",
            input {
              r#type: "radio",
              name: "soundpack-tab",
              checked: current_tab() == TabType::Keyboard,
              onchange: move |_| {
                  current_tab.set(TabType::Keyboard);
              },
            }
            Keyboard { class: "w-5 h-5 mr-2" }
            "{strings.soundpacks.keyboard_label}"
          }
          div { class: "tab-content overflow-hidden bg-base-200 border-base-300 py-4 px-0",
            SoundpackTable {
              soundpacks: keyboard_soundpacks,
              soundpack_type: crate::state::soundpack::SoundpackType::Keyboard,
              on_add_click: Some(
                  EventHandler::new(move |_| {
                      current_tab.set(TabType::Keyboard);
                      eval("soundpack_import_modal.showModal()");
                  }),
              ),
            }
          }

          // Mouse tab
          label { class: "tab [--tab-border-color:var(--color-base-300)] [--tab-bg:var(--color-base-200)]",
            input { 
              r#type: "radio", 
              name: "soundpack-tab",
              checked: current_tab() == TabType::Mouse,
              onchange: move |_| {
                  current_tab.set(TabType::Mouse);
              },
            }
            Mouse { class: "w-5 h-5 mr-2" }
            "{strings.soundpacks.mouse_label}"
          }
          div { class: "tab-content overflow-hidden bg-base-200 border-base-300 py-4 px-0",
            SoundpackTable {
              soundpacks: mouse_soundpacks,
              soundpack_type: crate::state::soundpack::SoundpackType::Mouse,
              on_add_click: Some(
                  EventHandler::new(move |_| {
                      current_tab.set(TabType::Mouse);
                      eval("soundpack_import_modal.showModal()");
                  }),
              ),
            }
          }

          // Manage tab
          label { class: "tab [--tab-border-color:var(--color-base-300)] [--tab-bg:var(--color-base-200)]",
            input { 
              r#type: "radio", 
              name: "soundpack-tab",
              checked: current_tab() == TabType::Manage,
              onchange: move |_| {
                  current_tab.set(TabType::Manage);
              },
            }
            Settings2 { class: "w-5 h-5 mr-2" }
            "{strings.soundpacks.tab_manage}"
          }
          div { class: "tab-content overflow-hidden bg-base-200 border-base-300 {crate::utils::spacing::CARD_PADDING}",
            SoundpackManager {
              on_import_click: EventHandler::new(move |_| {
                  eval("soundpack_import_modal.showModal()");
              }),
            }
          }
        }        // Import modal
        SoundpackImportModal {
          modal_id: "soundpack_import_modal".to_string(),
          audio_ctx,
          target_soundpack_type: match current_tab() {
              TabType::Keyboard => Some(crate::state::soundpack::SoundpackType::Keyboard),
              TabType::Mouse => Some(crate::state::soundpack::SoundpackType::Mouse),
              TabType::Manage => None, // Let user choose in manage tab
          },
          on_import_success: EventHandler::new(move |_| {
              trigger_update(());
          }),
        }
      }
    }
}
