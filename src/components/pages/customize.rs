use crate::components::customize::{
    BackgroundTab,
    ColorsTab,
    CurrentThemeRow,
    CustomizeTab,
    CustomizeTabs,
    LogoTab,
    ThemesTab,
    CURRENT_TAB,
};
use crate::components::ui::PageHeader;
use crate::i18n::use_strings;
use dioxus::prelude::*;
use lucide_dioxus::Palette;

#[component]
pub fn CustomizePage() -> Element {
    // Reads the config signal, so choosing a language re-renders the page.
    let strings = use_strings();

    rsx! {
      div {
        PageHeader {
          title: strings.customize.page_title.to_string(),
          subtitle: strings.customize.page_subtitle.to_string(),
          icon: Some(rsx! {
            Palette { class: "w-8 h-8 mx-auto" }
          }),
        }
        div { class: "{crate::utils::spacing::SECTION_SPACING}",
          CurrentThemeRow {}
          CustomizeTabs {
            match CURRENT_TAB() {
                CustomizeTab::Themes => rsx! {
                  ThemesTab {}
                },
                CustomizeTab::Colors => rsx! {
                  ColorsTab {}
                },
                CustomizeTab::Logo => rsx! {
                  LogoTab {}
                },
                CustomizeTab::Background => rsx! {
                  BackgroundTab {}
                },
            }
          }
        }
      }
    }
}
