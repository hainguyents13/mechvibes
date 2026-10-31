use crate::state::themes::ThemesConfig;
use crate::utils::notice::{ self, NoticeKind };
use dioxus::prelude::*;
use once_cell::sync::Lazy;
use std::rc::Rc;
use std::sync::{ Arc, Mutex, MutexGuard };

static THEMES_CONFIG: Lazy<Arc<Mutex<ThemesConfig>>> = Lazy::new(||
    Arc::new(Mutex::new(ThemesConfig::load()))
);

/// Global signal to trigger refresh of all theme components
static REFRESH_TRIGGER: GlobalSignal<u32> = Signal::global(|| 0);

/// Changes the colors list and saves it. Gives the save error back, after
/// putting the list in memory back as it was and telling the user.
pub type UpdateThemes = Rc<dyn Fn(Box<dyn FnOnce(&mut ThemesConfig)>) -> Result<(), String>>;

/// The shared list. A lock left poisoned by a panic still holds a whole
/// list, so it is used rather than taking the window down with it.
fn themes_config() -> MutexGuard<'static, ThemesConfig> {
    THEMES_CONFIG.lock().unwrap_or_else(|e| e.into_inner())
}

/// Hook for accessing and updating themes configuration
pub fn use_themes() -> (Signal<ThemesConfig>, UpdateThemes) {
    // Load initial themes config
    let mut themes = use_signal(|| themes_config().clone());

    // Watch for refresh trigger changes
    use_effect(move || {
        let _trigger_value = REFRESH_TRIGGER();
        // When trigger changes, reload themes from global config
        themes.set(themes_config().clone());
    });

    let update_themes: UpdateThemes = Rc::new(|updater: Box<dyn FnOnce(&mut ThemesConfig)>| {
        let saved = apply_and_save(&mut themes_config(), updater, ThemesConfig::save);
        if let Err(e) = &saved {
            crate::always_eprint!("❌ Failed to save themes: {}", e);
            notice::report(NoticeKind::Error, format!("Could not save your changes. {}", e), None);
            return saved;
        }
        // Trigger refresh of all components using themes
        let current = REFRESH_TRIGGER();
        *REFRESH_TRIGGER.write() = current + 1;
        saved
    });

    (themes, update_themes)
}

/// Run `mutate` on `themes`, then `save` them. When the save fails the list
/// goes back to what it was, so memory never shows a change the file does
/// not have.
fn apply_and_save(
    themes: &mut ThemesConfig,
    mutate: impl FnOnce(&mut ThemesConfig),
    save: impl FnOnce(&ThemesConfig) -> Result<(), String>
) -> Result<(), String> {
    let before = themes.clone();
    mutate(themes);
    save(themes).inspect_err(|_| {
        *themes = before;
    })
}

/// Get a reference to the global themes config (read-only)
pub fn get_themes_config() -> ThemesConfig {
    themes_config().clone()
}

/// Change the shared colors list outside a window and save it when `mutate`
/// changed something. A failed save puts the list back and tells the user.
/// Windows already showing the list are not refreshed, so this is for work
/// done before the window is up.
pub fn change_themes_config<R>(mutate: impl FnOnce(&mut ThemesConfig) -> R) -> Result<R, String> {
    let mut themes = themes_config();
    let before = themes.clone();
    let result = mutate(&mut themes);
    if *themes != before {
        if let Err(e) = themes.save() {
            *themes = before;
            crate::always_eprint!("❌ Failed to save themes: {}", e);
            notice::report(NoticeKind::Error, format!("Could not save your changes. {}", e), None);
            return Err(e);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_save_leaves_the_list_as_it_was() {
        let mut themes = ThemesConfig::default();
        themes.import_theme("Night", "--color-primary: red;").unwrap();
        let before = themes.clone();

        let result = apply_and_save(
            &mut themes,
            |themes| {
                themes.import_theme("Day", "--color-primary: blue;").unwrap();
            },
            |_| Err("the disk is full".to_string())
        );
        assert_eq!(result, Err("the disk is full".to_string()));
        assert_eq!(themes, before);

        let result = apply_and_save(
            &mut themes,
            |themes| {
                themes.import_theme("Day", "--color-primary: blue;").unwrap();
            },
            |_| Ok(())
        );
        assert_eq!(result, Ok(()));
        assert_eq!(themes.custom_themes.len(), 2);
    }
}
