use dioxus::prelude::*;
use lucide_dioxus::{Check, TriangleAlert};

#[component]
pub fn ConfirmDeleteModal(
    modal_id: String,
    soundpack_name: String,
    on_confirm: EventHandler<()>,
) -> Element {
    // The catalog, read at render time: a `use_memo` cannot see the language change.
    let strings = crate::i18n::use_strings();
    rsx! {
      dialog { class: "modal", id: "{modal_id}",
        div { class: "modal-box",
          form { method: "dialog",
            button { class: "btn btn-sm btn-circle btn-ghost absolute right-2 top-2",
              "✕"
            }
          }
          h3 { class: "text-lg font-bold", "{strings.soundpacks.delete_title}" }

          // Content
          div { class: "space-y-4 mt-6",
            // Warning icon and message
            div { class: "flex items-start gap-3",
              div { class: "flex-shrink-0 w-10 h-10 rounded-full bg-error/10 flex items-center justify-center",
                TriangleAlert { class: "w-5 h-5 text-error" }
              }
              div { class: "flex-1",
                div { class: "font-medium text-base-content mb-1",
                  "{strings.soundpacks.delete_question}"
                }
                div { class: "text-sm text-base-content/70 mb-3",
                  {crate::i18n::fill(strings.soundpacks.delete_body, &[&soundpack_name])}
                }
              }
            }            // Action buttons
            div { class: "flex justify-end gap-2 pt-2",
              form { method: "dialog",
                button { class: "btn btn-ghost", "{strings.common.cancel}" }
              }              form { method: "dialog",
                button {
                  class: "btn btn-error",
                  r#type: "submit",
                  onclick: move |_| {
                      on_confirm.call(());
                  },
                  Check { class: "w-4 h-4 mr-1" }
                  "{strings.common.delete}"
                }
              }
            }
          }
        }
        form { method: "dialog", class: "modal-backdrop",
          button { "close" }
        }
      }
    }
}
