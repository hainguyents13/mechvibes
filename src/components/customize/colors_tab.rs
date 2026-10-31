//! The Colors tab: the ready-made colors, the user's own, and the editor
//! for new ones.

use crate::components::customize::colors_editor_modal::{ ColorsEditor, ColorsEditorModal };
use crate::components::ui::ConfirmModal;
use crate::libs::theme::{ BuiltInTheme, Theme };
use crate::state::config::AppConfig;
use crate::state::themes::ThemesConfig;
use crate::utils::config::use_config;
use crate::utils::theme::use_themes;
use dioxus::prelude::*;
use lucide_dioxus::{ Ellipsis, Palette, Pencil, Trash2 };

/// After colors are deleted: if they were the ones in use, the app goes back
/// to the default colors rather than pointing at colors that are gone.
fn after_deleting_colors(config: &mut AppConfig, id: &str) {
    if config.theme == Theme::Custom(id.to_string()) {
        config.theme = Theme::BuiltIn(BuiltInTheme::System);
    }
}

#[component]
pub fn ColorsTab() -> Element {
    let (config, update_config) = use_config();
    let (themes, update_themes) = use_themes();
    let mut editor = use_signal(|| None::<ColorsEditor>);
    // The id and name of the colors waiting for the user to confirm their
    // deletion.
    let mut deleting = use_signal(|| None::<(String, String)>);

    let current = config().theme.clone();
    let list = themes();
    let mine = list.list_themes();

    // Only the config is written; the layout follows it.
    let put_on = {
        let update_config = update_config.clone();
        move |theme: Theme| update_config(Box::new(move |cfg| cfg.theme = theme))
    };

    rsx! {
      div { class: "space-y-6",
        div { class: "text-sm text-base-content/70", "Pick the app colors, or make your own" }
        div { class: "space-y-2",
          div { class: "text-sm text-base-content", "Ready-made colors" }
          div { class: "grid grid-cols-3 gap-2",
            for built_in in BuiltInTheme::all() {
              {
                  let is_active = current == Theme::BuiltIn(built_in.clone());
                  let put_on = put_on.clone();
                  rsx! {
                    button {
                      class: if is_active { "btn btn-soft text-left pl-2 justify-start text-xs btn-disabled" } else { "btn btn-soft text-left pl-2 justify-start text-xs" },
                      onclick: {
                          let built_in = built_in.clone();
                          move |_| put_on(Theme::BuiltIn(built_in.clone()))
                      },
                      ColorDots {
                        data_theme: built_in.to_daisy_theme(),
                        css: String::new(),
                        dimmed: is_active,
                      }
                      div { class: "line-clamp-1", {format!("{:?}", built_in)} }
                    }
                  }
              }
            }
          }
        }
        div { class: "space-y-2",
          div { class: "text-sm text-base-content", "My colors" }
          if mine.is_empty() {
            div { class: "text-sm text-base-content/50", "You have not made any colors yet" }
          }
          for colors in mine.iter() {
            {
                let id = colors.id.clone();
                let name = colors.name.clone();
                let is_active = current == Theme::Custom(id.clone());
                let put_on = put_on.clone();
                rsx! {
                  div { class: "flex w-full max-w-full items-center gap-2",
                    button {
                      class: if is_active { "gap-3 flex btn btn-soft px-2 grow text-left justify-start btn-disabled" } else { "gap-3 flex btn btn-soft px-2 grow text-left justify-start" },
                      onclick: {
                          let id = id.clone();
                          move |_| put_on(Theme::Custom(id.clone()))
                      },
                      ColorDots {
                        data_theme: id.clone(),
                        css: colors.css.clone(),
                        dimmed: is_active,
                      }
                      div { class: "line-clamp-1", "{name}" }
                    }
                    div { class: "dropdown dropdown-left dropdown-center",
                      div {
                        class: "btn btn-ghost",
                        tabindex: "0",
                        role: "button",
                        title: "More",
                        Ellipsis { class: "w-4 h-4" }
                      }
                      ul {
                        class: "dropdown-content menu bg-base-100 rounded-box z-1 p-2 shadow-sm",
                        tabindex: "0",
                        li {
                          a {
                            onclick: {
                                let id = id.clone();
                                move |_| editor.set(Some(ColorsEditor::Edit(id.clone())))
                            },
                            Pencil { class: "w-4 h-4 mr-1" }
                            "Edit"
                          }
                        }
                        li {
                          a {
                            class: "text-error",
                            onclick: {
                                let (id, name) = (id.clone(), name.clone());
                                move |_| deleting.set(Some((id.clone(), name.clone())))
                            },
                            Trash2 { class: "w-4 h-4 mr-1" }
                            "Delete"
                          }
                        }
                      }
                    }
                  }
                }
            }
          }
          button {
            class: "btn btn-neutral btn-sm mt-2",
            onclick: move |_| editor.set(Some(ColorsEditor::New)),
            Palette { class: "w-4 h-4 mr-1" }
            "New colors"
          }
        }
        if let Some(open) = editor() {
          ColorsEditorModal { editor: open, on_close: move |_| editor.set(None) }
        }
        if let Some((id, name)) = deleting() {
          ConfirmModal {
            title: format!("Delete {}?", name),
            message: "This cannot be undone.".to_string(),
            confirm_label: "Delete".to_string(),
            on_confirm: {
                let update_themes = update_themes.clone();
                let update_config = update_config.clone();
                move |_| {
                    deleting.set(None);
                    // A failed save is shown by the updater itself, and then
                    // the colors are still there, so the config keeps them.
                    let deleted = update_themes(
                        Box::new({
                            let id = id.clone();
                            move |themes: &mut ThemesConfig| {
                                let _ = themes.delete_theme(&id);
                            }
                        }),
                    );
                    if deleted.is_ok() {
                        let id = id.clone();
                        update_config(Box::new(move |cfg| after_deleting_colors(cfg, &id)));
                    }
                }
            },
            on_cancel: move |_| deleting.set(None),
          }
        }
      }
    }
}

/// Four dots in the colors of a theme, drawn from its daisyUI name or its CSS.
#[component]
fn ColorDots(data_theme: String, css: String, dimmed: bool) -> Element {
    rsx! {
      div {
        class: if dimmed { "bg-base-100 grid shrink-0 grid-cols-2 gap-0.5 rounded-md p-1 shadow-sm opacity-30" } else { "bg-base-100 grid shrink-0 grid-cols-2 gap-0.5 rounded-md p-1 shadow-sm" },
        "data-theme": data_theme,
        style: css,
        div { class: "bg-primary size-2 rounded-full" }
        div { class: "bg-secondary size-2 rounded-full" }
        div { class: "bg-warning size-2 rounded-full" }
        div { class: "bg-success size-2 rounded-full" }
      }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deleting_the_colors_in_use_falls_back_to_default() {
        let mut config = AppConfig::default();
        config.theme = Theme::Custom("night".to_string());
        after_deleting_colors(&mut config, "day");
        assert_eq!(config.theme, Theme::Custom("night".to_string()), "other colors in use stay on");

        after_deleting_colors(&mut config, "night");
        assert_eq!(config.theme, Theme::BuiltIn(BuiltInTheme::System));
    }
}
