use dioxus::prelude::*;

/// Asks before something that cannot be taken back. The caller shows it
/// while it holds the thing to confirm, and drops it on either answer.
#[component]
pub fn ConfirmModal(
    title: String,
    message: String,
    confirm_label: String,
    on_confirm: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    rsx! {
      div { class: "modal modal-open",
        div { class: "modal-box",
          h3 { class: "font-bold text-lg", "{title}" }
          div { class: "text-sm text-base-content/70 mt-3", "{message}" }
          div { class: "modal-action",
            button { class: "btn btn-sm btn-ghost", onclick: move |_| on_cancel.call(()), "Cancel" }
            button {
              class: "btn btn-sm btn-error",
              onclick: move |_| on_confirm.call(()),
              "{confirm_label}"
            }
          }
        }
        div { class: "modal-backdrop", onclick: move |_| on_cancel.call(()) }
      }
    }
}
