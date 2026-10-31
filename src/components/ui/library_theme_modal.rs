//! Asks before putting on a theme that a `mechvibes://theme/<id>` link points
//! at, then downloads it and puts it on the way Import does.
//!
//! Mounted once at the root, so a link opens it whatever page is showing. A
//! link only names a theme; nothing is downloaded until the user has seen its
//! name, author and what it changes, and clicked Use this theme.

use crate::components::theme_share::put_on;
use crate::components::ui::library_install_modal::format_size;
use crate::libs::theme::use_theme;
use crate::utils::config::use_config;
use crate::utils::library_client::{ self, LibraryTheme };
use crate::utils::theme::use_themes;
use crate::utils::theme_share::{ self, ImportParts };
use dioxus::prelude::*;
use lucide_dioxus::Download;

#[derive(Debug, Clone, PartialEq)]
enum Stage {
    Hidden,
    Loading,
    Confirm(LibraryTheme),
    Working(LibraryTheme),
    Done(String),
    Failed(String),
}

#[component]
pub fn LibraryThemeModal() -> Element {
    let (_, update_config) = use_config();
    let (_, update_themes) = use_themes();
    let theme_signal = use_theme();
    let mut stage = use_signal(|| Stage::Hidden);

    // Each link that arrives opens the dialog, unless one is already asking
    // or working, so a page cannot swap the theme under the user's cursor.
    use_hook(move || {
        spawn(async move {
            let Some(mut ids) = crate::utils::deep_link::take_theme_receiver() else {
                return;
            };
            while let Some(id) = ids.recv().await {
                if matches!(*stage.peek(), Stage::Loading | Stage::Confirm(_) | Stage::Working(_)) {
                    continue;
                }
                stage.set(Stage::Loading);
                stage.set(match library_client::fetch_theme(&id).await {
                    Ok(theme) => Stage::Confirm(theme),
                    Err(e) => Stage::Failed(e),
                });
            }
        })
    });

    let apply = move |_| {
        let Stage::Confirm(theme) = stage() else {
            return;
        };
        stage.set(Stage::Working(theme.clone()));
        let (update_config, update_themes) = (update_config.clone(), update_themes.clone());
        spawn(async move {
            let result = match library_client::download_theme(&theme).await {
                Ok(bytes) => {
                    // The same checks a file picked with Import goes through.
                    let opened = tokio::task
                        ::spawn_blocking(move || theme_share::from_bytes(&bytes)).await
                        .unwrap_or_else(|_| Err("Opening stopped unexpectedly.".to_string()));
                    match opened {
                        Ok(mut look) => {
                            // The name the user just saw, not the one inside the file.
                            look.rename(&theme.name);
                            put_on(look, ImportParts::ALL, update_config, update_themes, theme_signal).await
                        }
                        Err(e) => Err(e),
                    }
                }
                Err(e) => Err(e),
            };
            stage.set(match result {
                Ok(name) => Stage::Done(format!("{} is on. Enjoy the new look!", name)),
                Err(e) => Stage::Failed(e),
            });
        });
    };

    let close = move |_| {
        if !matches!(*stage.peek(), Stage::Loading | Stage::Working(_)) {
            stage.set(Stage::Hidden);
        }
    };

    let current = stage();
    if current == Stage::Hidden {
        return rsx! {};
    }
    let busy = matches!(current, Stage::Loading | Stage::Working(_));

    rsx! {
      div { class: "modal modal-open",
        div { class: "modal-box",
          h3 { class: "font-bold text-lg mb-4",
            if matches!(current, Stage::Done(_)) { "Theme installed" } else { "Install theme" }
          }

          match &current {
              Stage::Loading => rsx! {
                div { class: "flex items-center gap-3 text-sm text-base-content/70",
                  span { class: "loading loading-spinner loading-sm" }
                  "Looking up the theme..."
                }
              },
              Stage::Confirm(theme) | Stage::Working(theme) => rsx! {
                div { class: "card border border-base-300 bg-base-200 p-4 space-y-1",
                  div { class: "font-medium text-base-content", "{theme.name}" }
                  if let Some(author) = &theme.author {
                    div { class: "text-sm text-base-content/70", "by {author}" }
                  }
                  ul { class: "text-sm text-base-content/70 list-disc ml-5",
                    li { "Colors: {theme.colors_label}" }
                    if theme.has_logo {
                      li { "Logo style" }
                    }
                    if theme.has_background {
                      li { "Background" }
                    }
                  }
                  div { class: "text-xs text-base-content/60", "{format_size(theme.size)}" }
                }
                div { class: "text-sm text-base-content/70 mt-3",
                  "This replaces your current theme, logo and background. Your own themes stay in the list."
                }
              },
              Stage::Done(message) => rsx! {
                div { class: "alert alert-success alert-soft", "{message}" }
              },
              Stage::Failed(message) => rsx! {
                div { class: "alert alert-error alert-soft", "{message}" }
              },
              Stage::Hidden => rsx! {},
          }

          div { class: "modal-action mt-6",
            button { class: "btn btn-sm btn-ghost", disabled: busy, onclick: close,
              if matches!(current, Stage::Confirm(_)) { "Cancel" } else { "Close" }
            }
            match &current {
                Stage::Confirm(_) => rsx! {
                  button { class: "btn btn-sm btn-neutral", onclick: apply,
                    Download { class: "w-4 h-4 mr-2" }
                    "Use this theme"
                  }
                },
                Stage::Working(_) => rsx! {
                  button { class: "btn btn-sm btn-neutral", disabled: true,
                    span { class: "loading loading-spinner loading-sm mr-2" }
                    "Installing..."
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
