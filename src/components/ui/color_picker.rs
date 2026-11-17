use crate::utils::theme_share::{ is_one_value, safe_css_value };
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
        let closed =
            lower.ends_with(')') &&
            lower.len() > open + 2 &&
            safe_css_value(value).is_some() &&
            is_one_value(value);
        if function == "var" {
            return closed && lower.starts_with("var(--color-");
        }
        return closed && COLOR_FUNCTIONS.contains(&function);
    }
    COLOR_NAMES.contains(&lower.as_str())
}

/// Whether a whole colour is saved while the user is still typing. A short hex
/// colour can be the start of a longer one: "#abc" on the way to "#abcdef",
/// or "#ff00", which draws nothing, on the way to "#ff0080". Those wait until
/// the user leaves the field or presses Enter.
fn saves_while_typing(value: &str) -> bool {
    let is_short_hex = value.trim().strip_prefix('#').is_some_and(|hex| hex.len() < 6);
    is_complete_color(value) && !is_short_hex
}

/// What leaving the field saves: the typed colour when it is whole, or else
/// the colour from before typing began, when something was saved on the way.
/// `None` saves nothing.
fn saved_on_finish(typed: &str, before_typing: Option<String>) -> Option<String> {
    if is_complete_color(typed) { Some(typed.trim().to_string()) } else { before_typing }
}

/// The theme's colours a logo or background colour can follow, so it changes
/// with the theme.
///
/// The second value is the identity: it is what goes in the `style`, and what
/// [`theme_color_label`] looks the displayed word up by. The first value is
/// only a fallback for a value the catalog does not name yet — the labels the
/// screen shows come from the catalog, because `ui/fill_field.rs` builds its
/// pickers from this list and its labels have to follow the language too.
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

/// The word the screen shows for one of [`THEME_COLOR_OPTIONS`], in the
/// language the UI is being drawn in.
///
/// Looked up by the colour, not by the English name: the name is what the
/// fallback shows, and matching on it would make an English word the key.
/// `Error` is the `common` word, spelled the same in Spanish and needed in the
/// catalog anyway for the languages where it is not.
pub(crate) fn theme_color_label(
    strings: &crate::i18n::Strings,
    color: &str
) -> Option<&'static str> {
    let customize = &strings.customize;
    Some(match color {
        "var(--color-primary)" => customize.color_main,
        "var(--color-primary-content)" => customize.color_text_on_main,
        "var(--color-secondary)" => customize.color_second,
        "var(--color-secondary-content)" => customize.color_text_on_second,
        "var(--color-base-content)" => customize.color_text,
        "var(--color-base-100)" => customize.color_background,
        "var(--color-base-200)" => customize.color_panel,
        "var(--color-base-300)" => customize.color_panel_edge,
        "var(--color-success)" => customize.color_success,
        "var(--color-success-content)" => customize.color_text_on_success,
        "var(--color-warning)" => customize.color_warning,
        "var(--color-warning-content)" => customize.color_text_on_warning,
        "var(--color-error)" => strings.common.error,
        "var(--color-error-content)" => customize.color_text_on_error,
        _ => return None,
    })
}

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
    let strings = crate::i18n::use_strings();
    // What is typed in the field while the user is still typing it. `None`
    // shows the saved colour, so a pick from the list or a reset shows up.
    let mut draft = use_signal(|| None::<String>);
    // The colour from before this round of typing saved anything, so typing
    // that is left half done puts it back.
    let mut before_typing = use_signal(|| None::<String>);

    // The field shows a typed colour; a theme colour shows in the list button.
    let saved_text = if options.iter().any(|(_, c)| *c == selected_value) {
        String::new()
    } else {
        selected_value.clone()
    };

    // Leaving the field, or Enter, saves a whole colour and puts anything
    // else back to the colour from before typing began.
    let mut finish_typing = move || {
        let before = before_typing.take();
        if let Some(typed) = draft.take() {
            if let Some(value) = saved_on_finish(&typed, before) {
                on_change.call(value);
            }
        }
    };

    // Find the display name for the selected value: the catalog's word for a
    // theme colour, the option's own name for a list the catalog does not know,
    // or the placeholder for a colour that was typed.
    let selected_display = theme_color_label(strings, &selected_value)
        .or_else(|| options.iter().find(|(_, value)| *value == selected_value).map(|(name, _)| *name))
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
                                    span { class: "truncate text-sm",
                                        "{theme_color_label(strings, color).unwrap_or(name)}"
                                    }
                                }
                            }
                        }
                    }
                }
                input {
                    r#type: "text",
                    class: "input",
                    placeholder: "{strings.customize.color_type_placeholder}",
                    value: draft().unwrap_or(saved_text),
                    // Saved as soon as it is a whole colour that is not the
                    // start of a longer one, so the logo follows along, but
                    // never half typed.
                    oninput: {
                        let selected_value = selected_value.clone();
                        move |evt: FormEvent| {
                            let typed = evt.value();
                            if saves_while_typing(&typed) {
                                if before_typing.peek().is_none() {
                                    before_typing.set(Some(selected_value.clone()));
                                }
                                on_change.call(typed.trim().to_string());
                            }
                            draft.set(Some(typed));
                        }
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
    use crate::i18n::Language;

    /// Every colour a logo or background can follow has a word in the catalog,
    /// and it is the word the picker showed before the migration. A new entry
    /// in `THEME_COLOR_OPTIONS` without a catalog key fails here instead of
    /// falling back to English on a translated screen.
    #[test]
    fn every_theme_colour_has_its_english_word_in_the_catalog() {
        let strings = Language::En.strings();
        for (shipped, value) in THEME_COLOR_OPTIONS {
            let shown = theme_color_label(strings, value)
                .unwrap_or_else(|| panic!("{value} has no catalog key"));
            assert_eq!(shown, shipped, "the catalog word for {value} changed");
        }
    }

    /// Spanish and Chinese get their own word; `Error` is the one that is
    /// spelled the same, and it is in the catalog anyway for the languages
    /// where it is not (German `Fehler`, French `Erreur`).
    #[test]
    fn the_colour_words_follow_the_language() {
        let es = Language::Es.strings();
        assert_eq!(theme_color_label(es, "var(--color-primary)"), Some("Color principal"));
        assert_eq!(theme_color_label(es, "var(--color-error)"), Some("Error"));
        assert_eq!(theme_color_label(es, "#ff0000"), None, "a typed colour has no name");
        let zh = Language::Zh.strings();
        assert_eq!(theme_color_label(zh, "var(--color-base-200)"), Some("面板"));
    }

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
    fn a_short_hex_color_waits_until_typing_ends() {
        for start_of_longer in ["#abc", "#abcd", "#ff00", "#fff"] {
            assert!(!saves_while_typing(start_of_longer), "{:?} may be the start of a longer color", start_of_longer);
        }
        for whole in ["#abcdef", "#ff008080", "rgb(255,0,0)", "var(--color-primary)", "red"] {
            assert!(saves_while_typing(whole), "{:?}", whole);
        }
    }

    #[test]
    fn typing_left_half_done_puts_the_color_from_before_back() {
        let before = Some("#336699".to_string());
        assert_eq!(saved_on_finish("#abcde", before.clone()).as_deref(), Some("#336699"));
        assert_eq!(saved_on_finish("#abc", before).as_deref(), Some("#abc"), "a whole short color is kept");
        assert_eq!(saved_on_finish("#ab", None), None, "nothing was saved, so nothing changes");
    }

    #[test]
    fn a_typed_color_that_could_break_the_style_is_not_whole() {
        assert!(!is_complete_color("url(x.png)"));
        assert!(!is_complete_color("var(--size-field)"));
        assert!(!is_complete_color("rgb(0,0,0);color:red)"));
        assert!(!is_complete_color("rgb(1 2 3), rgb(0 0 0)"), "a list would not import");
    }
}