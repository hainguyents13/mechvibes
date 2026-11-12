use dioxus::prelude::*;
use std::collections::HashMap;

/// Which section is open in each accordion group (`group_name` -> title).
/// `Some(None)` means the user closed everything; no entry means the group has
/// not been touched yet, so each section falls back to its `default_open`.
static OPEN_SECTIONS: GlobalSignal<HashMap<String, Option<String>>> = Signal::global(HashMap::new);

#[derive(Props, Clone, PartialEq)]
pub struct CollapseProps {
    /// The title displayed in the collapse header
    pub title: String,
    /// The content to be displayed when expanded
    pub children: Element,
    /// The radio group name for accordion behavior
    pub group_name: String,
    /// Whether this collapse item is expanded by default
    #[props(default = false)]
    pub default_open: bool,
    /// Optional variant for styling (border, base-300, etc.)
    #[props(default = "border border-base-300 bg-base-200 text-base-content")]
    pub variant: &'static str,
    /// Optional title styling
    #[props(default = "collapse-title font-semibold")]
    pub title_class: &'static str,
    /// Optional content styling
    #[props(default = "collapse-content")]
    pub content_class: &'static str,
    /// Custom CSS classes for the container
    #[props(default = "")]
    pub class: &'static str,
    /// Show indicator dot when enabled
    #[props(default = false)]
    pub show_indicator: bool,
    /// Only build `children` while this section is expanded.
    ///
    /// This is a DaisyUI CSS accordion: the radio input drives visibility
    /// through stylesheet rules, so by default every section's children are
    /// mounted in the DOM at all times and merely hidden. That is fine for
    /// static content, but a section whose content runs a poll loop or holds
    /// other live state would keep paying for it while collapsed. Opting in
    /// here makes the content mount on expand and unmount on collapse, which
    /// drops any `use_future` it owns.
    #[props(default = false)]
    pub lazy_children: bool,
}

/// Whether `children` should be built this render.
///
/// Split out from the component so the rule can be asserted directly: an
/// ordinary section always renders (preserving the CSS-accordion behavior every
/// existing caller relies on), and a lazy one renders only while expanded.
fn should_render_children(lazy: bool, is_open: bool) -> bool {
    !lazy || is_open
}

#[component]
pub fn Collapse(props: CollapseProps) -> Element {
    let container_class = format!(
        "collapse relative w-full collapse-arrow {} {}",
        props.variant,
        props.class
    );

    // Open/closed is owned here, not by the browser, so a click on the open
    // section closes it and opening another still closes the previous one.
    let group = props.group_name.clone();
    let title = props.title.clone();

    // Forget the group when this section goes away (e.g. leaving Settings), so
    // coming back starts from `default_open` exactly as before.
    let group_for_drop = group.clone();
    use_drop(move || {
        OPEN_SECTIONS.write().remove(&group_for_drop);
    });
    let is_open = match OPEN_SECTIONS.read().get(&group) {
        Some(open) => open.as_deref() == Some(title.as_str()),
        None => props.default_open,
    };
    let render_children = should_render_children(props.lazy_children, is_open);

    rsx! {
        div { class: "{container_class}",
            input {
                // A checkbox, not a radio: a checked radio cannot be unchecked
                // by clicking it. One-open-at-a-time is enforced by the shared
                // state above, and DaisyUI styles checkbox collapses the same.
                r#type: "checkbox",
                checked: is_open,
                onclick: move |_| {
                    let next = if is_open { None } else { Some(title.clone()) };
                    OPEN_SECTIONS.write().insert(group.clone(), next);
                },
            }
            div { class: "{props.title_class}",
                span { class: "flex items-center gap-2",
                    "{props.title}"
                    if props.show_indicator {
                        span { class: "w-2 h-2 rounded-full bg-primary" }
                    }
                }
            }
            div { class: "{props.content_class}",
                if render_children {
                    {props.children}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::should_render_children;

    #[test]
    fn ordinary_sections_always_render_their_children() {
        // Every pre-existing caller depends on this: the CSS accordion hides
        // collapsed content rather than unmounting it, and changing that for
        // non-opted-in sections would alter how they behave.
        assert!(should_render_children(false, true));
        assert!(should_render_children(false, false));
    }

    #[test]
    fn lazy_sections_render_only_while_expanded() {
        // What makes the Debug viewer's poll loop stop when the section is
        // closed. If this ever returns true while collapsed, that loop runs
        // for every user forever.
        assert!(should_render_children(true, true));
        assert!(!should_render_children(true, false));
    }
}
