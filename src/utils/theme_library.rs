//! Keeps the look on screen saved as a theme the user can switch back to.
//!
//! The config holds the look the app shows, and `active_theme_id` names the
//! card that look belongs to. At startup, after a burst of edits and before
//! another card is put on, `run_sync` compares the two. An edit to the
//! user's own theme saves it in place. An edit to any other card saves
//! "<name> (my copy)" and leaves the card as it was. A look that already has
//! a card points at it, so nothing is ever saved twice.
//!
//! `sync_action` decides and is pure. `run_sync` carries the decision out.
//!
//! A theme installed from a file or the theme library gets a card of its own
//! that holds every part of it, whatever parts the user put on. Installing
//! the same library theme again updates its card, and importing the same
//! file again reuses its card (`add_installed`).

use crate::libs::theme::{ BuiltInTheme, Theme };
use crate::state::config::AppConfig;
use crate::state::config_writer;
use crate::state::saved_themes::{ self, checked_look, SavedTheme, SavedThemes, ThemeKind };
use crate::state::themes::ThemesConfig;
use crate::utils::theme;
use crate::utils::theme_share::{ self, safe_theme_css, ImportParts, Look, LookColors, SharedColors, SharedTheme, MAX_NAME_CHARS };
use chrono::{ DateTime, Utc };
use serde_json::Map;
use std::path::PathBuf;
use std::time::{ Duration, Instant };

/// How long after the last edit the look is saved, so typing a color or
/// dragging a slider saves once, with the last value.
pub const SYNC_DELAY: Duration = Duration::from_millis(300);

/// The name hint of edits that started while the Default theme was on.
pub const DEFAULT_NAME: &str = "Default";

/// The name of a look saved when it came from no card at all.
const NEW_THEME_NAME: &str = "My theme";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardKind {
    /// The user's own. Edits change it in place.
    Mine,
    Imported,
    Library,
    /// Comes with the app.
    Starter,
}

/// A card the look on screen can belong to, with its look in the form the
/// config would hold.
#[derive(Debug, Clone, PartialEq)]
pub struct CardLook {
    /// A saved theme's id, or "starter:<slug>".
    pub pointer: String,
    pub name: String,
    pub kind: CardKind,
    pub look: Look,
    pub updated_at: DateTime<Utc>,
}

impl CardLook {
    pub fn saved(theme: &SavedTheme) -> Self {
        Self {
            pointer: theme.id.clone(),
            name: theme.name.clone(),
            kind: match theme.kind {
                ThemeKind::Mine => CardKind::Mine,
                ThemeKind::Imported => CardKind::Imported,
                ThemeKind::Library => CardKind::Library,
            },
            look: theme.look(),
            updated_at: theme.updated_at,
        }
    }
}

/// Every card the look on screen can belong to.
pub fn cards(store: &SavedThemes) -> Vec<CardLook> {
    store.themes().map(CardLook::saved).collect()
}

/// What to do so that the look on screen is never lost.
#[derive(Debug, Clone, PartialEq)]
pub enum SyncAction {
    Nothing,
    /// The colors list lost the colors on screen. Put the active card's
    /// colors back, then sync again.
    HealColors {
        colors: SharedColors,
        name: String,
    },
    /// Save the look in the user's own theme with this id.
    UpdateMine {
        id: String,
        look: Look,
    },
    /// Save the look as a new theme of the user's own and point at it.
    CreateMine {
        name: String,
        look: Look,
    },
    /// A card with exactly this look exists. Point at it.
    Point {
        pointer: String,
    },
    /// The look is the Default theme again.
    ClearPointer,
}

impl SyncAction {
    /// This action, or `Nothing` when it saves a theme and the saved themes
    /// cannot be written this session.
    pub fn unless_read_only(self, read_only: bool) -> Self {
        match self {
            SyncAction::UpdateMine { .. } | SyncAction::CreateMine { .. } if read_only => SyncAction::Nothing,
            action => action,
        }
    }
}

/// What to do so that the look on screen is never lost.
///
/// `live` is the look on screen, `pointer` the card the config says it
/// belongs to, `cards` every card there is. A pointer that names no card
/// counts as none. `name_hint` is `Some(DEFAULT_NAME)` when the edits being
/// saved started from the Default theme.
///
/// Colors missing from the colors list are never saved: the active card's
/// colors are put back instead. A look with a value a saved theme cannot
/// hold stays only in the config, as it always has.
pub fn sync_action(live: &Look, pointer: Option<&str>, cards: &[CardLook], name_hint: Option<&str>) -> SyncAction {
    let active = pointer.and_then(|pointer| cards.iter().find(|card| card.pointer == pointer));

    if let LookColors::Missing(_) = live.colors {
        return match active {
            Some(CardLook { name, look: Look { colors: LookColors::Known(colors), .. }, .. }) =>
                SyncAction::HealColors { colors: colors.clone(), name: name.clone() },
            _ => SyncAction::Nothing,
        };
    }
    let Some(live) = comparable(live) else {
        crate::always_eprint!("⚠️  The look on screen has a value a saved theme cannot hold, so it is not saved as one");
        return SyncAction::Nothing;
    };
    let is_default = live == Look::default();

    match active {
        Some(card) if same_look(&live, &card.look) => {
            return SyncAction::Nothing;
        }
        Some(card) if card.kind == CardKind::Mine => {
            return SyncAction::UpdateMine { id: card.pointer.clone(), look: live };
        }
        Some(_) if is_default => {
            return SyncAction::ClearPointer;
        }
        // A pointer to a card that is gone is dropped.
        None if is_default => {
            return if pointer.is_some() { SyncAction::ClearPointer } else { SyncAction::Nothing };
        }
        _ => {}
    }

    if let Some(card) = identical_card(&live, cards) {
        return SyncAction::Point { pointer: card.pointer.clone() };
    }
    let name = match active {
        Some(card) => copy_name(&card.name, cards),
        None if name_hint == Some(DEFAULT_NAME) => copy_name(DEFAULT_NAME, cards),
        None => first_free_name(cards, |n| with_suffix(NEW_THEME_NAME, &if n == 1 { String::new() } else { format!(" {}", n) })),
    };
    SyncAction::CreateMine { name, look: live }
}

/// The card whose look is exactly `live`: the user's own first, newest
/// first, then installed themes, then the ones that come with the app.
fn identical_card<'a>(live: &Look, cards: &'a [CardLook]) -> Option<&'a CardLook> {
    let rank = |kind: CardKind| match kind {
        CardKind::Mine => 0,
        CardKind::Imported | CardKind::Library => 1,
        CardKind::Starter => 2,
    };
    cards
        .iter()
        .filter(|card| same_look(live, &card.look))
        .min_by(|a, b| rank(a.kind).cmp(&rank(b.kind)).then_with(|| b.updated_at.cmp(&a.updated_at)))
}

/// `look` in the form a saved theme holds it, so two looks compare by what
/// they show: colors CSS cleaned, empty colors read as their default. `None`
/// when a saved theme could not hold it.
fn comparable(look: &Look) -> Option<Look> {
    checked_look(look.clone(), false)
}

/// Whether two looks show the same thing.
pub fn same_look(a: &Look, b: &Look) -> bool {
    match (comparable(a), comparable(b)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

/// "<name> (my copy)", or "(my copy 2)" and on when a card has that name.
pub fn copy_name(name: &str, cards: &[CardLook]) -> String {
    first_free_name(cards, |n| {
        with_suffix(name, &if n == 1 { " (my copy)".to_string() } else { format!(" (my copy {})", n) })
    })
}

/// The first name `candidate(1)`, `candidate(2)`, ... that no card has,
/// ignoring case.
fn first_free_name(cards: &[CardLook], candidate: impl Fn(u32) -> String) -> String {
    let taken = |name: &str| cards.iter().any(|card| card.name.to_lowercase() == name.to_lowercase());
    (1..)
        .map(candidate)
        .find(|name| !taken(name))
        .unwrap_or_else(|| NEW_THEME_NAME.to_string())
}

/// `name` with `suffix` after it, cut so the whole fits in a theme name and
/// the suffix is never the part cut off.
fn with_suffix(name: &str, suffix: &str) -> String {
    let room = MAX_NAME_CHARS.saturating_sub(suffix.chars().count());
    let name: String = name.chars().take(room).collect();
    format!("{}{}", name.trim_end(), suffix)
}

/// Whether the colors with this id are the ones the theme in use shows, so
/// deleting them would take them from under it.
pub fn palette_in_use(palette_id: &str, config: &AppConfig, cards: &[CardLook]) -> bool {
    config.theme == Theme::Custom(palette_id.to_string()) &&
        config.active_theme_id.as_deref().is_some_and(|pointer| cards.iter().any(|card| card.pointer == pointer))
}

/// Whether the config shows the Default theme: no saved theme, the colors
/// that follow the system, and the logo and background off.
pub fn shows_default(config: &AppConfig) -> bool {
    config.active_theme_id.is_none() && has_default_look(config)
}

fn has_default_look(config: &AppConfig) -> bool {
    config.theme == Theme::BuiltIn(BuiltInTheme::System) &&
        !config.enable_logo_customization &&
        !config.enable_background_customization
}

/// The name of the theme on screen: the card the config points at, or
/// Default. `None` when the look belongs to no card.
pub fn active_card_name(config: &AppConfig, cards: &[CardLook]) -> Option<String> {
    let pointer = config.active_theme_id.as_deref();
    match pointer.and_then(|pointer| cards.iter().find(|card| card.pointer == pointer)) {
        Some(card) => Some(card.name.clone()),
        // A pointer to a card that is gone counts as none.
        None if has_default_look(config) => Some(DEFAULT_NAME.to_string()),
        None => None,
    }
}

/// The theme `config.theme` takes to show `colors`. Custom colors reuse a
/// colors list entry whose CSS is exactly the cleaned CSS, or are added to
/// the list under `name`. The caller saves the list.
pub fn colors_choice(colors: &SharedColors, name: &str, themes: &mut ThemesConfig) -> Result<Theme, String> {
    match colors {
        SharedColors::BuiltIn(built_in) => Ok(Theme::BuiltIn(built_in.clone())),
        SharedColors::Custom { css } => {
            let css = safe_theme_css(css);
            if css.is_empty() {
                return Err("These colors have nothing this app can use.".to_string());
            }
            match themes.find_by_css(&css) {
                Some(id) => Ok(Theme::Custom(id)),
                None => themes.import_theme(name, &css).map(Theme::Custom),
            }
        }
    }
}

/// A new theme of the user's own showing `look`, or `None` when its colors
/// are missing.
pub fn new_mine(name: String, look: Look, now: DateTime<Utc>) -> Option<SavedTheme> {
    let LookColors::Known(colors) = look.colors else {
        return None;
    };
    Some(SavedTheme {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        kind: ThemeKind::Mine,
        library_id: None,
        library_checksum: None,
        fingerprint: None,
        colors,
        logo: look.logo,
        background: look.background,
        created_at: now,
        updated_at: now,
        extra: Map::new(),
    })
}

/// Coalesces edits, so one burst of edits makes one sync with the last
/// value. The name hint is the one of the burst's first edit.
#[derive(Debug, Default)]
pub struct SyncDebounce {
    burst: Option<Burst>,
}

#[derive(Debug)]
struct Burst {
    name_hint: Option<String>,
    last_edit: Instant,
}

impl SyncDebounce {
    pub fn note_edit(&mut self, now: Instant, name_hint: Option<String>) {
        match &mut self.burst {
            Some(burst) => {
                burst.last_edit = now;
            }
            None => {
                self.burst = Some(Burst { name_hint, last_edit: now });
            }
        }
    }

    /// The burst's name hint once `SYNC_DELAY` has passed since its last
    /// edit. The burst is then over.
    pub fn take_due(&mut self, now: Instant) -> Option<Option<String>> {
        let due = self.burst
            .as_ref()
            .is_some_and(|burst| now.saturating_duration_since(burst.last_edit) >= SYNC_DELAY);
        if due { self.take_now() } else { None }
    }

    /// The waiting burst's name hint, due or not, so it can be synced before
    /// another card is put on.
    pub fn take_now(&mut self) -> Option<Option<String>> {
        self.burst.take().map(|burst| burst.name_hint)
    }
}

// ===== Installing =====

/// Where a theme being installed came from.
#[derive(Debug, Clone, PartialEq)]
pub enum ThemeSource {
    /// A `.mvtheme` file, told apart from others by `SharedTheme::fingerprint`.
    File {
        fingerprint: String,
    },
    /// The theme library: the id from the link, already checked, and the
    /// checksum the library gave for the file.
    Library {
        id: String,
        checksum: String,
    },
}

/// What putting an installed theme on writes to the config.
#[derive(Debug, Clone, PartialEq)]
pub struct Installed {
    /// The theme with only the ticked parts.
    pub shown: SharedTheme,
    /// The theme its colors became, when the colors are ticked.
    pub colors: Option<Theme>,
    /// The id of the installed theme's card.
    pub card: String,
}

/// The colors, then the card, of a theme whose images are installed already:
/// `full` holds every part, pointed at the installed images, and `written`
/// are the image files that install made. The card holds every part, so
/// putting it on later shows the whole theme; only `Installed::shown` leaves
/// out the parts the user did not tick.
///
/// The order keeps a failure from losing what was installed before. Colors
/// that cannot be added take `written` back and change nothing else. A card
/// that cannot be saved leaves the list as it was (`save_list` gives the
/// change to the list and says whether it was saved), and the images stay
/// for the clean-up of unused images, since the old card may use some.
pub fn add_installed(
    full: SharedTheme,
    written: &[PathBuf],
    parts: ImportParts,
    source: &ThemeSource,
    choose_colors: impl FnOnce(&SharedColors, &str) -> Result<Theme, String>,
    save_list: impl FnOnce(&mut dyn FnMut(&mut SavedThemes)) -> Result<(), String>
) -> Result<Installed, String> {
    let colors = if parts.colors {
        match choose_colors(&full.colors, &full.name) {
            Ok(theme) => Some(theme),
            Err(e) => {
                theme_share::remove_images(written);
                return Err(e);
            }
        }
    } else {
        None
    };

    let mut card = None;
    save_list(&mut |store| {
        card = Some(upsert_installed(store, &full, source, Utc::now()));
    })?;
    let card = card.ok_or_else(|| "Could not save the theme.".to_string())?;

    let mut shown = full;
    shown.keep_parts(parts);
    Ok(Installed { shown, colors, card })
}

/// Put an installed theme's card on the list and give its id. A library
/// theme installed before is updated in place: same id and creation time,
/// the content, name and checksum of this install. A file imported before
/// keeps its card as it is. A copy the user made of either is never touched,
/// since it is a theme of their own with no library id or fingerprint.
pub fn upsert_installed(store: &mut SavedThemes, theme: &SharedTheme, source: &ThemeSource, now: DateTime<Utc>) -> String {
    let existing = store
        .themes()
        .find(|saved| match source {
            ThemeSource::Library { id, .. } =>
                saved.kind == ThemeKind::Library && saved.library_id.as_deref() == Some(id.as_str()),
            ThemeSource::File { fingerprint } =>
                saved.kind == ThemeKind::Imported && saved.fingerprint.as_deref() == Some(fingerprint.as_str()),
        })
        .map(|saved| saved.id.clone());

    match (existing, source) {
        (Some(card), ThemeSource::Library { checksum, .. }) => {
            store.update(&card, |saved| {
                saved.name = theme.name.clone();
                saved.colors = theme.colors.clone();
                saved.logo = theme.logo.clone();
                saved.background = theme.background.clone();
                saved.library_checksum = Some(checksum.clone());
                saved.updated_at = now;
            });
            card
        }
        (Some(card), ThemeSource::File { .. }) => card,
        (None, source) => {
            let (kind, library_id, library_checksum, fingerprint) = match source {
                ThemeSource::Library { id, checksum } =>
                    (ThemeKind::Library, Some(id.clone()), Some(checksum.clone()), None),
                ThemeSource::File { fingerprint } => (ThemeKind::Imported, None, None, Some(fingerprint.clone())),
            };
            let card = SavedTheme {
                id: uuid::Uuid::new_v4().to_string(),
                name: theme.name.clone(),
                kind,
                library_id,
                library_checksum,
                fingerprint,
                colors: theme.colors.clone(),
                logo: theme.logo.clone(),
                background: theme.background.clone(),
                created_at: now,
                updated_at: now,
                extra: Map::new(),
            };
            let id = card.id.clone();
            store.insert(card);
            id
        }
    }
}

// ===== Carrying it out =====

/// Make sure the look on screen is saved as a theme, as `sync_action` says.
/// Lost colors are put back from the active card first, then the rest is
/// synced once more, and only once, so it can never loop. A new theme is
/// pointed at right after it is saved: if the app stops in between, the next
/// sync finds the copy and points at it rather than making another.
///
/// Changes the colors list without telling a window that shows it, so a
/// caller with a window refreshes the list afterwards.
pub fn run_sync(name_hint: Option<&str>) {
    for first_pass in [true, false] {
        let store = saved_themes::current();
        let config = config_writer::current();
        let live = Look::from_config(&config, &theme::get_themes_config());
        let action = sync_action(&live, config.active_theme_id.as_deref(), &cards(&store), name_hint).unless_read_only(
            store.read_only.is_some()
        );

        match action {
            SyncAction::Nothing => {}
            SyncAction::HealColors { colors, name } => {
                if !first_pass {
                    crate::always_eprint!("⚠️  The colors of '{}' are still missing after putting them back", name);
                    return;
                }
                match theme::change_themes_config(|themes| colors_choice(&colors, &name, themes)).and_then(|r| r) {
                    Ok(choice) => {
                        crate::always_print!("🎨 Put the colors of '{}' back in the colors list", name);
                        config_writer::apply(|cfg| {
                            cfg.theme = choice;
                        });
                        continue;
                    }
                    Err(e) => crate::always_eprint!("❌ Could not put the colors of '{}' back: {}", name, e),
                }
            }
            SyncAction::UpdateMine { id, look } => {
                let LookColors::Known(colors) = look.colors else {
                    return;
                };
                let _ = saved_themes::apply(|store| {
                    store.update(&id, |theme| {
                        theme.colors = colors;
                        theme.logo = look.logo;
                        theme.background = look.background;
                        theme.updated_at = Utc::now();
                    });
                });
            }
            SyncAction::CreateMine { name, look } => {
                let Some(theme) = new_mine(name, look, Utc::now()) else {
                    return;
                };
                let id = theme.id.clone();
                crate::always_print!("💾 Saving the look on screen as '{}'", theme.name);
                if saved_themes::apply(|store| store.insert(theme)).is_ok() {
                    config_writer::apply(|cfg| {
                        cfg.active_theme_id = Some(id);
                    });
                }
            }
            SyncAction::Point { pointer } => {
                config_writer::apply(|cfg| {
                    cfg.active_theme_id = Some(pointer);
                });
            }
            SyncAction::ClearPointer => {
                config_writer::apply(|cfg| {
                    cfg.active_theme_id = None;
                });
            }
        }
        return;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::libs::theme::BuiltInTheme;
    use crate::state::config::{ BackgroundCustomization, LogoCustomization };

    fn look_with_logo(border: &str) -> Look {
        Look {
            colors: LookColors::Known(SharedColors::BuiltIn(BuiltInTheme::Dark)),
            logo: Some(LogoCustomization { border_color: border.to_string(), ..LogoCustomization::default() }),
            background: None,
        }
    }

    fn custom(css: &str) -> Look {
        Look { colors: LookColors::Known(SharedColors::Custom { css: css.to_string() }), ..Look::default() }
    }

    fn card(pointer: &str, name: &str, kind: CardKind, look: Look) -> CardLook {
        CardLook { pointer: pointer.to_string(), name: name.to_string(), kind, look, updated_at: DateTime::default() }
    }

    fn starter(look: Look) -> CardLook {
        card("starter:retro-beige", "Retro Beige", CardKind::Starter, look)
    }

    /// Does to `cards` and `pointer` what `run_sync` does to the store and
    /// the config.
    fn carry_out(action: SyncAction, cards: &mut Vec<CardLook>, pointer: &mut Option<String>) {
        match action {
            SyncAction::Nothing | SyncAction::HealColors { .. } => {}
            SyncAction::UpdateMine { id, look } => {
                cards.iter_mut().find(|card| card.pointer == id).unwrap().look = look;
            }
            SyncAction::CreateMine { name, look } => {
                let theme = new_mine(name, look, Utc::now()).unwrap();
                *pointer = Some(theme.id.clone());
                cards.push(CardLook::saved(&theme));
            }
            SyncAction::Point { pointer: to } => {
                *pointer = Some(to);
            }
            SyncAction::ClearPointer => {
                *pointer = None;
            }
        }
    }

    fn creates(action: &SyncAction) -> Option<&str> {
        match action {
            SyncAction::CreateMine { name, .. } => Some(name),
            _ => None,
        }
    }

    #[test]
    fn editing_my_theme_saves_it_in_place() {
        let cards = [card("a", "Night drive", CardKind::Mine, look_with_logo("red"))];
        let live = look_with_logo("blue");
        assert_eq!(sync_action(&live, Some("a"), &cards, None), SyncAction::UpdateMine {
            id: "a".to_string(),
            look: live.clone(),
        });
    }

    #[test]
    fn editing_a_starter_makes_one_copy() {
        let cards = [starter(look_with_logo("red"))];
        let live = look_with_logo("blue");
        assert_eq!(sync_action(&live, Some("starter:retro-beige"), &cards, None), SyncAction::CreateMine {
            name: "Retro Beige (my copy)".to_string(),
            look: live,
        });
    }

    #[test]
    fn the_second_edit_updates_the_copy() {
        let mut cards = vec![starter(look_with_logo("red"))];
        let mut pointer = Some("starter:retro-beige".to_string());
        let first = sync_action(&look_with_logo("blue"), pointer.as_deref(), &cards, None);
        carry_out(first, &mut cards, &mut pointer);
        let copy = pointer.clone().unwrap();
        assert_eq!(cards.len(), 2);
        assert_eq!(sync_action(&look_with_logo("blue"), pointer.as_deref(), &cards, None), SyncAction::Nothing);

        let second = sync_action(&look_with_logo("green"), pointer.as_deref(), &cards, None);
        assert_eq!(second, SyncAction::UpdateMine { id: copy, look: look_with_logo("green") });
        carry_out(second, &mut cards, &mut pointer);
        assert_eq!(cards.len(), 2, "no second copy");
        assert_eq!(cards[0].look, look_with_logo("red"), "the starter is untouched");
    }

    #[test]
    fn editing_an_installed_theme_leaves_it_untouched() {
        for kind in [CardKind::Imported, CardKind::Library] {
            let cards = [card("i", "Rainy cafe", kind, look_with_logo("red"))];
            let action = sync_action(&look_with_logo("blue"), Some("i"), &cards, None);
            assert_eq!(creates(&action), Some("Rainy cafe (my copy)"), "{:?}", kind);
        }
    }

    #[test]
    fn editing_default_makes_default_my_copy() {
        let action = sync_action(&look_with_logo("blue"), None, &[], Some(DEFAULT_NAME));
        assert_eq!(creates(&action), Some("Default (my copy)"));
    }

    #[test]
    fn an_old_config_with_its_own_look_becomes_my_theme() {
        let mut config = AppConfig::default();
        config.theme = Theme::BuiltIn(BuiltInTheme::Synthwave);
        config.enable_logo_customization = true;
        config.logo_customization.border_color = "#ff4fa3".to_string();
        config.enable_background_customization = true;
        config.background_customization.background_color = "#101020".to_string();
        let live = Look::from_config(&config, &ThemesConfig::default());

        let action = sync_action(&live, config.active_theme_id.as_deref(), &[], None);
        assert_eq!(action, SyncAction::CreateMine { name: "My theme".to_string(), look: live });
    }

    #[test]
    fn an_old_default_config_makes_no_theme() {
        let config = AppConfig::default();
        let live = Look::from_config(&config, &ThemesConfig::default());
        assert_eq!(live, Look::default());
        assert_eq!(sync_action(&live, None, &[], None), SyncAction::Nothing);
    }

    #[test]
    fn a_look_already_saved_is_not_saved_twice() {
        let mut older = card("old", "Older", CardKind::Mine, look_with_logo("red"));
        older.updated_at = DateTime::from_timestamp(1_000, 0).unwrap();
        let mut newer = card("new", "Newer", CardKind::Mine, look_with_logo("red"));
        newer.updated_at = DateTime::from_timestamp(2_000, 0).unwrap();
        let cards = [
            starter(look_with_logo("red")),
            card("lib", "Library", CardKind::Library, look_with_logo("red")),
            older,
            newer,
        ];
        assert_eq!(
            sync_action(&look_with_logo("red"), None, &cards, None),
            SyncAction::Point { pointer: "new".to_string() },
            "the user's own newest theme comes first"
        );
    }

    #[test]
    fn going_back_to_the_default_look_clears_the_pointer() {
        for kind in [CardKind::Starter, CardKind::Imported, CardKind::Library] {
            let cards = [card("c", "Card", kind, look_with_logo("red"))];
            assert_eq!(sync_action(&Look::default(), Some("c"), &cards, None), SyncAction::ClearPointer);
        }
    }

    #[test]
    fn copy_names_never_repeat() {
        let mut cards = vec![
            card("n", "Night", CardKind::Starter, look_with_logo("red")),
            card("c1", "night (my copy)", CardKind::Mine, look_with_logo("green"))
        ];
        assert_eq!(copy_name("Night", &cards), "Night (my copy 2)");
        cards.push(card("c2", "Night (my copy 2)", CardKind::Mine, look_with_logo("green")));
        assert_eq!(copy_name("Night", &cards), "Night (my copy 3)");

        let long = "A".repeat(MAX_NAME_CHARS);
        let name = copy_name(&long, &cards);
        assert!(name.ends_with(" (my copy)"), "{}", name);
        assert_eq!(name.chars().count(), MAX_NAME_CHARS);

        cards.push(card("m", "My theme", CardKind::Mine, look_with_logo("green")));
        let action = sync_action(&look_with_logo("blue"), None, &cards, None);
        assert_eq!(creates(&action), Some("My theme 2"));
    }

    #[test]
    fn a_missing_active_theme_counts_as_none() {
        let cards = [card("a", "Night drive", CardKind::Mine, look_with_logo("red"))];
        let action = sync_action(&look_with_logo("blue"), Some("gone"), &cards, None);
        assert_eq!(creates(&action), Some("My theme"));
        assert_eq!(sync_action(&look_with_logo("red"), Some("gone"), &cards, None), SyncAction::Point {
            pointer: "a".to_string(),
        });
        assert_eq!(sync_action(&Look::default(), Some("gone"), &cards, None), SyncAction::ClearPointer);
    }

    #[test]
    fn cleaned_and_raw_css_compare_equal() {
        let raw = custom("--color-primary: red; } body { display: none; }");
        let clean = custom("--color-primary: red;");
        assert!(same_look(&raw, &clean));
        assert!(!same_look(&raw, &custom("--color-primary: blue;")));
        let cards = [card("a", "Red", CardKind::Mine, clean)];
        assert_eq!(sync_action(&raw, Some("a"), &cards, None), SyncAction::Nothing);
    }

    fn lost_colors() -> Look {
        Look { colors: LookColors::Missing("gone".to_string()), ..look_with_logo("blue") }
    }

    #[test]
    fn a_lost_colors_list_never_overwrites_my_theme() {
        let css = "--color-primary: red;";
        let cards = [card("a", "Night drive", CardKind::Mine, custom(css))];
        assert_eq!(sync_action(&lost_colors(), Some("a"), &cards, None), SyncAction::HealColors {
            colors: SharedColors::Custom { css: css.to_string() },
            name: "Night drive".to_string(),
        });
    }

    #[test]
    fn lost_colors_on_a_starter_make_no_copy() {
        for kind in [CardKind::Starter, CardKind::Imported, CardKind::Library] {
            let cards = [card("c", "Card", kind, custom("--color-primary: red;"))];
            let action = sync_action(&lost_colors(), Some("c"), &cards, Some(DEFAULT_NAME));
            assert!(matches!(action, SyncAction::HealColors { .. }), "{:?}: {:?}", kind, action);
        }
    }

    #[test]
    fn lost_colors_with_no_theme_change_nothing() {
        let cards = [card("a", "Night drive", CardKind::Mine, custom("--color-primary: red;"))];
        assert_eq!(sync_action(&lost_colors(), None, &cards, None), SyncAction::Nothing);
        assert_eq!(sync_action(&lost_colors(), Some("gone"), &cards, None), SyncAction::Nothing);
        assert_eq!(sync_action(&lost_colors(), None, &[], Some(DEFAULT_NAME)), SyncAction::Nothing);
    }

    #[test]
    fn a_dropped_pointer_finds_the_starter_on_screen() {
        let cards = [starter(look_with_logo("red"))];
        assert_eq!(sync_action(&look_with_logo("red"), None, &cards, None), SyncAction::Point {
            pointer: "starter:retro-beige".to_string(),
        });
    }

    #[test]
    fn a_lost_pointer_on_an_installed_theme_points_back_at_it() {
        let cards = [card("lib", "From the library", CardKind::Library, look_with_logo("red"))];
        assert_eq!(sync_action(&look_with_logo("red"), None, &cards, None), SyncAction::Point {
            pointer: "lib".to_string(),
        });
    }

    #[test]
    fn a_crash_between_the_copy_and_the_pointer_makes_no_second_copy() {
        let cards = [
            starter(look_with_logo("red")),
            card("copy", "Retro Beige (my copy)", CardKind::Mine, look_with_logo("blue")),
        ];
        assert_eq!(sync_action(&look_with_logo("blue"), Some("starter:retro-beige"), &cards, None), SyncAction::Point {
            pointer: "copy".to_string(),
        });
    }

    #[test]
    fn typing_a_color_makes_one_copy_with_the_last_value() {
        let mut cards = vec![starter(look_with_logo("red"))];
        let mut pointer = Some("starter:retro-beige".to_string());
        let mut debounce = SyncDebounce::default();
        let start = Instant::now();
        // "#" and "#f" are never valid, so the color box commits only the last.
        for (at, _typed) in [(0, "#"), (100, "#f"), (200, "#ff0000")] {
            debounce.note_edit(start + Duration::from_millis(at), None);
            assert_eq!(debounce.take_due(start + Duration::from_millis(at + 50)), None, "still typing");
        }
        let live = look_with_logo("#ff0000");
        assert_eq!(debounce.take_due(start + Duration::from_millis(450)), None, "300 ms from the last edit");
        let hint = debounce.take_due(start + Duration::from_millis(500)).expect("due");
        assert_eq!(debounce.take_due(start + Duration::from_millis(900)), None, "one sync per burst");

        let action = sync_action(&live, pointer.as_deref(), &cards, hint.as_deref());
        assert_eq!(action, SyncAction::CreateMine { name: "Retro Beige (my copy)".to_string(), look: live.clone() });
        carry_out(action, &mut cards, &mut pointer);
        assert_eq!(sync_action(&live, pointer.as_deref(), &cards, None), SyncAction::Nothing);
        assert_eq!(cards.len(), 2);
    }

    #[test]
    fn a_card_switch_runs_the_waiting_sync_first() {
        let mut debounce = SyncDebounce::default();
        let start = Instant::now();
        debounce.note_edit(start, Some(DEFAULT_NAME.to_string()));
        debounce.note_edit(start + Duration::from_millis(50), None);
        assert_eq!(debounce.take_now(), Some(Some(DEFAULT_NAME.to_string())), "the first edit's hint, not yet due");
        assert_eq!(debounce.take_due(start + Duration::from_secs(5)), None);
        assert_eq!(debounce.take_now(), None);
    }

    #[test]
    fn colors_that_clean_to_nothing_are_never_saved() {
        let nothing = custom("} body { display: none; }");
        let cards = [card("a", "Night drive", CardKind::Mine, look_with_logo("red"))];
        assert_eq!(sync_action(&nothing, None, &cards, None), SyncAction::Nothing);
        assert_eq!(sync_action(&nothing, Some("a"), &cards, None), SyncAction::Nothing);
        assert_eq!(sync_action(&nothing, None, &[], Some(DEFAULT_NAME)), SyncAction::Nothing);
    }

    #[test]
    fn a_look_a_saved_theme_cannot_hold_stays_in_the_config() {
        let mut spaced = look_with_logo("red");
        spaced.background = Some(BackgroundCustomization {
            background_image: Some("C:\\Users\\Jo Doe\\wall.png".to_string()),
            use_image: true,
            ..BackgroundCustomization::default()
        });
        assert_eq!(sync_action(&spaced, None, &[], None), SyncAction::Nothing);
    }

    #[test]
    fn a_read_only_store_makes_no_copy() {
        let created = sync_action(&look_with_logo("blue"), None, &[], None);
        assert!(matches!(created, SyncAction::CreateMine { .. }));
        assert_eq!(created.clone().unless_read_only(true), SyncAction::Nothing);
        assert_eq!(created.clone().unless_read_only(false), created);

        let cards = [card("a", "Night drive", CardKind::Mine, look_with_logo("red"))];
        let updated = sync_action(&look_with_logo("blue"), Some("a"), &cards, None);
        assert!(matches!(updated, SyncAction::UpdateMine { .. }));
        assert_eq!(updated.unless_read_only(true), SyncAction::Nothing);

        // Pointing and putting colors back write no theme, so they still happen.
        let point = SyncAction::Point { pointer: "a".to_string() };
        assert_eq!(point.clone().unless_read_only(true), point);
        let heal = sync_action(&lost_colors(), Some("a"), &cards, None);
        assert_eq!(heal.clone().unless_read_only(true), heal);
        assert_eq!(SyncAction::ClearPointer.unless_read_only(true), SyncAction::ClearPointer);
    }

    #[test]
    fn the_colors_of_the_theme_in_use_cannot_be_deleted() {
        let cards = [card("a", "Night drive", CardKind::Mine, custom("--color-primary: red;"))];
        let mut config = AppConfig::default();
        config.theme = Theme::Custom("p1".to_string());
        config.active_theme_id = Some("a".to_string());
        assert!(palette_in_use("p1", &config, &cards));
        assert!(!palette_in_use("p2", &config, &cards), "other colors can go");

        config.active_theme_id = None;
        assert!(!palette_in_use("p1", &config, &cards), "no theme in use");
        config.active_theme_id = Some("gone".to_string());
        assert!(!palette_in_use("p1", &config, &cards));
    }

    #[test]
    fn the_current_theme_is_named_after_its_card() {
        let cards = [card("a", "Night drive", CardKind::Mine, look_with_logo("red"))];
        let mut config = AppConfig::default();
        assert!(shows_default(&config));
        assert_eq!(active_card_name(&config, &cards).as_deref(), Some("Default"));

        config.active_theme_id = Some("a".to_string());
        config.enable_logo_customization = true;
        assert!(!shows_default(&config));
        assert_eq!(active_card_name(&config, &cards).as_deref(), Some("Night drive"));

        config.active_theme_id = Some("gone".to_string());
        assert_eq!(active_card_name(&config, &cards), None, "a look that belongs to no card");
        config.enable_logo_customization = false;
        assert_eq!(active_card_name(&config, &cards).as_deref(), Some("Default"));
        assert!(!shows_default(&config), "until the pointer is cleared");
    }

    #[test]
    fn a_new_theme_of_my_own_holds_the_look() {
        let now = Utc::now();
        let theme = new_mine("Night drive".to_string(), look_with_logo("red"), now).unwrap();
        assert_eq!(theme.kind, ThemeKind::Mine);
        assert_eq!(theme.look(), look_with_logo("red"));
        assert_eq!((theme.created_at, theme.updated_at), (now, now));
        assert!(saved_themes::checked(theme).is_some(), "it passes the saved list's checks");
        assert_eq!(new_mine("Lost".to_string(), lost_colors(), now), None);
    }

    // ===== Installing =====

    /// A theme with custom colors, a logo with two images and a background
    /// image, named "Night drive".
    const NIGHT: &[u8] = include_bytes!("theme_share/fixtures/v1/custom-full.mvtheme");
    /// Another theme with a logo and background, and images of its own.
    const SUNSET: &[u8] = include_bytes!("theme_share/fixtures/v1/library-rewritten.mvtheme");
    /// Colors only.
    const PLAIN: &[u8] = include_bytes!("theme_share/fixtures/v1/built-in-plain.mvtheme");
    const LIBRARY_ID: &str = "th-0123456789abcdef0123456789abcdef";

    /// An empty folder of its own under the system temp folder.
    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mechvibes-theme-library-{}-{}", name, uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn files_in(dir: &std::path::Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs
            ::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        names.sort();
        names
    }

    fn library(checksum: &str) -> ThemeSource {
        ThemeSource::Library { id: LIBRARY_ID.to_string(), checksum: checksum.to_string() }
    }

    fn file(bytes: &[u8]) -> ThemeSource {
        ThemeSource::File { fingerprint: theme_share::from_bytes(bytes).unwrap().fingerprint() }
    }

    /// Install `bytes` the way `put_on` does: every image into `dir`, the
    /// colors into `themes`, the card into `store`.
    fn install(
        bytes: &[u8],
        parts: ImportParts,
        source: &ThemeSource,
        dir: &std::path::Path,
        store: &mut SavedThemes,
        themes: &mut ThemesConfig
    ) -> (Installed, Vec<PathBuf>) {
        let mut theme = theme_share::from_bytes(bytes).unwrap();
        let written = theme_share::install_images_in(&mut theme, dir).unwrap();
        let installed = add_installed(
            theme,
            &written,
            parts,
            source,
            |colors, name| colors_choice(colors, name, themes),
            |upsert| {
                upsert(store);
                Ok(())
            }
        ).unwrap();
        (installed, written)
    }

    fn card_of<'a>(store: &'a SavedThemes, id: &str) -> &'a SavedTheme {
        store.themes().find(|theme| theme.id == id).unwrap()
    }

    /// Puts the installed theme on a config the way `put_on` does.
    fn wear(installed: &Installed, parts: ImportParts, config: &mut AppConfig) {
        theme_share::apply_to_config(&installed.shown, installed.colors.clone(), parts, config);
        config.active_theme_id = Some(installed.card.clone());
    }

    #[test]
    fn reinstalling_a_library_theme_updates_it_in_place() {
        let dir = temp_dir("reinstall");
        let (mut store, mut themes) = (SavedThemes::default(), ThemesConfig::default());
        let (first, _) = install(NIGHT, ImportParts::ALL, &library("aaa"), &dir, &mut store, &mut themes);
        let before = card_of(&store, &first.card).clone();
        assert_eq!(before.kind, ThemeKind::Library);
        assert_eq!(before.library_id.as_deref(), Some(LIBRARY_ID));
        assert!(saved_themes::checked(before.clone()).is_some(), "it passes the saved list's checks");

        let (second, _) = install(SUNSET, ImportParts::ALL, &library("bbb"), &dir, &mut store, &mut themes);
        assert_eq!(second.card, first.card);
        assert_eq!(store.themes().count(), 1, "one card for each library theme");
        let card = card_of(&store, &second.card);
        assert_eq!((&card.id, card.created_at), (&before.id, before.created_at));
        assert_eq!(card.name, "Library sunset");
        assert_eq!(card.library_checksum.as_deref(), Some("bbb"));
        assert_eq!(card.colors, theme_share::from_bytes(SUNSET).unwrap().colors);
        assert_ne!(card.logo, before.logo);
        assert!(card.updated_at >= before.updated_at);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reinstalling_leaves_my_copy_alone() {
        let dir = temp_dir("copy");
        let (mut store, mut themes) = (SavedThemes::default(), ThemesConfig::default());
        let (installed, _) = install(NIGHT, ImportParts::ALL, &library("aaa"), &dir, &mut store, &mut themes);

        // An edit to the installed theme saves a copy.
        let mut live = card_of(&store, &installed.card).look();
        live.background = None;
        let SyncAction::CreateMine { name, look } = sync_action(&live, Some(&installed.card), &cards(&store), None) else {
            panic!("an edit to an installed theme makes a copy");
        };
        let copy = new_mine(name, look, Utc::now()).unwrap();
        store.insert(copy.clone());

        install(SUNSET, ImportParts::ALL, &library("bbb"), &dir, &mut store, &mut themes);
        assert_eq!(card_of(&store, &copy.id), &copy, "the copy is the user's own");
        assert_eq!(store.themes().filter(|theme| theme.kind == ThemeKind::Library).count(), 1);
        assert_eq!(card_of(&store, &installed.card).name, "Library sunset");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn importing_the_same_file_twice_reuses_its_card() {
        let dir = temp_dir("twice");
        let (mut store, mut themes) = (SavedThemes::default(), ThemesConfig::default());
        let (first, _) = install(NIGHT, ImportParts::ALL, &file(NIGHT), &dir, &mut store, &mut themes);
        let card = card_of(&store, &first.card).clone();
        assert_eq!(card.kind, ThemeKind::Imported);
        assert_eq!(card.library_id, None);
        assert!(saved_themes::checked(card.clone()).is_some(), "it passes the saved list's checks");

        let (again, written) = install(NIGHT, ImportParts::ALL, &file(NIGHT), &dir, &mut store, &mut themes);
        assert_eq!(again.card, first.card);
        assert_eq!(store.themes().count(), 1);
        assert_eq!(card_of(&store, &again.card), &card, "the card is reused as it is");
        assert!(written.is_empty(), "its images are there already");
        assert_eq!(themes.custom_themes.len(), 1, "and so are its colors");

        let (other, _) = install(SUNSET, ImportParts::ALL, &file(SUNSET), &dir, &mut store, &mut themes);
        assert_ne!(other.card, first.card, "another file gets a card of its own");
        assert_eq!(store.themes().count(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_mixed_import_keeps_the_original_and_saves_my_copy() {
        let dir = temp_dir("mixed");
        let (mut store, mut themes) = (SavedThemes::default(), ThemesConfig::default());
        let mut config = AppConfig::default();
        config.enable_logo_customization = true;
        config.logo_customization.text_color = "#00ff00".to_string();

        // The user keeps their own logo.
        let parts = ImportParts { logo: false, ..ImportParts::ALL };
        let (installed, _) = install(NIGHT, parts, &file(NIGHT), &dir, &mut store, &mut themes);
        wear(&installed, parts, &mut config);
        let live = Look::from_config(&config, &themes);
        let action = sync_action(&live, config.active_theme_id.as_deref(), &cards(&store), None);
        let SyncAction::CreateMine { name, look } = action else {
            panic!("a mixed look is saved as a copy, not {:?}", action);
        };
        assert_eq!(name, "Night drive (my copy)");
        assert_eq!(look.logo.as_ref().unwrap().text_color, "#00ff00", "the copy has the user's logo");
        assert!(look.background.is_some(), "and the theme's background");
        let original = card_of(&store, &installed.card);
        assert_eq!(original.logo.as_ref().unwrap().border_color, "#ff4fa3", "the installed theme keeps its own");

        // A theme without a logo leaves the user's on, which mixes the look too.
        let plain = theme_share::from_bytes(PLAIN).unwrap();
        let (installed, _) = install(PLAIN, ImportParts::ALL, &file(PLAIN), &dir, &mut store, &mut themes);
        wear(&installed, ImportParts::ALL, &mut config);
        let live = Look::from_config(&config, &themes);
        let action = sync_action(&live, config.active_theme_id.as_deref(), &cards(&store), None);
        assert_eq!(creates(&action), Some(format!("{} (my copy)", plain.name).as_str()));

        // With the user's own logo off, the look is the theme itself.
        config.enable_logo_customization = false;
        config.enable_background_customization = false;
        let live = Look::from_config(&config, &themes);
        assert_eq!(sync_action(&live, config.active_theme_id.as_deref(), &cards(&store), None), SyncAction::Nothing);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_installed_card_keeps_unticked_part_images() {
        let dir = temp_dir("unticked");
        let (mut store, mut themes) = (SavedThemes::default(), ThemesConfig::default());
        let parts = ImportParts { background: false, ..ImportParts::ALL };
        let (installed, _) = install(NIGHT, parts, &file(NIGHT), &dir, &mut store, &mut themes);

        let card = card_of(&store, &installed.card);
        let image = card.background.as_ref().unwrap().background_image.clone().unwrap();
        assert!(image.starts_with("/custom-images/img-"), "{}", image);
        assert!(dir.join(image.trim_start_matches("/custom-images/")).is_file(), "the background image is installed");
        assert_eq!(installed.shown.background, None, "but not put on");

        let mut config = AppConfig::default();
        config.enable_background_customization = true;
        config.background_customization.background_color = "#202020".to_string();
        let before = config.clone();
        wear(&installed, parts, &mut config);
        assert_eq!(config.enable_background_customization, before.enable_background_customization);
        assert_eq!(config.background_customization, before.background_customization);
        assert!(config.enable_logo_customization, "the ticked logo is on");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failed_reinstall_keeps_the_installed_version() {
        let dir = temp_dir("failed");
        let (mut store, mut themes) = (SavedThemes::default(), ThemesConfig::default());
        install(NIGHT, ImportParts::ALL, &library("aaa"), &dir, &mut store, &mut themes);
        let installed_version = store.clone();
        let images_before = files_in(&dir);

        // The colors cannot be added: the images this install made go again.
        let mut theme = theme_share::from_bytes(SUNSET).unwrap();
        let written = theme_share::install_images_in(&mut theme, &dir).unwrap();
        assert!(!written.is_empty());
        let failed = add_installed(
            theme,
            &written,
            ImportParts::ALL,
            &library("bbb"),
            |_, _| Err("The disk is full.".to_string()),
            |upsert| {
                upsert(&mut store);
                Ok(())
            }
        );
        assert_eq!(failed.unwrap_err(), "The disk is full.");
        assert_eq!(store, installed_version);
        assert_eq!(files_in(&dir), images_before, "the old images stay, the new ones are gone");

        // The card cannot be saved: the list stays as it was, and the images
        // stay for the clean-up of unused images.
        let mut theme = theme_share::from_bytes(SUNSET).unwrap();
        let written = theme_share::install_images_in(&mut theme, &dir).unwrap();
        let failed = add_installed(
            theme,
            &written,
            ImportParts::ALL,
            &library("bbb"),
            |colors, name| colors_choice(colors, name, &mut themes),
            |upsert| {
                let mut attempt = store.clone();
                upsert(&mut attempt);
                Err("Could not save your theme.".to_string())
            }
        );
        assert_eq!(failed.unwrap_err(), "Could not save your theme.");
        assert_eq!(store, installed_version);
        assert!(written.iter().all(|image| image.is_file()));
        assert_eq!(card_of(&store, &store.themes().next().unwrap().id).name, "Night drive");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
