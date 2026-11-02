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
//! on, Done closes the dialog and Undo, kept apart from it, puts back the
//! look from before.
//!
//! Reading, looking up and downloading can be cancelled. Each request has its
//! own number, and a result that comes back after its request was cancelled
//! is dropped, so a read that never finishes cannot hold the dialog.

use crate::components::customize::theme_card::{ CardPreview, ColorDots, ThemePicture };
use crate::components::theme_share::{ put_on, undo_look };
use crate::components::ui::library_install_modal::format_size;
use crate::libs::theme::use_theme;
use crate::utils::config::use_config;
use crate::utils::deep_link;
use crate::utils::library_client;
use crate::utils::theme::use_themes;
use crate::utils::theme_library::ThemeSource;
use crate::utils::theme_share::{ self, after_import, checklist, ImportParts, Look, Part, SharedTheme };
use dioxus::prelude::*;
use lucide_dioxus::{ Check, Download, Image as ImageIcon, Minus, Stamp, Undo2 };
use std::rc::Rc;

/// A theme read and checked, waiting for the user's answer.
#[derive(Debug, PartialEq)]
struct Offer {
    theme: SharedTheme,
    /// Where the theme came from, so installing it again finds its card.
    source: ThemeSource,
    /// The theme's look with its images shrunk into `data:` URIs, for the
    /// picture.
    preview: Look,
    /// Known only for a theme from the library.
    size: Option<u64>,
}

/// A theme with its picture, made off the window thread with the reading,
/// since shrinking a large wallpaper takes a moment.
fn with_preview(theme: SharedTheme) -> (SharedTheme, Look) {
    let preview = theme.preview_look();
    (theme, preview)
}

/// A theme just put on, for the dialog to say what changed.
#[derive(Debug, PartialEq)]
struct Applied {
    offer: Rc<Offer>,
    /// The parts put on; the rest stayed the user's own.
    parts: ImportParts,
}

/// A line of the list saying what a theme changed.
#[derive(Debug, PartialEq)]
struct ChangeRow {
    label: String,
    /// The theme's part went on. Otherwise the user's own stayed.
    changed: bool,
}

/// One line per part: what the theme brought for each part put on, and the
/// part's name for each one that stayed the user's own.
fn changes(theme: &SharedTheme, parts: ImportParts) -> [ChangeRow; 3] {
    let row = |part: Part, name: &str, label: Option<String>| match label.filter(|_| parts.has(part)) {
        Some(label) => ChangeRow { label, changed: true },
        None => ChangeRow { label: format!("Kept your {}", name), changed: false },
    };
    [
        row(Part::Colors, "colors", Some(format!("Colors: {}", theme.colors_label()))),
        row(Part::Logo, "logo", theme.logo_label().map(str::to_string)),
        row(Part::Background, "background", theme.background_label().map(str::to_string)),
    ]
}

#[derive(Debug, Clone, PartialEq)]
enum Stage {
    Hidden,
    Opening,
    LookingUp,
    Downloading,
    Confirm(Rc<Offer>),
    Working,
    Done(Rc<Applied>),
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
    let (themes, update_themes) = use_themes();
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
                            // Its author and description there too, when it has them.
                            if listed.author.is_some() {
                                theme.author = listed.author;
                            }
                            if listed.description.is_some() {
                                theme.description = listed.description;
                            }
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
                Ok(_) => Stage::Done(Rc::new(Applied { offer, parts: ticked })),
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
                  OfferCard {
                    key: "{number}",
                    offer: offer.clone(),
                    // The screen once this theme is on with the ticked parts.
                    picture: CardPreview::of(
                        &after_import(
                            &Look::from_config(&config(), &themes()),
                            offer.preview.clone(),
                            parts().within(offer.theme.parts()),
                        ),
                    ),
                  }
                }
                div { class: "mt-4",
                  div { class: "text-sm font-medium text-base-content", "Choose what to use" }
                  div { class: "text-xs text-base-content/60", "Anything you untick stays as you have it now." }
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
              Stage::Done(applied) => rsx! {
                DoneCard {
                  applied: applied.clone(),
                  // The screen as it is now, the theme on.
                  picture: CardPreview::of(&Look::from_config(&config(), &themes())),
                }
                div { class: "text-xs text-base-content/60 mt-3",
                  "Find it any time in Customize > Themes. The look you had before is there too."
                }
              },
              Stage::Failed(message) => rsx! {
                div { class: "alert alert-error alert-soft", "{message}" }
              },
              Stage::Hidden => rsx! {},
          }

          // Undo sits at the far left, away from Done, so a quick click on the
          // main button never takes the theme off again.
          div { class: if matches!(current, Stage::Done(_)) { "modal-action mt-6 justify-between" } else { "modal-action mt-6" },
            match &current {
                Stage::Done(_) => rsx! {
                  button { class: "btn btn-sm btn-ghost", onclick: undo,
                    Undo2 { class: "w-4 h-4 mr-2" }
                    "Undo"
                  }
                  button { class: "btn btn-sm btn-neutral", onclick: close, "Done" }
                },
                _ => rsx! {
                  button {
                    class: "btn btn-sm btn-ghost",
                    disabled: current == Stage::Working,
                    onclick: close,
                    if current.cancellable() || matches!(current, Stage::Confirm(_)) { "Cancel" } else { "Close" }
                  }
                  if let Stage::Confirm(offer) = &current {
                    button {
                      class: "btn btn-sm btn-neutral",
                      disabled: !parts().within(offer.theme.parts()).any(),
                      onclick: apply,
                      Download { class: "w-4 h-4 mr-2" }
                      "Use this theme"
                    }
                  }
                },
            }
          }
        }
        div { class: "modal-backdrop", onclick: close }
      }
    }
}

/// The picture's frame has the app window's shape at its default size
/// (470 x 820 pt), as on the theme library's cards.
const FRAME_STYLE: &str = "width: 7rem; aspect-ratio: 47 / 82; align-self: flex-start;";
const FRAME_CLASS: &str = "shrink-0 rounded-lg border border-base-300";

/// The theme the way the theme library's card shows it: its picture, its
/// name, who made it, its description and what it holds. The picture is the
/// screen as it will look once the theme is on, drawn as a card in the
/// Themes tab draws a look: `picture` follows the ticks in the checklist.
#[component]
fn OfferCard(offer: Rc<Offer>, picture: CardPreview) -> Element {
    let theme = &offer.theme;

    rsx! {
      div { class: "card border border-base-300 bg-base-200 p-4",
        div { class: "flex gap-4",
          // Drawn anew whenever a tick changes what it shows.
          for key in [picture.key()] {
            ThemePicture { key: "{key}", preview: picture.clone(), class: FRAME_CLASS, style: FRAME_STYLE }
          }

          div { class: "flex min-w-0 flex-1 flex-col gap-2",
            div {
              div { class: "truncate text-sm font-medium text-base-content", title: "{theme.name}",
                "{theme.name}"
              }
              if let Some(author) = &theme.author {
                div { class: "truncate text-xs text-base-content/60", "by {author}" }
              }
            }
            // Six lines hold a usual description with its credit at the
            // app's width; a longer one is cut and shown whole on hover.
            if let Some(description) = &theme.description {
              div {
                class: "text-sm text-base-content/70",
                title: "{description}",
                style: "display: -webkit-box; -webkit-box-orient: vertical; -webkit-line-clamp: 6; overflow: hidden; overflow-wrap: anywhere;",
                "{description}"
              }
            }
            div {
              class: "flex flex-wrap items-center text-xs text-base-content/60",
              style: "column-gap: 0.75rem; row-gap: 0.25rem;",
              span { class: "badge badge-sm gap-2",
                ColorDots { preview: CardPreview::of(&offer.preview) }
                "{theme.colors_label()}"
              }
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

/// The theme once it is on: the screen now, its name and who made it, and
/// what it changed, part by part.
#[component]
fn DoneCard(applied: Rc<Applied>, picture: CardPreview) -> Element {
    let theme = &applied.offer.theme;

    rsx! {
      div { class: "card border border-base-300 bg-base-200 p-4",
        div { class: "flex gap-4",
          for key in [picture.key()] {
            ThemePicture { key: "{key}", preview: picture.clone(), class: FRAME_CLASS, style: FRAME_STYLE }
          }

          div { class: "flex min-w-0 flex-1 flex-col gap-3",
            div {
              div { class: "truncate text-sm font-medium text-base-content", title: "{theme.name}",
                "{theme.name}"
              }
              if let Some(author) = &theme.author {
                div { class: "truncate text-xs text-base-content/60", "by {author}" }
              }
            }
            div { class: "space-y-1",
              for row in changes(theme, applied.parts) {
                div {
                  key: "{row.label}",
                  class: if row.changed { "flex items-center gap-2 text-sm text-base-content" } else { "flex items-center gap-2 text-sm text-base-content/50" },
                  if row.changed {
                    Check { class: "w-3.5 h-3.5 shrink-0 text-success" }
                  } else {
                    Minus { class: "w-3.5 h-3.5 shrink-0" }
                  }
                  span { class: "min-w-0", style: "overflow-wrap: anywhere;", "{row.label}" }
                }
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

    fn offer(bytes: &[u8]) -> Rc<Offer> {
        let theme = theme_share::from_bytes(bytes).unwrap();
        let source = ThemeSource::File { fingerprint: theme.fingerprint() };
        let preview = theme.preview_look();
        Rc::new(Offer { theme, source, preview, size: None })
    }

    fn asking() -> Stage {
        Stage::Confirm(offer(include_bytes!("../../utils/theme_share/fixtures/v1/built-in-plain.mvtheme")))
    }

    fn done() -> Stage {
        let offer = offer(include_bytes!("../../utils/theme_share/fixtures/v1/built-in-plain.mvtheme"));
        Stage::Done(Rc::new(Applied { offer, parts: ImportParts::ALL }))
    }

    #[test]
    fn a_new_request_waits_until_the_dialog_is_free() {
        for stage in [Stage::Opening, Stage::LookingUp, Stage::Downloading, asking(), Stage::Working] {
            assert!(stage.busy(), "{:?}", stage);
        }
        for stage in [Stage::Hidden, done(), Stage::Failed("No".to_string())] {
            assert!(!stage.busy(), "{:?}", stage);
        }
    }

    #[test]
    fn the_done_list_says_which_parts_the_theme_changed() {
        let theme = &offer(include_bytes!("../../utils/theme_share/fixtures/v1/custom-full.mvtheme")).theme;
        let rows = changes(theme, ImportParts::ALL.with(Part::Background, false));
        assert_eq!(rows[0], ChangeRow { label: format!("Colors: {}", theme.colors_label()), changed: true });
        assert_eq!(rows[1], ChangeRow { label: theme.logo_label().unwrap().to_string(), changed: true });
        assert_eq!(rows[2], ChangeRow { label: "Kept your background".to_string(), changed: false });
    }

    #[test]
    fn a_part_the_theme_leaves_out_stays_yours_in_the_done_list() {
        let theme = &offer(include_bytes!("../../utils/theme_share/fixtures/v1/built-in-plain.mvtheme")).theme;
        let rows = changes(theme, ImportParts::ALL);
        assert!(rows[0].changed, "a theme always brings its colors");
        assert_eq!(rows[1], ChangeRow { label: "Kept your logo".to_string(), changed: false });
        assert_eq!(rows[2], ChangeRow { label: "Kept your background".to_string(), changed: false });
    }

    #[test]
    fn only_reading_looking_up_and_downloading_can_be_cancelled() {
        assert!(Stage::Opening.cancellable());
        assert!(Stage::LookingUp.cancellable());
        assert!(Stage::Downloading.cancellable());
        assert!(!asking().cancellable(), "nothing runs while the dialog asks");
        assert!(!Stage::Working.cancellable(), "half a theme is never left on");
        assert!(!done().cancellable());
    }
}
