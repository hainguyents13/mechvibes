//! Renames a theme of the user's own.

use crate::i18n::{ catalog::CustomizeStrings, use_strings };
use crate::state::saved_themes;
use crate::utils::theme_share::{ clean_name, MAX_NAME_CHARS };
use dioxus::prelude::*;

/// The name the user typed, cleaned the way every theme name is. The refusal
/// message comes from the catalog: an empty name is said in the language the
/// dialog is in.
fn new_name(typed: &str, strings: &CustomizeStrings) -> Result<String, &'static str> {
    clean_name(typed).ok_or(strings.name_first)
}

/// Asks for a new name for the saved theme with this `id`, starting from
/// `name`.
#[component]
pub fn RenameModal(id: String, name: String, on_close: EventHandler<()>) -> Element {
    let mut typed = use_signal(move || name);
    let mut error = use_signal(String::new);
    let strings = use_strings();

    let save = use_callback(move |_: ()| {
        let name = match new_name(&typed(), &strings.customize) {
            Ok(name) => name,
            Err(e) => {
                error.set(e.to_string());
                return;
            }
        };
        let saved = saved_themes::apply(|store| {
            store.update(&id, |theme| theme.name = name);
        });
        match saved {
            Ok(_) => on_close.call(()),
            Err(e) => error.set(e),
        }
    });

    rsx! {
      div { class: "modal modal-open",
        div { class: "modal-box",
          h3 { class: "font-bold text-lg mb-4", "{strings.customize.rename_theme_title}" }
          div { class: "space-y-4",
            fieldset { class: "fieldset",
              legend { class: "fieldset-legend", "{strings.customize.name}" }
              input {
                class: "input w-full",
                r#type: "text",
                maxlength: "{MAX_NAME_CHARS}",
                value: typed(),
                onmounted: move |e| async move {
                    let _ = e.set_focus(true).await;
                },
                oninput: move |e| {
                    typed.set(e.value());
                    error.set(String::new());
                },
                onkeydown: move |e| {
                    if e.key() == Key::Enter {
                        save.call(());
                    }
                },
              }
            }
            if !error().is_empty() {
              div { class: "alert alert-error alert-soft text-sm", "{error}" }
            }
          }
          div { class: "modal-action",
            button { class: "btn btn-sm btn-ghost", onclick: move |_| on_close.call(()), "{strings.common.cancel}" }
            button { class: "btn btn-sm btn-primary", onclick: move |_| save.call(()), "{strings.common.save}" }
          }
        }
        div { class: "modal-backdrop", onclick: move |_| on_close.call(()) }
      }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Language;

    fn en() -> &'static CustomizeStrings {
        &Language::En.strings().customize
    }

    #[test]
    fn a_new_name_is_cleaned_like_every_theme_name() {
        assert_eq!(new_name("  Rainy \n  evening ", en()).unwrap(), "Rainy evening");
        assert_eq!(
            new_name(&"A".repeat(MAX_NAME_CHARS + 10), en()).unwrap().chars().count(),
            MAX_NAME_CHARS
        );
    }

    #[test]
    fn an_empty_name_is_refused() {
        assert_eq!(new_name("   ", en()).unwrap_err(), en().name_first);
    }
}
