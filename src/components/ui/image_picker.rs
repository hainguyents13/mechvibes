use crate::utils::notice::{ self, NoticeKind };
use crate::utils::path;
use dioxus::prelude::*;
use lucide_dioxus::Upload;

/// An image setting: a field for a web link or a path, and a button that
/// picks an image file and copies it into the app's images folder.
///
/// The field publishes on every change rather than debouncing. A debounce here
/// meant a write could still be in flight when the user pressed Reset or
/// switched the section off, and it then landed afterwards and restored the
/// image that had just been cleared - which is why resetting appeared to work
/// only after changing tabs, the one action that unmounts the pending task.
/// Nothing is gained by deferring: a URL is pasted or typed once, not dragged
/// like a volume slider, and `config_writer::apply` already skips the disk
/// write when a mutation leaves the data unchanged.
#[component]
pub fn ImagePicker(value: Option<String>, on_change: EventHandler<Option<String>>) -> Element {
    rsx! {
        div { class: "space-y-2",
            div { class: "flex gap-2",
                input {
                    r#type: "text",
                    placeholder: "Paste an image link",
                    class: "input w-full input-sm",
                    // Rendered straight from the config, so a Reset that clears
                    // the stored path empties the field with it.
                    value: value.clone().unwrap_or_default(),
                    oninput: move |evt| {
                        let typed = evt.value();
                        let new_value = if typed.is_empty() { None } else { Some(typed) };
                        on_change.call(new_value);
                    },
                }
                button {
                    class: "btn btn-neutral btn-sm",
                    title: "Pick an image",
                    onclick: move |_| {
                        spawn(async move {
                            let file_dialog = rfd::AsyncFileDialog::new()
                                .add_filter("Images", &["png", "jpg", "jpeg", "gif", "webp", "bmp"])
                                .set_title("Pick an image")
                                .pick_file()
                                .await;

                            if let Some(file_handle) = file_dialog {
                                let source_path = file_handle.path().to_string_lossy().to_string();

                                // Copy image to custom images directory and get asset URL.
                                // Reading and hashing a large image takes a moment, so it
                                // runs off the window's thread.
                                let copied = tokio::task
                                    ::spawn_blocking(move || path::copy_to_custom_images(&source_path)).await
                                    .unwrap_or_else(|e| Err(format!("Could not copy the image: {}", e)));
                                match copied {
                                    Ok(asset_url) => {
                                        on_change.call(Some(asset_url));
                                    }
                                    Err(e) => {
                                        crate::always_eprint!("Failed to copy image: {}", e);
                                        notice::report(
                                            NoticeKind::Error,
                                            "Could not use this image. Try another one.",
                                            None,
                                        );
                                    }
                                }
                            }
                        });
                    },
                    Upload { class: "w-4 h-4" }
                }
            }
            div { class: "text-xs text-base-content/50", "Pick an image, or paste a web link" }
        }
    }
}
