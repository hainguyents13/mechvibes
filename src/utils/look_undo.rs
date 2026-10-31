//! One step back after a theme is put on.
//!
//! Putting a theme on replaces the colours, the logo and the background in
//! one go, and a theme can be one the user does not like or can hardly read.
//! Right before it is written, the fields it changes are remembered here, in
//! memory only, so an Undo button can put them back. Only the last step is
//! kept, and only while the look on screen is still the one that step left:
//! once something else changed it, going back would throw that change away.

use crate::libs::theme::Theme;
use crate::state::config::{ AppConfig, BackgroundCustomization, LogoCustomization };
use std::sync::Mutex;

/// The config fields a theme can change.
#[derive(Debug, Clone, PartialEq)]
pub struct LookFields {
    pub theme: Theme,
    pub logo: LogoCustomization,
    pub enable_logo: bool,
    pub background: BackgroundCustomization,
    pub enable_background: bool,
}

impl LookFields {
    pub fn of(config: &AppConfig) -> Self {
        Self {
            theme: config.theme.clone(),
            logo: config.logo_customization.clone(),
            enable_logo: config.enable_logo_customization,
            background: config.background_customization.clone(),
            enable_background: config.enable_background_customization,
        }
    }

    pub fn write_to(&self, config: &mut AppConfig) {
        config.theme = self.theme.clone();
        config.logo_customization = self.logo.clone();
        config.enable_logo_customization = self.enable_logo;
        config.background_customization = self.background.clone();
        config.enable_background_customization = self.enable_background;
    }
}

/// The last step: the look before it and the look it left.
#[derive(Debug, Default)]
struct UndoStep {
    step: Option<(LookFields, LookFields)>,
}

impl UndoStep {
    fn remember(&mut self, before: LookFields, after: LookFields) {
        self.step = Some((before, after));
    }

    fn take_undo(&mut self, now: &LookFields) -> Option<LookFields> {
        let (before, after) = self.step.take()?;
        (after == *now).then_some(before)
    }
}

/// Written from the window and from the config writer, which may run a
/// mutation on another thread, so it sits behind a lock.
static LAST: Mutex<UndoStep> = Mutex::new(UndoStep { step: None });

/// Remember one step. Replaces whatever was remembered before.
pub fn remember(before: LookFields, after: LookFields) {
    LAST.lock()
        .unwrap_or_else(|e| e.into_inner())
        .remember(before, after);
}

/// The look to go back to, only while the look on screen is still the one
/// the last step left. Clears the step either way, so it is used at most once.
pub fn take_undo(now: &LookFields) -> Option<LookFields> {
    LAST.lock()
        .unwrap_or_else(|e| e.into_inner())
        .take_undo(now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::libs::theme::BuiltInTheme;

    fn looks() -> (LookFields, LookFields) {
        let before = LookFields::of(&AppConfig::default());
        let mut config = AppConfig::default();
        config.theme = Theme::BuiltIn(BuiltInTheme::Synthwave);
        config.enable_logo_customization = true;
        config.logo_customization.text_color = "red".to_string();
        config.enable_background_customization = true;
        config.background_customization.background_color = "#202020".to_string();
        (before, LookFields::of(&config))
    }

    #[test]
    fn undo_puts_the_previous_look_back() {
        let (before, after) = looks();
        let mut last = UndoStep::default();
        last.remember(before.clone(), after.clone());
        assert_eq!(last.take_undo(&after), Some(before));
        assert_eq!(last.take_undo(&after), None, "one step, used once");
    }

    #[test]
    fn undo_does_nothing_after_another_change() {
        let (before, after) = looks();
        let mut last = UndoStep::default();
        last.remember(before.clone(), after.clone());
        let mut changed = after.clone();
        changed.enable_logo = false;
        assert_eq!(last.take_undo(&changed), None);
        assert_eq!(last.take_undo(&after), None, "the step is gone once the look moved on");
    }

    #[test]
    fn only_the_last_step_is_kept() {
        let (before, after) = looks();
        let mut last = UndoStep::default();
        last.remember(before.clone(), after.clone());
        last.remember(after.clone(), before.clone());
        assert_eq!(last.take_undo(&before), Some(after));
    }

    #[test]
    fn the_fields_a_theme_changes_are_written_back() {
        let (before, after) = looks();
        let mut config = AppConfig::default();
        config.volume = 0.25;
        after.write_to(&mut config);
        assert_eq!(LookFields::of(&config), after);
        before.write_to(&mut config);
        assert_eq!(LookFields::of(&config), before);
        assert_eq!(config.volume, 0.25, "nothing outside the look is touched");
    }
}
