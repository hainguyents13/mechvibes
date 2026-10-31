//! The panel the Logo and Background tabs share: a preview, a switch for the
//! user's own part, the fields, and Start over.
//!
//! The fields always show, even while the part is off. Then they show where
//! an edit would start from, and the first edit switches the part on in the
//! same write, so there is no separate step to turn it on first.

use crate::components::ui::Toggler;
use dioxus::prelude::*;
use lucide_dioxus::RotateCcw;

/// What the fields show while the part is off, and what the first edit starts from:
/// the saved settings, unless they are still the defaults.
///
/// Default settings look different from the theme's own part (the default
/// logo is drawn in the text colour, the theme's in its main colour), so
/// starting from them would change the look before the user changed anything.
/// Settings the user made are kept as they are.
pub(crate) fn starting_point<T: Clone + PartialEq + Default>(saved: &T, theme_look: T) -> T {
    if *saved == T::default() { theme_look } else { saved.clone() }
}

/// One change to a part. While the part is off, the change starts from
/// `starting_point` and switches the part on, so only the field the user
/// touched differs from what they saw.
pub(crate) fn edit_part<T: Clone + PartialEq + Default>(
    saved: &mut T,
    enabled: &mut bool,
    theme_look: T,
    change: impl FnOnce(&mut T)
) {
    if !*enabled {
        *saved = starting_point(saved, theme_look);
        *enabled = true;
    }
    change(saved);
}

#[component]
pub fn PartPanel(
    // The switch for the user's own part, as "Use my own logo".
    switch_label: String,
    enabled: bool,
    on_switch: EventHandler<bool>,
    on_start_over: EventHandler<()>,
    preview: Element,
    children: Element,
) -> Element {
    rsx! {
      div { class: "space-y-4",
        div { class: "space-y-2",
          div { class: "text-sm text-base-content", "Preview" }
          {preview}
        }
        Toggler {
          title: switch_label,
          description: Some("Changing anything below turns this on.".to_string()),
          checked: enabled,
          on_change: move |on: bool| on_switch.call(on),
        }
        {children}
        div { class: "flex items-center gap-2",
          button {
            class: "btn btn-ghost btn-sm",
            onclick: move |_| on_start_over.call(()),
            RotateCcw { class: "w-4 h-4 mr-1" }
            "Start over"
          }
          div { class: "text-sm text-base-content/50", "Changes save on their own." }
        }
      }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::config::LogoCustomization;

    #[test]
    fn the_first_edit_keeps_the_other_saved_settings() {
        let saved = LogoCustomization {
            border_color: "blue".to_string(),
            text_color: "red".to_string(),
            background_image: Some("/custom-images/a.png".to_string()),
            use_background_image: true,
            ..LogoCustomization::default()
        };
        let mut logo = saved.clone();
        let mut enabled = false;
        edit_part(&mut logo, &mut enabled, LogoCustomization::theme_look(), |l| {
            l.border_color = "green".to_string();
        });
        assert!(enabled, "the first edit switches the logo on");
        assert_eq!(logo, LogoCustomization { border_color: "green".to_string(), ..saved });
    }

    #[test]
    fn the_first_edit_on_default_settings_starts_from_the_theme_look() {
        let mut logo = LogoCustomization::default();
        let mut enabled = false;
        edit_part(&mut logo, &mut enabled, LogoCustomization::theme_look(), |l| {
            l.text_color = "red".to_string();
        });
        assert!(enabled);
        assert_eq!(logo, LogoCustomization { text_color: "red".to_string(), ..LogoCustomization::theme_look() });
    }

    #[test]
    fn an_edit_while_on_changes_only_that_field() {
        let mut logo = LogoCustomization::default();
        let mut enabled = true;
        edit_part(&mut logo, &mut enabled, LogoCustomization::theme_look(), |l| {
            l.text_color = "red".to_string();
        });
        assert_eq!(logo, LogoCustomization { text_color: "red".to_string(), ..LogoCustomization::default() });
    }
}
