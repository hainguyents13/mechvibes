//! The Share and Import buttons for a whole look, the dialog Share opens, and
//! putting an opened look on.
//!
//! Share saves the theme colours, the logo and the background, images
//! included, to one `.mvtheme` file. Import hands the file it picks to the
//! import dialog at the root (`ui::theme_import_modal`), which shows what the
//! file holds and puts it on only after the user agrees.

use crate::libs::theme::Theme;
use crate::state::config::AppConfig;
use crate::utils::config::use_config;
use crate::utils::look_undo::{ self, LookFields };
use crate::utils::notice::{ self, NoticeKind };
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
    Failed(String),
}

impl Stage {
    /// Work is running that will set the next stage itself.
    fn busy(&self) -> bool {
        matches!(self, Stage::Sharing)
    }
}

/// Put the ticked `parts` of an opened look on: their images saved, custom
/// colours added to the list, then the theme and the rest of the config
/// switched over. An unticked part brings nothing and changes nothing. Gives
/// the look's name back. Images saved before a failure are removed again. The
/// look from before is remembered, so `undo_look` can put it back.
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
    // The look is remembered inside the write, so `before` is exactly what
    // the config held right before this theme went on.
    update_config(
        Box::new(move |cfg| {
            let before = LookFields::of(cfg);
            theme_share::apply_to_config(&look, choice, parts, cfg);
            look_undo::remember(before, LookFields::of(cfg));
        })
    );
    Ok(name)
}

/// Go back to the look from before the last theme was put on, if nothing has
/// changed it since. Tells the user when there is nothing to go back to.
/// Returns whether the look went back.
pub fn undo_look(
    update_config: &Rc<dyn Fn(Box<dyn FnOnce(&mut AppConfig)>)>,
    mut theme_signal: Signal<Theme>
) -> bool {
    let restored = Rc::new(RefCell::new(None));
    update_config(
        Box::new({
            let restored = restored.clone();
            move |cfg| {
                if let Some(before) = look_undo::take_undo(&LookFields::of(cfg)) {
                    before.write_to(cfg);
                    *restored.borrow_mut() = Some(before.theme);
                }
            }
        })
    );
    let restored = restored.borrow_mut().take();
    match restored {
        Some(theme) => {
            theme_signal.set(theme);
            true
        }
        None => {
            notice::report(NoticeKind::Info, "Nothing to undo. The look changed since.", None);
            false
        }
    }
}

#[component]
pub fn ThemeShareButtons() -> Element {
    let (config, _) = use_config();
    let (themes, _) = use_themes();
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

    // The import dialog at the root reads the file and asks.
    let open_import = move |_| {
        spawn(async move {
            if
                let Some(handle) = rfd::AsyncFileDialog
                    ::new()
                    .add_filter("Mechvibes theme", &[FILE_EXTENSION])
                    .set_title("Import theme")
                    .pick_file().await
            {
                crate::utils::deep_link::deliver_theme_file(handle.path().to_path_buf());
            }
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
          h3 { class: "font-bold text-lg mb-4", "Share theme" }

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
              Stage::Failed(message) => rsx! {
                div { class: "alert alert-error alert-soft", "{message}" }
              },
              Stage::Hidden => rsx! {},
          }

          div { class: "modal-action mt-6",
            button { class: "btn btn-sm btn-ghost", disabled: busy, onclick: close,
              if current == Stage::Share { "Cancel" } else { "Close" }
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
                _ => rsx! {},
            }
          }
        }
        div { class: "modal-backdrop", onclick: close }
      }
      }
    }
}
