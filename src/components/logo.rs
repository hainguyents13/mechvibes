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

    let style = if let Some(logo) = logo {
        // Normal state uses the background image if it is on, otherwise the
        // colour; the muted state has its own pair.
        let (use_image, image, color) = if sound_on {
            (logo.use_background_image, &logo.background_image, &logo.background_color)
        } else {
            (logo.use_muted_background_image, &logo.muted_background_image, &logo.muted_background)
        };
        let background_style = match image {
            Some(img) if use_image =>
                format!("background-image: url('{}'); background-size: cover; background-position: center", img),
            _ => format!("background: {}", color),
        };

        if !sound_on {
            classes.push("logo-muted");
            if logo.dimmed_when_muted {
                classes.push("opacity-50");
            }
        }

        format!(
            "border-color: {}; color: {}; {}; {}",
            logo.border_color,
            logo.text_color,
            background_style,
            if shadow {
                format!("box-shadow: 0 5px 0 {}", logo.shadow_color)
            } else {
                String::new()
            }
        )
    } else {
        // Let the theme's colours do the work.
        classes.push("border-primary text-primary bg-transparent");
        if !sound_on {
            classes.push("logo-muted opacity-50");
        }
        if shadow {
            "box-shadow: 0 5px 0 var(--color-primary); background: oklch(from var(--color-primary) l c h / 0.05)".to_string()
        } else {
            "background: oklch(from var(--color-primary) l c h / 0.05)".to_string()
        }
    };

    (classes.join(" "), style)
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
            "box-shadow: 0 5px 0 var(--color-primary); background: oklch(from var(--color-primary) l c h / 0.05)"
        );

        let (class, style) = logo_style(None, false, false);
        assert_eq!(class, "logo-pressed border-primary text-primary bg-transparent logo-muted opacity-50");
        assert_eq!(style, "background: oklch(from var(--color-primary) l c h / 0.05)");
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
        assert!(style.contains("background: red"), "{}", style);
        assert!(!style.contains("box-shadow"), "{}", style);

        let logo = LogoCustomization { use_muted_background_image: true, dimmed_when_muted: true, ..logo };
        let (class, style) = logo_style(Some(&logo), false, false);
        assert_eq!(class, "logo-pressed logo-muted opacity-50");
        assert!(style.contains("background-image: url('/custom-images/a.png')"), "{}", style);
    }
}
