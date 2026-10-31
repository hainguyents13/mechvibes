use dioxus::{document::eval, prelude::*};
use lucide_dioxus::ChevronDown;

/// The theme's colours a logo or background colour can follow, so it changes
/// with the theme.
pub const THEME_COLOR_OPTIONS: [(&str, &str); 14] = [
    ("Main color", "var(--color-primary)"),
    ("Text on main color", "var(--color-primary-content)"),
    ("Second color", "var(--color-secondary)"),
    ("Text on second color", "var(--color-secondary-content)"),
    ("Text", "var(--color-base-content)"),
    ("Background", "var(--color-base-100)"),
    ("Panel", "var(--color-base-200)"),
    ("Panel edge", "var(--color-base-300)"),
    ("Success", "var(--color-success)"),
    ("Text on success", "var(--color-success-content)"),
    ("Warning", "var(--color-warning)"),
    ("Text on warning", "var(--color-warning-content)"),
    ("Error", "var(--color-error)"),
    ("Text on error", "var(--color-error-content)"),
];

#[component]
pub fn ColorPicker(
    label: String,
    selected_value: String,
    options: Vec<(&'static str, &'static str)>,
    on_change: EventHandler<String>,
    placeholder: String,
    field: String,
    description: Option<String>,
) -> Element {
    let mut is_open = use_signal(|| false);

    // Find the display name for the selected value
    let selected_display = options
        .iter()
        .find(|(_, value)| *value == selected_value)
        .map(|(name, _)| *name)
        .unwrap_or(&placeholder);

    rsx! {
        div { class: "space-y-2",
            if !label.is_empty() {
                div { class: "text-sm text-base-content", "{label}" }
            }
            div { class: "grid grid-cols-2 gap-2",
                div {
                    class: format!(
                        "dropdown w-full {}",
                        if field == "background_color" || field == "shadow_color"
                            || field == "muted_background"
                        {
                            "dropdown-top"
                        } else {
                            "dropdown-bottom"
                        },
                    ),
                    button {
                        class: "btn btn-soft w-full justify-between",
                        "tabindex": "0",
                        "role": "button",
                        div { class: "flex items-center gap-2",
                            // Color circle indicator
                            div {
                                class: "w-4 h-4 rounded-full border border-base-300 flex-shrink-0",
                                style: format!("background-color: {}", selected_value),
                            }
                            span { class: "text-left truncate w-26", "{selected_display}" }
                        }
                        ChevronDown { class: "w-4 h-4 " }
                    }
                    ul {
                        class: "dropdown-content bg-base-100 rounded-box z-1 flex-col p-2 h-52 overflow-y-auto w-full shadow-sm",
                        "tabindex": "0",
                        for (name , color) in options.iter() {
                            li { class: "w-full",
                                a {
                                    class: format!(
                                        "flex w-full cursor-pointer items-center gap-2 p-2 rounded hover:bg-base-200 text-left {}",
                                        if *color == selected_value { "bg-primary/10" } else { "" },
                                    ),
                                    onclick: {
                                        let color = color.to_string();
                                        move |_| {
                                            on_change.call(color.clone());
                                            is_open.set(false);
                                            eval("document.activeElement.blur()");
                                        }
                                    },
                                    div {
                                        class: "w-4 h-4 rounded-full border border-base-300 flex-shrink-0",
                                        style: format!("background-color: {}", color),
                                    }
                                    span { class: "truncate text-sm", "{name}" }
                                }
                            }
                        }
                    }
                }
                input {
                    r#type: "text",
                    class: "input",
                    placeholder: "Or enter custom color (e.g., #ff0000, rgb(255,0,0))",
                    value: if options.iter().any(|(_, c)| *c == selected_value) { "" } else { selected_value },
                    oninput: move |evt| on_change.call(evt.value()),
                }
            }
            if let Some(desc) = description {
                div { class: "text-xs text-base-content/50",
                    "{desc}"
                }
            }
        }
    }
}
