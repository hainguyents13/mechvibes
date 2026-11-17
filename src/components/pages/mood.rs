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
    Music,
    Plus,
    Trash2,
};
use crate::utils::notice::{ self, NoticeKind };

use crate::components::ui::{ PageHeader };
use crate::i18n::{
    catalog::mood::{ ambience_text, MoodStrings },
    fill,
    plural,
    use_strings,
};
use crate::state::ambiance::{
    AmbiancePlayerState,
    AmbianceSound,
    initialize_global_ambiance_player_state,
    update_global_ambiance_player_state,
    get_global_ambiance_player_state_copy,
    ambiance_mute_flags,
};
use dioxus::core::spawn_forever;
use futures_timer::Delay;
use std::sync::atomic::{ AtomicU64, Ordering };
use std::sync::Arc;
use std::time::Duration;

/// The name and description the mixer shows for `sound`.
///
/// A built-in ambience takes its text from the catalog, keyed by its id, so it
/// follows the language. A sound the user added shows its own file name and the
/// catalog's "your sound" line, and a built-in id the catalog does not know yet
/// falls back to the fields the sound was listed with.
fn shown_text<'a>(strings: &'a MoodStrings, sound: &'a AmbianceSound) -> SoundText<'a> {
    if sound.custom {
        SoundText {
            name: sound.name.as_str(),
            description: strings.custom_sound_description,
        }
    } else {
        match ambience_text(strings, &sound.id) {
            Some((name, description)) => SoundText { name, description },
            None => SoundText {
                name: sound.name.as_str(),
                description: sound.description.as_str(),
            },
        }
    }
}

/// See [`shown_text`].
struct SoundText<'a> {
    name: &'a str,
    description: &'a str,
}

/// "1 active sound" / "3 active sounds", from the catalog's pair.
fn active_sounds_line(strings: &MoodStrings, count: usize) -> String {
    fill(plural(strings.active_sounds_one, strings.active_sounds_other, count), &[&count])
}

/// Writes the ambiance settings once a slider drag settles.
///
/// Only the trailing call writes. `save_config` goes through
/// `config_writer::apply`, which serialises and writes the whole config and
/// makes the window re-render from the root, so saving on every slider step
/// made the ambiance sound lag during a drag. The sliders change the
/// in-memory state and the engine right away; this only persists it.
///
/// It saves the global state as it is when the timer fires, so the last
/// position of whichever slider moved is what lands, and a sound toggle or
/// mute saved while the timer runs is written again as is rather than
/// reverted. `spawn_forever` keeps
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

    // Reads the config signal, so choosing a language re-renders the mixer with
    // the new text. Not memoized: a memo would keep the first language.
    let strings = use_strings();

    rsx! {
      div { class: "bg-base-200 border border-base-300 rounded-box p-6 space-y-4",
        // Header
        div { class: "space-y-1",
          h3 { class: "text-lg font-semibold", "{strings.mood.mixer_title}" }
          p { class: "text-sm text-base-content/70",
            if ambiance_player().active_sounds.is_empty() {
              "{strings.mood.no_sounds_selected}"
            } else {
              { active_sounds_line(&strings.mood, ambiance_player().active_sounds.len()) }
            }
          }
        }
        // Global Controls
        div { class: "flex items-center gap-4 w-full justify-between",
          // Play/Pause Button
          button {
            class: "btn btn-primary btn-square",
            // Like the mute button and sound toggles, this only calls the
            // state method: it already tells the engine, and saves when the
            // change is one that is kept.
            onclick: move |_| {
                update_global_ambiance_player_state(|player| {
                    player.toggle_play_pause();
                });
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
                });
                // The tray has a "Mute Mood" checkmark for the same mute.
                crate::libs::tray_service::request_tray_update();
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
        // Mood is playing as far as its own controls go, but the app is muted.
        if ambiance_player().silenced && ambiance_player().is_playing && !ambiance_player().is_muted {
          p { class: "text-xs text-base-content/70", "{strings.mood.silenced_hint}" }
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
                        "music" => rsx! {
                          Music { class: "w-5 h-5 text-base-content/50" }
                        },
                        _ => rsx! {
                          TreePine { class: "w-5 h-5 text-base-content/50" }
                        },
                    }
                  }
                  // Sound Name and Description, from the catalog for a built-in
                  // ambience and from the sound's own fields otherwise.
                  div { class: "flex-1 min-w-0",
                    div { class: "text-sm font-medium text-base-content truncate",
                      "{shown_text(&strings.mood, sound).name}"
                    }
                    div { class: "text-xs text-base-content/60 truncate",
                      "{shown_text(&strings.mood, sound).description}"
                    }
                  }
                }
                // Right side: Remove (own sounds only) + Toggle Switch
                div { class: "flex-shrink-0 flex items-center gap-2",
                  if sound.custom {
                    button {
                      class: "btn btn-ghost btn-xs btn-square",
                      title: "{strings.mood.remove_sound_title}",
                      onclick: {
                          let sound_id = sound.id.clone();
                          move |_| {
                              update_global_ambiance_player_state(|player| {
                                  player.remove_custom_sound(&sound_id);
                              });
                              refresh_trigger.set(refresh_trigger() + 1);
                          }
                      },
                      Trash2 { class: "w-4 h-4 text-base-content/50" }
                    }
                  }
                  input {
                    r#type: "checkbox",
                    class: "toggle toggle-xs toggle-primary",
                    checked: ambiance_player().is_sound_active(&sound.id),
                    onchange: {
                        let sound_id = sound.id.clone();
                        move |_| {
                            update_global_ambiance_player_state(|player| {
                                player.toggle_sound(sound_id.clone());
                            });
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
        AddSoundButton { refresh_trigger }
      }
    }
}

/// Picks a sound file of the user's own and adds it to the list, switched on.
#[component]
fn AddSoundButton(refresh_trigger: Signal<i32>) -> Element {
    let mut adding = use_signal(|| false);
    // The button, the file picker's title and the notice that reports a failure
    // are all copy, so they follow the language like the rest of the page.
    let strings = use_strings();

    rsx! {
      div { class: "space-y-1",
        button {
          class: "btn btn-sm btn-soft w-full",
          disabled: adding(),
          onclick: move |_| {
              spawn(async move {
                  let picked = rfd::AsyncFileDialog::new()
                      .add_filter(strings.mood.sounds_filter, crate::utils::mood_sounds::SUPPORTED_EXTENSIONS)
                      .set_title(strings.mood.add_sound_title)
                      .pick_file()
                      .await;
                  let Some(handle) = picked else {
                      return;
                  };
                  let source = handle.path().to_path_buf();

                  // Copying a long music file takes a moment, so it runs off
                  // the window's thread.
                  adding.set(true);
                  let added = tokio::task
                      ::spawn_blocking(move || crate::utils::mood_sounds::add(&source)).await
                      .unwrap_or_else(|e| Err(fill(strings.mood.add_failed, &[&e])));
                  adding.set(false);

                  match added {
                      Ok(sound) => {
                          let mut is_new = false;
                          update_global_ambiance_player_state(|player| {
                              is_new = player.add_custom_sound(sound);
                          });
                          if !is_new {
                              notice::report(NoticeKind::Info, strings.mood.already_added, None);
                          }
                          refresh_trigger.set(refresh_trigger() + 1);
                      }
                      Err(e) => {
                          crate::always_eprint!("Failed to add a Mood sound: {}", e);
                          notice::report(NoticeKind::Error, e, None);
                      }
                  }
              });
          },
          if adding() {
            span { class: "loading loading-spinner loading-xs" }
            "{strings.mood.adding}"
          } else {
            Plus { class: "w-4 h-4" }
            "{strings.mood.add_sound}"
          }
        }
        p { class: "text-xs text-base-content/50 text-center",
          "{strings.mood.sound_formats_hint}"
        }
      }
    }
}

#[component]
pub fn MoodPage() -> Element {
    // Use global state instead of local signal
    let mut ambiance_player = use_signal(|| AmbiancePlayerState::new());

    // Reads the config signal, so choosing a language re-renders the page with
    // the new text. Not memoized: a memo would keep the first language.
    let strings = use_strings();

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

    // Mood's mute also changes from the tray, and the global silence from the
    // audio engine (a call starting, Ctrl+Alt+M), neither of which goes through
    // this page. Check the two flags while the page is open and pick up a change.
    let mut refresh_trigger_for_watch = refresh_trigger;
    use_future(move || async move {
        loop {
            Delay::new(Duration::from_millis(500)).await;
            let local = ambiance_player.peek();
            let shown = (local.is_muted, local.silenced);
            drop(local);
            if ambiance_mute_flags().is_some_and(|flags| flags != shown) {
                let next = *refresh_trigger_for_watch.peek() + 1;
                refresh_trigger_for_watch.set(next);
            }
        }
    });

    rsx! {
      div { class: "{crate::utils::spacing::SECTION_SPACING_LG}",
        PageHeader {
          title: strings.mood.page_title.to_string(),
          subtitle: strings.mood.page_subtitle.to_string(),
          icon: Some(rsx! {
            CloudSunRain { class: "w-8 h-8 mx-auto" }
          }),
        }

        SoundSelectionPanel { ambiance_player, refresh_trigger }
      }
    }
}
