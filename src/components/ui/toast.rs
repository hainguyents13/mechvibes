//! The messages `utils::notice` collects, shown above the dock.
//!
//! Mounted once at the root, outside the area a theme colours, so a theme
//! that makes its own text hard to read cannot hide them. A message that
//! only says something went well goes away by itself; an error stays until
//! the user closes it. A message may offer a button: Show file, or Undo,
//! which works here because a theme cannot hide it.

use crate::components::theme_share::undo_look;
use crate::libs::theme::use_theme;
use crate::utils::config::use_config;
use crate::utils::notice::{ Notice, NoticeAction, NoticeKind };
use dioxus::prelude::*;
use lucide_dioxus::X;

const MAX_SHOWN: usize = 3;
const INFO_SECONDS: u64 = 6;

/// Put `new` on screen next to the messages already there, keeping the
/// newest few. An error already showing is not shown twice, so a save that
/// keeps failing does not push every other message away.
pub fn show_notices(mut shown: Signal<Vec<Notice>>, new: Vec<Notice>) {
    let mut timed = Vec::new();
    {
        let mut list = shown.write();
        for notice in new {
            let repeated = notice.kind == NoticeKind::Error && list.iter().any(|old| old.kind == notice.kind && old.text == notice.text);
            if repeated {
                continue;
            }
            if notice.kind == NoticeKind::Info {
                timed.push(notice.id);
            }
            list.push(notice);
        }
        let extra = list.len().saturating_sub(MAX_SHOWN);
        list.drain(..extra);
    }
    for id in timed {
        spawn(async move {
            futures_timer::Delay::new(std::time::Duration::from_secs(INFO_SECONDS)).await;
            shown.write().retain(|notice| notice.id != id);
        });
    }
}

#[component]
pub fn Toasts() -> Element {
    let mut shown: Signal<Vec<Notice>> = use_context();
    let (_, update_config) = use_config();
    let theme_signal = use_theme();
    let notices = shown();
    if notices.is_empty() {
        return rsx! {};
    }

    rsx! {
      div {
        class: "toast toast-center toast-bottom",
        // Above the dock, and above an open dialog so an error from it shows.
        style: "bottom: 6rem; z-index: 1000; max-width: calc(100vw - 4rem);",
        for notice in notices {
          div {
            key: "{notice.id}",
            class: if notice.kind == NoticeKind::Error { "alert alert-error alert-soft text-sm" } else { "alert alert-success alert-soft text-sm" },
            div { class: "flex items-center gap-1 min-w-0",
              // A long file path breaks anywhere, so the buttons stay in
              // the window.
              span { class: "grow min-w-0", style: "overflow-wrap: anywhere;", "{notice.text}" }
              if let Some(NoticeAction::ShowFile(path)) = notice.action.clone() {
                button {
                  class: "btn btn-xs btn-ghost shrink-0",
                  onclick: move |_| crate::utils::log_buffer::reveal_in_file_manager(&path),
                  "Show file"
                }
              }
              if notice.action == Some(NoticeAction::Undo) {
                button {
                  class: "btn btn-xs btn-ghost shrink-0",
                  onclick: {
                      let update_config = update_config.clone();
                      move |_| {
                          shown.write().retain(|other| other.id != notice.id);
                          undo_look(&update_config, theme_signal);
                      }
                  },
                  "Undo"
                }
              }
              button {
                class: "btn btn-xs btn-ghost shrink-0",
                title: "Close",
                onclick: move |_| shown.write().retain(|other| other.id != notice.id),
                X { class: "w-3 h-3" }
              }
            }
          }
        }
      }
    }
}
