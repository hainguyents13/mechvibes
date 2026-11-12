use dioxus::prelude::*;
use lucide_dioxus::{ House, Music, Palette, Settings, CloudSunRain };

#[allow(non_snake_case)]
#[component]
pub fn Dock() -> Element {
    let route = use_route::<crate::libs::routes::Route>();
    let (config, _) = crate::utils::config::use_config();
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
          span { class: "dock-label mt-1", "Home" }
        }
        // Button Soundpacks
        button {
          class: if matches!(route, crate::libs::routes::Route::Soundpacks {}) { "dock-active" } else { "" },
          onclick: move |_| {
              crate::libs::routes::navigate("/soundpacks");
          },
          Music { class: "w-5 h-5" }
          span { class: "dock-label mt-1", "Sound packs" }
        }
        // Button Customize
        button {
          class: if matches!(route, crate::libs::routes::Route::Customize {}) { "dock-active" } else { "" },
          onclick: move |_| {
              crate::libs::routes::navigate("/customize");
          },
          Palette { class: "w-5 h-5" }
          span { class: "dock-label mt-1", "Customize" }
        }
        // Button Mood
        button {
          class: if matches!(route, crate::libs::routes::Route::Mood {}) { "dock-active" } else { "" },
          onclick: move |_| {
              crate::libs::routes::navigate("/mood");
          },
          CloudSunRain { class: "w-5 h-5" }
          span { class: "dock-label mt-1", "Mood" }
        }
        // Button Settings
        button {
          class: if matches!(route, crate::libs::routes::Route::Settings {}) { "dock-active" } else { "" },
          onclick: move |_| {
              crate::libs::routes::navigate("/settings");
          },
          Settings { class: "w-5 h-5" }
          span { class: "dock-label mt-1", "Settings" }
        }
      }
    }
}
