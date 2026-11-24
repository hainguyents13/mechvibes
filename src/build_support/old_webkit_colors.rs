//! Color fallbacks for the WebKit in macOS 12, which has no `color-mix()`.
//!
//! daisyUI writes a plain fallback first and the `color-mix()` value inside
//! `@supports (color: color-mix(in lab, red, red))`. Many of those fallbacks
//! are the strong color of the mix (a soft button falls back to the text color
//! for its background), so on macOS 12 buttons and cards came up as dark text
//! on a dark fill. Mixes are usually built on CSS variables, so they cannot be
//! computed at build time. Instead every mix is replaced by the color that
//! makes up most of it, and the `@supports` blocks are opened up so that
//! choice is used.

/// Stands in for a color faded mostly to transparent (separators, a slider's
/// empty track, card fills, disabled text). Over the page background those
/// read as a soft shade, which is what every theme's `base-300` is: still
/// visible, never as loud as the full color.
const FAINT_TINT: &str = "var(--color-base-300)";

const SUPPORTS_COLOR_MIX: &str = "@supports (color: color-mix(in lab, red, red))";

/// Rewrites `css` so it needs no `color-mix()` support.
pub fn replace_color_mix(css: &str) -> String {
    let mut css = css.replace(SUPPORTS_COLOR_MIX, "@supports (color: red)");
    while let Some(start) = css.find("color-mix(") {
        let open = start + "color-mix".len();
        let Some(close) = matching_paren(&css, open) else { break };
        let replacement = dominant_color(&css[open + 1..close]);
        css.replace_range(start..=close, &replacement);
    }
    css
}

/// Index of the `)` that closes the `(` at `open`.
fn matching_paren(s: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (i, c) in s[open..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open + i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Splits on commas that are not inside parentheses.
fn split_top_level(args: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let (mut depth, mut from) = (0usize, 0usize);
    for (i, c) in args.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(&args[from..i]);
                from = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&args[from..]);
    parts
}

/// Splits `var(--a) 8%` into the color and its percentage, if any.
fn color_and_weight(operand: &str) -> (&str, Option<f32>) {
    let operand = operand.trim();
    if let Some((color, last)) = operand.rsplit_once(char::is_whitespace) {
        if let Some(number) = last.strip_suffix('%') {
            if let Ok(weight) = number.parse::<f32>() {
                return (color.trim(), Some(weight));
            }
        }
    }
    (operand, None)
}

fn is_transparent(color: &str) -> bool {
    matches!(color, "#0000" | "#00000000" | "transparent")
}

/// The color that makes up most of `in <space>, <a> [p%], <b> [q%]`.
///
/// A color faded toward transparent keeps the color from 50% up and becomes
/// [`FAINT_TINT`] below that.
fn dominant_color(args: &str) -> String {
    let parts = split_top_level(args);
    let [_space, a, b] = parts.as_slice() else {
        // Not the shape daisyUI writes; keep the last argument, a color.
        return parts.last().map(|s| s.trim().to_string()).unwrap_or_default();
    };
    let ((a, wa), (b, wb)) = (color_and_weight(a), color_and_weight(b));
    let (wa, wb) = match (wa, wb) {
        (Some(wa), Some(wb)) => (wa, wb),
        (Some(wa), None) => (wa, 100.0 - wa),
        (None, Some(wb)) => (100.0 - wb, wb),
        (None, None) => (50.0, 50.0),
    };
    let pick = if is_transparent(b) {
        if wa >= 50.0 { a } else { FAINT_TINT }
    } else if is_transparent(a) {
        if wb >= 50.0 { b } else { FAINT_TINT }
    } else if wb > wa {
        b
    } else {
        a
    };
    pick.to_string()
}

#[cfg(test)]
mod tests {
    use super::replace_color_mix;

    #[test]
    fn a_soft_button_falls_back_to_the_light_background() {
        let css = "--btn-bg: var(--btn-color, var(--color-base-content));\n\
                   @supports (color: color-mix(in lab, red, red)) {\n\
                   --btn-bg: color-mix(in oklab, var(--btn-color, var(--color-base-content)) 8%, var(--color-base-100));\n}";
        let out = replace_color_mix(css);
        assert!(!out.contains("color-mix"), "{out}");
        assert!(out.contains("@supports (color: red)"), "{out}");
        assert!(out.contains("--btn-bg: var(--color-base-100);"), "{out}");
    }

    #[test]
    fn the_larger_share_wins() {
        assert_eq!(replace_color_mix("color-mix(in oklab, var(--a) 70%, var(--b))"), "var(--a)");
        assert_eq!(replace_color_mix("color-mix(in oklab, var(--a), var(--b) 60%)"), "var(--b)");
        assert_eq!(replace_color_mix("color-mix(in oklab, var(--a), #000 7%)"), "var(--a)");
        assert_eq!(replace_color_mix("color-mix(in oklab, var(--a), var(--b))"), "var(--a)");
    }

    #[test]
    fn a_mostly_transparent_color_becomes_a_soft_shade() {
        let soft = "var(--color-base-300)";
        assert_eq!(replace_color_mix("color-mix(in oklch, var(--line) 5%, #0000)"), soft);
        assert_eq!(replace_color_mix("color-mix(in oklab, currentColor 10%, transparent)"), soft);
        assert_eq!(replace_color_mix("color-mix(in oklch, transparent, var(--text) 40%)"), soft);
        assert_eq!(replace_color_mix("color-mix(in oklch, var(--text) 60%, #0000)"), "var(--text)");
    }

    #[test]
    fn nested_mixes_are_resolved_too() {
        let css = "color: color-mix(in oklab, color-mix(in oklab, var(--a) 90%, #fff) 80%, var(--b));";
        assert_eq!(replace_color_mix(css), "color: var(--a);");
    }

    #[test]
    fn the_real_stylesheet_has_no_mix_left() {
        let css = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/style.css")).unwrap();
        assert!(css.contains("color-mix("));
        let out = replace_color_mix(&css);
        assert!(!out.contains("color-mix"));
        assert!(!out.contains("@supports (color: color-mix"));
    }
}
