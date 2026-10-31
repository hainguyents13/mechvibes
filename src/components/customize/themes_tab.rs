//! The Themes tab: the default look, a way to find more themes, and how to
//! use a theme file.

use crate::components::customize::current_theme_row::FIND_MORE_URL;
use crate::libs::theme::{ BuiltInTheme, Theme };
use crate::state::config::AppConfig;
use crate::utils::config::use_config;
use crate::utils::look_undo::{ self, LookFields };
use crate::utils::notice::{ self, NoticeAction, NoticeKind };
use dioxus::prelude::*;
use lucide_dioxus::ExternalLink;
use std::cell::Cell;
use std::rc::Rc;

/// How to use a theme file. A double-click opens one in Mechvibes on Windows
/// and Linux only: macOS has no file type registered for it yet.
const THEME_FILE_HINT: &str = if cfg!(target_os = "macos") {
    "Got a theme file? Drop it on this window, or use Import."
} else {
    "Got a theme file? Double-click it, or drop it on this window."
};

/// The look the app starts with: the default colours, and the theme's own
/// logo and background. The user's logo and background settings stay, so
/// switching them back on brings them back.
fn put_on_default_look(config: &mut AppConfig) {
    config.theme = Theme::BuiltIn(BuiltInTheme::System);
    config.enable_logo_customization = false;
    config.enable_background_customization = false;
}

fn is_default_look(config: &AppConfig) -> bool {
    config.theme == Theme::BuiltIn(BuiltInTheme::System) &&
        !config.enable_logo_customization &&
        !config.enable_background_customization
}

#[component]
pub fn ThemesTab() -> Element {
    let (config, update_config) = use_config();
    let is_default = is_default_look(&config());

    let use_default = move |_| {
        // Remembered inside the write, like a theme being put on, so Undo
        // goes back to exactly what the config held. A click while the
        // default look is already on changes nothing and keeps the way back.
        let changed = Rc::new(Cell::new(false));
        update_config(
            Box::new({
                let changed = changed.clone();
                move |cfg| {
                    let before = LookFields::of(cfg);
                    put_on_default_look(cfg);
                    changed.set(look_undo::remember(before, LookFields::of(cfg)));
                }
            })
        );
        if changed.get() {
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
    fn the_default_look_keeps_your_logo_and_background_settings() {
        let mut config = AppConfig::default();
        config.theme = Theme::BuiltIn(BuiltInTheme::Synthwave);
        config.enable_logo_customization = true;
        config.logo_customization.text_color = "red".to_string();
        config.enable_background_customization = true;
        config.background_customization.background_color = "#202020".to_string();
        let logo = config.logo_customization.clone();
        let background = config.background_customization.clone();
        assert!(!is_default_look(&config));

        put_on_default_look(&mut config);
        assert!(is_default_look(&config));
        assert_eq!(config.logo_customization, logo, "the logo settings are kept for later");
        assert_eq!(config.background_customization, background, "and so are the background ones");
    }

    #[test]
    fn the_theme_file_hint_offers_a_double_click_only_where_it_opens_the_file() {
        assert_eq!(THEME_FILE_HINT.contains("Double-click"), !cfg!(target_os = "macos"));
    }
}
