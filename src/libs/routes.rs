use dioxus::prelude::*;

use crate::libs::theme::{ use_theme, BuiltInTheme, Theme };
use crate::state::themes::ThemesConfig;
use crate::utils::{ config::use_config, theme::use_themes };

#[derive(Clone, Routable, Debug, PartialEq)]
pub enum Route {
    #[layout(Layout)] #[route("/")] Home {},
    #[route("/customize")] Customize {},
    #[route("/soundpacks")] Soundpacks {},
    #[route("/mood")] Mood {},
    #[route("/settings")] Settings {},
}

/// The inline `style` the layout carries for the app background.
///
/// An empty return means "emit nothing and let the theme show through", which
/// is the only way to defer to the theme: any declaration written here wins
/// over the stylesheet.
pub(crate) fn background_style_for(
    enabled: bool,
    bg: &crate::state::config::BackgroundCustomization
) -> String {
    if !enabled {
        return String::new();
    }

    match &bg.background_image {
        Some(image) if bg.use_image => {
            format!("background: url({}) center center / cover no-repeat;", image)
        }
        // Configs written before the default became a theme variable still hold
        // an empty string here, and `background: ;` is invalid CSS - the whole
        // declaration is dropped, so the app kept the theme background while the
        // panel showed a customisation as active. Defer to the theme explicitly.
        _ if bg.background_color.trim().is_empty() => String::new(),
        _ => format!("background: {};", bg.background_color),
    }
}

/// The daisyUI theme the layout carries for `theme`. Custom colors that are
/// no longer in the list draw as the default colors, the way Customize names
/// them, rather than as daisyUI's own fallback that nobody picked.
fn daisy_theme_for(theme: &Theme, themes: &ThemesConfig) -> String {
    match theme {
        Theme::Custom(id) if themes.get_theme_by_id(id).is_none() => BuiltInTheme::System.to_daisy_theme(),
        _ => theme.to_daisy_theme(),
    }
}

#[component]
pub fn Layout() -> Element {
    let (config_signal, _set_config) = use_config();
    let (themes, _) = use_themes();

    // Theme state - use theme context and initialize from config
    let mut theme = use_theme();

    // Adopt the theme the config was saved with.
    //
    // The guard keeps a re-render of this component from cascading. `theme` is
    // a global signal, so this effect re-runs whenever the config signal moves,
    // and it would otherwise write the value it already holds. `Signal::set`
    // has no equality gate and notifies every subscriber regardless, which
    // scheduled a second render here and re-ran every effect subscribed to the
    // theme - including the font/CSS injection in `Header`, i.e. a webview
    // round trip producing the frame already on screen. Only publish a real
    // change.
    use_effect(move || {
        let from_config = config_signal.read().theme.clone();
        if *theme.peek() != from_config {
            theme.set(from_config);
        }
    });

    // Convert theme to DaisyUI theme name
    let daisy_theme = daisy_theme_for(&theme(), &themes());

    // Report the theme only when it actually moves. Navigating remounts this
    // component (the router builds a separate `rsx! { Layout {} }` per route,
    // so each carries its own template identity and gets replaced rather than
    // diffed), which re-runs this line with an unchanged theme and made the
    // console read as though every tab switch re-themed the app.
    {
        use std::sync::Mutex;
        static LAST_LOGGED: Mutex<Option<String>> = Mutex::new(None);
        let mut last = LAST_LOGGED.lock().unwrap_or_else(|e| e.into_inner());
        if last.as_deref() != Some(daisy_theme.as_str()) {
            *last = Some(daisy_theme.clone());
            crate::debug_print!(
                "🎨 Theme applied: {:?} -> DaisyUI: {}",
                theme(),
                daisy_theme
            );
        }
    }

    // Get background customization settings (reactive to config changes)
    let background_style = use_memo(move || {
        let config = config_signal.read();
        background_style_for(
            config.enable_background_customization,
            &config.background_customization
        )
    });

    rsx! {
      div {
        class: "h-screen flex flex-col",
        "data-theme": "{daisy_theme}",
        style: "{background_style()}",
        // Custom title bar for window controls
        crate::components::titlebar::TitleBar {}

        // Main content area with padding to account for title bar
        div { class: "flex-1 overflow-auto {crate::utils::spacing::CONTENT_PADDING}",
          // Outlet for nested routes
          Outlet::<Route> {}
        }
        // Dock at the bottom
        crate::components::dock::Dock {}
      }
    }
}

/// Navigate to `route`, logging a divider and the destination first.
///
/// The log has to happen here rather than in a component, because it must come
/// out *before* the render work the navigation triggers. Rendering is where
/// every other line in the console originates, and a component - even one
/// placed ahead of the `Outlet` in the layout's `rsx!` - is not guaranteed to
/// run before its siblings: sibling order in `rsx!` is not render order, and a
/// log written from `use_effect` lands after the whole pass. Emitting at the
/// call site is the only point that provably precedes all of it, and it covers
/// every navigation because it is the one way the app changes route.
pub fn navigate(route: &'static str) {
    crate::always_print!("────────────────────");
    crate::always_print!("📍 Tab: {}", tab_name_for(route));
    navigator().push(route);
}

/// The human-readable name for a route path, falling back to the path itself.
fn tab_name_for(route: &str) -> &str {
    match route {
        "/" => "Home",
        "/customize" => "Customize",
        "/soundpacks" => "Soundpacks",
        "/mood" => "Mood",
        "/settings" => "Settings",
        other => other,
    }
}

#[component]
pub fn Home() -> Element {
    use crate::libs::AudioContext;
    use std::sync::Arc;

    // Use audio context from the layout provider instead of creating new one
    let audio_context: Arc<AudioContext> = use_context();
    rsx! {
      crate::components::pages::HomePage { audio_ctx: audio_context }
    }
}

#[component]
pub fn Soundpacks() -> Element {
    rsx! {
      crate::components::pages::Soundpacks {}
    }
}

#[component]
pub fn Mood() -> Element {
    rsx! {
      crate::components::pages::MoodPage {}
    }
}

#[component]
pub fn Customize() -> Element {
    rsx! {
      crate::components::pages::CustomizePage {}
    }
}

#[component]
pub fn Settings() -> Element {
    rsx! {
      crate::components::pages::SettingsPage {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::config::BackgroundCustomization;
    use std::str::FromStr;

    /// Resetting the background must actually revert the app to the theme.
    ///
    /// The default used to carry an empty `background_color`, which the layout
    /// interpolated into `background: ;` - a declaration the browser drops
    /// whole. The theme background stayed on screen either way, so pressing
    /// Reset produced no visible change and read as a dead button. The default
    /// now names a theme variable, and it has to reach the page.
    #[test]
    fn the_default_background_applies_the_theme_colour_rather_than_empty_css() {
        let style = background_style_for(true, &BackgroundCustomization::default());

        assert!(
            !style.contains("background: ;"),
            "an empty colour must never reach the page as a broken declaration"
        );
        assert_eq!(
            style, "background: var(--color-base-100);",
            "the default has to resolve to a real theme colour"
        );
    }

    /// A config saved before that default changed still holds `""`. Rather than
    /// emitting broken CSS for those users, defer to the theme.
    #[test]
    fn a_background_colour_left_empty_by_an_older_config_defers_to_the_theme() {
        let stored = BackgroundCustomization {
            background_color: String::new(),
            background_image: None,
            use_image: false,
        };

        assert_eq!(
            background_style_for(true, &stored),
            "",
            "an empty stored colour must emit nothing so the theme shows through"
        );
    }

    /// Turning the toggle off keeps the customisation on disk - the user asked
    /// for it to stop applying, not to be forgotten - so the switch has to be
    /// what decides whether the style is emitted at all.
    #[test]
    fn disabling_the_toggle_stops_applying_a_background_it_still_remembers() {
        let customised = BackgroundCustomization {
            background_color: "var(--color-primary)".to_string(),
            background_image: None,
            use_image: false,
        };

        assert_eq!(
            background_style_for(false, &customised),
            "",
            "a disabled customisation must not style the page"
        );
        assert_eq!(
            background_style_for(true, &customised),
            "background: var(--color-primary);",
            "and turning it back on must restore the colour that was kept"
        );
    }

    /// Pasting an image URL and then resetting must drop the image. The stored
    /// path is cleared along with everything else, so nothing is left to build
    /// a `url()` out of.
    #[test]
    fn resetting_after_pasting_an_image_url_stops_showing_that_image() {
        let pasted = BackgroundCustomization {
            background_color: "var(--color-base-100)".to_string(),
            background_image: Some("https://example.com/wallpaper.png".to_string()),
            use_image: true,
        };
        assert!(
            background_style_for(true, &pasted).contains("wallpaper.png"),
            "the pasted image is what the page shows to begin with"
        );

        let after_reset = BackgroundCustomization::default();
        let style = background_style_for(true, &after_reset);

        assert!(!style.contains("url("), "reset must not leave the image behind: {style}");
        assert!(!style.contains("wallpaper.png"), "reset must not leave the image behind: {style}");
    }

    /// And switching the section off must stop applying it too, even though the
    /// image itself is deliberately kept so turning it back on restores it.
    #[test]
    fn switching_the_section_off_stops_showing_a_pasted_image() {
        let pasted = BackgroundCustomization {
            background_color: "var(--color-base-100)".to_string(),
            background_image: Some("https://example.com/wallpaper.png".to_string()),
            use_image: true,
        };

        assert_eq!(
            background_style_for(false, &pasted),
            "",
            "a disabled section must emit no background at all"
        );
    }

    /// "Use image" can be on before a file has been picked. Falling through to
    /// the colour keeps the page styled instead of emitting `url()` with
    /// nothing in it.
    #[test]
    fn asking_for_an_image_before_choosing_one_falls_back_to_the_colour() {
        let pending = BackgroundCustomization {
            background_color: "var(--color-primary)".to_string(),
            background_image: None,
            use_image: true,
        };

        assert_eq!(background_style_for(true, &pending), "background: var(--color-primary);");
    }

    #[test]
    fn missing_custom_colors_render_as_default() {
        let mut themes = ThemesConfig::default();
        let id = themes.import_theme("Night", "--color-primary: red;").unwrap();

        assert_eq!(daisy_theme_for(&Theme::Custom("gone".to_string()), &themes), "light");
        assert_eq!(daisy_theme_for(&Theme::Custom(id.clone()), &themes), format!("custom-{}", id));
        assert_eq!(daisy_theme_for(&Theme::BuiltIn(BuiltInTheme::Dark), &themes), "dark");
    }

    /// Every path `navigate` is called with must parse back to a real route and
    /// must produce a tab name rather than falling through to the raw path.
    ///
    /// `navigate` takes a `&str` because it is called from `rsx!` event
    /// handlers, so a typo would otherwise be a silent 404 at runtime with a
    /// divider logged for a tab the user never reached.
    #[test]
    fn every_navigable_path_maps_to_a_route_and_a_tab_name() {
        let expected = [
            ("/", "Home"),
            ("/customize", "Customize"),
            ("/soundpacks", "Soundpacks"),
            ("/mood", "Mood"),
            ("/settings", "Settings"),
        ];

        for (path, tab) in expected {
            assert!(
                Route::from_str(path).is_ok(),
                "{path} is passed to navigate() but does not parse as a route"
            );
            assert_eq!(tab_name_for(path), tab, "wrong tab name logged for {path}");
        }
    }

    /// The five paths above are the whole routing table; if a route is added
    /// without teaching `navigate` about it, the divider would log a raw URL.
    #[test]
    fn the_tab_name_table_covers_every_route() {
        for route in [
            Route::Home {},
            Route::Customize {},
            Route::Soundpacks {},
            Route::Mood {},
            Route::Settings {},
        ] {
            let path = route.to_string();
            let name = tab_name_for(&path);
            assert!(
                !name.starts_with('/'),
                "{path} has no tab name - navigate() would log the raw path"
            );
        }
    }
}
