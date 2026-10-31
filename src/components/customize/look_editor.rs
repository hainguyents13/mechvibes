//! Every change the Customize page makes to the look on screen.
//!
//! An edit is written to the config at once, so the screen follows it. The
//! look is saved as a theme a moment after the last edit (`SYNC_DELAY`), so
//! typing a color saves once, with the last value. Putting a card on first
//! saves an edit still waiting and the look on screen, so switching never
//! loses a look. What saving does is in `utils::theme_library`.

use crate::libs::theme::{ BuiltInTheme, Theme };
use crate::state::config::AppConfig;
use crate::state::config_writer;
use crate::state::saved_themes::{ SavedTheme, SavedThemes };
use crate::utils::config::use_config;
use crate::utils::look_undo::{ self, LookFields };
use crate::utils::notice::{ self, NoticeKind };
use crate::utils::theme;
use crate::utils::theme_library::{ self, colors_choice, shows_default, SyncDebounce, DEFAULT_NAME };
use crate::utils::theme_share::Look;
use dioxus::prelude::*;
use once_cell::sync::Lazy;
use std::cell::Cell;
use std::rc::Rc;
use std::sync::{ Mutex, MutexGuard };
use std::time::{ Duration, Instant };

/// How often the window checks whether a burst of edits is over.
const SYNC_CHECK_EVERY: Duration = Duration::from_millis(50);

/// The edits waiting to be saved. One for the whole window, so a card put on
/// from one tab first saves the edits made in another.
static WAITING: Lazy<Mutex<SyncDebounce>> = Lazy::new(Default::default);

fn waiting() -> MutexGuard<'static, SyncDebounce> {
    WAITING.lock().unwrap_or_else(|e| e.into_inner())
}

/// A card the user can put on.
#[derive(Debug, Clone, PartialEq)]
pub enum Card {
    /// The look Mechvibes starts with.
    Default,
    Saved(SavedTheme),
}

impl Card {
    /// The look putting this card on shows, and the card the config then
    /// points at.
    fn look_and_pointer(&self) -> (Look, Option<String>) {
        match self {
            Card::Default => (Look::default(), None),
            Card::Saved(saved) => (saved.look(), Some(saved.id.clone())),
        }
    }
}

/// Changes the look on screen. Get one with `use_look_editor`.
#[derive(Clone)]
pub struct LookEditor {
    update_config: Rc<dyn Fn(Box<dyn FnOnce(&mut AppConfig)>)>,
}

pub fn use_look_editor() -> LookEditor {
    let (_, update_config) = use_config();
    LookEditor { update_config }
}

impl LookEditor {
    /// Change the look. The screen follows at once; the look is saved as a
    /// theme once the edits stop for a moment.
    pub fn edit(&self, change: impl FnOnce(&mut AppConfig) + 'static) {
        // Edits that start from the Default theme save as "Default (my copy)".
        let name_hint = shows_default(&config_writer::current()).then(|| DEFAULT_NAME.to_string());
        (self.update_config)(Box::new(change));
        waiting().note_edit(Instant::now(), name_hint);
    }

    /// Put a whole card on: a part it does not have is switched off, and its
    /// settings are kept. The look from before is saved first and kept for
    /// Undo. Returns whether the look changed, so the caller offers Undo only
    /// then. `Err` when the card's colors could not be added to the colors
    /// list; the user has been told and nothing changed.
    pub fn apply_card(&self, card: &Card) -> Result<bool, String> {
        save_look_on_screen();
        let theme = match card {
            Card::Default => Theme::BuiltIn(BuiltInTheme::System),
            Card::Saved(saved) => saved_colors(saved)?,
        };
        let changed = Rc::new(Cell::new(false));
        (self.update_config)(
            Box::new({
                let (card, changed) = (card.clone(), changed.clone());
                move |cfg| {
                    let before = LookFields::of(cfg);
                    put_card_on(&card, theme, cfg);
                    changed.set(look_undo::remember(before, LookFields::of(cfg)));
                }
            })
        );
        Ok(changed.get())
    }
}

/// What putting `card` on writes, with `theme` as the theme its colors became.
fn put_card_on(card: &Card, theme: Theme, config: &mut AppConfig) {
    let (look, pointer) = card.look_and_pointer();
    look.write_to(theme, config);
    config.active_theme_id = pointer;
}

/// The theme a saved card's colors take, added to the colors list if needed.
fn saved_colors(saved: &SavedTheme) -> Result<Theme, String> {
    let chosen = refreshing_colors_list(|| {
        theme::change_themes_config(|themes| colors_choice(&saved.colors, &saved.name, themes))
    });
    match chosen {
        Ok(Ok(theme)) => Ok(theme),
        Ok(Err(e)) => {
            notice::report(NoticeKind::Error, format!("Could not put {} on. {}", saved.name, e), None);
            Err(e)
        }
        // A failed save of the colors list has told the user already.
        Err(e) => Err(e),
    }
}

/// Save an edit still waiting, then the look on screen, so that another
/// look can be put on without losing this one.
pub fn save_look_on_screen() {
    let burst = waiting().take_now();
    if let Some(name_hint) = burst {
        sync(name_hint.as_deref());
    }
    sync(None);
}

/// `run_sync`, with the colors list shown again when it put colors back.
fn sync(name_hint: Option<&str>) {
    refreshing_colors_list(|| theme_library::run_sync(name_hint));
}

/// Run `change`, then show the colors list again if it changed the list
/// without telling the window.
fn refreshing_colors_list<R>(change: impl FnOnce() -> R) -> R {
    let before = theme::get_themes_config();
    let result = change();
    if theme::get_themes_config() != before {
        theme::refresh_themes();
    }
    result
}

/// Saves the look once a burst of edits is over. Mounted once at the root, so
/// leaving a tab or the Customize page never drops a save.
pub fn use_save_after_edits() {
    use_future(|| async {
        loop {
            futures_timer::Delay::new(SYNC_CHECK_EVERY).await;
            let due = waiting().take_due(Instant::now());
            if let Some(name_hint) = due {
                sync(name_hint.as_deref());
            }
        }
    });
}

/// The saved themes, as the window shows them. Provided at the root, which
/// shows the list again whenever it changes.
pub fn use_saved_themes() -> Signal<SavedThemes> {
    use_context()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::saved_themes::ThemeKind;
    use crate::state::config::LogoCustomization;
    use crate::utils::theme_share::SharedColors;
    use chrono::Utc;
    use serde_json::Map;

    #[test]
    fn the_default_look_keeps_your_logo_and_background_settings() {
        let mut config = AppConfig::default();
        config.theme = Theme::BuiltIn(BuiltInTheme::Synthwave);
        config.enable_logo_customization = true;
        config.logo_customization.text_color = "red".to_string();
        config.enable_background_customization = true;
        config.background_customization.background_color = "#202020".to_string();
        config.active_theme_id = Some("my-theme".to_string());
        let logo = config.logo_customization.clone();
        let background = config.background_customization.clone();
        assert!(!shows_default(&config));

        put_card_on(&Card::Default, Theme::BuiltIn(BuiltInTheme::System), &mut config);
        assert!(shows_default(&config));
        assert_eq!(config.logo_customization, logo, "the logo settings are kept for later");
        assert_eq!(config.background_customization, background, "and so are the background ones");
    }

    #[test]
    fn a_saved_card_goes_on_whole_and_is_pointed_at() {
        let now = Utc::now();
        let logo = LogoCustomization { text_color: "#ff0000".to_string(), ..LogoCustomization::default() };
        let saved = SavedTheme {
            id: "night".to_string(),
            name: "Night drive".to_string(),
            kind: ThemeKind::Mine,
            library_id: None,
            library_checksum: None,
            fingerprint: None,
            colors: SharedColors::BuiltIn(BuiltInTheme::Dark),
            logo: Some(logo.clone()),
            background: None,
            created_at: now,
            updated_at: now,
            extra: Map::new(),
        };
        let mut config = AppConfig::default();
        config.enable_background_customization = true;
        config.background_customization.background_color = "#202020".to_string();
        let background = config.background_customization.clone();

        put_card_on(&Card::Saved(saved), Theme::BuiltIn(BuiltInTheme::Dark), &mut config);
        assert_eq!(config.theme, Theme::BuiltIn(BuiltInTheme::Dark));
        assert_eq!(config.active_theme_id.as_deref(), Some("night"));
        assert!(config.enable_logo_customization);
        assert_eq!(config.logo_customization, logo);
        assert!(!config.enable_background_customization, "the card has no background, so yours goes off");
        assert_eq!(config.background_customization, background, "and waits for later");
    }
}
