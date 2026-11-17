use crate::components::customize::look_editor::{ use_look_editor, Card };
use crate::components::ui::{ Collapse, PageHeader, Toggler };
use crate::components::device_selector::AudioOutputSelector;
use crate::i18n::{ Language, use_strings };
use crate::libs::AudioContext;
use crate::libs::theme::{ use_theme, BuiltInTheme, Theme };
use crate::libs::tray_service::request_tray_update;
use crate::utils::config::use_config;
use crate::utils::constants::{ APP_NAME_DISPLAY, APP_NAME };
use crate::utils::auto_updater::{ check_for_updates_simple, UpdateInfo };
use crate::state::app::use_update_info_setter;
use crate::utils::time::{ format_relative_time_in, now_secs };
use dioxus::prelude::*;
use lucide_dioxus::{ Heart, PartyPopper, Settings };
use std::sync::Arc;

/// The noun for the place the app hides to while its window is hidden: the
/// menu bar on macOS, the tray elsewhere.
///
/// Both words are catalog entries because they sit inside sentences ("Start
/// hidden in the {}", "Click the {} icon"): the platform picks the word, the
/// language picks its spelling. It was a `&'static str` constant until the copy
/// moved into the catalog, and it cannot be one any more because the value now
/// depends on the language.
fn tray_noun(strings: &'static crate::i18n::Strings) -> &'static str {
    if cfg!(target_os = "macos") {
        strings.settings.tray_noun_menu_bar
    } else {
        strings.settings.tray_noun_tray
    }
}

// The flags shown next to each language's name.
//
// SVG artwork embedded as assets, like the fonts in `components/header.rs`, and
// never emoji: Windows does not render a regional-indicator pair as a flag, so
// an emoji flag degrades to the two letters and S4 stops holding on the platform
// where most people run the app.
const US_FLAG: Asset = asset!("/assets/flags/us.svg");
const CN_FLAG: Asset = asset!("/assets/flags/cn.svg");
const ES_FLAG: Asset = asset!("/assets/flags/es.svg");

/// The flag for `language`.
fn flag_for(language: Language) -> Asset {
    match language {
        Language::En => US_FLAG,
        Language::Zh => CN_FLAG,
        Language::Es => ES_FLAG,
    }
}

#[component]
pub fn SettingsPage() -> Element {
    // Use shared config hook
    let (config, update_config) = use_config();
    // `use_strings` reads the config signal, so choosing a language re-renders
    // the page with the new copy. Never inside a `use_memo`: the memo would keep
    // the first language until the component unmounted.
    let strings = use_strings();
    // The platform word that goes inside two of the sentences below.
    let tray = tray_noun(strings);
    let look_editor = use_look_editor();

    // Mute/volume changes must reach the audio engine thread, which caches
    // these flags in its own state - a config write alone does not move them.
    let audio_ctx = use_context::<Arc<AudioContext>>();

    // Use computed signals that always reflect current config state
    let enable_volume_boost = use_memo(move || config().enable_volume_boost);
    let auto_start = use_memo(move || config().auto_start);
    let start_minimized = use_memo(move || config().start_minimized);
    let start_hidden = use_memo(move || config().start_hidden);
    let floating_dock = use_memo(move || config().floating_dock);
    let mute_while_mic_in_use = use_memo(move || config().mute_while_mic_in_use);
    let mute_mood_with_sounds = use_memo(move || config().mute_mood_with_sounds);
    let enable_telemetry = use_memo(move || config().enable_telemetry);
    let auto_update_config = use_memo(move || config().auto_update.clone());

    // Update states
    let mut update_info = use_signal(|| None::<UpdateInfo>);
    let mut is_checking_updates = use_signal(|| false);
    let mut check_error = use_signal(|| None::<String>);
    let update_info_setter = use_update_info_setter();

    // Load saved update info on component mount
    use_effect(move || {
        if let Some(saved_update) = crate::utils::auto_updater::get_saved_update_info() {
            update_info.set(Some(saved_update));
        }
    });

    // Get current version for display
    let current_version = crate::utils::constants::APP_VERSION;

    // Theme state - use theme context (initialized in Layout component)
    let mut theme = use_theme();
    rsx! {
      div { class: "", // Page header
        PageHeader {
          title: strings.settings.page_title.to_string(),
          subtitle: crate::i18n::fill(strings.settings.page_subtitle, &[&APP_NAME_DISPLAY]),
          icon: Some(rsx! {
            Settings { class: "w-8 h-8 mx-auto" }
          }),
        }

        // Settings sections
        div { class: "space-y-4",
          // General Settings Section
          Collapse {
            title: strings.settings.section_general.to_string(),
            group_name: "setting-accordion".to_string(),
            default_open: true,
            content_class: "collapse-content text-sm",
            children: rsx! {
              div { class: "{crate::utils::spacing::SECTION_SPACING_LG}",
                // "Enable all sounds" lives on the Home page now, next to the
                // volume sliders it affects.
                // Volume Boost
                Toggler {
                  title: strings.settings.volume_boost_title.to_string(),
                  description: Some(
                      strings.settings.volume_boost_description.to_string(),
                  ),
                  checked: enable_volume_boost(),
                  on_change: {
                      let update_config = update_config.clone();
                      move |new_value: bool| {
                          update_config(
                              Box::new(move |config| {
                                  config.enable_volume_boost = new_value;
                                  if !new_value {
                                      if config.volume > 1.0 {
                                          config.volume = 1.0;
                                      }
                                      if config.mouse_volume > 1.0 {
                                          config.mouse_volume = 1.0;
                                      }
                                  }
                              }),
                          );
                      }
                  },
                }
                // Mute while the microphone is in use
                Toggler {
                  title: strings.settings.mute_mic_title.to_string(),
                  description: Some(
                      crate::i18n::fill(strings.settings.mute_mic_description, &[&APP_NAME]),
                  ),
                  checked: mute_while_mic_in_use(),
                  on_change: {
                      let update_config = update_config.clone();
                      move |new_value: bool| {
                          update_config(
                              Box::new(move |config| {
                                  config.mute_while_mic_in_use = new_value;
                              }),
                          );
                      }
                  },
                }
                // Mute Mood along with the app
                Toggler {
                  title: strings.settings.mute_mood_title.to_string(),
                  description: Some(
                      strings.settings.mute_mood_description.to_string(),
                  ),
                  checked: mute_mood_with_sounds(),
                  on_change: {
                      let update_config = update_config.clone();
                      move |new_value: bool| {
                          update_config(
                              Box::new(move |config| {
                                  config.mute_mood_with_sounds = new_value;
                              }),
                          );
                          crate::state::ambiance::set_mood_follows_mute(new_value);
                      }
                  },
                }
                // Auto Start
                Toggler {
                  title: strings.settings.auto_start_title.to_string(),
                  description: Some(
                      crate::i18n::fill(strings.settings.auto_start_description, &[&APP_NAME]),
                  ),
                  checked: auto_start(),
                  on_change: {
                      let update_config = update_config.clone();
                      move |new_value: bool| {
                          update_config(
                              Box::new(move |config| {
                                  config.auto_start = new_value;
                              }),
                          );
                          // Persist the preference to disk now, before the login item is
                          // touched.
                          crate::state::config_writer::flush_to_disk();
                          spawn(async move {
                              match crate::utils::auto_startup::set_auto_startup(new_value) {
                                  Ok(_) => {
                                      let status = if new_value { "enabled" } else { "disabled" };
                                      crate::always_print!("✅ Auto startup {}", status);
                                  }
                                  Err(e) => {
                                      crate::always_eprint!("❌ Failed to set auto startup: {}", e);
                                      // macOS: registration can fail (not running from an
                                      // app bundle, or the user must approve it under Login
                                      // Items). Don't leave the toggle on for a login item
                                      // that does not exist.
                                      #[cfg(target_os = "macos")]
                                      if new_value {
                                          crate::state::config_writer::apply(|config| {
                                              config.auto_start = false;
                                          });
                                          crate::state::config_writer::flush_to_disk();
                                      }
                                  }
                              }
                          });
                      }
                  },
                }
                // Start Minimized (only show when auto start is enabled, and
                // not when every start is hidden anyway)
                if auto_start() && !start_hidden() {
                  Toggler {
                    title: strings.settings.start_minimized_title.to_string(),
                    description: Some(strings.settings.start_minimized_description.to_string()),
                    checked: start_minimized(),
                    on_change: {
                        let update_config = update_config.clone();
                        move |new_value: bool| {
                            update_config(
                                Box::new(move |config| {
                                    config.start_minimized = new_value;
                                }),
                            );
                            crate::state::config_writer::flush_to_disk();
                            spawn(async move {
                                if crate::state::config_writer::current().auto_start {
                                    match crate::utils::auto_startup::set_auto_startup(true) {
                                        Ok(_) => {
                                            let status = if new_value {
                                                "with minimized flag"
                                            } else {
                                                "without minimized flag"
                                            };
                                            crate::always_print!("✅ Auto startup updated {}", status);
                                        }
                                        Err(e) => {
                                            crate::always_eprint!("❌ Failed to update auto startup: {}", e);
                                        }
                                    }
                                }
                            });
                        }
                    },
                  }
                }
                // Start Hidden: every start, not only at login
                Toggler {
                  title: crate::i18n::fill(strings.settings.start_hidden_title, &[&tray]),
                  description: Some(
                      crate::i18n::fill(
                          strings.settings.start_hidden_description,
                          &[&APP_NAME, &tray, &APP_NAME],
                      ),
                  ),
                  checked: start_hidden(),
                  on_change: {
                      let update_config = update_config.clone();
                      move |new_value: bool| {
                          update_config(
                              Box::new(move |config| {
                                  config.start_hidden = new_value;
                              }),
                          );
                      }
                  },
                }
                // Floating dock: the bottom bar as a pill
                Toggler {
                  title: strings.settings.floating_dock_title.to_string(),
                  description: Some(
                      strings.settings.floating_dock_description.to_string(),
                  ),
                  checked: floating_dock(),
                  on_change: {
                      let update_config = update_config.clone();
                      move |new_value: bool| {
                          update_config(
                              Box::new(move |config| {
                                  config.floating_dock = new_value;
                              }),
                          );
                      }
                  },
                }
                // Language. Last row of General on purpose: it changes every
                // label on the page, so it sits after the settings people
                // actually come here for instead of in front of them.
                div { class: "space-y-2",
                  div { class: "text-sm font-medium text-base-content",
                    "{strings.settings.language_section_title}"
                  }
                  div { class: "bg-base-100 px-4 py-3 rounded-box space-y-2",
                    // "System" is not a fourth language: it stores `None`, which
                    // means "follow the OS" (see `i18n::resolve`). It gets no
                    // flag (S6), only an invisible separator of the flag's
                    // width, so its label starts on the same x as the other
                    // three.
                    label { class: "flex items-center gap-3 rounded-lg hover:bg-base-200 cursor-pointer",
                      input {
                        r#type: "radio",
                        name: "app-language",
                        class: "radio radio-xs radio-primary",
                        checked: config().language.is_none(),
                        onchange: {
                            let update_config = update_config.clone();
                            move |_| {
                                update_config(Box::new(|config| { config.language = None; }));
                                // The tray menu lives outside the Dioxus tree, so
                                // it is re-labelled once per choice here instead
                                // of on every render.
                                request_tray_update();
                            }
                        },
                      }
                      // The invisible separator: same width as a flag.
                      div { class: "w-5" }
                      span { class: "text-xs font-medium", "{strings.settings.language_system}" }
                    }
                    for language in Language::ALL {
                      label {
                        key: "{language.tag()}",
                        class: "flex items-center gap-3 rounded-lg hover:bg-base-200 cursor-pointer",
                        input {
                          r#type: "radio",
                          name: "app-language",
                          class: "radio radio-xs radio-primary",
                          checked: config().language == Some(language),
                          onchange: {
                              let update_config = update_config.clone();
                              move |_| {
                                  update_config(
                                      Box::new(move |config| { config.language = Some(language); }),
                                  );
                                  request_tray_update();
                              }
                          },
                        }
                        img {
                          src: flag_for(language),
                          class: "w-5 h-auto",
                          alt: "{language.native_label()}",
                        }
                        // A language's name is never translated: someone who
                        // reads only Chinese has to find their own language in
                        // the list, and "Chinese" written in English does not
                        // help them. `Language::native_label` is the source.
                        span { class: "text-xs font-medium", "{language.native_label()}" }
                      }
                    }
                  }
                }
              }
            },
          }
          // Devices Section
          Collapse {
            title: strings.settings.section_devices.to_string(),
            group_name: "setting-accordion".to_string(),
            content_class: "collapse-content text-sm",
            children: rsx! {
              div { class: "space-y-2",
                // Audio Output Device
                AudioOutputSelector {}
                // Device Information
                div { class: "text-xs mt-2 text-base-content/70",
                  "{strings.settings.devices_hint}"
                }
              }
            },
          }
          // Auto-Update Section
          Collapse {
            title: strings.settings.section_updates.to_string(),
            group_name: "setting-accordion".to_string(),
            content_class: "collapse-content text-sm",
            children: rsx! {
              div { class: "space-y-4",
                p { class: "text-sm text-base-content/70",
                  if crate::utils::update_installer::silent_update_supported() {
                    "{strings.settings.updates_silent_hint}"
                  } else {
                    "{strings.settings.updates_manual_hint}"
                  }
                }
                div { class: "flex items-center gap-3",
                  button {
                    class: "btn btn-soft btn-sm",
                    disabled: is_checking_updates(),
                    onclick: move |_| {
                        crate::always_print!("Manual update check requested");
                        is_checking_updates.set(true);
                        check_error.set(None);
                        let mut update_info = update_info.clone();
                        let mut is_checking_updates = is_checking_updates.clone();
                        let mut check_error = check_error.clone();
                        let update_info_setter = update_info_setter.clone();
                        spawn(async move {
                            match check_for_updates_simple().await {
                                Ok(info) => {
                                    update_info.set(Some(info.clone()));
                                    // Only this check's own fields are written.
                                    // The network round trip above gave the
                                    // user time to change anything else, and a
                                    // mutation cannot carry a config across it.
                                    let available = info.update_available.then(|| (
                                        info.latest_version.clone(),
                                        info.download_url.clone(),
                                    ));
                                    let checked_at = std::time::SystemTime::now()
                                        .duration_since(std::time::SystemTime::UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_secs();
                                    crate::state::config_writer::apply(move |config| {
                                        config.auto_update.last_check = Some(checked_at);
                                        match available {
                                            Some((version, download_url)) => {
                                                config.auto_update.available_version = Some(version);
                                                config.auto_update.available_download_url =
                                                    download_url;
                                            }
                                            None => {
                                                config.auto_update.available_version = None;
                                                config.auto_update.available_download_url = None;
                                            }
                                        }
                                    });
                                    if info.update_available {
                                        update_info_setter.call(Some(info));
                                    } else {
                                        update_info_setter.call(None);
                                    }
                                    check_error.set(None);
                                }
                                Err(e) => {
                                    // Only the reason is stored: the sentence
                                    // around it is read from the catalog at
                                    // render time, so the alert is not left in
                                    // the language it was raised in.
                                    check_error.set(Some(e.to_string()));
                                    update_info.set(None);
                                    update_info_setter.call(None);
                                }
                            }
                            is_checking_updates.set(false);
                        });
                    },
                    if is_checking_updates() {
                      span { class: "loading loading-spinner loading-xs mr-1" }
                    }
                    if is_checking_updates() {
                      "{strings.settings.checking}"
                    } else {
                      "{strings.settings.check_for_updates}"
                    }
                  } // Display last check time
                  if let Some(last_check) = auto_update_config().last_check {
                    div { class: "text-xs text-base-content/60",
                      {crate::i18n::fill(strings.settings.last_checked, &[&format_relative_time_in(&strings.home, last_check, now_secs())])}
                    }
                  } else {
                    div { class: "text-xs text-base-content/60", "{strings.settings.never_checked}" }
                  }
                }
                // Display update status
                if let Some(error) = check_error() {
                  div { class: "alert alert-error text-sm",
                    "❌ {crate::i18n::fill(strings.settings.update_check_failed, &[&error])}"
                  }
                } else if let Some(info) = update_info() {
                  if info.update_available {
                    // Not a DaisyUI `alert` on purpose: its grid layout packs
                    // the icon, text, button and link into one flow and lets
                    // the link wrap around the button. A plain box with
                    // explicit rows keeps the spacing predictable.
                    div { class: "rounded-box border border-success/30 bg-success/10 p-4 space-y-3 text-sm",
                      div { class: "flex items-center gap-2 font-medium text-success",
                        PartyPopper { class: "w-5 h-5 shrink-0" }
                        span { {crate::i18n::fill(strings.settings.update_available, &[&info.latest_version])} }
                      }
                      // Stacked on purpose: the settings column is ~400px and
                      // the install button alone nearly fills it, so anything
                      // placed beside the button is permanently cramped.
                      div { class: "space-y-2",
                        // Owns the whole download -> verify -> restart
                        // sequence, including its own error states.
                        crate::components::update_button::UpdateInstallButton {
                          info: info.clone(),
                        }
                        if let Some(release_url) = &info.release_notes {
                          div {
                            a {
                              href: "{release_url}",
                              target: "_blank",
                              class: "link link-hover text-xs text-base-content/70",
                              "{strings.settings.view_release_notes}"
                            }
                          }
                        }
                      }
                    }
                  } else {
                    div { class: "alert alert-success alert-soft ",
                      {crate::i18n::fill(strings.settings.up_to_date, &[&info.current_version])}
                    }
                  }
                } else {
                  // A saved update from a previous session. Read through
                  // get_saved_update_info(), never straight off the config:
                  // it drops any entry that is not actually newer than the
                  // running build. Reading the raw field here is what once
                  // made an upgraded app advertise an update to itself.
                  if let Some(saved) = crate::utils::auto_updater::get_saved_update_info() {
                    // Same structure as the fresh-check card above, so the
                    // two states are visually identical.
                    div { class: "rounded-box border border-success/30 bg-success/10 p-4 space-y-3 text-sm",
                      div { class: "flex items-center gap-2 font-medium text-success",
                        PartyPopper { class: "w-5 h-5 shrink-0" }
                        span { {crate::i18n::fill(strings.settings.update_available, &[&saved.latest_version])} }
                      }
                      div { class: "space-y-2",
                        crate::components::update_button::UpdateInstallButton {
                          info: saved.clone(),
                        }
                        div {
                          a {
                            href: "https://github.com/hainguyents13/mechvibes/releases/tag/v{saved.latest_version}",
                            target: "_blank",
                            class: "link link-hover text-xs text-base-content/70",
                            "{strings.settings.view_release_notes}"
                          }
                        }
                      }
                      p { class: "text-xs text-base-content/60",
                        {crate::i18n::fill(strings.settings.current_version, &[&current_version])}
                      }
                    }
                  }
                }
              }
            },
          }
          // Privacy Section
          Collapse {
            title: strings.settings.section_privacy.to_string(),
            group_name: "setting-accordion".to_string(),
            content_class: "collapse-content text-sm",
            children: rsx! {
              div { class: "{crate::utils::spacing::SECTION_SPACING_LG}",
                Toggler {
                  title: strings.settings.telemetry_title.to_string(),
                  description: Some(
                      strings.settings.telemetry_description.to_string(),
                  ),
                  checked: enable_telemetry(),
                  on_change: {
                      let update_config = update_config.clone();
                      move |new_value: bool| {
                          update_config(
                              Box::new(move |config| {
                                  config.enable_telemetry = new_value;
                              }),
                          );
                      }
                  },
                }
              }
            },
          }
          // App info Section
          Collapse {
            title: strings.settings.section_app_info.to_string(),
            group_name: "setting-accordion".to_string(),
            content_class: "collapse-content text-sm",
            children: rsx! {
              crate::components::app_info::AppInfoDisplay {}
            },
          }
          // Debug Section
          //
          // `lazy_children` is required, not cosmetic: this Collapse is a CSS
          // accordion, so without it the viewer's poll loop would be mounted
          // and running for every user whether or not they ever open Debug.
          Collapse {
            title: strings.settings.section_debug.to_string(),
            group_name: "setting-accordion".to_string(),
            content_class: "collapse-content text-sm",
            lazy_children: true,
            children: rsx! {
              crate::components::debug_log_viewer::DebugLogViewer {}
            },
          }
          // Danger Zone Section
          Collapse {
            title: strings.settings.section_danger_zone.to_string(),
            group_name: "setting-accordion".to_string(),
            title_class: "collapse-title font-semibold text-error",
            variant: "border border-base-300 bg-base-200",
            content_class: "collapse-content text-sm",
            children: rsx! {
              p { class: "mb-4 text-base-content/70",
                "{strings.settings.danger_zone_body}"
              }
              div { class: " justify-start",
                button {
                  class: "btn btn-error btn-soft btn-sm",
                  onclick: {
                      let update_config = update_config.clone();
                      let audio_ctx = audio_ctx.clone();
                      let look_editor = look_editor.clone();
                      move |_| {
                          // The Default theme, as its card puts it on: the look
                          // on screen is saved as a theme first, and the logo
                          // and background settings wait for later.
                          if look_editor.apply_card(&Card::Default).is_ok() {
                              theme.set(Theme::BuiltIn(BuiltInTheme::System));
                          }
                          // Push the audio-affecting defaults through the
                          // engine as well, otherwise a reset would restore
                          // the config but leave the running engine muted /
                          // at the old volume until the next restart.
                          audio_ctx.set_sound_enabled(true);
                          audio_ctx.set_volume(1.0);
                          update_config(
                              Box::new(|config| {
                                  config.volume = 1.0;
                                  config.enable_sound = true;
                                  config.enable_volume_boost = false;
                                  config.auto_start = false;
                              }),
                          );
                          request_tray_update();
                      }
                  },
                  "{strings.settings.reset_to_defaults}"
                }
              }
            },
          }
          // Footer: app name, version and credits
          div { class: "text-center space-y-2 mt-8 pb-4",
            div { class: "text-sm text-base-content/70 font-bold",
              "{APP_NAME_DISPLAY} (v{current_version})"
            }
            div { class: "text-xs leading-relaxed text-base-content/50",
              span { "{strings.settings.made_with} " }
              Heart { class: "inline w-3.5 h-3.5 -mt-1 text-primary/70 fill-primary/30" }
              span { " {strings.settings.made_by} " }
              a {
                href: "https://github.com/hainguyents13/mechvibes",
                target: "_blank",
                class: "link ",
                "hainguyents13"
              }
              br {}
              " {strings.settings.footer_and} "
              a {
                href: "https://github.com/hainguyents13/mechvibes/graphs/contributors?all=1",
                target: "_blank",
                class: "link ",
                "{strings.settings.footer_contributors}"
              }
            }
          }
        }
      }
    }
}
