//! The Background tab: the user's own app background, a colour or an image.

use crate::components::customize::part_panel::{ edit_part, starting_point, PartPanel };
use crate::components::ui::FillField;
use crate::libs::routes::background_style_for;
use crate::state::config::BackgroundCustomization;
use crate::utils::config::use_config;
use dioxus::prelude::*;
use std::rc::Rc;

#[component]
pub fn BackgroundTab() -> Element {
    let (config, update_config) = use_config();
    let enabled = config().enable_background_customization;
    let saved = config().background_customization.clone();
    // The theme's own background is the default one, so with nothing saved
    // the fields start from the defaults.
    let background = if enabled { saved.clone() } else { starting_point(&saved, BackgroundCustomization::default()) };

    let edit: Rc<dyn Fn(Box<dyn FnOnce(&mut BackgroundCustomization)>)> = {
        let update_config = update_config.clone();
        Rc::new(move |change: Box<dyn FnOnce(&mut BackgroundCustomization)>| {
            update_config(
                Box::new(move |cfg| {
                    edit_part(
                        &mut cfg.background_customization,
                        &mut cfg.enable_background_customization,
                        BackgroundCustomization::default(),
                        change
                    );
                })
            );
        })
    };

    let on_switch = {
        let edit = edit.clone();
        let update_config = update_config.clone();
        move |on: bool| {
            if on {
                edit(Box::new(|_| {}));
            } else {
                update_config(Box::new(|cfg| cfg.enable_background_customization = false));
            }
        }
    };
    let on_start_over = {
        let update_config = update_config.clone();
        move |_| {
            update_config(
                Box::new(|cfg| {
                    cfg.background_customization = BackgroundCustomization::default();
                    cfg.enable_background_customization = false;
                })
            );
        }
    };

    // The preview is the background on screen; with nothing of the user's
    // on, the theme's own colour shows through.
    let preview_style = format!("height: 4rem; {}", background_style_for(enabled, &saved));

    rsx! {
      PartPanel {
        switch_label: "Use my own background".to_string(),
        enabled,
        on_switch,
        on_start_over,
        preview: rsx! {
          div { class: "rounded-box border border-base-300 bg-base-100", style: "{preview_style}" }
        },
        FillField {
          label: "Background".to_string(),
          description: None,
          use_image: background.use_image,
          color: background.background_color.clone(),
          image: background.background_image.clone(),
          on_use_image: {
              let edit = edit.clone();
              move |value: bool| edit(Box::new(move |b| b.use_image = value))
          },
          on_color: {
              let edit = edit.clone();
              move |value: String| edit(Box::new(move |b| b.background_color = value))
          },
          on_image: {
              let edit = edit.clone();
              move |value: Option<String>| edit(Box::new(move |b| b.background_image = value))
          },
          field: "background_color".to_string(),
        }
      }
    }
}
