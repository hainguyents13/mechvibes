//! A card in the Themes tab: a small picture of the theme, its name and its
//! colours. Clicking it puts the theme on; its "..." menu shares, renames,
//! duplicates or deletes it.
//!
//! The picture is drawn with the theme's own values: its colours on the
//! picture alone (a daisyUI theme by name, or custom colours set inline, never
//! a `data-theme` that a page rule could match), its background, and the logo
//! drawn by `logo_style`, as the app draws it. Saved values were checked when
//! the saved themes were read, so they are safe in a `style`.

use crate::components::logo::logo_style;
use crate::libs::theme::BuiltInTheme;
use crate::state::config::{ BackgroundCustomization, LogoCustomization };
use crate::utils::theme_share::{ Look, LookColors, SharedColors };
use dioxus::prelude::*;
use lucide_dioxus::{ Check, CopyPlus, Ellipsis, Pencil, Share2, Trash2 };
use std::hash::{ Hash, Hasher };

/// What a card's picture shows.
#[derive(Debug, Clone, PartialEq)]
pub struct CardPreview {
    pub colors: SharedColors,
    pub logo: Option<LogoCustomization>,
    pub background: Option<BackgroundCustomization>,
}

impl CardPreview {
    /// The picture of `look`. Colors that are missing show as the default
    /// colors, as the screen does.
    pub fn of(look: &Look) -> Self {
        let colors = match &look.colors {
            LookColors::Known(colors) => colors.clone(),
            LookColors::Missing(_) => SharedColors::BuiltIn(BuiltInTheme::System),
        };
        Self { colors, logo: look.logo.clone(), background: look.background.clone() }
    }

    /// The daisyUI theme the picture takes its colours from, and the custom
    /// colours set on it instead.
    fn colors(&self) -> (Option<String>, String) {
        match &self.colors {
            SharedColors::BuiltIn(built_in) => (Some(built_in.to_daisy_theme()), String::new()),
            SharedColors::Custom { css } => (None, css.replace('\n', " ")),
        }
    }

    /// The wallpaper the picture shows, if any.
    fn wallpaper(&self) -> Option<&str> {
        self.background
            .as_ref()
            .filter(|background| background.use_image)
            .and_then(|background| background.background_image.as_deref())
    }

    /// The colour behind the picture when the background is on, which shows
    /// until a wallpaper loads. Otherwise the theme's own base colour shows.
    fn background_color(&self) -> Option<&str> {
        self.background
            .as_ref()
            .map(|background| background.background_color.trim())
            .filter(|color| !color.is_empty())
    }

    /// Tells two pictures apart, so a card whose look changed is drawn anew.
    pub fn key(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        format!("{:?}", self).hash(&mut hasher);
        hasher.finish()
    }
}

/// The card's frame: two pixels wide either way, so picking a card never
/// moves the cards around it.
pub(crate) const FRAME_CLASS: &str = "card w-full overflow-hidden text-left cursor-pointer border";
pub(crate) const FRAME_STYLE: &str = "border-width: 2px;";

/// The picture's shape.
pub(crate) const PICTURE_STYLE: &str = "aspect-ratio: 4 / 3;";

/// What a card's "..." menu can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardAction {
    Share,
    Rename,
    Duplicate,
    Delete,
}

impl CardAction {
    fn label(self) -> &'static str {
        match self {
            CardAction::Share => "Share",
            CardAction::Rename => "Rename",
            CardAction::Duplicate => "Duplicate",
            CardAction::Delete => "Delete",
        }
    }
}

#[component]
pub fn ThemeCard(
    name: String,
    preview: CardPreview,
    active: bool,
    actions: &'static [CardAction],
    on_apply: EventHandler<()>,
    on_action: EventHandler<CardAction>
) -> Element {
    let border = if active { "border-primary" } else { "border-base-300" };
    let key = preview.key();

    rsx! {
      // The menu sits beside the card's button, not in it, and outside its
      // frame, which would clip the open menu.
      div { class: "relative",
        button {
          class: "{FRAME_CLASS} {border}",
          style: FRAME_STYLE,
          title: "{name}",
          "aria-pressed": if active { "true" } else { "false" },
          onclick: move |_| on_apply.call(()),
          // Drawn anew whenever the look changes. Dioxus keeps any inline
          // style property a changed `style` leaves out, so a picture reused
          // for a new look could keep colours or a wallpaper of the old one.
          for key in [key] {
            CardFace { key: "{key}", name: name.clone(), preview: preview.clone(), active }
          }
        }
        if !actions.is_empty() {
          div { class: "absolute top-2 right-2",
            div { class: "dropdown dropdown-left",
              div {
                class: "btn btn-xs btn-circle shadow-sm",
                tabindex: "0",
                role: "button",
                title: "More",
                "aria-label": "More for {name}",
                Ellipsis { class: "w-4 h-4" }
              }
              ul {
                class: "dropdown-content menu bg-base-100 rounded-box z-1 p-2 shadow-sm",
                style: "min-width: 9rem;",
                tabindex: "0",
                for action in actions.iter().copied() {
                  li { key: "{action.label()}",
                    a {
                      class: if action == CardAction::Delete { "text-error" } else { "" },
                      onclick: move |_| {
                          // The menu stays open while it has focus, so it
                          // would stay open behind a dialog this opens.
                          dioxus::document::eval("document.activeElement?.blur()");
                          on_action.call(action);
                      },
                      match action {
                          CardAction::Share => rsx! {
                            Share2 { class: "w-4 h-4 mr-1" }
                          },
                          CardAction::Rename => rsx! {
                            Pencil { class: "w-4 h-4 mr-1" }
                          },
                          CardAction::Duplicate => rsx! {
                            CopyPlus { class: "w-4 h-4 mr-1" }
                          },
                          CardAction::Delete => rsx! {
                            Trash2 { class: "w-4 h-4 mr-1" }
                          },
                      }
                      "{action.label()}"
                    }
                  }
                }
              }
            }
          }
        }
      }
    }
}

#[component]
fn CardFace(name: String, preview: CardPreview, active: bool) -> Element {
    rsx! {
      ThemePicture { preview: preview.clone(), class: "", style: PICTURE_STYLE }
      div { class: "bg-base-200 flex items-center gap-2 px-2 py-2",
        div { class: "min-w-0 flex-1 truncate text-sm font-medium text-base-content", "{name}" }
        ColorDots { preview }
        if active {
          Check { class: "w-4 h-4 shrink-0 text-primary" }
        }
      }
    }
}

/// The picture of a look: its colours, its background and its logo, as the
/// app draws them. `class` and `style` give it its frame and shape. Mount it
/// keyed by the look, or under a parent that is: Dioxus keeps any inline
/// style property a changed `style` leaves out.
#[component]
pub fn ThemePicture(preview: CardPreview, class: &'static str, style: &'static str) -> Element {
    let (data_theme, colors_style) = preview.colors();
    let background = preview
        .background_color()
        .map(|color| format!("background-color: {};", color))
        .unwrap_or_default();
    let logo = preview.logo.as_ref();
    let (logo_class, logo_style) = logo_style(logo, true, false);
    // A wallpaper whose file is gone shows the colour behind it, as the app
    // does, not the browser's broken image.
    let mut lost = use_signal(|| false);

    rsx! {
      div {
        class: "relative overflow-hidden flex items-center justify-center bg-base-100 text-base-content {class}",
        style: "{style} {colors_style} {background}",
        "data-theme": data_theme,
        if let (Some(src), false) = (preview.wallpaper(), lost()) {
          img {
            class: "absolute inset-0 w-full h-full object-cover",
            src: "{src}",
            alt: "",
            loading: "lazy",
            decoding: "async",
            onerror: move |_| lost.set(true),
          }
        }
        // The logo as the Customize preview draws it, at half its size, so
        // its border and shadow shrink with it.
        div { class: "relative", style: "zoom: 0.5;",
          div {
            class: "select-none border-3 font-black py-2 px-4 text-2xl rounded-box flex justify-center items-center {logo_class}",
            style: "{logo_style}",
            "Mechvibes"
          }
        }
      }
    }
}

/// Four dots of a look's colours: primary, secondary, accent and neutral.
#[component]
pub fn ColorDots(preview: CardPreview) -> Element {
    let (data_theme, colors_style) = preview.colors();

    rsx! {
      div {
        class: "flex shrink-0 items-center gap-1",
        // daisyUI paints the base colour behind anything with a `data-theme`.
        style: "background-color: transparent; {colors_style}",
        "data-theme": data_theme,
        span { class: "bg-primary size-2 rounded-full" }
        span { class: "bg-secondary size-2 rounded-full" }
        span { class: "bg-accent size-2 rounded-full" }
        span { class: "bg-neutral size-2 rounded-full" }
      }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_shows_the_wallpaper_only_when_it_is_on() {
        let mut background = BackgroundCustomization {
            background_color: "#101010".to_string(),
            background_image: Some("/custom-images/img-0.webp".to_string()),
            use_image: true,
        };
        let look = |background: &BackgroundCustomization| Look {
            background: Some(background.clone()),
            ..Look::default()
        };
        let preview = CardPreview::of(&look(&background));
        assert_eq!(preview.wallpaper(), Some("/custom-images/img-0.webp"));

        background.use_image = false;
        let preview = CardPreview::of(&look(&background));
        assert_eq!(preview.wallpaper(), None);
        assert_eq!(preview.background_color(), Some("#101010"));

        let preview = CardPreview::of(&Look::default());
        assert_eq!((preview.wallpaper(), preview.background_color()), (None, None), "the theme's own base shows");
    }

    #[test]
    fn custom_colors_are_set_on_the_picture_itself() {
        let look = Look {
            colors: LookColors::Known(SharedColors::Custom { css: "--color-primary: red;\n--color-base-100: #fff;".to_string() }),
            ..Look::default()
        };
        let (data_theme, style) = CardPreview::of(&look).colors();
        assert_eq!(data_theme, None, "no data-theme a page rule could match");
        assert_eq!(style, "--color-primary: red; --color-base-100: #fff;");

        let look = Look { colors: LookColors::Missing("gone".to_string()), ..Look::default() };
        assert_eq!(CardPreview::of(&look).colors.clone(), SharedColors::BuiltIn(BuiltInTheme::System));
    }

    #[test]
    fn a_changed_look_draws_a_new_picture() {
        let plain = CardPreview::of(&Look::default());
        let with_logo = CardPreview::of(&Look { logo: Some(LogoCustomization::default()), ..Look::default() });
        assert_eq!(plain.key(), CardPreview::of(&Look::default()).key());
        assert_ne!(plain.key(), with_logo.key());
    }
}
