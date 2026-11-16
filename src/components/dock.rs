use dioxus::prelude::*;
use lucide_dioxus::{ House, Music, Palette, Settings, CloudSunRain };

#[allow(non_snake_case)]
#[component]
pub fn Dock() -> Element {
    let route = use_route::<crate::libs::routes::Route>();
    let (config, _) = crate::utils::config::use_config();
    // Reads the config signal, so choosing a language in Settings re-renders
    // this dock with the new labels - that is the on-screen proof the strings
    // are reactive. Not memoized: a `use_memo` would freeze the first language.
    let strings = crate::i18n::use_strings();
    let floating = config().floating_dock;

    rsx! {
      div {
        class: "dock dock-xl bg-base-300/30 backdrop-blur-lg ",
        // The floating pill is opt-in (Settings > General > Floating dock).
        // Inline on purpose: the Tailwind stylesheet (assets/style.css) is a
        // committed build artifact, so new utility classes would not exist in
        // it until it is regenerated.
        style: if floating {
            Some(
                "left: 0.75rem; right: 0.75rem; bottom: 0.75rem; width: auto; padding-inline: 0.75rem; border-radius: 9999px; border: 1px solid color-mix(in oklab, var(--color-base-content) 10%, transparent); box-shadow: 0 10px 15px -3px rgb(0 0 0 / 0.15);"
            )
        } else {
            None
        },
        // Button Home
        button {
          class: if matches!(route, crate::libs::routes::Route::Home {}) { "dock-active" } else { "" },
          onclick: move |_| {
              crate::libs::routes::navigate("/");
          },
          House { class: "w-5 h-5" }
          span { class: "dock-label mt-1", "{strings.nav.home}" }
        }
        // Button Soundpacks
        button {
          class: if matches!(route, crate::libs::routes::Route::Soundpacks {}) { "dock-active" } else { "" },
          onclick: move |_| {
              crate::libs::routes::navigate("/soundpacks");
          },
          Music { class: "w-5 h-5" }
          span { class: "dock-label mt-1", "{strings.nav.soundpacks}" }
        }
        // Button Customize
        button {
          class: if matches!(route, crate::libs::routes::Route::Customize {}) { "dock-active" } else { "" },
          onclick: move |_| {
              crate::libs::routes::navigate("/customize");
          },
          Palette { class: "w-5 h-5" }
          span { class: "dock-label mt-1", "{strings.nav.customize}" }
        }
        // Button Mood
        button {
          class: if matches!(route, crate::libs::routes::Route::Mood {}) { "dock-active" } else { "" },
          onclick: move |_| {
              crate::libs::routes::navigate("/mood");
          },
          CloudSunRain { class: "w-5 h-5" }
          span { class: "dock-label mt-1", "{strings.nav.mood}" }
        }
        // Button Settings
        button {
          class: if matches!(route, crate::libs::routes::Route::Settings {}) { "dock-active" } else { "" },
          onclick: move |_| {
              crate::libs::routes::navigate("/settings");
          },
          Settings { class: "w-5 h-5" }
          span { class: "dock-label mt-1", "{strings.nav.settings}" }
        }
      }
    }
}
