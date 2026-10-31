use crate::components::ui::color_picker::{ ColorPicker, THEME_COLOR_OPTIONS };
use crate::components::ui::image_picker::ImagePicker;
use dioxus::prelude::*;

/// A fill that is either a colour or an image: two buttons choose which, and
/// the matching picker sits below them. The logo's two backgrounds and the
/// app background all use it.
#[component]
pub fn FillField(
    label: String,
    description: Option<String>,
    use_image: bool,
    color: String,
    image: Option<String>,
    on_use_image: EventHandler<bool>,
    on_color: EventHandler<String>,
    on_image: EventHandler<Option<String>>,
    // Passed on to `ColorPicker`, which opens its list up or down by it.
    field: String,
) -> Element {
    let choice = |active: bool| if active { "btn btn-sm join-item btn-active" } else { "btn btn-sm join-item" };

    rsx! {
        div { class: "space-y-2",
            div { class: "flex items-center justify-between gap-2",
                div { class: "text-sm text-base-content", "{label}" }
                div { class: "join",
                    button {
                        class: choice(!use_image),
                        onclick: move |_| on_use_image.call(false),
                        "Color"
                    }
                    button {
                        class: choice(use_image),
                        onclick: move |_| on_use_image.call(true),
                        "Image"
                    }
                }
            }
            if use_image {
                ImagePicker { value: image, on_change: on_image }
            } else {
                ColorPicker {
                    label: String::new(),
                    selected_value: color,
                    options: THEME_COLOR_OPTIONS.to_vec(),
                    placeholder: "Pick a color".to_string(),
                    on_change: on_color,
                    field,
                    description: None,
                }
            }
            if let Some(desc) = description {
                div { class: "text-xs text-base-content/50", "{desc}" }
            }
        }
    }
}
