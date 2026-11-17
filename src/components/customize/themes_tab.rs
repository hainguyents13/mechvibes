//! The Themes tab: a gallery of every theme the user can switch to. The
//! default look and the themes that come with the app first, then the user's
//! own, then the ones they installed, and a way to find more. Clicking a card
//! puts that theme on; the look from before stays in the gallery. Each card's
//! menu shares, renames, duplicates or deletes it, as its kind allows.

use crate::components::customize::current_theme_row::FIND_MORE_URL;
use crate::components::customize::look_editor::{ self, use_look_editor, use_saved_themes, Card };
use crate::components::customize::rename_modal::RenameModal;
use crate::components::customize::theme_card::{
    CardAction,
    CardPreview,
    ThemeCard,
    FRAME_CLASS,
    FRAME_STYLE,
    PICTURE_STYLE,
};
use crate::components::theme_share::share_card;
use crate::components::ui::ConfirmModal;
use crate::i18n::{ fill, Strings, use_strings };
use crate::state::saved_themes::{ SavedThemes, ThemeKind };
use crate::utils::config::use_config;
use crate::utils::notice::{ self, NoticeAction, NoticeKind };
use crate::utils::starter_themes;
use dioxus::prelude::*;
use lucide_dioxus::ExternalLink;

/// As many cards a row as fit, two in the default window.
const GRID_STYLE: &str = "grid-template-columns: repeat(auto-fill, minmax(10rem, 1fr));";

/// How to use a theme file: a double-click opens one in Mechvibes on Windows
/// and Linux only, since macOS has no file type registered for it yet.
///
/// The two sentences are two catalog keys, not one with a word swapped: on
/// macOS the hint has to name Import instead.
fn theme_file_hint(can_double_click: bool, strings: &Strings) -> &'static str {
    if can_double_click {
        strings.customize.themes_file_hint_double_click
    } else {
        strings.customize.themes_file_hint_drop
    }
}

/// A group of cards under a title, in the order the gallery shows them.
#[derive(Debug, PartialEq)]
struct Group {
    title: &'static str,
    cards: Vec<Card>,
}

/// The gallery's groups: the default look and the themes that come with the
/// app, the user's own themes, then the installed ones, each newest first. A
/// group with no cards is left out. The titles are the catalog's, so they
/// follow the language like everything else.
fn gallery(store: &SavedThemes, strings: &Strings) -> Vec<Group> {
    let mut ready_made = vec![Card::Default];
    ready_made.extend(starter_themes::all().iter().map(Card::Starter));
    let newest_first = |kinds: &[ThemeKind]| {
        let mut saved: Vec<_> = store
            .themes()
            .filter(|theme| kinds.contains(&theme.kind))
            .collect();
        saved.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        saved.into_iter().cloned().map(Card::Saved).collect::<Vec<_>>()
    };
    [
        Group { title: strings.customize.group_ready_made, cards: ready_made },
        Group { title: strings.customize.group_my_themes, cards: newest_first(&[ThemeKind::Mine]) },
        Group {
            title: strings.customize.group_installed,
            cards: newest_first(&[ThemeKind::Imported, ThemeKind::Library]),
        },
    ]
        .into_iter()
        .filter(|group| !group.cards.is_empty())
        .collect()
}

/// What a card's menu offers. Only the user's own themes can be renamed, and
/// only saved themes deleted; every card can be shared and duplicated.
fn actions(card: &Card) -> &'static [CardAction] {
    use CardAction::*;
    match card {
        Card::Default | Card::Starter(_) => &[Share, Duplicate],
        Card::Saved(saved) =>
            match saved.kind {
                ThemeKind::Mine => &[Share, Rename, Duplicate, Delete],
                ThemeKind::Imported | ThemeKind::Library => &[Share, Duplicate, Delete],
            }
    }
}

/// A card as the gallery draws it.
struct Shown {
    key: String,
    name: String,
    preview: CardPreview,
    active: bool,
    card: Card,
}

#[component]
pub fn ThemesTab() -> Element {
    let (config, _) = use_config();
    let saved_themes = use_saved_themes();
    let editor = use_look_editor();
    let strings = use_strings();
    // The id and name of the saved theme being renamed, or waiting for the
    // user to confirm its deletion.
    let mut renaming = use_signal(|| None::<(String, String)>);
    let mut deleting = use_signal(|| None::<(String, String)>);
    // The save dialog and the writing take a moment; one share at a time.
    let mut sharing = use_signal(|| false);

    // The look from before is saved first, so its card can be clicked again.
    // Only the default look offers Undo, since it is the one card that keeps
    // nothing of the look before it. A click on the card already on changes
    // nothing and keeps the way back.
    let put_on = {
        let editor = editor.clone();
        move |card: Card| {
            if editor.apply_card(&card) == Ok(true) && card == Card::Default {
                notice::report(
                    NoticeKind::Info,
                    strings.customize.default_look_is_on,
                    Some(NoticeAction::Undo)
                );
            }
        }
    };

    let on_action = move |card: Card, action: CardAction| {
        match (action, card) {
            (CardAction::Share, card) => {
                if *sharing.peek() {
                    return;
                }
                sharing.set(true);
                spawn(async move {
                    share_card(card).await;
                    sharing.set(false);
                });
            }
            // The copy shows up under "My themes". A failed one has told the
            // user why.
            (CardAction::Duplicate, card) => {
                let _ = look_editor::duplicate(&card);
            }
            (CardAction::Rename, Card::Saved(saved)) => renaming.set(Some((saved.id, saved.name))),
            (CardAction::Delete, Card::Saved(saved)) => deleting.set(Some((saved.id, saved.name))),
            (CardAction::Rename | CardAction::Delete, _) => {}
        }
    };

    let config = config();
    let groups: Vec<(&'static str, Vec<Shown>)> = gallery(&saved_themes(), strings)
        .into_iter()
        .map(|group| {
            let shown = group.cards
                .into_iter()
                .map(|card| Shown {
                    key: card.pointer().unwrap_or_else(|| "default".to_string()),
                    name: card.name().to_string(),
                    preview: CardPreview::of(&card.preview_look()),
                    active: card.is_on(&config),
                    card,
                })
                .collect();
            (group.title, shown)
        })
        .collect();
    let last = groups.len().saturating_sub(1);
    // Read once, before the `rsx!`, so the platform test stays in plain Rust.
    let file_hint = theme_file_hint(!cfg!(target_os = "macos"), strings);

    rsx! {
      div { class: "space-y-4",
        div { class: "text-sm text-base-content/70", "{strings.customize.themes_intro}" }
        for (index, (title, cards)) in groups.into_iter().enumerate() {
          div { key: "{title}", class: "space-y-2",
            div { class: "text-xs font-semibold text-base-content/60", "{title}" }
            div { class: "grid gap-3", style: GRID_STYLE,
              for shown in cards {
                ThemeCard {
                  key: "{shown.key}",
                  name: shown.name,
                  preview: shown.preview,
                  active: shown.active,
                  actions: actions(&shown.card),
                  on_apply: {
                      let put_on = put_on.clone();
                      let card = shown.card.clone();
                      move |_| put_on(card.clone())
                  },
                  on_action: {
                      let (card, mut on_action) = (shown.card, on_action);
                      move |action| on_action(card.clone(), action)
                  },
                }
              }
              if index == last {
                FindMoreCard {}
              }
            }
          }
        }
        div { class: "text-sm text-base-content/70", "{file_hint}" }
        if let Some((id, name)) = renaming() {
          RenameModal { id, name, on_close: move |_| renaming.set(None) }
        }
        if let Some((id, name)) = deleting() {
          ConfirmModal {
            title: fill(strings.customize.delete_title, &[&name]),
            message: strings.customize.delete_message.to_string(),
            confirm_label: strings.common.delete.to_string(),
            on_confirm: {
                let editor = editor.clone();
                move |_| {
                    deleting.set(None);
                    // A failed delete has told the user why.
                    let _ = editor.delete_theme(&id);
                }
            },
            on_cancel: move |_| deleting.set(None),
          }
        }
      }
    }
}

/// Opens the theme library on the website, shaped like the cards around it.
#[component]
fn FindMoreCard() -> Element {
    let strings = use_strings();

    rsx! {
      a {
        class: "{FRAME_CLASS} border-base-300",
        style: FRAME_STYLE,
        href: FIND_MORE_URL,
        target: "_blank",
        rel: "noopener noreferrer",
        div {
          class: "flex flex-col items-center justify-center gap-2 bg-base-100 text-base-content/70",
          style: PICTURE_STYLE,
          ExternalLink { class: "w-5 h-5" }
          div { class: "text-xs px-3 text-center", "{strings.customize.themes_made_by_others}" }
        }
        div { class: "bg-base-200 px-2 py-2 text-sm font-medium text-base-content",
          "{strings.customize.find_more}"
        }
      }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Language;
    use crate::libs::theme::BuiltInTheme;
    use crate::state::saved_themes::SavedTheme;
    use crate::utils::theme_share::SharedColors;
    use chrono::{ Duration, Utc };

    /// The catalog the assertions below compare against. The tests used to
    /// spell the English text out; they now name the same key the screen reads,
    /// so a wording change is one decision instead of two.
    fn en() -> &'static Strings {
        Language::En.strings()
    }

    #[test]
    fn the_theme_file_hint_offers_a_double_click_only_where_it_opens_the_file() {
        // Both hints keep saying what to do with the file; only the platform
        // decides which one shows.
        assert!(theme_file_hint(true, en()).contains("Double-click"));
        assert!(theme_file_hint(false, en()).contains("Drop it on this window"));
        assert_eq!(theme_file_hint(true, en()), en().customize.themes_file_hint_double_click);
        assert_eq!(theme_file_hint(false, en()), en().customize.themes_file_hint_drop);
    }

    fn saved(id: &str, kind: ThemeKind, minutes_ago: i64) -> SavedTheme {
        let at = Utc::now() - Duration::minutes(minutes_ago);
        SavedTheme {
            id: id.to_string(),
            name: id.to_string(),
            kind,
            library_id: None,
            library_checksum: None,
            fingerprint: None,
            colors: SharedColors::BuiltIn(BuiltInTheme::Dark),
            logo: None,
            background: None,
            created_at: at,
            updated_at: at,
            extra: serde_json::Map::new(),
        }
    }

    fn pointers(group: &Group) -> Vec<Option<String>> {
        group.cards.iter().map(Card::pointer).collect()
    }

    #[test]
    fn a_new_user_sees_only_the_ready_made_themes() {
        let groups = gallery(&SavedThemes::default(), en());
        assert_eq!(groups.len(), 1, "empty groups are left out");
        assert_eq!(groups[0].title, en().customize.group_ready_made);
        assert_eq!(pointers(&groups[0]), [
            None,
            Some("starter:retro-beige".to_string()),
            Some("starter:midnight-blue".to_string()),
            Some("starter:rgb-night".to_string()),
            Some("starter:rainy-cafe".to_string()),
        ]);
    }

    #[test]
    fn my_themes_and_installed_ones_are_grouped_newest_first() {
        let mut store = SavedThemes::default();
        store.insert(saved("old mine", ThemeKind::Mine, 30));
        store.insert(saved("file", ThemeKind::Imported, 20));
        store.insert(saved("new mine", ThemeKind::Mine, 1));
        store.insert(saved("library", ThemeKind::Library, 5));

        let groups = gallery(&store, en());
        let titles: Vec<&str> = groups.iter().map(|group| group.title).collect();
        assert_eq!(
            titles,
            [
                en().customize.group_ready_made,
                en().customize.group_my_themes,
                en().customize.group_installed,
            ],
            "the three groups are the catalog's, in the order the gallery shows them"
        );
        assert_eq!(pointers(&groups[1]), [Some("new mine".to_string()), Some("old mine".to_string())]);
        assert_eq!(pointers(&groups[2]), [Some("library".to_string()), Some("file".to_string())]);
    }

    #[test]
    fn each_kind_of_card_offers_its_own_menu() {
        use CardAction::*;
        let starter = &starter_themes::all()[0];
        assert_eq!(actions(&Card::Default), [Share, Duplicate]);
        assert_eq!(actions(&Card::Starter(starter)), [Share, Duplicate]);
        assert_eq!(actions(&Card::Saved(saved("mine", ThemeKind::Mine, 0))), [Share, Rename, Duplicate, Delete]);
        for kind in [ThemeKind::Imported, ThemeKind::Library] {
            assert_eq!(actions(&Card::Saved(saved("installed", kind, 0))), [Share, Duplicate, Delete]);
        }
    }
}
