//! The Logo tab: the user's own logo colours and backgrounds.

use crate::components::customize::part_panel::{ edit_part, starting_point, PartPanel };
use crate::components::logo::logo_style;
use crate::components::ui::{ ColorPicker, FillField, Toggler, THEME_COLOR_OPTIONS };
use crate::state::config::LogoCustomization;
use crate::utils::config::use_config;
use dioxus::prelude::*;
use std::rc::Rc;

#[component]
pub fn LogoTab() -> Element {
    let (config, update_config) = use_config();
    let enabled = config().enable_logo_customization;
    let saved = config().logo_customization.clone();
    // The config is the single source of truth: every field reads it and
    // every change writes it, with no local copy to fall out of step.
    let logo = if enabled { saved.clone() } else { starting_point(&saved, LogoCustomization::theme_look()) };

    let edit: Rc<dyn Fn(Box<dyn FnOnce(&mut LogoCustomization)>)> = {
        let update_config = update_config.clone();
        Rc::new(move |change: Box<dyn FnOnce(&mut LogoCustomization)>| {
            update_config(
                Box::new(move |cfg| {
                    edit_part(
                        &mut cfg.logo_customization,
                        &mut cfg.enable_logo_customization,
                        LogoCustomization::theme_look(),
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
                // Turned on as the fields show it.
                edit(Box::new(|_| {}));
            } else {
                update_config(Box::new(|cfg| cfg.enable_logo_customization = false));
            }
        }
    };
    let on_start_over = {
        let update_config = update_config.clone();
        move |_| {
            update_config(
                Box::new(|cfg| {
                    cfg.logo_customization = LogoCustomization::default();
                    cfg.enable_logo_customization = false;
                })
            );
        }
    };

    // The preview is the logo on screen, only smaller.
    let preview_base =
        "select-none border-3 font-black py-2 px-4 text-2xl rounded-box flex justify-center items-center w-full mt-1 ";
    let on_screen = enabled.then(|| saved.clone());
    let (normal_class, normal_style) = logo_style(on_screen.as_ref(), true, false);
    let (muted_class, muted_style) = logo_style(on_screen.as_ref(), false, false);

    rsx! {
      PartPanel {
        switch_label: "Use my own logo".to_string(),
        enabled,
        on_switch,
        on_start_over,
        preview: rsx! {
          div { class: "grid grid-cols-2 gap-2 p-4 bg-base-100 rounded-box border border-base-300",
            div {
              div { class: "text-xs text-base-content/70", "Sound on" }
              div { class: "{preview_base}{normal_class}", style: "{normal_style}", "Mechvibes" }
            }
            div {
              div { class: "text-xs text-base-content/70", "Sound off" }
              div { class: "{preview_base}{muted_class}", style: "{muted_style}", "Mechvibes" }
            }
          }
        },
        ColorPicker {
          label: "Border".to_string(),
          selected_value: logo.border_color.clone(),
          options: THEME_COLOR_OPTIONS.to_vec(),
          placeholder: "Pick a color".to_string(),
          on_change: {
              let edit = edit.clone();
              move |value: String| edit(Box::new(move |l| l.border_color = value))
          },
          field: "border_color".to_string(),
          description: None,
        }
        ColorPicker {
          label: "Text".to_string(),
          selected_value: logo.text_color.clone(),
          options: THEME_COLOR_OPTIONS.to_vec(),
          placeholder: "Pick a color".to_string(),
          on_change: {
              let edit = edit.clone();
              move |value: String| edit(Box::new(move |l| l.text_color = value))
          },
          field: "text_color".to_string(),
          description: None,
        }
        ColorPicker {
          label: "Shadow".to_string(),
          selected_value: logo.shadow_color.clone(),
          options: THEME_COLOR_OPTIONS.to_vec(),
          placeholder: "Pick a color".to_string(),
          on_change: {
              let edit = edit.clone();
              move |value: String| edit(Box::new(move |l| l.shadow_color = value))
          },
          field: "shadow_color".to_string(),
          description: None,
        }
        div { class: "space-y-3 p-3 border border-base-300 rounded-box bg-base-100",
          FillField {
            label: "Logo background".to_string(),
            description: None,
            use_image: logo.use_background_image,
            color: logo.background_color.clone(),
            image: logo.background_image.clone(),
            on_use_image: {
                let edit = edit.clone();
                move |value: bool| edit(Box::new(move |l| l.use_background_image = value))
            },
            on_color: {
                let edit = edit.clone();
                move |value: String| edit(Box::new(move |l| l.background_color = value))
            },
            on_image: {
                let edit = edit.clone();
                move |value: Option<String>| edit(Box::new(move |l| l.background_image = value))
            },
            field: "background_color".to_string(),
          }
        }
        div { class: "space-y-3 p-3 border border-base-300 rounded-box bg-base-100",
          FillField {
            label: "Logo background when sound is off".to_string(),
            description: None,
            use_image: logo.use_muted_background_image,
            color: logo.muted_background.clone(),
            image: logo.muted_background_image.clone(),
            on_use_image: {
                let edit = edit.clone();
                move |value: bool| edit(Box::new(move |l| l.use_muted_background_image = value))
            },
            on_color: {
                let edit = edit.clone();
                move |value: String| edit(Box::new(move |l| l.muted_background = value))
            },
            on_image: {
                let edit = edit.clone();
                move |value: Option<String>| edit(Box::new(move |l| l.muted_background_image = value))
            },
            field: "muted_background".to_string(),
          }
        }
        Toggler {
          title: "Fade the logo when sound is off".to_string(),
          description: None,
          checked: logo.dimmed_when_muted,
          on_change: {
              let edit = edit.clone();
              move |value: bool| edit(Box::new(move |l| l.dimmed_when_muted = value))
          },
        }
      }
    }
}
