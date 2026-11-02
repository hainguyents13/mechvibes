use dioxus::prelude::*;
use lucide_dioxus::{
    CloudSunRain,
    Play,
    Pause,
    Volume2,
    VolumeOff,
    TreePine,
    CloudRain,
    Waves,
    Zap,
    Flame,
    Wind,
    Moon,
    Coffee,
    Radio,
};

use crate::components::ui::{ PageHeader };
use crate::state::ambiance::{
    AmbiancePlayerState,
    initialize_global_ambiance_player_state,
    update_global_ambiance_player_state,
    get_global_ambiance_player_state_copy,
    play_ambiance_sound,
    stop_ambiance_sound,
    pause_all_ambiance_sounds,
    resume_all_ambiance_sounds,
    set_global_ambiance_mute,
};
use dioxus::core::spawn_forever;
use futures_timer::Delay;
use std::sync::atomic::{ AtomicU64, Ordering };
use std::sync::Arc;
use std::time::Duration;

/// Writes the ambiance settings once a slider drag settles.
///
/// Only the trailing call writes. `save_config` goes through
/// `config_writer::apply`, which serialises and writes the whole config and
/// makes the window re-render from the root, so saving on every slider step
/// made the ambiance sound lag during a drag. The sliders change the
/// in-memory state and the engine right away; this only persists it.
///
/// It saves the global state as it is when the timer fires, so the last
/// position of whichever slider moved is what lands. `spawn_forever` keeps
/// the write alive if the page is left before the timer ends, since the
/// in-memory state outlives the page.
fn schedule_ambiance_save(save_counter: Arc<AtomicU64>) {
    let task_id = save_counter.fetch_add(1, Ordering::SeqCst) + 1;
    spawn_forever(async move {
        // Long enough that a drag, whose input events arrive milliseconds
        // apart, collapses into one write.
        Delay::new(Duration::from_millis(150)).await;
        if save_counter.load(Ordering::SeqCst) == task_id {
            update_global_ambiance_player_state(|player| player.save_config());
        }
    });
}

// Sound Selection Panel Component
#[component]
fn SoundSelectionPanel(
    ambiance_player: Signal<AmbiancePlayerState>,
    refresh_trigger: Signal<i32>
) -> Element {
    // Shared by both kinds of slider: each one saves the whole ambiance
    // state, so a newer move on any slider supersedes an older pending save.
    let save_counter = use_hook(|| Arc::new(AtomicU64::new(0)));

    rsx! {
      div { class: "bg-base-200 border border-base-300 rounded-box p-6 space-y-4",
        // Header
        div { class: "space-y-1",
          h3 { class: "text-lg font-semibold", "Ambiance sounds" }
          p { class: "text-sm text-base-content/70",
            if ambiance_player().active_sounds.is_empty() {
              "Select at least one sound to play"
            } else {
              {
                  format!(
                      "{} active sound{}",
                      ambiance_player().active_sounds.len(),
                      if ambiance_player().active_sounds.len() == 1 { "" } else { "s" },
                  )
              }
            }
          }
        }
        // Global Controls
        div { class: "flex items-center gap-4 w-full justify-between",
          // Play/Pause Button
          button {
            class: "btn btn-primary btn-square",
            onclick: move |_| {
                update_global_ambiance_player_state(|player| {
                    player.toggle_play_pause();
                });
                if let Some(current_state) = get_global_ambiance_player_state_copy() {
                    if current_state.is_playing {
                        let _ = resume_all_ambiance_sounds();
                    } else {
                        let _ = pause_all_ambiance_sounds();
                    }
                }
                refresh_trigger.set(refresh_trigger() + 1);
            },
            if ambiance_player().is_playing {
              Pause { class: "w-4 h-4" }
            } else {
              Play { class: "w-4 h-4" }
            }
          }
          // Mute Button
          button {
            class: "btn btn-ghost btn-sm btn-square",
            onclick: move |_| {
                update_global_ambiance_player_state(|player| {
                    player.toggle_mute();
                    player.save_config();
                });
                if let Some(current_state) = get_global_ambiance_player_state_copy() {
                    let _ = set_global_ambiance_mute(current_state.is_muted);
                }
                refresh_trigger.set(refresh_trigger() + 1);
            },
            if ambiance_player().is_muted {
              VolumeOff { class: "w-4 h-4" }
            } else {
              Volume2 { class: "w-4 h-4" }
            }
          }
          input {
            r#type: "range",
            class: "range range-xs",
            min: "0",
            max: "100",
            value: "{(ambiance_player().global_volume * 100.0) as i32}",
            disabled: ambiance_player().is_muted || !ambiance_player().is_playing,
            oninput: {
                let save_counter = save_counter.clone();
                move |evt: dioxus::prelude::Event<dioxus::html::FormData>| {
                    if let Ok(val) = evt.value().parse::<f32>() {
                        update_global_ambiance_player_state(|player| {
                            player.set_global_volume(val / 100.0);
                        });
                        schedule_ambiance_save(save_counter.clone());
                        refresh_trigger.set(refresh_trigger() + 1);
                    }
                }
            },
          }
          div { class: "text-xs text-base-content/70 whitespace-nowrap",
            "{(ambiance_player().global_volume * 100.0) as i32}%"
          }
        }
        // Sound Selection List
        div { class: "{crate::utils::spacing::SECTION_SPACING}",
          for sound in ambiance_player().sounds.iter() {
            div {
              class: format!(
                  " rounded-box border {} {} {}",
                  crate::utils::spacing::CARD_PADDING,
                  crate::utils::spacing::SECTION_SPACING,
                  if ambiance_player().is_sound_active(&sound.id) {
                      "bg-base-100 border-base-100"
                  } else {
                      "bg-base-300 border-base-300"
                  },
              ),
              // First line: Icon, Name, Description (left) + Toggle (right)
              div { class: "flex items-center justify-between",
                // Left side: Icon + Name + Description
                div { class: "flex items-center gap-3 flex-1",
                  // Sound Icon
                  div { class: "flex-shrink-0",
                    match sound.icon.as_str() {
                        "cloud-rain" => rsx! {
                          CloudRain { class: "w-5 h-5 text-base-content/50" }
                        },
                        "tree-pine" => rsx! {
                          TreePine { class: "w-5 h-5 text-base-content/50" }
                        },
                        "waves" => rsx! {
                          Waves { class: "w-5 h-5 text-base-content/50" }
                        },
                        "zap" => rsx! {
                          Zap { class: "w-5 h-5 text-base-content/50" }
                        },
                        "flame" => rsx! {
                          Flame { class: "w-5 h-5 text-base-content/50" }
                        },
                        "wind" => rsx! {
                          Wind { class: "w-5 h-5 text-base-content/50" }
                        },
                        "moon" => rsx! {
                          Moon { class: "w-5 h-5 text-base-content/50" }
                        },
                        "coffee" => rsx! {
                          Coffee { class: "w-5 h-5 text-base-content/50" }
                        },
                        "radio" => rsx! {
                          Radio { class: "w-5 h-5 text-base-content/50" }
                        },
                        _ => rsx! {
                          TreePine { class: "w-5 h-5 text-base-content/50" }
                        },
                    }
                  }
                  // Sound Name and Description
                  div { class: "flex-1 min-w-0",
                    div { class: "text-sm font-medium text-base-content truncate",
                      "{sound.name}"
                    }
                    div { class: "text-xs text-base-content/60 truncate",
                      "{sound.description}"
                    }
                  }
                }
                // Right side: Toggle Switch
                div { class: "flex-shrink-0",
                  input {
                    r#type: "checkbox",
                    class: "toggle toggle-xs toggle-primary",
                    checked: ambiance_player().is_sound_active(&sound.id),
                    onchange: {
                        let sound_id = sound.id.clone();
                        let sound_url = sound.audio_url.clone();
                        move |_| {
                            let is_currently_active = if let Some(current_state) = get_global_ambiance_player_state_copy() {
                                current_state.is_sound_active(&sound_id)
                            } else {
                                false
                            };
                            update_global_ambiance_player_state(|player| {
                                player.toggle_sound(sound_id.clone());
                                player.save_config();
                            });
                            if let Some(current_state) = get_global_ambiance_player_state_copy() {
                                if current_state.is_sound_active(&sound_id) && !is_currently_active {
                                    if current_state.is_playing {
                                        let volume = current_state.get_sound_volume(&sound_id);
                                        let global_volume = if current_state.is_muted {
                                            0.0
                                        } else {
                                            current_state.global_volume
                                        };
                                        let _ = play_ambiance_sound(
                                            sound_id.clone(),
                                            sound_url.clone(),
                                            volume * global_volume,
                                        );
                                    }
                                } else if !current_state.is_sound_active(&sound_id)
                                    && is_currently_active
                                {
                                    let _ = stop_ambiance_sound(&sound_id);
                                }
                            }
                            refresh_trigger.set(refresh_trigger() + 1);
                        }
                    },
                  }
                }
              }
              // Second line: Volume Control (only show if sound is active)
              if ambiance_player().is_sound_active(&sound.id) {
                div { class: "flex items-center gap-3",
                  input {
                    r#type: "range",
                    class: "range range-xs ml-8",
                    min: "0",
                    max: "100",
                    value: "{(ambiance_player().get_sound_volume(&sound.id) * 100.0) as i32}",
                    oninput: {
                        let sound_id = sound.id.clone();
                        let save_counter = save_counter.clone();
                        move |evt: dioxus::prelude::Event<dioxus::html::FormData>| {
                            if let Ok(val) = evt.value().parse::<f32>() {
                                let volume = val / 100.0;
                                update_global_ambiance_player_state(|player| {
                                    player.set_sound_volume(sound_id.clone(), volume);
                                });
                                schedule_ambiance_save(save_counter.clone());
                                refresh_trigger.set(refresh_trigger() + 1);
                            }
                        }
                    },
                  }
                  span { class: "text-xs text-base-content/70 w-10 text-right",
                    "{(ambiance_player().get_sound_volume(&sound.id) * 100.0) as i32}%"
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
pub fn MoodPage() -> Element {
    // Use global state instead of local signal
    let mut ambiance_player = use_signal(|| AmbiancePlayerState::new());

    // Force re-render when global state changes
    let refresh_trigger = use_signal(|| 0);

    // Initialize global ambiance player on component mount
    use_effect(move || {
        initialize_global_ambiance_player_state();
        if let Some(global_ambiance_state) = get_global_ambiance_player_state_copy() {
            ambiance_player.set(global_ambiance_state);
        }
    });

    // Update local state when refresh trigger changes
    use_effect(move || {
        let _trigger = refresh_trigger();
        if let Some(global_ambiance_state) = get_global_ambiance_player_state_copy() {
            ambiance_player.set(global_ambiance_state);
        }
    });

    rsx! {
      div { class: "{crate::utils::spacing::SECTION_SPACING_LG}",
        PageHeader {
          title: "Mood".to_string(),
          subtitle: "Ambient sounds to set the perfect atmosphere".to_string(),
          icon: Some(rsx! {
            CloudSunRain { class: "w-8 h-8 mx-auto" }
          }),
        }

        SoundSelectionPanel { ambiance_player, refresh_trigger }
      }
    }
}
