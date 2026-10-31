//! The editor for the user's own colors: a name and the color code.
//!
//! The code is cleaned with the same filter a theme file goes through, so
//! what is saved can only change the app's colors and the few daisyUI sizes
//! the filter knows. A failure shows inside the dialog and leaves it open,
//! so nothing typed is lost.

use crate::libs::theme::Theme;
use crate::state::themes::ThemesConfig;
use crate::utils::config::use_config;
use crate::utils::notice::{ self, NoticeKind };
use crate::utils::theme::use_themes;
use crate::utils::theme_share::{ safe_theme_css, theme_css_declarations };
use dioxus::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

/// What new colors start from: every color daisyUI uses, and its sizes.
pub(crate) const THEME_DEFAULT_CSS: &str =
    r#"/* Colors */
--color-base-100: oklch(98% 0.02 240);
--color-base-200: oklch(95% 0.03 240);
--color-base-300: oklch(92% 0.04 240);
--color-base-content: oklch(20% 0.05 240);
--color-primary: oklch(55% 0.3 240);
--color-primary-content: oklch(98% 0.01 240);
--color-secondary: oklch(70% 0.25 200);
--color-secondary-content: oklch(98% 0.01 200);
--color-accent: oklch(65% 0.25 160);
--color-accent-content: oklch(98% 0.01 160);
--color-neutral: oklch(50% 0.05 240);
--color-neutral-content: oklch(98% 0.01 240);
--color-success: oklch(65% 0.25 140);
--color-success-content: oklch(98% 0.01 140);
--color-warning: oklch(80% 0.25 80);
--color-warning-content: oklch(20% 0.05 80);
--color-error: oklch(65% 0.3 30);
--color-error-content: oklch(98% 0.01 30);

/* border radius */
--radius-selector: 1rem;
--radius-field: 0.25rem;
--radius-box: 0.5rem;

/* base sizes */
--size-selector: 0.25rem;
--size-field: 0.25rem;

/* border size */
--border: 1px;

/* effects */
--depth: 1;
--noise: 0;
          "#;

/// Which colors the editor is open for.
#[derive(Debug, Clone, PartialEq)]
pub enum ColorsEditor {
    New,
    /// The id of colors in the list.
    Edit(String),
}

/// Colors ready to save: the name, and the code with only the lines the
/// filter keeps.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CleanColors {
    pub name: String,
    pub css: String,
    /// Some lines were dropped because they are not colors this app uses.
    pub left_out: bool,
}

/// Check and clean what the user typed before it is saved.
pub(crate) fn clean_colors(name: &str, css: &str) -> Result<CleanColors, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Give it a name first.".to_string());
    }
    let clean = safe_theme_css(css);
    if clean.is_empty() {
        return Err("There are no colors here. Each line should look like --color-primary: #3b82f6;".to_string());
    }
    Ok(CleanColors {
        name: name.to_string(),
        left_out: theme_css_declarations(css) > clean.lines().count(),
        css: clean,
    })
}

/// The colors to put on after a save: new colors go on straight away, edited
/// ones stay as they are (they are already on if they were in use).
pub(crate) fn colors_after_save(editor: &ColorsEditor, saved_id: &str) -> Option<Theme> {
    match editor {
        ColorsEditor::New => Some(Theme::Custom(saved_id.to_string())),
        ColorsEditor::Edit(_) => None,
    }
}

#[component]
pub fn ColorsEditorModal(editor: ColorsEditor, on_close: EventHandler<()>) -> Element {
    let (themes, update_themes) = use_themes();
    let (_, update_config) = use_config();

    // Mounted fresh each time the editor opens, so these start from the
    // colors being edited, or from the template for new ones.
    let (start_name, start_css) = match &editor {
        ColorsEditor::New => (String::new(), THEME_DEFAULT_CSS.to_string()),
        ColorsEditor::Edit(id) =>
            themes
                .peek()
                .get_theme_by_id(id)
                .map(|theme| (theme.name.clone(), theme.css.clone()))
                .unwrap_or_default(),
    };
    let mut name = use_signal(move || start_name);
    let mut css = use_signal(move || start_css);
    let mut error = use_signal(String::new);

    let on_save = {
        let editor = editor.clone();
        move |_| {
            let clean = match clean_colors(&name(), &css()) {
                Ok(clean) => clean,
                Err(e) => {
                    error.set(e);
                    return;
                }
            };

            // What the list said about the change, such as a name in use.
            let outcome = Rc::new(RefCell::new(Ok(String::new())));
            let saved = update_themes(
                Box::new({
                    let outcome = outcome.clone();
                    let editor = editor.clone();
                    let CleanColors { name, css, .. } = clean.clone();
                    move |themes: &mut ThemesConfig| {
                        *outcome.borrow_mut() = match &editor {
                            ColorsEditor::New => themes.add_theme(name, String::new(), css),
                            ColorsEditor::Edit(id) =>
                                themes.update_theme(id, name, String::new(), css).map(|()| id.clone()),
                        };
                    }
                })
            );
            let saved_id = match saved.and_then(|()| outcome.replace(Ok(String::new()))) {
                Ok(id) => id,
                Err(e) => {
                    error.set(e);
                    return;
                }
            };

            if let Some(theme) = colors_after_save(&editor, &saved_id) {
                update_config(Box::new(move |cfg| cfg.theme = theme));
            }
            if clean.left_out {
                notice::report(NoticeKind::Info, "Some lines were left out because they are not colors.", None);
            }
            on_close.call(());
        }
    };

    rsx! {
      div { class: "modal modal-open",
        div { class: "modal-box",
          h3 { class: "font-bold text-lg mb-4",
            if editor == ColorsEditor::New {
              "New colors"
            } else {
              "Edit colors"
            }
          }
          div { class: "space-y-4",
            fieldset { class: "fieldset",
              legend { class: "fieldset-legend", "Name" }
              input {
                class: "input w-full",
                r#type: "text",
                placeholder: "My colors",
                value: name(),
                oninput: move |e| name.set(e.value()),
              }
            }
            fieldset { class: "fieldset",
              legend { class: "fieldset-legend", "Color code (advanced)" }
              textarea {
                class: "textarea w-full h-64 font-mono text-sm",
                value: css(),
                oninput: move |e| css.set(e.value()),
              }
              div { class: "label whitespace-break-spaces",
                "Change the values after each colon. Each line is one color."
              }
            }
            if !error().is_empty() {
              div { class: "alert alert-error alert-soft text-sm", "{error}" }
            }
          }
          div { class: "modal-action",
            button { class: "btn btn-sm btn-ghost", onclick: move |_| on_close.call(()), "Cancel" }
            button { class: "btn btn-sm btn-primary", onclick: on_save, "Save" }
          }
        }
        div { class: "modal-backdrop", onclick: move |_| on_close.call(()) }
      }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_colors_keep_only_color_lines() {
        let typed = "--color-primary: oklch(55% 0.3 240);\n--evil: red;\nfont-size: 40px;\n--color-base-100: #fff;";
        let clean = clean_colors("  Night  ", typed).unwrap();
        assert_eq!(clean.name, "Night");
        assert_eq!(clean.css, "--color-primary: oklch(55% 0.3 240);\n--color-base-100: #fff;");
        assert!(clean.left_out, "the lines that are not colors were dropped");

        let clean = clean_colors("Night", THEME_DEFAULT_CSS).unwrap();
        assert!(!clean.left_out, "the template loses nothing");
    }

    #[test]
    fn colors_need_a_name_and_at_least_one_color() {
        assert_eq!(clean_colors(" ", THEME_DEFAULT_CSS), Err("Give it a name first.".to_string()));
        assert!(clean_colors("Night", "--evil: red;").is_err());
        assert!(clean_colors("Night", "--color-primary: transparent;").is_err());
    }

    #[test]
    fn new_colors_are_put_on() {
        assert_eq!(colors_after_save(&ColorsEditor::New, "abc"), Some(Theme::Custom("abc".to_string())));
        assert_eq!(colors_after_save(&ColorsEditor::Edit("abc".to_string()), "abc"), None);
    }
}
