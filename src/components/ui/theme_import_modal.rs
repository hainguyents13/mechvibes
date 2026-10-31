//! The one dialog that asks before putting on a theme, whatever brought it:
//! the Import button, a theme file from anywhere else, or a
//! `mechvibes://theme/<id>` link from the theme library.
//!
//! Mounted once at the root, outside the area a theme colours, so it opens
//! whatever page is showing and stays readable whatever a theme does.
//!
//! A file is read and checked first, and the dialog lists what it holds. A
//! library link only names a theme, so the theme is looked up, downloaded and
//! checked against its checksum before the dialog asks, and what the dialog
//! lists comes from that file, not from what the library says about it.
//! Either way nothing is put on until the user clicks Use this theme, and
//! what is put on then is exactly the file the dialog showed. The dialog
//! lists the colours, the logo and the background with a tick each; an
//! unticked part brings nothing and leaves the user's own as it is. Once a
//! theme is on, Undo puts back the look from before it.
//!
//! Reading, looking up and downloading can be cancelled. Each request has its
//! own number, and a result that comes back after its request was cancelled
//! is dropped, so a read that never finishes cannot hold the dialog.

use crate::components::theme_share::{ put_on, undo_look };
use crate::components::ui::library_install_modal::format_size;
use crate::libs::theme::use_theme;
use crate::utils::config::use_config;
use crate::utils::deep_link;
use crate::utils::library_client;
use crate::utils::theme::use_themes;
use crate::utils::theme_share::{ self, checklist, ImportParts, SharedTheme };
use dioxus::prelude::*;
use lucide_dioxus::{ Download, Undo2 };
use std::rc::Rc;

/// A theme read and checked, waiting for the user's answer.
#[derive(Debug, PartialEq)]
struct Offer {
    theme: SharedTheme,
    /// Known only for a theme from the library.
    author: Option<String>,
    size: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
enum Stage {
    Hidden,
    Opening,
    LookingUp,
    Downloading,
    Confirm(Rc<Offer>),
    Working,
    Done(String),
    Failed(String),
}

impl Stage {
    /// The dialog is on a request, so a new one is ignored: a page cannot
    /// swap the theme under the user's cursor.
    fn busy(&self) -> bool {
        matches!(self, Stage::Opening | Stage::LookingUp | Stage::Downloading | Stage::Confirm(_) | Stage::Working)
    }

    /// Work runs that the user may stop. Putting a theme on is not one: it
    /// is short, and stopping it halfway would leave half a theme.
    fn cancellable(&self) -> bool {
        matches!(self, Stage::Opening | Stage::LookingUp | Stage::Downloading)
    }
}

/// Start a request at `first`, unless the dialog is busy. Gives the
/// request's number, which its results must still match to be shown.
fn begin(mut stage: Signal<Stage>, mut request: Signal<u64>, first: Stage) -> Option<u64> {
    if stage.peek().busy() {
        return None;
    }
    let number = *request.peek() + 1;
    request.set(number);
    stage.set(first);
    Some(number)
}

/// Move request `number` on to `next`. Returns false, and changes nothing,
/// once the request was cancelled or another one began.
fn advance(mut stage: Signal<Stage>, request: Signal<u64>, number: u64, next: Stage) -> bool {
    if *request.peek() != number {
        return false;
    }
    stage.set(next);
    true
}

#[component]
pub fn ThemeImportModal() -> Element {
    let (config, update_config) = use_config();
    let (_, update_themes) = use_themes();
    let theme_signal = use_theme();
    let mut stage = use_signal(|| Stage::Hidden);
    let mut request = use_signal(|| 0u64);
    // The parts ticked in the checklist. Every new request starts with all.
    let mut parts = use_signal(|| ImportParts::ALL);

    // Theme files: read and checked off the window thread. Each read is a
    // task of its own, so one that hangs never holds up the stream.
    use_hook(move || {
        spawn(async move {
            let Some(mut files) = deep_link::take_theme_file_receiver() else {
                return;
            };
            while let Some(path) = files.recv().await {
                let Some(number) = begin(stage, request, Stage::Opening) else {
                    continue;
                };
                parts.set(ImportParts::ALL);
                spawn(async move {
                    let opened = tokio::task
                        ::spawn_blocking(move || theme_share::open(&path)).await
                        .unwrap_or_else(|_| Err("Opening stopped unexpectedly.".to_string()));
                    advance(stage, request, number, match opened {
                        Ok(theme) => Stage::Confirm(Rc::new(Offer { theme, author: None, size: None })),
                        Err(e) => Stage::Failed(e),
                    });
                });
            }
        })
    });

    // Library links: looked up, downloaded and checked before the dialog asks.
    use_hook(move || {
        spawn(async move {
            let Some(mut ids) = deep_link::take_theme_receiver() else {
                return;
            };
            while let Some(id) = ids.recv().await {
                let Some(number) = begin(stage, request, Stage::LookingUp) else {
                    continue;
                };
                parts.set(ImportParts::ALL);
                spawn(async move {
                    let listed = match library_client::fetch_theme(&id).await {
                        Ok(listed) => listed,
                        Err(e) => {
                            advance(stage, request, number, Stage::Failed(e));
                            return;
                        }
                    };
                    if !advance(stage, request, number, Stage::Downloading) {
                        return;
                    }
                    let opened = match library_client::download_theme(&listed).await {
                        // The same checks a theme file goes through.
                        Ok(bytes) =>
                            tokio::task
                                ::spawn_blocking(move || theme_share::from_bytes(&bytes)).await
                                .unwrap_or_else(|_| Err("Opening stopped unexpectedly.".to_string())),
                        Err(e) => Err(e),
                    };
                    advance(stage, request, number, match opened {
                        Ok(mut theme) => {
                            // The name the library page showed, not the one inside the file.
                            theme.rename(&listed.name);
                            Stage::Confirm(
                                Rc::new(Offer { theme, author: listed.author, size: Some(listed.size) })
                            )
                        }
                        Err(e) => Stage::Failed(e),
                    });
                });
            }
        })
    });

    // Here as well as in a toast: this dialog sits outside the area a theme
    // colours, so the way back stays readable whatever the theme did.
    let undo = {
        let update_config = update_config.clone();
        move |_| {
            undo_look(&update_config, theme_signal);
            stage.set(Stage::Hidden);
        }
    };

    let apply = move |_| {
        let Stage::Confirm(offer) = stage() else {
            return;
        };
        let ticked = parts();
        if !ticked.any() {
            return;
        }
        // Exactly the theme the dialog showed; nothing is read again.
        let theme = offer.theme.clone();
        stage.set(Stage::Working);
        let (update_config, update_themes) = (update_config.clone(), update_themes.clone());
        spawn(async move {
            stage.set(match put_on(theme, ticked, update_config, update_themes, theme_signal).await {
                Ok(name) => Stage::Done(name),
                Err(e) => Stage::Failed(e),
            });
        });
    };

    // Closing while work runs cancels it: the request number moves on, so
    // its result is dropped when it comes. Putting a theme on is never
    // cancelled, so a finished one always lands in this dialog.
    let close = move |_| {
        let current = stage.peek().clone();
        if current == Stage::Working {
            return;
        }
        if current.cancellable() {
            let next = *request.peek() + 1;
            request.set(next);
        }
        stage.set(Stage::Hidden);
    };

    let current = stage();
    if current == Stage::Hidden {
        return rsx! {};
    }

    rsx! {
      div { class: "modal modal-open",
        div { class: "modal-box",
          h3 { class: "font-bold text-lg mb-4",
            match &current {
                Stage::Done(_) => "Theme is on",
                Stage::Failed(_) => "Could not use this theme",
                _ => "Use this theme?",
            }
          }

          match &current {
              Stage::Opening => rsx! {
                Waiting { text: "Opening the theme..." }
              },
              Stage::LookingUp => rsx! {
                Waiting { text: "Looking up the theme..." }
              },
              Stage::Downloading => rsx! {
                Waiting { text: "Downloading the theme..." }
              },
              Stage::Working => rsx! {
                Waiting { text: "Putting the theme on..." }
              },
              Stage::Confirm(offer) => rsx! {
                div { class: "card border border-base-300 bg-base-200 p-4 space-y-1",
                  div { class: "font-medium text-base-content", "{offer.theme.name}" }
                  if let Some(author) = &offer.author {
                    div { class: "text-sm text-base-content/70", "by {author}" }
                  }
                  if let Some(size) = offer.size {
                    div { class: "text-xs text-base-content/60", "{format_size(size)}" }
                  }
                }
                div { class: "space-y-3 mt-3",
                  for row in checklist(&offer.theme, &config()) {
                    label { key: "{row.part:?}", class: "flex items-start gap-3 cursor-pointer",
                      input {
                        r#type: "checkbox",
                        class: "checkbox",
                        checked: parts().has(row.part),
                        onchange: move |evt| {
                            let ticked = parts().with(row.part, evt.checked());
                            parts.set(ticked);
                        },
                      }
                      div { class: "min-w-0",
                        div { class: "text-sm text-base-content", "{row.label}" }
                        if let Some(note) = row.note {
                          div { class: "text-xs text-base-content/60", "{note}" }
                        }
                      }
                    }
                  }
                }
                div { class: "text-xs text-base-content/60 mt-3", "Untick a part to keep yours." }
              },
              Stage::Done(name) => rsx! {
                div { class: "alert alert-success alert-soft", "{name} is on." }
              },
              Stage::Failed(message) => rsx! {
                div { class: "alert alert-error alert-soft", "{message}" }
              },
              Stage::Hidden => rsx! {},
          }

          div { class: "modal-action mt-6",
            button {
              class: "btn btn-sm btn-ghost",
              disabled: current == Stage::Working,
              onclick: close,
              if current.cancellable() || matches!(current, Stage::Confirm(_)) { "Cancel" } else { "Close" }
            }
            match &current {
                Stage::Done(_) => rsx! {
                  button { class: "btn btn-sm btn-neutral", onclick: undo,
                    Undo2 { class: "w-4 h-4 mr-2" }
                    "Undo"
                  }
                },
                Stage::Confirm(_) => rsx! {
                  button { class: "btn btn-sm btn-neutral", disabled: !parts().any(), onclick: apply,
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

/// A spinner with a line saying what the dialog is waiting for.
#[component]
fn Waiting(text: &'static str) -> Element {
    rsx! {
      div { class: "flex items-center gap-3 text-sm text-base-content/70",
        span { class: "loading loading-spinner loading-sm" }
        "{text}"
      }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_request_waits_until_the_dialog_is_free() {
        for stage in [Stage::Opening, Stage::LookingUp, Stage::Downloading, Stage::Working] {
            assert!(stage.busy(), "{:?}", stage);
        }
        for stage in [Stage::Hidden, Stage::Done("Night".to_string()), Stage::Failed("No".to_string())] {
            assert!(!stage.busy(), "{:?}", stage);
        }
    }

    #[test]
    fn only_reading_looking_up_and_downloading_can_be_cancelled() {
        assert!(Stage::Opening.cancellable());
        assert!(Stage::LookingUp.cancellable());
        assert!(Stage::Downloading.cancellable());
        assert!(!Stage::Working.cancellable(), "half a theme is never left on");
        assert!(!Stage::Done("Night".to_string()).cancellable());
    }
}
