use crate::utils::theme_share::safe_css_value;
use dioxus::html::Key;
use dioxus::{document::eval, prelude::*};
use lucide_dioxus::ChevronDown;

/// The colour functions a typed colour can be written with.
const COLOR_FUNCTIONS: [&str; 11] = [
    "rgb", "rgba", "hsl", "hsla", "hwb", "lab", "lch", "oklab", "oklch", "color", "color-mix",
];

/// The colour keywords of CSS.
const COLOR_NAMES: [&str; 150] = [
    "aliceblue", "antiquewhite", "aqua", "aquamarine", "azure", "beige", "bisque", "black",
    "blanchedalmond", "blue", "blueviolet", "brown", "burlywood", "cadetblue", "chartreuse",
    "chocolate", "coral", "cornflowerblue", "cornsilk", "crimson", "currentcolor", "cyan",
    "darkblue", "darkcyan", "darkgoldenrod", "darkgray", "darkgreen", "darkgrey", "darkkhaki",
    "darkmagenta", "darkolivegreen", "darkorange", "darkorchid", "darkred", "darksalmon",
    "darkseagreen", "darkslateblue", "darkslategray", "darkslategrey", "darkturquoise",
    "darkviolet", "deeppink", "deepskyblue", "dimgray", "dimgrey", "dodgerblue", "firebrick",
    "floralwhite", "forestgreen", "fuchsia", "gainsboro", "ghostwhite", "gold", "goldenrod",
    "gray", "green", "greenyellow", "grey", "honeydew", "hotpink", "indianred", "indigo", "ivory",
    "khaki", "lavender", "lavenderblush", "lawngreen", "lemonchiffon", "lightblue", "lightcoral",
    "lightcyan", "lightgoldenrodyellow", "lightgray", "lightgreen", "lightgrey", "lightpink",
    "lightsalmon", "lightseagreen", "lightskyblue", "lightslategray", "lightslategrey",
    "lightsteelblue", "lightyellow", "lime", "limegreen", "linen", "magenta", "maroon",
    "mediumaquamarine", "mediumblue", "mediumorchid", "mediumpurple", "mediumseagreen",
    "mediumslateblue", "mediumspringgreen", "mediumturquoise", "mediumvioletred", "midnightblue",
    "mintcream", "mistyrose", "moccasin", "navajowhite", "navy", "oldlace", "olive", "olivedrab",
    "orange", "orangered", "orchid", "palegoldenrod", "palegreen", "paleturquoise",
    "palevioletred", "papayawhip", "peachpuff", "peru", "pink", "plum", "powderblue", "purple",
    "rebeccapurple", "red", "rosybrown", "royalblue", "saddlebrown", "salmon", "sandybrown",
    "seagreen", "seashell", "sienna", "silver", "skyblue", "slateblue", "slategray", "slategrey",
    "snow", "springgreen", "steelblue", "tan", "teal", "thistle", "tomato", "transparent",
    "turquoise", "violet", "wheat", "white", "whitesmoke", "yellow", "yellowgreen",
];

/// Whether `value` is a whole colour rather than one still being typed: a
/// hex colour of 3, 4, 6 or 8 digits, a colour function with its bracket
/// closed, a theme colour `var(--color-*)`, or a CSS colour name. Only a
/// whole colour is saved, so the logo never flashes through "#" and "#f" on
/// the way to "#ff0000".
pub(crate) fn is_complete_color(value: &str) -> bool {
    let value = value.trim();
    if let Some(hex) = value.strip_prefix('#') {
        return matches!(hex.len(), 3 | 4 | 6 | 8) && hex.chars().all(|c| c.is_ascii_hexdigit());
    }
    let lower = value.to_ascii_lowercase();
    if let Some(open) = lower.find('(') {
        let function = &lower[..open];
        let closed = lower.ends_with(')') && lower.len() > open + 2 && safe_css_value(value).is_some();
        if function == "var" {
            return closed && lower.starts_with("var(--color-");
        }
        return closed && COLOR_FUNCTIONS.contains(&function);
    }
    COLOR_NAMES.contains(&lower.as_str())
}

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
    // What is typed in the field while the user is still typing it. `None`
    // shows the saved colour, so a pick from the list or a reset shows up.
    let mut draft = use_signal(|| None::<String>);

    // The field shows a typed colour; a theme colour shows in the list button.
    let saved_text = if options.iter().any(|(_, c)| *c == selected_value) {
        String::new()
    } else {
        selected_value.clone()
    };

    // Leaving the field, or Enter, saves a whole colour and puts anything
    // else back to the saved one.
    let mut finish_typing = move || {
        if let Some(typed) = draft.take() {
            if is_complete_color(&typed) {
                on_change.call(typed.trim().to_string());
            }
        }
    };

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
                    placeholder: "Or type a color, like #ff0000",
                    value: draft().unwrap_or(saved_text),
                    // Saved as soon as it is a whole colour, so the logo
                    // follows along, but never half typed.
                    oninput: move |evt| {
                        let typed = evt.value();
                        if is_complete_color(&typed) {
                            on_change.call(typed.trim().to_string());
                        }
                        draft.set(Some(typed));
                    },
                    onblur: move |_| finish_typing(),
                    onkeydown: move |evt| {
                        if evt.key() == Key::Enter {
                            finish_typing();
                        }
                    },
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_half_typed_color_is_not_saved() {
        for half in ["", "#", "#f", "#ff", "#ff000", "rgb(255,0", "rgb(", "var(--color-", "re", "f00"] {
            assert!(!is_complete_color(half), "{:?} is still being typed", half);
        }
        for whole in [
            "#fff",
            "#ff0000",
            "#FF000080",
            "rgb(255,0,0)",
            "oklch(60% 0.2 30)",
            "var(--color-primary)",
            "red",
            " Red ",
        ] {
            assert!(is_complete_color(whole), "{:?} is a whole color", whole);
        }
    }

    #[test]
    fn a_typed_color_that_could_break_the_style_is_not_whole() {
        assert!(!is_complete_color("url(x.png)"));
        assert!(!is_complete_color("var(--size-field)"));
        assert!(!is_complete_color("rgb(0,0,0);color:red)"));
    }
}