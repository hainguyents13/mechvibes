//! The Themes tab: the default look, a way to find more themes, and how to
//! use a theme file.

use crate::components::customize::current_theme_row::FIND_MORE_URL;
use crate::components::customize::look_editor::{ use_look_editor, Card };
use crate::libs::theme::BuiltInTheme;
use crate::utils::config::use_config;
use crate::utils::notice::{ self, NoticeAction, NoticeKind };
use crate::utils::theme_library::shows_default;
use dioxus::prelude::*;
use lucide_dioxus::ExternalLink;

/// How to use a theme file. A double-click opens one in Mechvibes on Windows
/// and Linux only: macOS has no file type registered for it yet.
const THEME_FILE_HINT: &str = if cfg!(target_os = "macos") {
    "Got a theme file? Drop it on this window, or use Import."
} else {
    "Got a theme file? Double-click it, or drop it on this window."
};

#[component]
pub fn ThemesTab() -> Element {
    let (config, _) = use_config();
    let editor = use_look_editor();
    let is_default = shows_default(&config());

    // The look from before is saved first, so it can be put on again. A
    // click while the default look is already on changes nothing and keeps
    // the way back.
    let use_default = move |_| {
        if editor.apply_card(&Card::Default) == Ok(true) {
            notice::report(NoticeKind::Info, "Default look is on.", Some(NoticeAction::Undo));
        }
    };

    rsx! {
      div { class: "space-y-4",
        div { class: "grid grid-cols-2 gap-2",
          button {
            class: if is_default { "card bg-base-100 border border-primary p-3 text-left space-y-2 cursor-pointer" } else { "card bg-base-100 border border-base-300 p-3 text-left space-y-2 cursor-pointer" },
            onclick: use_default,
            div {
              class: "bg-base-100 grid grid-cols-2 gap-0.5 rounded-md p-1 shadow-sm",
              style: "width: fit-content",
              "data-theme": BuiltInTheme::System.to_daisy_theme(),
              div { class: "bg-primary size-2 rounded-full" }
              div { class: "bg-secondary size-2 rounded-full" }
              div { class: "bg-warning size-2 rounded-full" }
              div { class: "bg-success size-2 rounded-full" }
            }
            div {
              div { class: "font-medium text-base-content", "Default" }
              div { class: "text-xs text-base-content/50", "The look Mechvibes starts with" }
            }
          }
          a {
            class: "card bg-base-100 border border-base-300 p-3 text-left space-y-2",
            href: FIND_MORE_URL,
            target: "_blank",
            rel: "noopener noreferrer",
            ExternalLink { class: "w-4 h-4 text-base-content" }
            div {
              div { class: "font-medium text-base-content", "Find more" }
              div { class: "text-xs text-base-content/50", "Themes made by other people" }
            }
          }
        }
        div { class: "text-sm text-base-content/70", "{THEME_FILE_HINT}" }
      }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_theme_file_hint_offers_a_double_click_only_where_it_opens_the_file() {
        assert_eq!(THEME_FILE_HINT.contains("Double-click"), !cfg!(target_os = "macos"));
    }
}
