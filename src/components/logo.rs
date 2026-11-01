use crate::state::config::LogoCustomization;
use crate::state::keyboard::KeyboardState;
use crate::utils::config::use_config;
use dioxus::prelude::*;

/// The classes and inline style that colour the logo. `logo` is the user's
/// own logo, or `None` when the theme's logo is used. The classes say only
/// what changes with the state; the size and shape stay with the caller, so
/// the Customize preview can draw the same logo smaller.
pub fn logo_style(logo: Option<&LogoCustomization>, sound_on: bool, pressed: bool) -> (String, String) {
    let mut classes = Vec::new();
    if pressed || !sound_on {
        classes.push("logo-pressed");
    }
    let shadow = !pressed && sound_on;

    // Every style below names the same properties, and the background ones
    // through a shorthand with nothing deferred in it: Dioxus puts back any
    // inline property a new `style` leaves out, which kept an own logo's image
    // and border on screen after switching back to the theme's logo.
    // `revert-layer` hands a property back to the stylesheet.
    let style = if let Some(logo) = logo {
        // Normal state uses the background image if it is on, otherwise the
        // colour; the muted state has its own pair.
        let (use_image, image, color) = if sound_on {
            (logo.use_background_image, &logo.background_image, &logo.background_color)
        } else {
            (logo.use_muted_background_image, &logo.muted_background_image, &logo.muted_background)
        };
        let background_style = match image {
            Some(img) if use_image => format!("background: url('{}') center / cover", img),
            _ => format!("background: revert-layer; background-color: {}", color),
        };

        if !sound_on {
            classes.push("logo-muted");
            if logo.dimmed_when_muted {
                classes.push("opacity-50");
            }
        }

        format!(
            "{}; color: {}; {}; box-shadow: {}",
            border_color(&logo.border_color),
            logo.text_color,
            background_style,
            if shadow {
                format!("0 5px 0 {}", logo.shadow_color)
            } else {
                "revert-layer".to_string()
            }
        )
    } else {
        // Let the theme's colours do the work.
        classes.push("border-primary text-primary bg-transparent");
        if !sound_on {
            classes.push("logo-muted opacity-50");
        }
        format!(
            "{}; color: revert-layer; background: revert-layer; background-color: oklch(from var(--color-primary) l c h / 0.05); box-shadow: {}",
            border_color("revert-layer"),
            if shadow { "0 5px 0 var(--color-primary)" } else { "revert-layer" }
        )
    };

    (classes.join(" "), style)
}

/// The border colour, set side by side rather than through `border-color`.
/// Dioxus puts back any side a new `style` seems to leave out, and a
/// shorthand holding `var(...)` reads as empty on every side until it is
/// drawn, so the old colour came back after switching to a theme colour.
fn border_color(color: &str) -> String {
    ["top", "right", "bottom", "left"]
        .map(|side| format!("border-{}-color: {}", side, color))
        .join("; ")
}

#[component]
pub fn Logo() -> Element {
    // Get the global keyboard state from context
    let keyboard_state = use_context::<Signal<KeyboardState>>();
    let (config, _) = use_config();

    // Use computed signals that always reflect current config state
    let enable_sound = use_memo(move || config().enable_sound);
    let enable_logo_customization = use_memo(move || config().enable_logo_customization);
    let logo_customization = use_memo(move || config().logo_customization.clone());

    // Get the current key press state
    let key_pressed = keyboard_state.read().key_pressed;

    // `duration-150` is left in the class list but overridden inline below:
    // the Tailwind stylesheet is a committed build artifact and does not
    // contain an arbitrary-value duration utility, so a class like
    // `duration-[30ms]` would silently do nothing.
    let base =
        "logo select-none border-4 font-black py-6 px-8 pt-7 text-5xl rounded-box transition-all duration-150 ease-in-out flex justify-center items-center ";

    // Custom colors apply only while logo customization is on.
    let logo = enable_logo_customization().then(|| logo_customization());
    let (state_class, dynamic_style) = logo_style(logo.as_ref(), enable_sound(), key_pressed);
    let final_class = format!("{}{}", base, state_class);

    // A fast typist can press and release well inside 100ms. A transition
    // longer than the gap between two keystrokes never reaches the pressed
    // state before being told to go back, so fast typing renders as almost no
    // movement at all: measured in the webview against that rhythm, the
    // stylesheet's 150ms produced 0 of 30 visible presses, against 29 of 30 at
    // 30ms. Set inline because it has to win over the `duration-150` class.
    let dynamic_style = format!("transition-duration: 30ms; {}", dynamic_style);

    rsx! {
      div { class: "{final_class}", style: "{dynamic_style}", "Mechvibes" }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_theme_logo_uses_the_main_color() {
        let (class, style) = logo_style(None, true, false);
        assert_eq!(class, "border-primary text-primary bg-transparent");
        assert_eq!(
            style,
            "border-top-color: revert-layer; border-right-color: revert-layer; border-bottom-color: revert-layer; border-left-color: revert-layer; color: revert-layer; background: revert-layer; background-color: oklch(from var(--color-primary) l c h / 0.05); box-shadow: 0 5px 0 var(--color-primary)"
        );

        let (class, style) = logo_style(None, false, false);
        assert_eq!(class, "logo-pressed border-primary text-primary bg-transparent logo-muted opacity-50");
        assert!(style.ends_with("box-shadow: revert-layer"), "{}", style);
    }

    /// Dioxus keeps an inline property that a new `style` leaves out, and
    /// counts a side of a shorthand holding `var(...)` as left out, so every
    /// state of the logo has to name the same properties, with no shorthand
    /// that defers anything. Otherwise an own logo's image, border or shadow
    /// stays after it is turned off, or a theme colour never shows.
    #[test]
    fn every_logo_style_names_the_same_properties() {
        fn properties(style: &str) -> Vec<String> {
            style
                .split(';')
                .filter_map(|d| d.split_once(':'))
                .map(|(property, _)| property.trim().to_string())
                .filter(|property| property != "background-color")
                .collect()
        }
        let image = LogoCustomization {
            background_image: Some("/custom-images/a.png".to_string()),
            use_background_image: true,
            ..LogoCustomization::default()
        };
        let colour = LogoCustomization {
            border_color: "var(--color-primary)".to_string(),
            ..LogoCustomization::default()
        };
        let mut styles = Vec::new();
        for (sound_on, pressed) in [(true, false), (true, true), (false, false)] {
            for logo in [None, Some(&image), Some(&colour)] {
                styles.push(logo_style(logo, sound_on, pressed).1);
            }
        }

        let expected = properties(&styles[0]);
        for style in &styles {
            assert_eq!(properties(style), expected, "{}", style);
            let background = style.split(';').map(str::trim).find(|d| d.starts_with("background:")).unwrap();
            assert!(!background.contains("var("), "{}", style);
            assert!(!style.contains("border-color"), "the border is set side by side: {}", style);
        }
    }

    #[test]
    fn an_own_logo_uses_its_muted_background_and_fades_only_when_asked() {
        let logo = LogoCustomization {
            muted_background: "red".to_string(),
            muted_background_image: Some("/custom-images/a.png".to_string()),
            use_muted_background_image: false,
            dimmed_when_muted: false,
            ..LogoCustomization::default()
        };
        let (class, style) = logo_style(Some(&logo), false, false);
        assert_eq!(class, "logo-pressed logo-muted");
        assert!(style.contains("background-color: red"), "{}", style);
        assert!(style.contains("box-shadow: revert-layer"), "{}", style);

        let logo = LogoCustomization { use_muted_background_image: true, dimmed_when_muted: true, ..logo };
        let (class, style) = logo_style(Some(&logo), false, false);
        assert_eq!(class, "logo-pressed logo-muted opacity-50");
        assert!(style.contains("background: url('/custom-images/a.png') center / cover"), "{}", style);
    }
}
