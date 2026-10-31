//! Sharing the look on screen, picking a theme file to import, and putting
//! an opened look on.
//!
//! Share saves the theme colours, the logo and the background, images
//! included, to one `.mvtheme` file. Import hands the file it picks to the
//! import dialog at the root (`ui::theme_import_modal`), which shows what the
//! file holds and puts it on only after the user agrees.

use crate::libs::theme::Theme;
use crate::state::config::AppConfig;
use crate::state::themes::ThemesConfig;
use crate::utils::look_undo::{ self, LookFields };
use crate::utils::notice::{ self, NoticeAction, NoticeKind };
use crate::utils::theme::UpdateThemes;
use crate::utils::theme_share::{ self, ImportParts, Look, SharedColors, SharedTheme, FILE_EXTENSION };
use dioxus::prelude::*;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

/// Put the ticked `parts` of an opened look on: their images saved, custom
/// colours added to the list, then the theme and the rest of the config
/// switched over. An unticked part brings nothing and changes nothing. Gives
/// the look's name back. Images saved before a failure are removed again. The
/// look from before is remembered, so `undo_look` can put it back.
pub async fn put_on(
    mut look: SharedTheme,
    parts: ImportParts,
    update_config: Rc<dyn Fn(Box<dyn FnOnce(&mut AppConfig)>)>,
    update_themes: UpdateThemes,
    mut theme_signal: Signal<Theme>
) -> Result<String, String> {
    look.keep_parts(parts);
    let (look, written) = tokio::task
        ::spawn_blocking(move || theme_share::install_images(&mut look).map(|written| (look, written))).await
        .unwrap_or_else(|_| Err("Importing stopped unexpectedly.".to_string()))?;

    let choice = match &look.colors {
        _ if !parts.colors => None,
        SharedColors::BuiltIn(theme) => Some(Theme::BuiltIn(theme.clone())),
        SharedColors::Custom { css } => {
            let added = Rc::new(RefCell::new(None));
            let saved = update_themes(
                Box::new({
                    let added = added.clone();
                    let (name, css) = (look.name.clone(), css.clone());
                    move |themes| {
                        *added.borrow_mut() = Some(themes.import_theme(&name, &css));
                    }
                })
            );
            let added = saved.and_then(|()| {
                added.borrow_mut().take().unwrap_or_else(|| Err("Could not save the theme.".to_string()))
            });
            match added {
                Ok(id) => Some(Theme::Custom(id)),
                Err(e) => {
                    theme_share::remove_images(&written);
                    return Err(e);
                }
            }
        }
    };

    if let Some(choice) = &choice {
        theme_signal.set(choice.clone());
    }
    let name = look.name.clone();
    // The look is remembered inside the write, so `before` is exactly what
    // the config held right before this theme went on.
    update_config(
        Box::new(move |cfg| {
            let before = LookFields::of(cfg);
            theme_share::apply_to_config(&look, choice, parts, cfg);
            look_undo::remember(before, LookFields::of(cfg));
        })
    );
    Ok(name)
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
    let mut path: PathBuf = handle.path().to_path_buf();
    if path.extension().and_then(|e| e.to_str()) != Some(FILE_EXTENSION) {
        path.set_extension(FILE_EXTENSION);
    }
    let name = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_default();

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