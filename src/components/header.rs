use crate::libs::theme::{ use_theme, Theme };
use crate::utils::theme::use_themes;
use crate::utils::constants::CSS_ID_PREFIX;
use dioxus::document::eval;
use dioxus::prelude::*;

const FAVICON: Asset = asset!("/assets/icon.ico");
const GLOBAL_STYLES_CSS: &str = include_str!("../../assets/style.css");

// Font assets
const LATO_REGULAR: Asset = asset!("/assets/fonts/Lato-Regular.ttf");
const LATO_BOLD: Asset = asset!("/assets/fonts/Lato-Bold.ttf");
const LATO_ITALIC: Asset = asset!("/assets/fonts/Lato-Italic.ttf");
const LATO_BOLD_ITALIC: Asset = asset!("/assets/fonts/Lato-BoldItalic.ttf");
const LATO_BLACK: Asset = asset!("/assets/fonts/Lato-Black.ttf");

#[component]
pub fn Header() -> Element {
    use crate::utils::config::use_config;

    let (config, _) = use_config();
    let (themes, _) = use_themes();
    let theme = use_theme();

    // Remembers the CSS last handed to the webview, so an effect run that
    // produces byte-identical CSS skips the DOM work instead of removing and
    // re-appending the same `<style>` element.
    let mut injected_css = use_signal(String::new);

    // Use effect to inject fonts and dynamic CSS
    use_effect(move || {
        let custom_css = config().custom_css.clone();

        // Create font-face declarations using Manganis assets
        let font_css = format!(
            r#"
            @font-face {{
                font-family: "Lato";
                src: url("{}") format("truetype");
                font-weight: 400;
                font-style: normal;
            }}
            @font-face {{
                font-family: "Lato";
                src: url("{}") format("truetype");
                font-weight: 700;
                font-style: normal;
            }}
            @font-face {{
                font-family: "Lato";
                src: url("{}") format("truetype");
                font-weight: 400;
                font-style: italic;
            }}
            @font-face {{
                font-family: "Lato";
                src: url("{}") format("truetype");
                font-weight: 700;
                font-style: italic;
            }}
            @font-face {{
                font-family: "Lato";
                src: url("{}") format("truetype");
                font-weight: 900;
                font-style: normal;
            }}
            body {{
                font-family: "Lato", sans-serif;
            }}
            "#,
            LATO_REGULAR,
            LATO_BOLD,
            LATO_ITALIC,
            LATO_BOLD_ITALIC,
            LATO_BLACK
        );

        // Get custom theme CSS if current theme is custom
        let custom_theme_css = if let Theme::Custom(theme_id) = &theme() {
            if let Some(theme_data) = themes().get_theme_by_id(theme_id) {
                // Wrap custom theme CSS with proper data-theme selectors
                format!("[data-theme=\"custom-{}\"] {{\n{}\n}}", theme_id, theme_data.css)
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        // Platform-specific CSS for Linux (solid backgrounds when blur not supported)
        #[cfg(target_os = "linux")]
        let platform_css = r#"
/* Linux-specific: Solid backgrounds for backdrop-blur fallback */
.backdrop-blur-sm,
.backdrop-blur-md,
.backdrop-blur-lg {
  background-color: var(--color-base-200) !important;
}

body {
  background-color: var(--color-base-100);
}

:where(:root),[data-theme=light] body {
  background-color: oklch(98% 0 0);
}

@media (prefers-color-scheme: dark) {
  :root body {
    background-color: oklch(23.26% 0.014 253.1);
  }
}

:root:has(input.theme-controller[value=forest]:checked) body,
[data-theme=forest] body {
  background-color: oklch(21.15% 0.012 254.09);
}

/* Linux-specific: Force border-radius and box-shadow rendering with hardware
 * acceleration.
 *
 * There used to be a blanket `* { transform: translateZ(0) }` here. It is gone
 * on purpose, and should not come back: a transform creates a stacking context,
 * so applying it to * put every layout div in its own context and trapped the
 * soundpack dropdown's z-index inside its ancestors'. Later sections then
 * painted over the open list, and the fixed click-outside backdrop stopped
 * covering the window because a transformed ancestor became its containing
 * block. The !important radius and shadow overrides below are what actually
 * fixed the Linux rendering; the blanket acceleration only cost. Compositor
 * hints stay scoped to the components that need them, in the block at the end
 * of this stylesheet. */

/* Explicitly force rounded corners with vendor prefixes */
.rounded-box {
  -webkit-border-radius: var(--radius-box, 0.5rem) !important;
  -moz-border-radius: var(--radius-box, 0.5rem) !important;
  border-radius: var(--radius-box, 0.5rem) !important;
}

.rounded-btn {
  -webkit-border-radius: var(--radius-btn, 0.5rem) !important;
  -moz-border-radius: var(--radius-btn, 0.5rem) !important;
  border-radius: var(--radius-btn, 0.5rem) !important;
}

.rounded-lg {
  -webkit-border-radius: 0.5rem !important;
  -moz-border-radius: 0.5rem !important;
  border-radius: 0.5rem !important;
}

.rounded-md {
  -webkit-border-radius: 0.375rem !important;
  -moz-border-radius: 0.375rem !important;
  border-radius: 0.375rem !important;
}

.rounded {
  -webkit-border-radius: 0.25rem !important;
  -moz-border-radius: 0.25rem !important;
  border-radius: 0.25rem !important;
}

/* Explicitly force box-shadow with vendor prefixes */
.shadow-lg {
  -webkit-box-shadow: 0 10px 15px -3px rgba(0, 0, 0, 0.1), 0 4px 6px -4px rgba(0, 0, 0, 0.1) !important;
  -moz-box-shadow: 0 10px 15px -3px rgba(0, 0, 0, 0.1), 0 4px 6px -4px rgba(0, 0, 0, 0.1) !important;
  box-shadow: 0 10px 15px -3px rgba(0, 0, 0, 0.1), 0 4px 6px -4px rgba(0, 0, 0, 0.1) !important;
}

.shadow-md {
  -webkit-box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.1), 0 2px 4px -2px rgba(0, 0, 0, 0.1) !important;
  -moz-box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.1), 0 2px 4px -2px rgba(0, 0, 0, 0.1) !important;
  box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.1), 0 2px 4px -2px rgba(0, 0, 0, 0.1) !important;
}

.shadow {
  -webkit-box-shadow: 0 1px 3px 0 rgba(0, 0, 0, 0.1), 0 1px 2px -1px rgba(0, 0, 0, 0.1) !important;
  -moz-box-shadow: 0 1px 3px 0 rgba(0, 0, 0, 0.1), 0 1px 2px -1px rgba(0, 0, 0, 0.1) !important;
  box-shadow: 0 1px 3px 0 rgba(0, 0, 0, 0.1), 0 1px 2px -1px rgba(0, 0, 0, 0.1) !important;
}

/* Ensure proper rendering with compositor */
.card,
.btn,
.input,
.select,
.modal-box,
.dropdown-content,
.menu,
.rounded-box {
  will-change: transform;
  -webkit-backface-visibility: hidden;
  backface-visibility: hidden;
  -webkit-perspective: 1000;
  perspective: 1000;
}
"#.to_string();

        #[cfg(not(target_os = "linux"))]
        let platform_css = String::new();

        #[cfg(target_os = "linux")]
        crate::always_print!("🐧 Injecting Linux-specific CSS for border-radius and box-shadow rendering");

        // Combine all dynamic CSS parts
        let dynamic_css = format!("{}\n{}\n{}\n{}", font_css, custom_theme_css, custom_css, platform_css); // Inject only dynamic CSS using eval

        // Nothing about the styling moved, so re-running the script would swap
        // the `<style>` element for an identical one - a webview round trip and
        // a style recalculation that can only produce the frame already on
        // screen. This effect is subscribed to the config and theme signals,
        // either of which can be republished with an unchanged value, so it
        // re-runs without any of its inputs having actually moved.
        if *injected_css.peek() == dynamic_css {
            return;
        }
        injected_css.set(dynamic_css.clone());

        crate::debug_print!("🎨 Header: Injecting fonts and dynamic CSS");
        let script = format!(
            r#"
              // Remove existing custom style if any
              const existingStyle = document.getElementById('{}-custom-styles');
              if (existingStyle) {{
                  existingStyle.remove();
              }}
              
              // Create new style element for dynamic CSS
              const style = document.createElement('style');
              style.id = '{}-custom-styles';
              style.textContent = `{}`;
              document.head.appendChild(style);
            "#,
            CSS_ID_PREFIX,
            CSS_ID_PREFIX,
            dynamic_css.replace('`', r#"\`"#).replace("${", r#"\${"#)
        );

        eval(&script);
    });

    rsx! {
      // prettier-ignore
      document::Link { rel: "icon", r#type: "image/x-icon", href: FAVICON }

      // Inline global styles since asset!() for CSS doesn't work in desktop apps
      style { dangerous_inner_html: GLOBAL_STYLES_CSS }
    }
}
