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
use dioxus::prelude::*;
use lucide_dioxus::Palette;

#[component]
pub fn CustomizePage() -> Element {
    rsx! {
      div {
        PageHeader {
          title: "Customize".to_string(),
          subtitle: "Vibe it your way!".to_string(),
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
