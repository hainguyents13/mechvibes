use crate::components::logo::logo_style;
use crate::components::theme_toggler::ThemeToggler;
use crate::components::ui::{ Collapse, ColorPicker, FillField, PageHeader, Toggler, THEME_COLOR_OPTIONS };
use crate::utils::config::use_config;
use dioxus::prelude::*;
use lucide_dioxus::{ Palette, RotateCcw };
use std::rc::Rc;

#[component]
pub fn CustomizePage() -> Element {
    // let (config, update_config) = use_config();
    // let mut saving = use_signal(|| false);

    // let custom_css = use_memo(move || config().custom_css.clone());
    // let mut css_input = use_signal(|| custom_css());
    // let on_save = move |_| {
    //     let css = css_input().clone();
    //     update_config(Box::new(move |cfg| {
    //         cfg.custom_css = css;
    //     }));
    //     saving.set(true);
    //     spawn(async move {
    //         futures_timer::Delay::new(std::time::Duration::from_millis(1500)).await;
    //         saving.set(false);
    //     });
    // };
    rsx! {
      div { class: "",
        PageHeader {
          title: "Customize".to_string(),
          subtitle: "Vibe it your way!".to_string(),
          icon: Some(rsx! {
            Palette { class: "w-8 h-8 mx-auto" }
          }),
        }
        // Settings sections
        div { class: "{crate::utils::spacing::SECTION_SPACING} mt-8", // Theme Section
          Collapse {
            title: "Themes".to_string(),
            group_name: "customize-accordion".to_string(),
            default_open: true,
            variant: "border border-base-300 bg-base-200 text-base-content",
            content_class: "collapse-content text-sm text-base-content/70",
            children: rsx! {
              div { "Choose your preferred theme or create custom ones" }
              // Built-in theme toggler
              ThemeToggler {}
            },
          }
          LogoCollapseSection {}
          BackgroundCollapseSection {}
                // Custom CSS Section
        // div { class: "collapse collapse-arrow border border-base-300 bg-base-200 text-base-content",
        //   input { r#type: "radio", name: "customize-accordion" }
        //   div { class: "collapse-title font-semibold", "Custom CSS" }
        //   div { class: "collapse-content",
        //     fieldset { class: "fieldset mb-2",
        //       legend { class: "fieldset-legend", "Add your custom CSS here" }
        //       textarea {
        //         class: "textarea w-full h-32 font-mono text-sm",
        //         value: css_input(),
        //         oninput: move |evt| css_input.set(evt.value()),
        //       }
        //       div { class: "label",
        //         "Apply your own styles to customize the look and feel of the app."
        //       }
        //     }
        //     button {
        //       class: "btn btn-neutral btn-sm",
        //       r#type: "button",
        //       disabled: saving(),
        //       onclick: on_save,
        //       if saving() {
        //         span { class: "loading loading-spinner loading-sm mr-2" }
        //       }
        //       "Save"
        //     }
        //   }
        // }
        }
      }
    }
}

#[component]
fn LogoCollapseSection() -> Element {
    let (config, _) = use_config();
    let enable_logo_customization = use_memo(move || config().enable_logo_customization);

    rsx! {
        Collapse {
            title: "Logo".to_string(),
            group_name: "customize-accordion".to_string(),
            variant: "border border-base-300 bg-base-200 text-base-content",
            content_class: "collapse-content overflow-visible text-base-content/70",
            show_indicator: enable_logo_customization(),
            children: rsx! {
                LogoCustomizationSection {}
            },
        }
    }
}

#[component]
fn BackgroundCollapseSection() -> Element {
    let (config, _) = use_config();
    let enable_background_customization = use_memo(move || config().enable_background_customization);

    rsx! {
        Collapse {
            title: "Background".to_string(),
            group_name: "customize-accordion".to_string(),
            variant: "border border-base-300 bg-base-200 text-base-content",
            content_class: "collapse-content overflow-visible text-base-content/70",
            show_indicator: enable_background_customization(),
            children: rsx! {
                BackgroundCustomizationSection {}
            },
        }
    }
}

#[component]
fn LogoCustomizationSection() -> Element {
    let (config, update_config) = use_config();
    let enable_logo_customization = use_memo(move || config().enable_logo_customization);

    // Create a local signal that syncs with config
    let mut local_enable = use_signal(|| enable_logo_customization());

    // Update local state when config changes
    use_effect(move || {
        local_enable.set(enable_logo_customization());
    });
    rsx! {
      div { class: "space-y-4", // Toggle switch for logo customization
        Toggler {
          title: "Enable logo customization".to_string(),
          description: Some("Customize border, text, shadow and background colors".to_string()),
          checked: local_enable(),
          on_change: move |new_value: bool| {
              local_enable.set(new_value);
              update_config(
                  Box::new(move |cfg| {
                      cfg.enable_logo_customization = new_value;
                  }),
              );
          },
        }
        // Show LogoCustomizationPanel only when enabled
        if local_enable() {
          LogoCustomizationPanel {}
        }
      }
    }
}

#[component]
fn LogoCustomizationPanel() -> Element {
    let (config, update_config) = use_config();

    // The config is the single source of truth. An earlier version mirrored
    // every field into a local signal and synced it back with a `use_effect`,
    // which re-ran on any re-render and overwrote the user's pending edit with
    // the stored value - so changing a colour, and Reset, both appeared dead.
    let logo = use_memo(move || config().logo_customization.clone());

    // Each edit publishes straight to the config, so the real logo updates as
    // the user picks. `config_writer::apply` skips writing when a mutation
    // leaves the data unchanged, so re-selecting the current colour is free.
    let set_logo: Rc<dyn Fn(Box<dyn FnOnce(&mut crate::state::config::LogoCustomization)>)> = {
        let update_config = update_config.clone();
        Rc::new(move |mutate: Box<dyn FnOnce(&mut crate::state::config::LogoCustomization)>| {
            update_config(
                Box::new(move |cfg| {
                    mutate(&mut cfg.logo_customization);
                })
            );
        })
    };

    let color_options = THEME_COLOR_OPTIONS.to_vec();
    let on_reset = {
        let update_config = update_config.clone();
        move |_| {
            update_config(
                Box::new(move |cfg| {
                    cfg.logo_customization =
                        crate::state::config::LogoCustomization::default();
                })
            );
        }
    };

    // The preview is styled exactly as the real logo, only smaller.
    let preview_base =
        "select-none border-3 font-black py-2 px-4 text-2xl rounded-box flex justify-center items-center w-full mt-1 ";
    let (normal_class, normal_style) = logo_style(Some(&logo()), true, false);
    let (muted_class, muted_style) = logo_style(Some(&logo()), false, false);

    rsx! {
      div { class: "space-y-4",
        // Preview
        div { class: "space-y-2",
          div { class: "text-sm text-base-content", "Preview" }
          div { class: "grid grid-cols-2 gap-2 p-4 bg-base-100 rounded-box border border-base-300 space-y-3",
            // Normal state preview
            div {
              div { class: "text-xs text-base-content/70", "Normal" }
              div {
                class: "{preview_base}{normal_class}",
                style: "{normal_style}",
                "Mechvibes"
              }
            }
            // Muted state preview
            div {
              div { class: "text-xs text-base-content/70", "Muted" }
              div {
                class: "{preview_base}{muted_class}",
                style: "{muted_style}",
                "Mechvibes"
              }
            }
          }
        }
        // Border Color
        ColorPicker {
          label: "Border Color".to_string(),
          selected_value: logo().border_color,
          options: color_options.clone(),
          placeholder: "Select a color...".to_string(),
          on_change: {
            let set_logo = set_logo.clone();
            move |color: String| set_logo(Box::new(move |l| { l.border_color = color; }))
          },
          field: "border_color".to_string(),
          description: None,
        }
        // Text Color
        ColorPicker {
          label: "Text Color".to_string(),
          selected_value: logo().text_color,
          options: color_options.clone(),
          placeholder: "Select a color...".to_string(),
          on_change: {
            let set_logo = set_logo.clone();
            move |value: String| set_logo(Box::new(move |l| { l.text_color = value; }))
          },
          field: "text_color".to_string(),
          description: None,
        } // Shadow Color
        ColorPicker {
          label: "Shadow Color".to_string(),
          selected_value: logo().shadow_color,
          options: color_options.clone(),
          placeholder: "Select a color...".to_string(),
          on_change: {
            let set_logo = set_logo.clone();
            move |value: String| set_logo(Box::new(move |l| { l.shadow_color = value; }))
          },
          field: "shadow_color".to_string(),
          description: None,
        }
        // Background Section
        div { class: "space-y-3 p-3 border border-base-300 rounded-box bg-base-100",
          FillField {
            label: "Background (Normal)".to_string(),
            description: None,
            use_image: logo().use_background_image,
            color: logo().background_color,
            image: logo().background_image,
            on_use_image: {
              let set_logo = set_logo.clone();
              move |new_value: bool| {
                  set_logo(Box::new(move |l| { l.use_background_image = new_value; }))
              }
            },
            on_color: {
              let set_logo = set_logo.clone();
              move |value: String| set_logo(Box::new(move |l| { l.background_color = value; }))
            },
            on_image: {
              let set_logo = set_logo.clone();
              move |value: Option<String>| {
                  set_logo(Box::new(move |l| { l.background_image = value; }))
              }
            },
            field: "background_color".to_string(),
          }
        }

        // Muted Background Section
        div { class: "space-y-3 p-3 border border-base-300 rounded-box bg-base-100",
          FillField {
            label: "Background (Muted)".to_string(),
            description: Some("Shown when sound is off".to_string()),
            use_image: logo().use_muted_background_image,
            color: logo().muted_background,
            image: logo().muted_background_image,
            on_use_image: {
              let set_logo = set_logo.clone();
              move |new_value: bool| {
                  set_logo(Box::new(move |l| { l.use_muted_background_image = new_value; }))
              }
            },
            on_color: {
              let set_logo = set_logo.clone();
              move |value: String| set_logo(Box::new(move |l| { l.muted_background = value; }))
            },
            on_image: {
              let set_logo = set_logo.clone();
              move |value: Option<String>| {
                  set_logo(Box::new(move |l| { l.muted_background_image = value; }))
              }
            },
            field: "muted_background".to_string(),
          }
        }
        // Dimmed logo when muted option
        Toggler {
          title: "Dimmed logo when muted".to_string(),
          description: Some("Applies opacity to the logo when sound is disabled".to_string()),
          checked: logo().dimmed_when_muted,
          on_change: {
            let set_logo = set_logo.clone();
            move |new_value: bool| {
                set_logo(Box::new(move |l| { l.dimmed_when_muted = new_value; }))
            }
          },
        }
      }

      // Action buttons
      div { class: "flex gap-2 mt-3",
        button { class: "btn btn-ghost btn-sm", onclick: on_reset,
          RotateCcw { class: "w-4 h-4 mr-1" }
          "Reset"
        }
      }
      div { class: "text-sm text-base-content/50 mt-3",
        "Changes apply immediately. Reset reverts the logo to the selected theme colors."
      }
    }
}

#[component]
fn BackgroundCustomizationSection() -> Element {
    let (config, update_config) = use_config();
    let enable_background_customization = use_memo(
        move || config().enable_background_customization
    );

    // Create a local signal that syncs with config
    let mut local_enable = use_signal(|| enable_background_customization());

    // Update local state when config changes
    use_effect(move || {
        local_enable.set(enable_background_customization());
    });

    rsx! {
      div { class: "space-y-4",
        // Toggle switch for background customization
        Toggler {
          title: "Enable background customization".to_string(),
          description: Some("Customize app background with colors or images".to_string()),
          checked: local_enable(),
          on_change: move |new_value: bool| {
              local_enable.set(new_value);
              update_config(
                  Box::new(move |cfg| {
                      cfg.enable_background_customization = new_value;
                  }),
              );
          },
        }
        // Show BackgroundCustomizationPanel only when enabled
        if local_enable() {
          BackgroundCustomizationPanel {}
        }
      }
    }
}

#[component]
fn BackgroundCustomizationPanel() -> Element {
    let (config, update_config) = use_config();

    // Config is the single source of truth here too - see the note in
    // `LogoCustomizationPanel` for why the mirrored local signals were removed.
    let bg = use_memo(move || config().background_customization.clone());

    let set_bg: Rc<
        dyn Fn(Box<dyn FnOnce(&mut crate::state::config::BackgroundCustomization)>)
    > = {
        let update_config = update_config.clone();
        Rc::new(move |mutate: Box<dyn FnOnce(&mut crate::state::config::BackgroundCustomization)>| {
            update_config(
                Box::new(move |cfg| {
                    mutate(&mut cfg.background_customization);
                })
            );
        })
    };

    let on_reset = {
        let update_config = update_config.clone();
        move |_| {
            update_config(
                Box::new(move |cfg| {
                    cfg.background_customization =
                        crate::state::config::BackgroundCustomization::default();
                })
            );
        }
    };

    rsx! {
      div { class: "space-y-4",
        FillField {
          label: "Background".to_string(),
          description: None,
          use_image: bg().use_image,
          color: bg().background_color,
          image: bg().background_image,
          on_use_image: {
            let set_bg = set_bg.clone();
            move |new_value: bool| set_bg(Box::new(move |b| { b.use_image = new_value; }))
          },
          on_color: {
            let set_bg = set_bg.clone();
            move |color: String| set_bg(Box::new(move |b| { b.background_color = color; }))
          },
          on_image: {
            let set_bg = set_bg.clone();
            move |value: Option<String>| {
                set_bg(Box::new(move |b| { b.background_image = value; }))
            }
          },
          field: "background_color".to_string(),
        }

        // Action buttons
        div { class: "flex gap-2 mt-4",
          button { class: "btn btn-ghost btn-sm", onclick: on_reset,
            RotateCcw { class: "w-4 h-4 mr-1" }
            "Reset"
          }
        }
        div { class: "text-sm text-base-content/50",
          "Changes apply immediately. Reset reverts the background to the selected theme."
        }
      }
    }
}
