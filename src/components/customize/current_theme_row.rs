//! The theme on screen, at the top of Customize: its colours and name, and
//! the buttons that share it, import another one, or find more online.

use crate::components::theme_share::{ pick_theme_file, share_current };
use crate::libs::theme::{ BuiltInTheme, Theme };
use crate::utils::config::use_config;
use crate::utils::theme::use_themes;
use dioxus::prelude::*;
use lucide_dioxus::{ Download, ExternalLink, Share2 };

const FIND_MORE_URL: &str =
    "https://mechvibes.com/themes?utm_source=mechvibes&utm_medium=app&utm_campaign=theme_creator";

#[component]
pub fn CurrentThemeRow() -> Element {
    let (config, _) = use_config();
    let (themes, _) = use_themes();
    // The save dialog and the writing take a moment; one share at a time.
    let mut sharing = use_signal(|| false);

    let share = move |_| {
        if *sharing.peek() {
            return;
        }
        sharing.set(true);
        let (config, themes) = (config(), themes());
        spawn(async move {
            share_current(config, themes).await;
            sharing.set(false);
        });
    };

    // The swatch takes the colours from the theme's own name or CSS, the way
    // the colour list draws its buttons. Custom colours that are gone show as
    // the default colours, which is what the screen falls back to.
    let list = themes();
    let (name, data_theme, css) = match &config().theme {
        Theme::BuiltIn(theme) => (format!("{:?}", theme), theme.to_daisy_theme(), String::new()),
        Theme::Custom(id) =>
            match list.get_theme_by_id(id) {
                Some(theme) => (theme.name.clone(), id.clone(), theme.css.clone()),
                None => (format!("{:?}", BuiltInTheme::System), BuiltInTheme::System.to_daisy_theme(), String::new()),
            }
    };

    rsx! {
      div { class: "card bg-base-200 border border-base-300 p-3 space-y-3",
        div { class: "flex items-center gap-3",
          div {
            class: "bg-base-100 grid shrink-0 grid-cols-2 gap-0.5 rounded-md p-1 shadow-sm",
            "data-theme": data_theme,
            style: css,
            div { class: "bg-primary size-2 rounded-full" }
            div { class: "bg-secondary size-2 rounded-full" }
            div { class: "bg-warning size-2 rounded-full" }
            div { class: "bg-success size-2 rounded-full" }
          }
          div { class: "min-w-0 truncate font-medium text-base-content", "{name}" }
        }
        div { class: "grid grid-cols-3 gap-2",
          button { class: "btn btn-sm btn-soft", disabled: sharing(), onclick: share,
            if sharing() {
              span { class: "loading loading-spinner loading-sm" }
            } else {
              Share2 { class: "w-4 h-4" }
            }
            "Share"
          }
          button {
            class: "btn btn-sm btn-soft",
            onclick: move |_| {
                spawn(pick_theme_file());
            },
            Download { class: "w-4 h-4" }
            "Import"
          }
          a {
            class: "btn btn-sm btn-soft",
            href: FIND_MORE_URL,
            target: "_blank",
            rel: "noopener noreferrer",
            ExternalLink { class: "w-4 h-4" }
            "Find more"
          }
        }
      }
    }
}
