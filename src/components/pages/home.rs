use crate::components::logo::Logo;
use crate::components::soundpack_selector::{ KeyboardSoundpackSelector, MouseSoundpackSelector };
use crate::components::volume_slider::{ KeyboardVolumeSlider, MouseVolumeSlider };
use crate::libs::AudioContext;
use crate::utils::config::use_config;
use crate::libs::tray_service::request_tray_update;
use dioxus::prelude::*;
use futures_timer::Delay;
use lucide_dioxus::{ ExternalLink, Pencil, Volume2 };
use std::sync::atomic::{ AtomicU64, Ordering };
use std::sync::Arc;
use std::time::Duration;

#[component]
pub fn HomePage(audio_ctx: Arc<AudioContext>) -> Element {
    // Use shared config hook
    let (config, update_config) = use_config();

    // Volume states from config
    let mut volume = use_signal(|| config().volume);
    let mut mouse_volume = use_signal(|| config().mouse_volume);
    let enable_sound = use_memo(move || config().enable_sound);

    // Use atomic counters to track save tasks and cancel old ones
    let save_counter = use_signal(|| Arc::new(AtomicU64::new(0)));
    let mouse_save_counter = use_signal(|| Arc::new(AtomicU64::new(0)));

    // Update audio system volume when the volume control changes (enable_sound is handled by sound_manager)
    let ctx = audio_ctx.clone();
    use_effect(move || {
        ctx.set_volume(volume());
    });

    // Update audio system mouse volume when the mouse volume control changes
    let ctx = audio_ctx.clone();
    use_effect(move || {
        ctx.set_mouse_volume(mouse_volume());
    });

    // Debounce effect for saving keyboard volume config changes.
    //
    // Only the trailing call writes. `update_config` goes straight through to
    // `config_writer::apply`, which serialises the whole config and does a
    // write-plus-rename on every call, so publishing on each slider step put
    // dozens of synchronous disk writes on the UI thread during one drag and
    // made every click feel stuck.
    //
    // `spawn` registers the task in this scope's `spawned_tasks`, so
    // navigating away unmounts HomePage and `Runtime::remove_scope` cancels it
    // mid-`Delay`. The volume the engine is already playing at is not lost by
    // that: `use_effect` above pushes it to the audio context synchronously,
    // and the slider re-seeds from this signal, which keeps the dragged value
    // for the life of the session either way.
    {
        let update_config = update_config.clone();
        use_effect(move || {
            let current_volume = volume();

            // Increment counter to invalidate previous save tasks
            let current_task_id = save_counter().fetch_add(1, Ordering::SeqCst) + 1;

            let update_config = update_config.clone();
            let save_counter_clone = save_counter();

            spawn(async move {
                // Short enough that letting go of the slider and immediately
                // switching tabs still lands the write, long enough that a
                // drag - whose input events arrive milliseconds apart -
                // collapses into one.
                Delay::new(Duration::from_millis(150)).await;

                // Check if this task is still the latest one
                if save_counter_clone.load(Ordering::SeqCst) == current_task_id {
                    // This is still the latest task, save the config
                    update_config(
                        Box::new(move |config| {
                            config.volume = current_volume;
                        })
                    );
                }
                // If not the latest, this task was "cancelled" by a newer one
            });
        });
    }

    // Debounce effect for saving mouse volume config changes. Same reasoning as
    // the keyboard volume above.
    {
        let update_config = update_config.clone();
        use_effect(move || {
            let current_mouse_volume = mouse_volume();

            // Increment counter to invalidate previous save tasks
            let current_task_id = mouse_save_counter().fetch_add(1, Ordering::SeqCst) + 1;

            let update_config = update_config.clone();
            let mouse_save_counter_clone = mouse_save_counter();

            spawn(async move {
                // Short enough that letting go of the slider and immediately
                // switching tabs still lands the write, long enough that a
                // drag - whose input events arrive milliseconds apart -
                // collapses into one.
                Delay::new(Duration::from_millis(150)).await;

                // Check if this task is still the latest one
                if mouse_save_counter_clone.load(Ordering::SeqCst) == current_task_id {
                    // This is still the latest task, save the config
                    update_config(
                        Box::new(move |config| {
                            config.mouse_volume = current_mouse_volume;
                        })
                    );
                }
                // If not the latest, this task was "cancelled" by a newer one
            });
        });
    }

    rsx! {
      div { class: "flex flex-col gap-10 px-3 pb-0",
        div { class: "mb-2 mt-4",
          // Mechvibes logo with animated press effect
          Logo {}
        }
        // Main content for home page
        div { class: "flex flex-col {crate::utils::spacing::GAP_SPACING}",
          div { class: "{crate::utils::spacing::SECTION_SPACING}",
            KeyboardSoundpackSelector {}
            KeyboardVolumeSlider {
              volume,
              on_change: move |new_volume: f32| {
                  volume.set(new_volume);
              },
            }
          }
          div { class: "divider m-0" }
          div { class: "{crate::utils::spacing::SECTION_SPACING}",
            // Mouse soundpack selector and volume control
            MouseSoundpackSelector {}
            MouseVolumeSlider {
              volume: mouse_volume,
              on_change: move |new_mouse_volume: f32| {
                  mouse_volume.set(new_mouse_volume);
              },
            }
          }
          div { class: "divider m-0" }
          div { class: "{crate::utils::spacing::SECTION_SPACING}",
            // Same header shape as the Keyboard/Mouse sections above: icon in
            // primary, label beside it, control on the right. Written out
            // rather than using Toggler, which stacks its description under
            // the title and so cannot put the hotkey next to it.
            label { class: "flex items-center justify-between w-full cursor-pointer",
              div { class: "flex items-center gap-2 text-sm font-bold text-base-content/80",
                span { class: "text-primary",
                  Volume2 { class: "w-4 h-4" }
                }
                "Enable all sounds"
                span { class: "text-xs font-normal text-base-content/50", "Ctrl+Alt+M" }
              }
              input {
                r#type: "checkbox",
                class: "toggle toggle-sm toggle-primary",
                checked: enable_sound(),
                onchange: {
                    let update_config = update_config.clone();
                    let audio_ctx = audio_ctx.clone();
                    move |evt: Event<FormData>| {
                        let new_value = evt.checked();
                        // The engine caches this flag; a config write alone
                        // would leave it playing until restart. Go through the
                        // audio context so the engine is notified too.
                        audio_ctx.set_sound_enabled(new_value);
                        update_config(
                            Box::new(move |config| {
                                config.enable_sound = new_value;
                            }),
                        );
                        request_tray_update();
                    }
                },
              }
            }
          }
          div { class: "divider m-0" }
          div { class: "{crate::utils::spacing::SECTION_SPACING}",
            div { class: "flex items-center justify-between w-full",
              div { class: "flex items-center gap-2 text-sm font-bold text-base-content/80",
                span { class: "text-primary",
                  Pencil { class: "w-4 h-4" }
                }
                "Sound pack editor"
              }
              a {
                class: "btn btn-soft btn-xs rounded-box",
                href: "https://editor.mechvibes.com/?utm_source=mechvibes&utm_medium=app&utm_campaign=home",
                target: "_blank",
                "Open"
                ExternalLink { class: "w-3 h-3 ml-1" }
              }
            }
          }
        }
      }
    }
}
