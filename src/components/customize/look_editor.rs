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
use crate::state::saved_themes::{ self, SavedTheme, SavedThemes };
use crate::state::themes::ThemesConfig;
use crate::utils::config::use_config;
use crate::utils::look_undo::{ self, LookFields };
use crate::utils::notice::{ self, NoticeKind };
use crate::utils::starter_themes::{ self, StarterTheme };
use crate::utils::theme;
use crate::utils::theme_library::{
    self,
    colors_choice,
    run_sync_in,
    same_look,
    shows_default,
    AppPlace,
    SyncCause,
    SyncDebounce,
    SyncPlace,
    DEFAULT_NAME,
};
use crate::utils::theme_share::{ Look, SharedColors };
use chrono::Utc;
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
    /// A theme that comes with the app.
    Starter(&'static StarterTheme),
}

impl Card {
    pub fn name(&self) -> &str {
        match self {
            Card::Default => DEFAULT_NAME,
            Card::Saved(saved) => &saved.name,
            Card::Starter(starter) => starter.name(),
        }
    }

    /// What `active_theme_id` holds while this card is on.
    pub fn pointer(&self) -> Option<String> {
        match self {
            Card::Default => None,
            Card::Saved(saved) => Some(saved.id.clone()),
            Card::Starter(starter) => Some(starter.pointer()),
        }
    }

    /// Whether the look on screen is this card.
    pub fn is_on(&self, config: &AppConfig) -> bool {
        match self {
            Card::Default => shows_default(config),
            _ => config.active_theme_id.is_some() && config.active_theme_id == self.pointer(),
        }
    }

    /// The look the card's picture shows. A starter's images are shown from
    /// the app, since they are saved only when it is put on.
    pub fn preview_look(&self) -> Look {
        match self {
            Card::Starter(starter) => starter.preview_look(),
            _ => self.look(),
        }
    }

    /// The look putting this card on shows.
    fn look(&self) -> Look {
        match self {
            Card::Default => Look::default(),
            Card::Saved(saved) => saved.look(),
            Card::Starter(starter) => starter.look(),
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
    /// Undo (see `switch_card`). Returns whether the look changed, so the
    /// caller offers Undo only then. `Err` when the look from before could
    /// not be saved, or the card's colors could not be added to the colors
    /// list; the user has been told and nothing changed.
    pub fn apply_card(&self, card: &Card) -> Result<bool, String> {
        let burst = waiting().take_now();
        switch_card(&mut window_place(), burst, card, |_, card| self.put_on_now(card))
    }

    /// Put `card` on right away, its colors and images first.
    fn put_on_now(&self, card: &Card) -> Result<bool, String> {
        let theme = match card {
            Card::Default => Theme::BuiltIn(BuiltInTheme::System),
            Card::Saved(saved) => card_colors(&saved.colors, &saved.name)?,
            Card::Starter(starter) => {
                // Its images go where its look says they are, before it is on.
                if let Err(e) = starter.save_images() {
                    notice::report(NoticeKind::Error, format!("Could not put {} on. {}", starter.name(), e), None);
                    return Err(e);
                }
                card_colors(&starter.theme.colors, starter.name())?
            }
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

    /// Delete a saved theme. Its images and colors stay (see
    /// `theme_library::delete_theme`). When it is the theme on screen, the
    /// Default theme goes on first, with no Undo, since the theme it would go
    /// back to is gone. An edit still waiting is saved first, so an edit to
    /// an installed theme lives on in its copy, and nothing is deleted when
    /// it could not be saved.
    pub fn delete_theme(&self, id: &str) -> Result<(), String> {
        save_look_on_screen()?;
        refuse_while_read_only()?;
        let id = id.to_string();
        (self.update_config)(
            Box::new({
                let id = id.clone();
                move |cfg| theme_library::after_deleting_theme(cfg, &id)
            })
        );
        saved_themes::apply(|store| {
            theme_library::delete_theme(store, &id);
        }).map(|_| ())
    }
}

/// Save a copy of `card` as a theme of the user's own, "<name> (copy)". The
/// copy is not put on. An edit still waiting is saved first, so a copy of the
/// theme on screen has it. A starter's images are saved first too, since the
/// copy points where putting the starter on saves them.
pub fn duplicate(card: &Card) -> Result<(), String> {
    save_look_on_screen()?;
    refuse_while_read_only()?;
    let card = &as_saved_now(card, &saved_themes::current());
    if let Card::Starter(starter) = card {
        if let Err(e) = starter.save_images() {
            notice::report(NoticeKind::Error, format!("Could not copy {}. {}", starter.name(), e), None);
            return Err(e);
        }
    }
    let name = theme_library::duplicate_name(card.name(), &theme_library::cards(&saved_themes::current()));
    let Some(copy) = theme_library::new_mine(name, card.look(), Utc::now()) else {
        return Err("This theme has no colors to copy.".to_string());
    };
    saved_themes::apply(|store| store.insert(copy)).map(|_| ())
}

/// `Err` while the saved themes cannot be written this session. The user
/// heard why once at startup; a change they ask for tells them again.
fn refuse_while_read_only() -> Result<(), String> {
    match saved_themes::current().read_only {
        Some(read_only) => {
            notice::report(NoticeKind::Error, read_only.message(), None);
            Err(read_only.message().to_string())
        }
        None => Ok(()),
    }
}

/// The card the look on screen is, once an edit still waiting is saved.
/// `None` when the look is no card's, as while saved themes cannot be
/// written.
pub fn card_on_screen() -> Option<Card> {
    // A look that could not be saved is no card's.
    save_look_on_screen().ok()?;
    card_showing(&config_writer::current(), &theme::get_themes_config(), &saved_themes::current())
}

/// The card `config` points at, if the look it shows is that card's, or the
/// Default card when it shows the Default theme.
fn card_showing(config: &AppConfig, themes: &ThemesConfig, store: &SavedThemes) -> Option<Card> {
    if shows_default(config) {
        return Some(Card::Default);
    }
    let pointer = config.active_theme_id.as_deref()?;
    let card = store
        .themes()
        .find(|saved| saved.id == pointer)
        .cloned()
        .map(Card::Saved)
        .or_else(|| {
            starter_themes
                ::all()
                .iter()
                .find(|starter| starter.pointer() == pointer)
                .map(Card::Starter)
        })?;
    same_look(&Look::from_config(config, themes), &card.look()).then_some(card)
}

/// `card` as the saved themes hold it now. A sync may have saved an edit
/// into it since the gallery showed it, and putting the old look on would
/// undo that edit. A saved theme gone from the list stays as it was.
fn as_saved_now(card: &Card, store: &SavedThemes) -> Card {
    match card {
        Card::Saved(saved) =>
            store
                .themes()
                .find(|now| now.id == saved.id)
                .map(|now| Card::Saved(now.clone()))
                .unwrap_or_else(|| card.clone()),
        _ => card.clone(),
    }
}

/// Put `card` on with `put_on`, once the look on screen is saved: the edits
/// in `burst` first, then the look itself, so switching never loses a look.
/// Nothing goes on when a save failed. The card already on screen is left
/// as it is, since an edit waiting for it has just been saved into it, and
/// any other card is put on as the saved themes hold it now.
fn switch_card<P: SyncPlace>(
    place: &mut P,
    burst: Option<Option<String>>,
    card: &Card,
    put_on: impl FnOnce(&mut P, &Card) -> Result<bool, String>
) -> Result<bool, String> {
    save_look_in(place, burst)?;
    if card.is_on(&place.config()) {
        return Ok(false);
    }
    let card = as_saved_now(card, &place.store());
    put_on(place, &card)
}

/// What putting `card` on writes, with `theme` as the theme its colors became.
fn put_card_on(card: &Card, theme: Theme, config: &mut AppConfig) {
    card.look().write_to(theme, config);
    config.active_theme_id = card.pointer();
}

/// The theme a card's colors take, added to the colors list if needed.
fn card_colors(colors: &SharedColors, name: &str) -> Result<Theme, String> {
    let chosen = refreshing_colors_list(|| {
        theme::change_themes_config(|themes| colors_choice(colors, name, themes))
    });
    match chosen {
        Ok(Ok(theme)) => Ok(theme),
        Ok(Err(e)) => {
            notice::report(NoticeKind::Error, format!("Could not put {} on. {}", name, e), None);
            Err(e)
        }
        // A failed save of the colors list has told the user already.
        Err(e) => Err(e),
    }
}

/// Save an edit still waiting, then the look on screen, so that another
/// look can be put on without losing this one. `Err` when it could not be
/// saved; the user has been told.
pub fn save_look_on_screen() -> Result<(), String> {
    let burst = waiting().take_now();
    save_look_in(&mut window_place(), burst)
}

/// Save the edits in `burst`, then the look on screen. Stops at the first
/// save that failed.
fn save_look_in(place: &mut impl SyncPlace, burst: Option<Option<String>>) -> Result<(), String> {
    if let Some(name_hint) = burst {
        run_sync_in(place, SyncCause::Edits { name_hint: name_hint.as_deref() })?;
    }
    run_sync_in(place, SyncCause::Check)
}

/// The app's own place, with the colors list shown again when a sync puts
/// colors back.
fn window_place() -> AppPlace {
    AppPlace { refresh_window: true }
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
                // A failed save has told the user; the next edit tries again.
                let _ = run_sync_in(&mut window_place(), SyncCause::Edits { name_hint: name_hint.as_deref() });
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
    use crate::utils::theme_library::MemoryPlace;
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

    #[test]
    fn a_starter_goes_on_whole_and_is_pointed_at() {
        let starter = &crate::utils::starter_themes::all()[0];
        let card = Card::Starter(starter);
        let mut config = AppConfig::default();
        config.enable_background_customization = true;
        assert!(!card.is_on(&config));

        put_card_on(&card, Theme::Custom("beige".to_string()), &mut config);
        assert_eq!(config.theme, Theme::Custom("beige".to_string()));
        assert_eq!(config.active_theme_id.as_deref(), Some("starter:retro-beige"));
        assert!(card.is_on(&config));
        assert!(!Card::Default.is_on(&config));
        let look = starter.look();
        assert_eq!(config.enable_logo_customization.then(|| config.logo_customization.clone()), look.logo);
        assert_eq!(config.enable_background_customization, look.background.is_some());
    }

    #[test]
    fn the_current_theme_shares_the_card_on_screen() {
        let (store, mut themes) = (SavedThemes::default(), ThemesConfig::default());
        let mut config = AppConfig::default();
        assert_eq!(card_showing(&config, &themes, &store), Some(Card::Default));

        // A starter on screen is shared as the file that comes with the app.
        let starter = &crate::utils::starter_themes::all()[0];
        let colors = colors_choice(&starter.theme.colors, starter.name(), &mut themes).unwrap();
        put_card_on(&Card::Starter(starter), colors, &mut config);
        assert_eq!(card_showing(&config, &themes, &store), Some(Card::Starter(starter)));

        // An edit not saved as a theme yet is no card's look.
        config.enable_logo_customization = !config.enable_logo_customization;
        assert_eq!(card_showing(&config, &themes, &store), None);
        config.active_theme_id = Some("gone".to_string());
        assert_eq!(card_showing(&config, &themes, &store), None);
    }

    /// Dark colors and a logo with this border.
    fn logo_look(border: &str) -> Look {
        Look {
            colors: crate::utils::theme_share::LookColors::Known(SharedColors::BuiltIn(BuiltInTheme::Dark)),
            logo: Some(LogoCustomization { border_color: border.to_string(), ..LogoCustomization::default() }),
            background: None,
        }
    }

    /// A place showing the first starter with its logo border edited, the
    /// edit still waiting to be saved.
    fn edited_starter() -> (MemoryPlace, Option<Option<String>>) {
        let starter = &crate::utils::starter_themes::all()[0];
        let mut place = MemoryPlace::default();
        let colors = colors_choice(&starter.theme.colors, starter.name(), &mut place.themes).unwrap();
        put_card_on(&Card::Starter(starter), colors, &mut place.config);
        place.config.logo_customization.border_color = "#ff0000".to_string();
        (place, Some(None))
    }

    #[test]
    fn putting_a_card_on_saves_the_waiting_edits_first() {
        let (mut place, burst) = edited_starter();
        let edited = Look::from_config(&place.config, &place.themes);
        let mut put = false;
        let changed = switch_card(&mut place, burst, &Card::Default, |place, card| {
            let copy = place.config.active_theme_id.clone().expect("the edit points at its copy");
            let saved = place.store.themes().find(|saved| saved.id == copy).expect("the copy is saved");
            assert_eq!(saved.name, "Retro Beige (my copy)");
            assert!(same_look(&saved.look(), &edited), "with the edit in it");
            put_card_on(card, Theme::BuiltIn(BuiltInTheme::System), &mut place.config);
            put = true;
            Ok(true)
        });
        assert_eq!(changed, Ok(true));
        assert!(put && shows_default(&place.config));
        assert_eq!(place.store.themes().count(), 1);
    }

    #[test]
    fn a_card_never_goes_on_over_a_look_that_could_not_be_saved() {
        let (mut place, burst) = edited_starter();
        place.store_fails = true;
        let before = place.config.clone();
        let changed = switch_card(&mut place, burst, &Card::Default, |_, _| panic!("the card went on"));
        assert!(changed.is_err());
        assert_eq!(place.config, before, "the edited look stays on screen");
    }

    #[test]
    fn clicking_the_card_on_screen_keeps_the_edit_waiting_for_it() {
        let theme = theme_library::new_mine("Night drive".to_string(), logo_look("red"), Utc::now()).unwrap();
        let mut place = MemoryPlace::default();
        place.store.insert(theme.clone());
        // The gallery still shows the card as it was before the edit.
        logo_look("blue").write_to(Theme::BuiltIn(BuiltInTheme::Dark), &mut place.config);
        place.config.active_theme_id = Some(theme.id.clone());

        let changed = switch_card(&mut place, Some(None), &Card::Saved(theme.clone()), |_, _| panic!("put on again"));
        assert_eq!(changed, Ok(false));
        assert_eq!(place.store.themes().next().unwrap().look(), logo_look("blue"), "the edit is saved in it");
        assert_eq!(Look::from_config(&place.config, &place.themes), logo_look("blue"), "and stays on screen");
    }

    #[test]
    fn a_saved_card_goes_on_as_it_is_saved_now() {
        let shown = theme_library::new_mine("Night drive".to_string(), logo_look("red"), Utc::now()).unwrap();
        let mut place = MemoryPlace::default();
        place.store.insert(SavedTheme { logo: logo_look("blue").logo, ..shown.clone() });

        let mut worn = None;
        let changed = switch_card(&mut place, None, &Card::Saved(shown), |_, card| {
            worn = Some(card.look());
            Ok(true)
        });
        assert_eq!(changed, Ok(true));
        assert_eq!(worn, Some(logo_look("blue")), "an edit saved into it since the gallery showed it is kept");
    }

    #[test]
    fn only_the_card_on_screen_is_on() {
        let mut config = AppConfig::default();
        assert!(Card::Default.is_on(&config));
        config.active_theme_id = Some("starter:retro-beige".to_string());
        assert!(!Card::Default.is_on(&config), "the default look under a pointer is that card");
        assert_eq!(Card::Default.name(), "Default");
        assert_eq!(Card::Default.pointer(), None);
    }
}
