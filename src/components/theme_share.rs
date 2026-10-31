//! The Share and Import buttons for a whole look, and the dialog they open.
//!
//! Share saves the theme colours, the logo and the background, images
//! included, to one `.mvtheme` file. Import shows what a file holds and puts
//! it on only after the user agrees.

use crate::libs::theme::{ use_theme, Theme };
use crate::state::config::AppConfig;
use crate::utils::config::use_config;
use crate::utils::theme::{ use_themes, UpdateThemes };
use crate::utils::theme_share::{ self, ImportParts, Look, SharedColors, SharedTheme, FILE_EXTENSION };
use dioxus::prelude::*;
use lucide_dioxus::{ Download, Share2 };
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
enum Stage {
    Hidden,
    Share,
    Sharing,
    Shared(String),
    Opening,
    Confirm(Rc<SharedTheme>),
    Applying,
    Applied(String),
    Failed(String),
}

impl Stage {
    /// Work is running that will set the next stage itself.
    fn busy(&self) -> bool {
        matches!(self, Stage::Sharing | Stage::Opening | Stage::Applying)
    }
}

/// Put the ticked `parts` of an opened look on: their images saved, custom
/// colours added to the list, then the theme and the rest of the config
/// switched over. An unticked part brings nothing and changes nothing. Gives
/// the look's name back. Images saved before a failure are removed again.
pub async fn put_on(
    mut look: SharedTheme,
    parts: ImportParts,
    update_config: Rc<dyn Fn(Box<dyn FnOnce(&mut AppConfig)>)>,
    update_themes: UpdateThemes,
    mut theme_signal: Signal<Theme>
) -> Result<String, String> {
    look.keep_parts(parts);
    let (look, written) = tokio::task
        ::spawn_blocking(move || theme_share::install_images(&mut look).map(|written| (look, written))).await
        .unwrap_or_else(|_| Err("Importing stopped unexpectedly.".to_string()))?;

    let choice = match &look.colors {
        _ if !parts.colors => None,
        SharedColors::BuiltIn(theme) => Some(Theme::BuiltIn(theme.clone())),
        SharedColors::Custom { css } => {
            let added = Rc::new(RefCell::new(None));
            let saved = update_themes(
                Box::new({
                    let added = added.clone();
                    let (name, css) = (look.name.clone(), css.clone());
                    move |themes| {
                        *added.borrow_mut() = Some(themes.import_theme(&name, &css));
                    }
                })
            );
            let added = saved.and_then(|()| {
                added.borrow_mut().take().unwrap_or_else(|| Err("Could not save the theme.".to_string()))
            });
            match added {
                Ok(id) => Some(Theme::Custom(id)),
                Err(e) => {
                    theme_share::remove_images(&written);
                    return Err(e);
                }
            }
        }
    };

    if let Some(choice) = &choice {
        theme_signal.set(choice.clone());
    }
    let name = look.name.clone();
    update_config(Box::new(move |cfg| theme_share::apply_to_config(&look, choice, parts, cfg)));
    Ok(name)
}

#[component]
pub fn ThemeShareButtons() -> Element {
    let (config, update_config) = use_config();
    let (themes, update_themes) = use_themes();
    let theme_signal = use_theme();
    let mut stage = use_signal(|| Stage::Hidden);
    let mut name = use_signal(String::new);

    let open_share = move |_| {
        name.set(theme_share::default_name(&config(), &themes()));
        stage.set(Stage::Share);
    };

    let save = move |_| {
        let chosen_name = name();
        spawn(async move {
            let suggested = format!("{}.{}", chosen_name.trim(), FILE_EXTENSION);
            let Some(handle) = rfd::AsyncFileDialog
                ::new()
                .add_filter("Mechvibes theme", &[FILE_EXTENSION])
                .set_file_name(&suggested)
                .set_title("Save theme")
                .save_file().await else {
                return;
            };
            let mut path: PathBuf = handle.path().to_path_buf();
            if path.extension().and_then(|e| e.to_str()) != Some(FILE_EXTENSION) {
                path.set_extension(FILE_EXTENSION);
            }
            // The dialog was closed, or another one opened, while the file
            // picker was up.
            if *stage.peek() != Stage::Share {
                return;
            }
            stage.set(Stage::Sharing);
            let look = Look::from_config(&config(), &themes());
            let result = match theme_share::gather(&chosen_name, &look).await {
                Ok(look) => {
                    let path = path.clone();
                    tokio::task
                        ::spawn_blocking(move || theme_share::save(&look, &path)).await
                        .unwrap_or_else(|_| Err("Saving stopped unexpectedly.".to_string()))
                }
                Err(e) => Err(e),
            };
            stage.set(match result {
                Ok(()) =>
                    Stage::Shared(
                        path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default()
                    ),
                Err(e) => Stage::Failed(e),
            });
        });
    };

    let open_import = move |_| {
        spawn(async move {
            let Some(handle) = rfd::AsyncFileDialog
                ::new()
                .add_filter("Mechvibes theme", &[FILE_EXTENSION])
                .set_title("Import theme")
                .pick_file().await else {
                return;
            };
            let path = handle.path().to_path_buf();
            if stage.peek().busy() {
                return;
            }
            stage.set(Stage::Opening);
            let opened = tokio::task
                ::spawn_blocking(move || theme_share::open(&path)).await
                .unwrap_or_else(|_| Err("Opening stopped unexpectedly.".to_string()));
            stage.set(match opened {
                Ok(look) => Stage::Confirm(Rc::new(look)),
                Err(e) => Stage::Failed(e),
            });
        });
    };

    let apply = move |_| {
        let Stage::Confirm(look) = stage() else {
            return;
        };
        let look = (*look).clone();
        stage.set(Stage::Applying);
        let (update_config, update_themes) = (update_config.clone(), update_themes.clone());
        spawn(async move {
            stage.set(match put_on(look, ImportParts::ALL, update_config, update_themes, theme_signal).await {
                Ok(name) => Stage::Applied(name),
                Err(e) => Stage::Failed(e),
            });
        });
    };

    // Closing is refused while work runs, so a finished task always lands in
    // the dialog that started it.
    let close = move |_| {
        if !stage.peek().busy() {
            stage.set(Stage::Hidden);
        }
    };

    let current = stage();
    let busy = current.busy();
    let cfg = config();

    rsx! {
      button { class: "btn btn-ghost btn-sm", onclick: open_share,
        Share2 { class: "w-4 h-4 mr-1" }
        "Share"
      }
      button { class: "btn btn-ghost btn-sm", onclick: open_import,
        Download { class: "w-4 h-4 mr-1" }
        "Import"
      }

      if current != Stage::Hidden {
      div { class: "modal modal-open",
        div { class: "modal-box",
          h3 { class: "font-bold text-lg mb-4",
            match &current {
                Stage::Share | Stage::Sharing | Stage::Shared(_) => "Share theme",
                Stage::Applied(_) => "Theme imported",
                _ => "Import theme",
            }
          }

          match &current {
              Stage::Share | Stage::Sharing => rsx! {
                div { class: "space-y-4",
                  label { class: "form-control w-full",
                    div { class: "label",
                      span { class: "label-text", "Theme name" }
                    }
                    input {
                      r#type: "text",
                      class: "input input-sm w-full",
                      maxlength: "64",
                      disabled: busy,
                      value: name(),
                      oninput: move |evt| name.set(evt.value()),
                    }
                  }
                  div { class: "text-sm text-base-content/70 space-y-1",
                    div { "The file includes:" }
                    ul { class: "list-disc ml-5",
                      li {
                        "Colors: "
                        match &cfg.theme {
                            Theme::BuiltIn(theme) => format!("{:?}", theme),
                            Theme::Custom(_) => "your custom theme".to_string(),
                        }
                      }
                      if cfg.enable_logo_customization {
                        li { "Your logo style and images" }
                      }
                      if cfg.enable_background_customization {
                        li { "Your background" }
                      }
                    }
                    div { class: "text-xs text-base-content/50 pt-1",
                      "Send the file to a friend. They open it with Import in Customize."
                    }
                  }
                }
              },
              Stage::Shared(file_name) => rsx! {
                div { class: "alert alert-success alert-soft",
                  "Saved as {file_name}. Send it to a friend, they open it with Import in Customize."
                }
              },
              Stage::Opening => rsx! {
                div { class: "flex items-center gap-3 text-sm text-base-content/70",
                  span { class: "loading loading-spinner loading-sm" }
                  "Opening the theme..."
                }
              },
              Stage::Confirm(look) => rsx! {
                div { class: "card border border-base-300 bg-base-200 p-4 space-y-1",
                  div { class: "font-medium text-base-content", "{look.name}" }
                  ul { class: "text-sm text-base-content/70 list-disc ml-5",
                    li { "Colors: {look.colors_label()}" }
                    if let Some(logo) = &look.logo {
                      li {
                        if logo.use_background_image || logo.use_muted_background_image {
                          "Logo style with images"
                        } else {
                          "Logo style"
                        }
                      }
                    }
                    if let Some(background) = &look.background {
                      li {
                        if background.use_image { "Background image" } else { "Background color" }
                      }
                    }
                  }
                }
                div { class: "text-sm text-base-content/70 mt-3",
                  "This replaces your current theme, logo and background. Your own themes stay in the list."
                }
              },
              Stage::Applying => rsx! {
                div { class: "flex items-center gap-3 text-sm text-base-content/70",
                  span { class: "loading loading-spinner loading-sm" }
                  "Importing..."
                }
              },
              Stage::Applied(applied) => rsx! {
                div { class: "alert alert-success alert-soft", "{applied} is on. Enjoy the new look!" }
              },
              Stage::Failed(message) => rsx! {
                div { class: "alert alert-error alert-soft", "{message}" }
              },
              Stage::Hidden => rsx! {},
          }

          div { class: "modal-action mt-6",
            button { class: "btn btn-sm btn-ghost", disabled: busy, onclick: close,
              if matches!(current, Stage::Share | Stage::Confirm(_)) { "Cancel" } else { "Close" }
            }
            match &current {
                Stage::Share | Stage::Sharing => rsx! {
                  button {
                    class: "btn btn-sm btn-neutral",
                    disabled: busy || name().trim().is_empty(),
                    onclick: save,
                    if busy {
                      span { class: "loading loading-spinner loading-sm mr-2" }
                      "Saving..."
                    } else {
                      Share2 { class: "w-4 h-4 mr-2" }
                      "Save file"
                    }
                  }
                },
                Stage::Confirm(_) => rsx! {
                  button { class: "btn btn-sm btn-neutral", onclick: apply,
                    Download { class: "w-4 h-4 mr-2" }
                    "Use this theme"
                  }
                },
                _ => rsx! {},
            }
          }
        }
        div { class: "modal-backdrop", onclick: close }
      }
      }
    }
}
