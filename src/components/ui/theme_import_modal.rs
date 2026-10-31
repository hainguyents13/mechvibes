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
//! unticked part leaves the user's own as it is. The installed theme keeps
//! every part either way, so it can be put on whole later. Once a theme is
//! on, Undo puts back the look from before it.
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
use crate::utils::theme_library::ThemeSource;
use crate::utils::theme_share::{ self, checklist, preview_stripes, ImportParts, SharedColors, SharedTheme };
use dioxus::prelude::*;
use lucide_dioxus::{ Download, Image as ImageIcon, Palette, Stamp, Undo2 };
use std::rc::Rc;

/// A theme read and checked, waiting for the user's answer.
#[derive(Debug, PartialEq)]
struct Offer {
    theme: SharedTheme,
    /// Where the theme came from, so installing it again finds its card.
    source: ThemeSource,
    /// The picture that stands for the theme, as a `data:` URI, or `None` to
    /// show its colours.
    preview: Option<String>,
    /// Known only for a theme from the library.
    author: Option<String>,
    description: Option<String>,
    size: Option<u64>,
}

/// A theme with its picture, made off the window thread with the reading,
/// since shrinking a large wallpaper takes a moment.
fn with_preview(theme: SharedTheme) -> (SharedTheme, Option<String>) {
    let preview = theme.preview_data_uri();
    (theme, preview)
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
                    // The fingerprint is read off the window thread too, as it
                    // hashes every image.
                    let opened = tokio::task
                        ::spawn_blocking(move || {
                            theme_share::open(&path).map(|theme| (theme.fingerprint(), with_preview(theme)))
                        }).await
                        .unwrap_or_else(|_| Err("Opening stopped unexpectedly.".to_string()));
                    advance(stage, request, number, match opened {
                        Ok((fingerprint, (theme, preview))) =>
                            Stage::Confirm(
                                Rc::new(Offer {
                                    theme,
                                    source: ThemeSource::File { fingerprint },
                                    preview,
                                    author: None,
                                    description: None,
                                    size: None,
                                })
                            ),
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
                                ::spawn_blocking(move || theme_share::from_bytes(&bytes).map(with_preview)).await
                                .unwrap_or_else(|_| Err("Opening stopped unexpectedly.".to_string())),
                        Err(e) => Err(e),
                    };
                    advance(stage, request, number, match opened {
                        Ok((mut theme, preview)) => {
                            // The name the library page showed, not the one inside the file.
                            theme.rename(&listed.name);
                            // The id is the one the link asked for, which the
                            // library's answer had to match, never one a file names.
                            let source = ThemeSource::Library {
                                id: listed.id.clone(),
                                checksum: listed.checksum.trim().to_lowercase(),
                            };
                            Stage::Confirm(
                                Rc::new(Offer {
                                    theme,
                                    source,
                                    preview,
                                    author: listed.author,
                                    description: listed.description,
                                    size: Some(listed.size),
                                })
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
        let ticked = parts().within(offer.theme.parts());
        if !ticked.any() {
            return;
        }
        // Exactly the theme the dialog showed; nothing is read again.
        let (theme, source) = (offer.theme.clone(), offer.source.clone());
        stage.set(Stage::Working);
        let (update_config, update_themes) = (update_config.clone(), update_themes.clone());
        spawn(async move {
            stage.set(match put_on(theme, ticked, source, update_config, update_themes, theme_signal).await {
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
                // A card of its own for each request. Dioxus keeps any inline
                // style property a changed `style` leaves out, so a picture
                // reused for the next theme could keep colours of the last one.
                for number in [request()] {
                  OfferCard { key: "{number}", offer: offer.clone() }
                }
                div { class: "space-y-3 mt-3",
                  for row in checklist(&offer.theme, &config()) {
                    label {
                      key: "{row.part:?}",
                      class: if row.in_theme { "flex items-start gap-3 cursor-pointer" } else { "flex items-start gap-3" },
                      input {
                        r#type: "checkbox",
                        class: "checkbox",
                        // A part the theme leaves out cannot be ticked.
                        disabled: !row.in_theme,
                        checked: row.in_theme && parts().has(row.part),
                        onchange: move |evt| {
                            let ticked = parts().with(row.part, evt.checked());
                            parts.set(ticked);
                        },
                      }
                      div { class: if row.in_theme { "min-w-0" } else { "min-w-0 opacity-50" },
                        div { class: "text-sm text-base-content", "{row.label}" }
                        if let Some(note) = row.note {
                          div { class: "text-xs text-base-content/60", "{note}" }
                        }
                      }
                    }
                  }
                }
                div { class: "text-xs text-base-content/60 mt-3", "Your current theme stays in the Themes tab." }
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
                Stage::Confirm(offer) => rsx! {
                  button {
                    class: "btn btn-sm btn-neutral",
                    disabled: !parts().within(offer.theme.parts()).any(),
                    onclick: apply,
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

/// The picture's frame has the app window's shape at its default size
/// (470 x 820 pt), as on the theme library's cards.
const FRAME_STYLE: &str = "width: 6rem; aspect-ratio: 47 / 82; align-self: flex-start;";
const FRAME_CLASS: &str = "shrink-0 overflow-hidden rounded-lg border border-base-300 bg-base-100";

/// The theme the way the theme library's card shows it: its picture, its
/// name, who made it, its description and what it holds. The picture is the
/// file's whole look, whatever the checklist below has ticked.
#[component]
fn OfferCard(offer: Rc<Offer>) -> Element {
    let theme = &offer.theme;
    let stripes = preview_stripes(&theme.colors);
    // A built-in theme's colours come from its daisyUI theme. Custom colours
    // are set on the frame itself, from CSS the file check already cut down
    // to colour and size variables, so they reach only the stripes.
    let (data_theme, colors_style) = match &theme.colors {
        SharedColors::BuiltIn(built_in) => (Some(built_in.to_daisy_theme()), String::new()),
        SharedColors::Custom { css } => (None, css.replace('\n', " ")),
    };

    rsx! {
      div { class: "card border border-base-300 bg-base-200 p-4",
        div { class: "flex gap-4",
          if let Some(src) = &offer.preview {
            div { class: FRAME_CLASS, style: FRAME_STYLE,
              img { class: "w-full h-full object-cover", src: "{src}", alt: "" }
            }
          } else if stripes.is_empty() {
            div {
              class: "{FRAME_CLASS} flex items-center justify-center",
              style: FRAME_STYLE,
              Palette { class: "w-6 h-6 text-base-content/50" }
            }
          } else {
            div {
              class: "{FRAME_CLASS} flex flex-col",
              style: "{FRAME_STYLE} {colors_style}",
              "data-theme": data_theme,
              for stripe in stripes {
                span { key: "{stripe}", class: "w-full flex-1 {stripe}" }
              }
            }
          }

          div { class: "flex min-w-0 flex-1 flex-col gap-2",
            div {
              div { class: "truncate text-sm font-medium text-base-content", title: "{theme.name}",
                "{theme.name}"
              }
              if let Some(author) = &offer.author {
                div { class: "truncate text-xs text-base-content/60", "by {author}" }
              }
            }
            if let Some(description) = &offer.description {
              div {
                class: "text-sm text-base-content/70",
                style: "display: -webkit-box; -webkit-box-orient: vertical; -webkit-line-clamp: 2; overflow: hidden; overflow-wrap: anywhere;",
                "{description}"
              }
            }
            div {
              class: "flex flex-wrap items-center text-xs text-base-content/60",
              style: "column-gap: 0.75rem; row-gap: 0.25rem;",
              span { class: "badge badge-sm", "{theme.colors_label()}" }
              if theme.logo.is_some() {
                span { class: "flex items-center gap-1",
                  Stamp { class: "w-3 h-3" }
                  "Logo"
                }
              }
              if theme.background.is_some() {
                span { class: "flex items-center gap-1",
                  ImageIcon { class: "w-3 h-3" }
                  "Background"
                }
              }
              if let Some(size) = offer.size {
                span { "{format_size(size)}" }
              }
            }
          }
        }
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

    fn asking() -> Stage {
        let bytes = include_bytes!("../../utils/theme_share/fixtures/v1/built-in-plain.mvtheme");
        let theme = theme_share::from_bytes(bytes).unwrap();
        let source = ThemeSource::File { fingerprint: theme.fingerprint() };
        Stage::Confirm(Rc::new(Offer { theme, source, preview: None, author: None, description: None, size: None }))
    }

    #[test]
    fn a_new_request_waits_until_the_dialog_is_free() {
        for stage in [Stage::Opening, Stage::LookingUp, Stage::Downloading, asking(), Stage::Working] {
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
        assert!(!asking().cancellable(), "nothing runs while the dialog asks");
        assert!(!Stage::Working.cancellable(), "half a theme is never left on");
        assert!(!Stage::Done("Night".to_string()).cancellable());
    }
}
