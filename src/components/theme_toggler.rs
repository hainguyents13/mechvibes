use crate::libs::theme::{ use_theme, BuiltInTheme, Theme };
use crate::state::config::AppConfig;
use crate::state::themes::ThemesConfig;
use crate::utils::config::use_config;
use crate::utils::theme::use_themes;
use dioxus::document::eval;
use dioxus::prelude::*;
use lucide_dioxus::{ Ellipsis, ExternalLink, Palette, Pencil, Plus, Trash2 };

#[component]
pub fn ThemeToggler() -> Element {
    // Get the config and update_config function
    let (_config, update_config) = use_config();

    // Theme management
    let (themes, update_themes) = use_themes();

    // Theme state - use theme context
    let mut theme = use_theme();

    // State for editing themes
    let mut editing_theme = use_signal(|| None::<String>); // Theme ID being edited

    rsx! {
      div { class: "space-y-8 mt-4",
        // Built-in themes
        div { class: "space-y-2",
          div { class: "text-sm text-base-content", "Built-in themes" }
          div { class: "grid grid-cols-3 gap-2",
            for builtin_theme in BuiltInTheme::all().iter() {
              {
                  let builtin_theme_clone = builtin_theme.clone();
                  let is_active = matches!(
                      *theme.read(),
                      Theme::BuiltIn(ref current)
                      if current == builtin_theme
                  );
                  rsx! {
                    button {
                      class: format!(
                          "btn btn-soft text-left pl-2 justify-start text-xs {}",
                          if is_active { "btn-disabled" } else { "" },
                      ),
                      onclick: {
                          let builtin_theme = builtin_theme_clone.clone();
                          let update_fn = update_config.clone();
                          move |_| {
                              theme.set(Theme::BuiltIn(builtin_theme.clone()));
                              update_fn(
                                  Box::new({
                                      let builtin_theme = builtin_theme.clone();
                                      move |config: &mut AppConfig| {
                                          config.theme = Theme::BuiltIn(builtin_theme);
                                      }
                                  }),
                              );
                          }
                      },
                      div {
                        class: format!(
                            "bg-base-100 grid shrink-0 grid-cols-2 gap-0.5 rounded-md p-1 shadow-sm {}",
                            if is_active { "opacity-30" } else { "" },
                        ),
                        "data-theme": builtin_theme_clone.to_daisy_theme(),
                        div { class: "bg-primary size-2 rounded-full" }
                        div { class: "bg-secondary size-2 rounded-full" }
                        div { class: "bg-warning size-2 rounded-full" }
                        div { class: "bg-success size-2 rounded-full" }
                      }
                      div { class: "line-clamp-1 w-60",
                        {format!("{:?}", builtin_theme_clone)}
                      }
                    }
                  }
              }
            }
          }
        }
        // Custom themes
        div { class: "space-y-2",
          div { class: "text-sm text-base-content", "Custom themes" }
          {
              let themes_config = themes();
              let custom_themes = themes_config.list_themes();
              if !custom_themes.is_empty() {
                  rsx! {
                    for theme_data in custom_themes.iter() {
                      CustomThemeButton {
                        name: theme_data.name.clone(),
                        theme_id: theme_data.id.clone(),
                        theme_css: theme_data.css.clone(),
                        is_active: matches!(*theme.read(), Theme::Custom(ref current) if current == &theme_data.id),
                        is_built_in: false,
                        on_select: {
                            let theme_id = theme_data.id.clone();
                            let update_fn = update_config.clone();
                            move |_| {
                                theme.set(Theme::Custom(theme_id.clone()));
                                update_fn(
                                    Box::new({
                                        let theme_id = theme_id.clone();
                                        move |config: &mut AppConfig| {
                                            config.theme = Theme::Custom(theme_id);
                                        }
                                    }),
                                );
                            }
                        },
                        on_delete: {
                            let theme_id = theme_data.id.clone();
                            let update_themes = update_themes.clone();
                            move |_| {
                                update_themes(
                                    Box::new({
                                        let theme_id = theme_id.clone();
                                        move |themes: &mut ThemesConfig| {
                                            let _ = themes.delete_theme(&theme_id);
                                        }
                                    }),
                                );
                            }
                        },
                        on_edit: {
                            let theme_id = theme_data.id.clone();
                            move |_| {
                                editing_theme.set(Some(theme_id.clone()));
                                eval("theme_creator_modal.showModal()");
                            }
                        },
                      }
                    }
                  }
              } else {
                  rsx! {
                    div { class: "text-sm text-base-content/50", "No custom themes available" }
                  }
              }
          }
          // Create new theme button
          CreateThemeButton { editing_theme_id: editing_theme }
        }
      }
    }
}

#[derive(Props, Clone, PartialEq)]
struct CustomThemeButtonProps {
    name: String,
    theme_css: String,
    theme_id: String,
    is_active: bool,
    on_select: EventHandler<MouseEvent>,
    on_delete: EventHandler<MouseEvent>,
    on_edit: EventHandler<MouseEvent>,
    is_built_in: bool,
}

#[component]
fn CustomThemeButton(props: CustomThemeButtonProps) -> Element {
    rsx! {
      div { class: "flex w-full max-w-full items-center gap-2",
        button {
          class: format!(
              "gap-3 flex btn btn-soft px-2 grow text-left justify-start {}",
              if props.is_active { "btn-disabled" } else { "" },
          ),
          onclick: props.on_select,
          div {
            class: format!(
                "bg-base-100 grid shrink-0 grid-cols-2 gap-0.5 rounded-md p-1 shadow-sm {}",
                if props.is_active { "opacity-30" } else { "" },
            ),
            "data-theme": props.theme_id.clone(),
            style: props.theme_css.clone(),
            div { class: "bg-primary size-2 rounded-full" }
            div { class: "bg-secondary size-2 rounded-full" }
            div { class: "bg-warning size-2 rounded-full" }
            div { class: "bg-success size-2 rounded-full" }
          }
          div { class: "line-clamp-1 w-60", {props.name.clone()} }
        }
        // Dropdown for actions
        if !props.is_built_in {
          div { class: "dropdown dropdown-left dropdown-center",
            div {
              class: "btn btn-ghost",
              tabindex: "0",
              role: "button",
              Ellipsis { class: "w-4 h-4" }
            }
            ul {
              class: "dropdown-content menu bg-base-100 rounded-box z-1 p-2 shadow-sm",
              tabindex: "0",
              li {
                a { onclick: props.on_edit,
                  Pencil { class: "w-4 h-4 mr-1" }
                  "Edit"
                }
              }
              // li {
              //   a { href: "", class: "disabled",
              //     Share2 { class: "w-4 h-4 mr-1" }
              //     "Share"
              //   }
              // }
              li {
                a {
                  class: "text-error",
                  onclick: props.on_delete,
                  Trash2 { class: "w-4 h-4 mr-1" }
                  "Delete"
                }
              }
            }
          }
        }
      }
    }
}

#[derive(Props, Clone, PartialEq)]
struct CreateThemeButtonProps {
    editing_theme_id: Signal<Option<String>>,
}

#[component]
fn CreateThemeButton(props: CreateThemeButtonProps) -> Element {
    rsx! {
      div { class: "flex justify-start gap-2 mt-4",
        button {
          class: "btn btn-neutral btn-sm",
          onclick: {
              let mut editing_theme_id = props.editing_theme_id;
              move |_| {
                  editing_theme_id.set(None);
                  eval("theme_creator_modal.showModal()");
              }
          },
          Palette { class: "w-4 h-4 mr-1" }
          "Create"
        }
        // Disabled button for adding themes
        // This is just a placeholder for future functionality
        div { class: "tooltip", "data-tip": "Coming soon!",
          button { class: "btn btn-ghost btn-sm", disabled: true,
            Plus { class: "w-4 h-4 mr-1" }
            "Add"
          }
        }
        a {
          class: "btn btn-ghost btn-sm",
          href: "https://mechvibes.com/themes?utm_source=mechvibes&utm_medium=app&utm_campaign=theme_creator",
          target: "_blank",
          rel: "noopener noreferrer",
          "Browse themes"
          ExternalLink { class: "w-4 h-4 ml-1" }
        }
        ThemeCreatorModal { editing_theme_id: props.editing_theme_id }
      }
    }
}

#[derive(Props, Clone, PartialEq)]
struct ThemeCreatorModalProps {
    editing_theme_id: Signal<Option<String>>,
}

const THEME_DEFAULT_CSS: &str =
    r#"/* Colors */
--color-base-100: oklch(98% 0.02 240);
--color-base-200: oklch(95% 0.03 240);
--color-base-300: oklch(92% 0.04 240);
--color-base-content: oklch(20% 0.05 240);
--color-primary: oklch(55% 0.3 240);
--color-primary-content: oklch(98% 0.01 240);
--color-secondary: oklch(70% 0.25 200);
--color-secondary-content: oklch(98% 0.01 200);
--color-accent: oklch(65% 0.25 160);
--color-accent-content: oklch(98% 0.01 160);
--color-neutral: oklch(50% 0.05 240);
--color-neutral-content: oklch(98% 0.01 240);
--color-success: oklch(65% 0.25 140);
--color-success-content: oklch(98% 0.01 140);
--color-warning: oklch(80% 0.25 80);
--color-warning-content: oklch(20% 0.05 80);
--color-error: oklch(65% 0.3 30);
--color-error-content: oklch(98% 0.01 30);

/* border radius */
--radius-selector: 1rem;
--radius-field: 0.25rem;
--radius-box: 0.5rem;

/* base sizes */
--size-selector: 0.25rem;
--size-field: 0.25rem;

/* border size */
--border: 1px;

/* effects */
--depth: 1;
--noise: 0;
          "#;

#[component]
fn ThemeCreatorModal(props: ThemeCreatorModalProps) -> Element {
    let (themes, update_themes) = use_themes();
    let mut theme_name = use_signal(String::new);
    let mut theme_css = use_signal(|| String::from(THEME_DEFAULT_CSS));
    let mut saving = use_signal(|| false);
    let mut error = use_signal(String::new);

    // Load existing theme data when editing
    use_effect(move || {
        if let Some(editing_id) = props.editing_theme_id.read().as_ref() {
            if let Some(theme_data) = themes().get_theme_by_id(editing_id) {
                theme_name.set(theme_data.name.clone());
                theme_css.set(theme_data.css.clone());
            }
        } else {
            // Reset for new theme
            theme_name.set(String::new());
            theme_css.set(String::from(THEME_DEFAULT_CSS));
        }
    });

    let is_editing = props.editing_theme_id.read().is_some();
    let on_save = {
        let theme_name = theme_name.clone();
        let theme_css = theme_css.clone();
        let update_themes = update_themes.clone();
        let mut editing_theme_id = props.editing_theme_id;

        move |_| {
            let name = theme_name().trim().to_string();
            let css = theme_css();

            if name.is_empty() {
                error.set("Theme name cannot be empty".to_string());
                return;
            }

            saving.set(true);
            error.set(String::new());

            if let Some(editing_id) = editing_theme_id.read().as_ref() {
                // Update existing theme
                let editing_id = editing_id.clone();
                update_themes(
                    Box::new(move |themes: &mut ThemesConfig| {
                        if let Err(e) = themes.update_theme(&editing_id, name, "".to_string(), css) {
                            crate::always_eprint!("Failed to update theme: {}", e);
                        }
                    })
                );
            } else {
                // Create new theme
                update_themes(
                    Box::new(move |themes: &mut ThemesConfig| {
                        if let Err(e) = themes.add_theme(name, "".to_string(), css) {
                            crate::always_eprint!("Failed to create theme: {}", e);
                        }
                    })
                );
            }

            saving.set(false);
            editing_theme_id.set(None); // Clear editing state
            eval("theme_creator_modal.close()");
        }
    };

    rsx! {
      dialog { class: "modal", id: "theme_creator_modal",
        div { class: "modal-box",
          form { method: "dialog",
            button { class: "btn btn-sm btn-circle btn-ghost absolute right-2 top-2",
              "✕"
            }
          }
          h3 { class: "font-bold text-lg mb-4",
            if is_editing {
              "Edit custom theme"
            } else {
              "Create custom theme"
            }
          }
          div { class: "space-y-4",
            fieldset { class: "fieldset",
              legend { class: "fieldset-legend",
                span { class: "label-text", "Theme name" }
              }
              input {
                class: "input w-full",
                r#type: "text",
                placeholder: "My custom theme",
                value: theme_name(),
                oninput: move |e| theme_name.set(e.value()),
              }
            }
            fieldset { class: "fieldset",
              legend { class: "fieldset-legend", "CSS" }
              textarea {
                class: "textarea w-full h-64 font-mono text-sm",
                placeholder: "Enter your theme CSS here...",
                value: theme_css(),
                oninput: move |e| theme_css.set(e.value()),
              }
              div { class: "label", "Use DaisyUI CSS variables to style your theme" }
            }
            if !error().is_empty() {
              div { class: "alert alert-error", {error()} }
            }
            div { class: "flex justify-end gap-2",
              button {
                class: "btn btn-primary btn-sm",
                disabled: saving() || theme_name().trim().is_empty(),
                onclick: on_save,
                if saving() {
                  span { class: "loading loading-spinner loading-sm mr-2" }
                  if is_editing {
                    "Updating..."
                  } else {
                    "Creating..."
                  }
                } else {
                  if is_editing {
                    "Update theme"
                  } else {
                    "Create theme"
                  }
                }
              }
            }
          }
        }
        form { method: "dialog", class: "modal-backdrop",
          button { "close" }
        }
      }
    }
}
