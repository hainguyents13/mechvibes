//! Sharing the look on screen, picking a theme file to import, and putting
//! an opened look on.
//!
//! Share saves the theme colours, the logo and the background, images
//! included, to one `.mvtheme` file. Import hands the file it picks to the
//! import dialog at the root (`ui::theme_import_modal`), which shows what the
//! file holds and puts it on only after the user agrees.

use crate::components::customize::look_editor;
use crate::libs::theme::Theme;
use crate::state::config::AppConfig;
use crate::state::config_writer;
use crate::state::saved_themes;
use crate::state::themes::ThemesConfig;
use crate::utils::look_undo::{ self, LookFields };
use crate::utils::notice::{ self, NoticeAction, NoticeKind };
use crate::utils::theme::UpdateThemes;
use crate::utils::theme_library::{ self, ThemeSource };
use crate::utils::theme_share::{ self, ImportParts, Look, SharedColors, SharedTheme, FILE_EXTENSION };
use dioxus::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

/// Install an opened theme and put its ticked `parts` on. Every part's images
/// are installed and the theme gets a card holding all of it (see
/// `theme_library::add_installed`), so an unticked part only stays off the
/// look on screen. Then the config takes the ticked parts and points at the
/// card. Gives the theme's name back.
///
/// The look from before is saved as a theme first and remembered, so
/// `undo_look` can put it back. When the look on screen ends up mixing the
/// theme with parts of the user's own, it is saved as "<name> (my copy)" and
/// the installed card stays as the file has it. Nothing is installed while
/// the saved themes cannot be written.
pub async fn put_on(
    mut theme: SharedTheme,
    parts: ImportParts,
    source: ThemeSource,
    update_config: Rc<dyn Fn(Box<dyn FnOnce(&mut AppConfig)>)>,
    update_themes: UpdateThemes,
    mut theme_signal: Signal<Theme>
) -> Result<String, String> {
    look_editor::save_look_on_screen();
    if let Some(read_only) = saved_themes::current().read_only {
        return Err(read_only.message().to_string());
    }
    let (full, written) = tokio::task
        ::spawn_blocking(move || theme_share::install_images(&mut theme).map(|written| (theme, written))).await
        .unwrap_or_else(|_| Err("Importing stopped unexpectedly.".to_string()))?;

    let installed = theme_library::add_installed(
        full,
        &written,
        parts,
        &source,
        |colors, name| add_colors(&update_themes, colors, name),
        |upsert| saved_themes::apply(|store| upsert(store)).map(|_| ())
    )?;

    if let Some(choice) = &installed.colors {
        theme_signal.set(choice.clone());
    }
    let name = installed.shown.name.clone();
    // `before` is read inside the write, so it is exactly what the config
    // held right before this theme went on.
    let before = Rc::new(RefCell::new(None));
    update_config(
        Box::new({
            let before = before.clone();
            move |cfg| {
                *before.borrow_mut() = Some(LookFields::of(cfg));
                theme_share::apply_to_config(&installed.shown, installed.colors, parts, cfg);
                cfg.active_theme_id = Some(installed.card);
            }
        })
    );
    // A look that differs from the card is saved as a copy, before Undo
    // remembers what this step left on screen.
    look_editor::save_look_on_screen();
    if let Some(before) = before.borrow_mut().take() {
        look_undo::remember(before, LookFields::of(&config_writer::current()));
    }
    Ok(name)
}

/// The theme `colors` take, added to the colors list under `name` unless the
/// list has them already.
fn add_colors(update_themes: &UpdateThemes, colors: &SharedColors, name: &str) -> Result<Theme, String> {
    let chosen = Rc::new(RefCell::new(None));
    let saved = update_themes(
        Box::new({
            let chosen = chosen.clone();
            let (colors, name) = (colors.clone(), name.to_string());
            move |themes| {
                *chosen.borrow_mut() = Some(theme_library::colors_choice(&colors, &name, themes));
            }
        })
    );
    let chosen = chosen.borrow_mut().take();
    saved.and_then(|()| chosen.unwrap_or_else(|| Err("Could not save the theme.".to_string())))
}

/// Go back to the look from before the last theme was put on, if nothing has
/// changed it since. Tells the user when there is nothing to go back to.
/// Returns whether the look went back.
pub fn undo_look(
    update_config: &Rc<dyn Fn(Box<dyn FnOnce(&mut AppConfig)>)>,
    mut theme_signal: Signal<Theme>
) -> bool {
    let restored = Rc::new(RefCell::new(None));
    update_config(
        Box::new({
            let restored = restored.clone();
            move |cfg| {
                if let Some(before) = look_undo::take_undo(&LookFields::of(cfg)) {
                    before.write_to(cfg);
                    *restored.borrow_mut() = Some(before.theme);
                }
            }
        })
    );
    let restored = restored.borrow_mut().take();
    match restored {
        Some(theme) => {
            theme_signal.set(theme);
            true
        }
        None => {
            notice::report(NoticeKind::Info, "Nothing to undo. The look changed since.", None);
            false
        }
    }
}

/// Save the look on screen to a `.mvtheme` file. The save dialog opens
/// straight away, and the theme in the file takes the name the user gives
/// the file. A toast says how it went and, once saved, offers to show the
/// file.
pub async fn share_current(config: AppConfig, themes: ThemesConfig) {
    let look = Look::from_config(&config, &themes);
    let suggested = format!("{}.{}", theme_share::default_name(&config, &themes), FILE_EXTENSION);
    let Some(handle) = rfd::AsyncFileDialog
        ::new()
        .add_filter("Mechvibes theme", &[FILE_EXTENSION])
        .set_file_name(&suggested)
        .set_title("Save theme")
        .save_file().await else {
        return;
    };
    let (path, name) = match theme_share::share_target(handle.path()) {
        Ok(target) => target,
        Err(e) => {
            notice::report(NoticeKind::Error, e, None);
            return;
        }
    };

    let result = match theme_share::gather(&name, &look).await {
        Ok(theme) => {
            let path = path.clone();
            tokio::task
                ::spawn_blocking(move || theme_share::save(&theme, &path)).await
                .unwrap_or_else(|_| Err("Saving stopped unexpectedly.".to_string()))
        }
        Err(e) => Err(e),
    };
    match result {
        Ok(()) =>
            notice::report(
                NoticeKind::Info,
                "Saved. Your friend can double-click the file to use it.",
                Some(NoticeAction::ShowFile(path))
            ),
        Err(e) => notice::report(NoticeKind::Error, e, None),
    }
}

/// Let the user pick a theme file and hand it to the import dialog at the
/// root, which reads it and asks.
pub async fn pick_theme_file() {
    if
        let Some(handle) = rfd::AsyncFileDialog
            ::new()
            .add_filter("Mechvibes theme", &[FILE_EXTENSION])
            .set_title("Import theme")
            .pick_file().await
    {
        crate::utils::deep_link::deliver_theme_file(handle.path().to_path_buf());
    }
}