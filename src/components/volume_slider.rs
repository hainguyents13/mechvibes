use crate::libs::AudioContext;
use crate::utils::config::use_config;
use dioxus::prelude::*;
use lucide_dioxus::{ Volume2, VolumeOff };
use std::sync::Arc;

#[derive(Clone, PartialEq, Copy)]
pub enum VolumeType {
    Keyboard, // Controls enable_keyboard_sound
    Mouse, // Controls enable_mouse_sound
}

#[component]
fn VolumeSliderBase(
    volume: Signal<f32>,
    on_change: Option<EventHandler<f32>>,
    id: String,
    volume_type: VolumeType
) -> Element {
    // Use shared config hook for enable_sound
    let (config, update_config) = use_config();
    // Read at render time: the memos below hold booleans, never a translated
    // string, so a language change still re-renders these labels.
    let strings = crate::i18n::use_strings();

    // The engine thread caches the enable flags in its own state and only
    // updates them on `AudioCommand::Set*SoundEnabled`. Writing the config
    // alone leaves the engine playing, so the mute button has to go through
    // the audio context (which persists config *and* notifies the engine).
    let audio_ctx = use_context::<Arc<AudioContext>>();

    // Get the appropriate enable state based on volume type
    let enable_sound = use_memo(move || {
        let config = config();
        match volume_type {
            VolumeType::Keyboard => config.enable_keyboard_sound,
            VolumeType::Mouse => config.enable_mouse_sound,
        }
    });

    // Get volume boost setting
    let enable_volume_boost = use_memo(move || config().enable_volume_boost);

    // Calculate max volume and percentage display
    let max_volume = if enable_volume_boost() { 2.0 } else { 1.0 };
    let volume_percentage = (volume() * 100.0) as u8;

    rsx! {
      div { class: "grid grid-cols-12",
        div { 
          class: format!(
            "rounded {} flex items-center", 
            if !enable_volume_boost() { "col-span-4" } else { "col-span-2" }
          ),

          if !enable_volume_boost()  {
            label { 
              r#for: "{id}", 
              class: "label label-text text-base", 
              // The trailing space is layout, so it stays here and not in the
              // catalog (an editor would trim it on save).
              "{strings.chrome.volume} "
            }
          }
          span {
            class: format!(
                "font-bold ml-1 {}",
                if enable_volume_boost() && volume() > 1.0 {
                    "text-warning"
                } else if enable_sound() {
                    "text-base-content"
                } else {
                    "text-base-content/50"
                },
            ),
            "{volume_percentage}%"
          }
        }
        div {
          class: format!("{} flex items-center gap-2", if !enable_volume_boost() { "col-span-8" } else { "col-span-10" }),
          input {
            class: format!("range range-xs grow {}", if volume() > 1.0 { "range-warning" } else { "range-primary" }),
            r#type: "range",
            min: 0.0,
            max: max_volume,
            step: 0.01,
            id: "{id}",
            value: volume(),
            disabled: !enable_sound(),
            oninput: move |evt| {
                if let Ok(val) = evt.value().parse::<f32>() {
                    volume.set(val);
                    if let Some(handler) = on_change {
                        handler.call(val);
                    }
                }
            },
          }
          div {
            class: "tooltip",
            "data-tip": if enable_sound() { strings.chrome.mute } else { strings.chrome.unmute },
            button {
              class: format!(
                  "btn btn-square btn-sm btn-ghost rounded-box {}",
                  if !enable_sound() { "btn-active" } else { "" },
              ),
              onclick: {
                  let update_config = update_config.clone();
                  let audio_ctx = audio_ctx.clone();
                  move |_| {
                      // Tell the engine first so the mute takes effect on the
                      // very next keystroke; `set_*_sound_enabled` also
                      // persists the flag. `update_config` then re-reads that
                      // saved config into the shared signal so the icon and
                      // the disabled slider re-render.
                      match volume_type {
                          VolumeType::Keyboard => {
                              let new_enable_keyboard = !config().enable_keyboard_sound;
                              audio_ctx.set_keyboard_sound_enabled(new_enable_keyboard);
                              update_config(
                                  Box::new(move |config| {
                                      config.enable_keyboard_sound = new_enable_keyboard;
                                  }),
                              );
                          }
                          VolumeType::Mouse => {
                              let new_enable_mouse = !config().enable_mouse_sound;
                              audio_ctx.set_mouse_sound_enabled(new_enable_mouse);
                              update_config(
                                  Box::new(move |config| {
                                      config.enable_mouse_sound = new_enable_mouse;
                                  }),
                              );
                          }
                      }
                  }
              },
              if enable_sound() {
                Volume2 { class: "w-5 h-5" }
              } else {
                VolumeOff { class: "w-5 h-5" }
              }
            }
          }
        }
      }
    }
}

#[component]
pub fn VolumeSlider(volume: Signal<f32>, on_change: Option<EventHandler<f32>>) -> Element {
    rsx! {
        VolumeSliderBase {
            volume,
            on_change,
            id: "volume-slider".to_string(),
            volume_type: VolumeType::Keyboard,
        }
    }
}

#[component]
pub fn MouseVolumeSlider(volume: Signal<f32>, on_change: Option<EventHandler<f32>>) -> Element {
    rsx! {
        VolumeSliderBase {
            volume,
            on_change,
            id: "mouse-volume-slider".to_string(),
            volume_type: VolumeType::Mouse,
        }
    }
}

#[component]
pub fn KeyboardVolumeSlider(volume: Signal<f32>, on_change: Option<EventHandler<f32>>) -> Element {
    rsx! {
        VolumeSliderBase {
            volume,
            on_change,
            id: "keyboard-volume-slider".to_string(),
            volume_type: VolumeType::Keyboard,
        }
    }
}
