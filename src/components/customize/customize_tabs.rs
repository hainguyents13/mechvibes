//! The tab bar of Customize: Themes, Colors, Logo and Background.

use crate::i18n::catalog::CustomizeStrings;
use crate::i18n::use_strings;
use dioxus::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomizeTab {
    Themes,
    Colors,
    Logo,
    Background,
}

impl CustomizeTab {
    pub const ALL: [CustomizeTab; 4] = [
        CustomizeTab::Themes,
        CustomizeTab::Colors,
        CustomizeTab::Logo,
        CustomizeTab::Background,
    ];

    /// The tab's label. Takes the catalog rather than reading it here, so the
    /// tab bar can pass the one it already has and nothing is looked up twice.
    pub fn label(self, strings: &CustomizeStrings) -> &'static str {
        match self {
            CustomizeTab::Themes => strings.tab_themes,
            CustomizeTab::Colors => strings.tab_colors,
            CustomizeTab::Logo => strings.tab_logo,
            CustomizeTab::Background => strings.tab_background,
        }
    }
}

/// The tab open on Customize. The router builds the page again on every
/// visit, so the tab lives here rather than in the page, and coming back
/// opens the one left open. It lasts for the session and is never saved.
pub static CURRENT_TAB: GlobalSignal<CustomizeTab> = Signal::global(|| CustomizeTab::Themes);

/// The tab bar, with `children` as the open tab's panel. Only that panel is
/// built; the others are not mounted at all.
#[component]
pub fn CustomizeTabs(children: Element) -> Element {
    let current = CURRENT_TAB();
    // The zone's catalog, not the whole one: this bar only shows tab labels.
    // Read here, in the body, so a language change re-renders the bar.
    let strings = &use_strings().customize;

    rsx! {
      div { class: "tabs tabs-lift",
        for tab in CustomizeTab::ALL {
          label {
            class: "tab [--tab-border-color:var(--color-base-300)] [--tab-bg:var(--color-base-200)]",
            input {
              r#type: "radio",
              name: "customize-tab",
              checked: current == tab,
              onchange: move |_| *CURRENT_TAB.write() = tab,
            }
            "{tab.label(strings)}"
          }
          // daisyUI shows the panel that comes right after the checked tab.
          if current == tab {
            div { class: "tab-content bg-base-200 border-base-300 p-4",
              {children.clone()}
            }
          }
        }
      }
    }
}
