use tray_icon::{
    Icon,
    menu::{ CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, MenuId },
    TrayIcon,
    TrayIconBuilder,
    TrayIconEvent,
};
use crate::i18n::catalog::TrayStrings;
use crate::i18n::fill;
use crate::state::config::AppConfig;
use crate::utils::constants::APP_NAME;
use std::sync::Mutex;
use std::time::Instant;

// Track last click time for double-click detection
static LAST_CLICK_TIME: Mutex<Option<Instant>> = Mutex::new(None);

// The tray lives on the GTK main thread.
//
// `TrayManager` owns GTK objects, which are `!Send`: they may only ever be
// touched from the thread running the GTK main loop. A thread-local is the
// correct home for it - `unsafe impl Send` would let another thread reach into
// GTK, and a `static Mutex` would need `TrayManager: Send` before it could
// even be stored. The component that builds it, the glib timeout source that
// drives the tray loop and the Dioxus runtime all run on that same main
// thread, so both ends can reach it without any synchronisation.
thread_local! {
    static TRAY: std::cell::RefCell<Option<TrayManager>> = const { std::cell::RefCell::new(None) };
}

/// Stores the tray, replacing any previous value.
pub fn store_tray(tray: TrayManager) {
    TRAY.with(|slot| *slot.borrow_mut() = Some(tray));
}

/// Runs `f` against the stored tray, or returns `None` when none is stored yet.
pub fn with_tray<R>(f: impl FnOnce(&mut TrayManager) -> R) -> Option<R> {
    TRAY.with(|slot| slot.borrow_mut().as_mut().map(f))
}

/// Takes the tray out of its store, for teardown.
///
/// The tray lives here rather than in the window, so nothing releases it by
/// itself: whoever closes the window has to take it out, or the icon stays
/// behind in the notification area. It is called twice on the way out, once by
/// the Exit item before it closes the window and once when the window is torn
/// down; the second call finds an empty store and does nothing.
pub fn take_tray() -> Option<TrayManager> {
    TRAY.with(|slot| slot.borrow_mut().take())
}

// Embed the icon at compile time for cross-platform reliability
const EMBEDDED_ICON: &[u8] = include_bytes!("../../assets/icon.ico");

/// Alpha multiplier applied to the normal icon to produce the muted variant.
/// Low enough to read as clearly "off" at 16x16 tray size, high enough that the
/// icon silhouette stays recognizable against both light and dark taskbars.
const MUTED_ICON_ALPHA: f32 = 0.4;

pub enum TrayMessage {
    Show,
    Exit,
    ToggleMute,
    ToggleMoodMute,
    OpenGitHub,
    OpenDiscord,
    OpenWebsite,
}

/// The two macOS permission lines in the tray menu.
///
/// Each shows "X: Granted" (nothing to do) or a red "Grant X…" (clickable, opens
/// the matching System Settings pane). The handles are cloned: the tray keeps one
/// set, so a language change can relabel the pair from `update_menu`, and the
/// permission watcher keeps the other, so the lines still update while the window
/// is hidden. Both point at the same native items.
///
/// **The words come from the catalog, the layout and the colour from
/// `permissions`.** It has to be that way round: the attributed title it builds
/// is made from the exact string the item shows, so a label put on the item
/// behind its back would be replaced the moment the row was styled.
#[cfg(target_os = "macos")]
#[derive(Clone)]
struct PermissionItems {
    accessibility: MenuItem,
    input_monitoring: MenuItem,
    /// The native menu the tray shows. The menu library does not expose its
    /// native items, so the colours are applied through this, finding each line
    /// by its tag (see `permissions::style_menu_item`). It lives as long as the
    /// tray does.
    ns_menu: *mut std::ffi::c_void,
}

/// The catalog words for one permission row, in the language the tray shows.
///
/// `permissions` lays them out; which words go in is decided here, next to the
/// menu labels they sit between.
#[cfg(target_os = "macos")]
fn permission_words<'a>(
    strings: &'a TrayStrings,
    name: &'a str
) -> crate::libs::permissions::PermissionText<'a> {
    crate::libs::permissions::PermissionText {
        name,
        granted_line: strings.permission_granted,
        grant_line: strings.permission_grant,
        granted_word: strings.permission_granted_word,
    }
}

#[cfg(target_os = "macos")]
impl PermissionItems {
    fn new(
        status: crate::libs::permissions::PermissionStatus,
        ns_menu: *mut std::ffi::c_void,
        strings: &TrayStrings
    ) -> Self {
        use crate::libs::permissions::menu_text;
        let accessibility = permission_words(strings, strings.accessibility);
        let input_monitoring = permission_words(strings, strings.input_monitoring);
        Self {
            accessibility: MenuItem::with_id(
                MenuId::new("perm_accessibility"),
                menu_text(&accessibility, status.accessibility),
                !status.accessibility,
                None
            ),
            input_monitoring: MenuItem::with_id(
                MenuId::new("perm_input_monitoring"),
                menu_text(&input_monitoring, status.input_monitoring),
                !status.input_monitoring,
                None
            ),
            ns_menu,
        }
    }

    /// Styles the two lines: a dull grey label with a green granted word, or the
    /// whole line in red when the permission is missing.
    ///
    /// The label is built here and handed over, never re-derived inside
    /// `permissions`, so the plain title and the attributed one are the same
    /// string in every language.
    fn style(&self, status: crate::libs::permissions::PermissionStatus, strings: &TrayStrings) {
        use crate::libs::permissions::{ menu_text, style_menu_item, ACCESSIBILITY_TAG, INPUT_MONITORING_TAG };
        let accessibility = permission_words(strings, strings.accessibility);
        let input_monitoring = permission_words(strings, strings.input_monitoring);
        style_menu_item(
            self.ns_menu,
            ACCESSIBILITY_TAG,
            &menu_text(&accessibility, status.accessibility),
            status.accessibility.then_some(accessibility.granted_word)
        );
        style_menu_item(
            self.ns_menu,
            INPUT_MONITORING_TAG,
            &menu_text(&input_monitoring, status.input_monitoring),
            status.input_monitoring.then_some(input_monitoring.granted_word)
        );
    }

    fn apply(&self, status: crate::libs::permissions::PermissionStatus, strings: &TrayStrings) {
        use crate::libs::permissions::menu_text;
        let accessibility = permission_words(strings, strings.accessibility);
        let input_monitoring = permission_words(strings, strings.input_monitoring);
        self.accessibility.set_text(menu_text(&accessibility, status.accessibility));
        self.accessibility.set_enabled(!status.accessibility);
        self.input_monitoring.set_text(menu_text(&input_monitoring, status.input_monitoring));
        self.input_monitoring.set_enabled(!status.input_monitoring);
        // Setting the text above replaces the title, so the colours go on after.
        self.style(status, strings);
    }
}

/// Scales the alpha channel of an RGBA buffer in place to dim the icon.
///
/// Only every 4th byte is touched: RGB must survive untouched, because the tray
/// composites the icon over the taskbar and dimming the color channels would
/// recolor the icon rather than fade it. Fully transparent pixels stay
/// transparent, so the silhouette never grows.
fn fade_alpha(pixels: &mut [u8]) {
    for pixel in pixels.chunks_exact_mut(4) {
        pixel[3] = ((pixel[3] as f32) * MUTED_ICON_ALPHA).round() as u8;
    }
}

/// Both tray icon variants, decoded once at startup.
///
/// The embedded ICO is decoded a single time and the faded variant is derived
/// from the same pixel buffer, so toggling mute never re-decodes an image -
/// it just hands a cheap `Icon` clone back to the tray.
struct TrayIcons {
    normal: Icon,
    muted: Icon,
}

impl TrayIcons {
    fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let img = image::load_from_memory_with_format(
            EMBEDDED_ICON,
            image::ImageFormat::Ico
        ).map_err(|e| {
            crate::always_eprint!("❌ Failed to load embedded tray icon data: {}", e);
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Failed to load embedded icon: {}", e)
            )
        })?;

        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        let normal_pixels = rgba.into_raw();

        // Derive the muted variant by scaling only the alpha channel, leaving
        // RGB untouched. The tray composites against the taskbar background,
        // so reducing alpha is what actually reads as "dimmed" - darkening RGB
        // instead would just look like a different-colored icon.
        let mut muted_pixels = normal_pixels.clone();
        fade_alpha(&mut muted_pixels);

        let normal = Icon::from_rgba(normal_pixels, width, height).map_err(|e| {
            crate::always_eprint!("❌ Failed to create tray icon from embedded data: {}", e);
            e
        })?;
        let muted = Icon::from_rgba(muted_pixels, width, height).map_err(|e| {
            crate::always_eprint!("❌ Failed to create muted tray icon: {}", e);
            e
        })?;

        crate::always_print!("✅ Loaded embedded tray icon ({}x{}, normal + muted)", width, height);
        Ok(Self { normal, muted })
    }

    /// The variant matching the current sound state.
    fn for_sound_enabled(&self, enabled: bool) -> Icon {
        if enabled { self.normal.clone() } else { self.muted.clone() }
    }
}

/// macOS: where menu events wait for the tray loop (see `install_menu_event_handler`).
#[cfg(target_os = "macos")]
static MENU_EVENTS: std::sync::OnceLock<crossbeam_channel::Receiver<MenuEvent>> = std::sync::OnceLock::new();

/// What a macOS tray menu item needs on top of (or instead of) the tray loop.
#[cfg(target_os = "macos")]
#[derive(Debug, PartialEq, Eq)]
enum Handling {
    /// Acted on at once, and still queued for the tray loop.
    NowAndQueue,
    /// Acted on at once; the tray loop has nothing to do.
    NowOnly,
    /// Left to the tray loop.
    QueueOnly,
}

#[cfg(target_os = "macos")]
fn handling(id: &str) -> Handling {
    match id {
        "show" => Handling::NowAndQueue,
        "perm_accessibility" | "perm_input_monitoring" => Handling::NowOnly,
        _ => Handling::QueueOnly,
    }
}

/// Routes one menu event according to `handling`.
///
/// Split out from the handler so the routing can be tested without AppKit.
#[cfg(target_os = "macos")]
fn route_menu_event<E>(event: E, id: &str, act_now: impl FnOnce(), queue: impl FnOnce(E)) {
    match handling(id) {
        Handling::NowAndQueue => {
            act_now();
            queue(event);
        }
        Handling::NowOnly => act_now(),
        Handling::QueueOnly => queue(event),
    }
}

/// macOS: takes over the tray menu's events so "Show" and the two "Grant …" lines
/// can be handled on the main thread without the tray loop.
///
/// The loop that normally handles menu events runs inside the Dioxus virtual DOM,
/// which stops being polled when a hidden web view stops acknowledging UI updates
/// (see `native_window`). Then nothing would ever read the "Show" click, and the
/// only thing that could show the window is that same loop. The menu library
/// calls this handler on the main thread, from AppKit's menu action, so the
/// window is shown here directly, and the "Grant …" lines open System Settings
/// here too: they are most needed when the app is not behaving. With a handler set the library no longer fills
/// its own channel, so the events are queued on one of ours that
/// `handle_tray_events` reads instead.
#[cfg(target_os = "macos")]
fn install_menu_event_handler() {
    let (sender, receiver) = crossbeam_channel::unbounded::<MenuEvent>();
    if MENU_EVENTS.set(receiver).is_err() {
        return; // already installed
    }
    MenuEvent::set_event_handler(
        Some(move |event: MenuEvent| {
            let id = event.id.0.clone();
            route_menu_event(
                event,
                &id,
                || match id.as_str() {
                    "show" => {
                        if crate::libs::native_window::show_main_window() {
                            crate::libs::window_manager::WINDOW_MANAGER.set_visible(true);
                            crate::always_print!("🔼 Tray menu: window shown natively");
                        }
                    }
                    "perm_accessibility" => {
                        crate::always_print!("🔐 Tray menu: Grant Accessibility clicked");
                        crate::libs::permissions::open_accessibility_settings();
                    }
                    "perm_input_monitoring" => {
                        crate::always_print!("🔐 Tray menu: Grant Input Monitoring clicked");
                        crate::libs::permissions::open_input_monitoring_settings();
                    }
                    _ => {}
                },
                |event| {
                    let _ = sender.send(event);
                }
            );
        })
    );
}

/// The next menu event, if any, from wherever this platform delivers them.
fn next_menu_event() -> Option<MenuEvent> {
    #[cfg(target_os = "macos")]
    {
        MENU_EVENTS.get().and_then(|receiver| receiver.try_recv().ok())
    }
    #[cfg(not(target_os = "macos"))]
    {
        MenuEvent::receiver().try_recv().ok()
    }
}

/// The catalog for the language the app is set to right now.
///
/// The tray is not a Dioxus component, so `use_strings()` is not available to
/// it: the config writer is the same authority the Settings screen writes the
/// choice through, and reading it here is what lets `request_tray_update()` —
/// sent from Settings when the language changes — reach the menu.
fn tray_strings(config: &AppConfig) -> &'static TrayStrings {
    &crate::i18n::resolve(config.language).strings().tray
}

/// What the tray menu should say, and which checkmarks should be on.
///
/// Everything that decides the menu's content lives in [`tray_menu_state`],
/// apart from the tray: the tray needs a running event loop and real menu
/// objects, so nothing that builds one can be tested. [`TrayManager`] only
/// copies this onto the items that already exist.
struct TrayMenu {
    /// "Show Mechvibes": the catalog's phrase around the brand. `APP_NAME` is a
    /// constant, not copy, so the name itself is never translated.
    show: String,
    mute_sounds: &'static str,
    mute_mood: &'static str,
    github: &'static str,
    discord: &'static str,
    website: &'static str,
    exit: &'static str,
    /// The state the two mute entries show. It is a **checkmark, never a
    /// label**: rebuilding the menu to say "Unmute" made the label flip on
    /// every refresh, so the wording is fixed and the check carries the state.
    /// Only a language change moves these two labels.
    sounds_muted: bool,
    mood_muted: bool,
}

/// The menu for `strings` and one sound state.
///
/// Pure on purpose: no tray, no config, no platform call, so a test can pin both
/// the labels English has always shown and the fact that another language
/// produces different ones.
fn tray_menu_state(strings: &TrayStrings, sounds_enabled: bool, mood_muted: bool) -> TrayMenu {
    TrayMenu {
        show: fill(strings.show, &[&APP_NAME]),
        mute_sounds: strings.mute_sounds,
        mute_mood: strings.mute_mood,
        github: strings.github,
        discord: strings.discord,
        website: strings.website,
        exit: strings.exit,
        // `enable_sound` is the positive flag, so muted is its inverse.
        sounds_muted: !sounds_enabled,
        mood_muted,
    }
}

pub struct TrayManager {
    tray_icon: TrayIcon,
    /// The items are held past menu construction so their labels can be replaced
    /// with `set_text` when the language changes, without building a second
    /// `Menu`: the [`MenuId`]s below are what `handle_tray_events` matches on.
    show_item: MenuItem,
    /// Held so the **checkmark** can be toggled in place. Rebuilding the whole
    /// menu to change one item made the label flip between "Mute"/"Unmute";
    /// holding the item lets the label stay fixed with respect to the state and
    /// show it as a checkmark instead.
    mute_item: CheckMenuItem,
    /// Mood's own mute, the same one as the button on the Mood page.
    mood_mute_item: CheckMenuItem,
    github_item: MenuItem,
    discord_item: MenuItem,
    website_item: MenuItem,
    exit_item: MenuItem,
    /// macOS: the two permission lines, so a language change relabels them from
    /// `update_menu` too. The permission watcher holds a clone of the same
    /// handles, which is what keeps them current while the window is hidden.
    #[cfg(target_os = "macos")]
    permissions: PermissionItems,
    icons: TrayIcons,
}

impl TrayManager {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Load current config to determine sound state
        let config = crate::state::config_writer::current();
        let strings = tray_strings(&config);
        let menu_state = tray_menu_state(strings, config.enable_sound, config.ambiance_is_muted);
        // `enable_sound` is the positive flag, so muted is its inverse. The
        // menu item is checked when muted.
        let muted = menu_state.sounds_muted;

        // Create the tray menu with specific IDs. These IDs are the contract with
        // `handle_tray_events`, so the menu is built exactly once and only ever
        // re-labelled in place (see `update_menu`).
        let show_item = MenuItem::with_id(MenuId::new("show"), &menu_state.show, true, None);
        let separator1 = PredefinedMenuItem::separator();

        // Sound control section. The label names the action, not the state: the
        // checkmark carries the state, and only a language change moves the
        // wording.
        let mute_item = CheckMenuItem::with_id(
            MenuId::new("toggle_mute"),
            menu_state.mute_sounds,
            true,
            muted,
            None
        );
        let mood_mute_item = CheckMenuItem::with_id(
            MenuId::new("toggle_mood_mute"),
            menu_state.mute_mood,
            true,
            menu_state.mood_muted,
            None
        );
        let separator2 = PredefinedMenuItem::separator();

        // External links section
        let github_item = MenuItem::with_id(MenuId::new("github"), menu_state.github, true, None);
        let discord_item = MenuItem::with_id(MenuId::new("discord"), menu_state.discord, true, None);
        let website_item = MenuItem::with_id(MenuId::new("website"), menu_state.website, true, None);
        let separator = PredefinedMenuItem::separator();

        let exit_item = MenuItem::with_id(MenuId::new("exit"), menu_state.exit, true, None);

        // Create the menu with the items
        let menu = Menu::with_items(
            &[
                &show_item,
                &separator1,
                &mute_item,
                &mood_mute_item,
                &separator2,
                &github_item,
                &discord_item,
                &website_item,
                &separator,
                &exit_item,
            ]
        )?;

        // macOS: show whether Accessibility and Input Monitoring are granted,
        // just after the sound items and their separator. Inserted rather than
        // listed above so the menu on Windows and Linux is built exactly as
        // before, and placed relative to the last sound item so it stays right
        // if items are added above it.
        //
        // The pair is kept on the manager as well as handed to the watcher: a
        // language change relabels it from `update_menu`, and the watcher needs
        // its own handles to keep the lines current while the window is hidden.
        #[cfg(target_os = "macos")]
        let permission_items = {
            use crate::libs::permissions::{ self, ACCESSIBILITY_TAG, INPUT_MONITORING_TAG };
            use tray_icon::menu::ContextMenu;

            let ns_menu = menu.ns_menu() as *mut std::ffi::c_void;
            let status = permissions::current();
            let items = PermissionItems::new(status, ns_menu, strings);

            let mood_mute_position = menu
                .items()
                .iter()
                .position(|item| item.id() == mood_mute_item.id())
                .ok_or("the Mood mute item is missing from the tray menu")?;
            let first = mood_mute_position + 2; // after the last sound item and its separator
            menu.insert(&items.accessibility, first)?;
            menu.insert(&items.input_monitoring, first + 1)?;
            menu.insert(&PredefinedMenuItem::separator(), first + 2)?;
            // Tagged now, while their positions are known, so that everything
            // after this finds them by tag and never by position.
            permissions::tag_menu_item(ns_menu, first, ACCESSIBILITY_TAG);
            permissions::tag_menu_item(ns_menu, first + 1, INPUT_MONITORING_TAG);
            items.style(status, strings);

            // Keep the lines current. This runs on the main dispatch queue, not
            // in the tray loop inside the web view, so it keeps working while the
            // window is hidden and that loop is frozen. The catalog is read
            // inside the callback, so a grant that arrives after a language
            // change is drawn in the language now in use.
            let watched = items.clone();
            permissions::watch(move |status| {
                watched.apply(status, tray_strings(&crate::state::config_writer::current()));
                crate::always_print!(
                    "🔐 Tray permissions: Accessibility {}, Input Monitoring {}",
                    if status.accessibility { "granted" } else { "missing" },
                    if status.input_monitoring { "granted" } else { "missing" }
                );
            });

            items
        };

        #[cfg(target_os = "macos")]
        install_menu_event_handler();

        // Decode both icon variants once; muting only swaps between them.
        let icons = TrayIcons::load()?;

        // Build the tray icon already showing the persisted state, so a config
        // that was muted before launch comes up dimmed rather than flashing the
        // normal icon until the first toggle.
        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip(APP_NAME)
            .with_icon(icons.for_sound_enabled(!muted))
            .build()?;

        Ok(TrayManager {
            tray_icon,
            show_item,
            mute_item,
            mood_mute_item,
            github_item,
            discord_item,
            website_item,
            exit_item,
            #[cfg(target_os = "macos")]
            permissions: permission_items,
            icons,
        })
    }

    /// Syncs the tray with the persisted state: the labels follow the chosen
    /// language and the checkmarks follow the sound state.
    ///
    /// Called after every path that can flip `enable_sound` (tray item, the
    /// Settings toggle, Reset to Defaults, and the Ctrl+Alt+M hotkey) and after a
    /// language change, which Settings announces with `request_tray_update()`. So
    /// the config on disk stays the single source of truth for what the tray
    /// shows.
    ///
    /// **The items are re-labelled in place; no `Menu`, no `TrayIcon` and no
    /// [`MenuId`] is built here.** The IDs are what `handle_tray_events` matches
    /// on, and on Linux the glib tray object has to survive an update, so a
    /// second `Menu` would leave the handlers watching a menu nobody shows.
    pub fn update_menu(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let config = crate::state::config_writer::current();
        let strings = tray_strings(&config);
        let menu_state = tray_menu_state(strings, config.enable_sound, config.ambiance_is_muted);
        let muted = menu_state.sounds_muted;

        // Mutate the existing items instead of rebuilding the menu. The mute
        // labels never follow the checkmarks: the same word covers on and off,
        // and the checkmark is what says which.
        self.show_item.set_text(&menu_state.show);
        self.mute_item.set_text(menu_state.mute_sounds);
        self.mood_mute_item.set_text(menu_state.mute_mood);
        self.github_item.set_text(menu_state.github);
        self.discord_item.set_text(menu_state.discord);
        self.website_item.set_text(menu_state.website);
        self.exit_item.set_text(menu_state.exit);

        self.mute_item.set_checked(muted);
        // Mood's mute is saved the moment it changes, from the tray or the
        // Mood page, so the config is the source here too.
        self.mood_mute_item.set_checked(menu_state.mood_muted);

        // macOS: the two permission rows follow the language as well. Their text
        // is built from the same catalog, and the colours have to be put back
        // after it, so they go through `permissions` in one go.
        #[cfg(target_os = "macos")]
        self.permissions.apply(crate::libs::permissions::current(), strings);

        self.tray_icon.set_icon(Some(self.icons.for_sound_enabled(!muted)))?;

        crate::always_print!(
            "🔄 Tray updated: sounds {}, language {}",
            if muted { "muted" } else { "on" },
            crate::i18n::resolve(config.language).tag()
        );

        Ok(())
    }
}

pub fn handle_tray_events() -> Option<TrayMessage> {
    // Handle tray icon click events
    if let Ok(event) = TrayIconEvent::receiver().try_recv() {
        match event {
            TrayIconEvent::Click {
                id: _,
                position: _,
                rect: _,
                button,
                button_state,
            } => {
                // Only respond to left button release (avoids duplicate events)
                if button == tray_icon::MouseButton::Left && button_state == tray_icon::MouseButtonState::Up {
                    // Double-click detection: 500ms window
                    let now = Instant::now();
                    let mut last_click = LAST_CLICK_TIME.lock().unwrap();

                    let is_double_click = if let Some(last_time) = *last_click {
                        now.duration_since(last_time).as_millis() < 500
                    } else {
                        false
                    };

                    *last_click = Some(now);
                    drop(last_click);

                    if is_double_click {
                        crate::always_print!("🔼 Tray icon double-clicked - showing window");
                        return Some(TrayMessage::Show);
                    }
                }
            }
            _ => {
                // Silently ignore other events (Move, Enter, Leave, etc.)
            }
        }
    }

    // Handle menu events
    if let Some(event) = next_menu_event() {
        crate::always_print!("🖱️ Tray menu event received: {:?}", event);
        match event.id.0.as_str() {
            "show" => {
                crate::always_print!("🔼 Tray menu: Show {} clicked", APP_NAME);
                return Some(TrayMessage::Show);
            }
            "toggle_mute" => {
                crate::always_print!("🔇 Tray menu: Toggle Mute clicked");
                return Some(TrayMessage::ToggleMute);
            }
            "toggle_mood_mute" => {
                crate::always_print!("🔇 Tray menu: Mute Mood clicked");
                return Some(TrayMessage::ToggleMoodMute);
            }
            "github" => {
                crate::always_print!("🐙 Tray menu: GitHub Repository clicked");
                return Some(TrayMessage::OpenGitHub);
            }
            "discord" => {
                crate::always_print!("💬 Tray menu: Discord Community clicked");
                return Some(TrayMessage::OpenDiscord);
            }
            "website" => {
                crate::always_print!("🌐 Tray menu: Official Website clicked");
                return Some(TrayMessage::OpenWebsite);
            }
            "exit" => {
                crate::always_print!("❌ Tray menu: Exit clicked");
                return Some(TrayMessage::Exit);
            }
            _ => {
                crate::always_print!("❓ Tray menu: Unknown menu item: {}", event.id.0);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Language;

    #[cfg(target_os = "macos")]
    fn route(id: &'static str) -> (bool, Vec<&'static str>) {
        let acted = std::cell::Cell::new(false);
        let queued = std::cell::RefCell::new(Vec::new());
        route_menu_event(id, id, || acted.set(true), |e| queued.borrow_mut().push(e));
        (acted.get(), queued.into_inner())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn show_is_acted_on_at_once_and_still_queued_for_the_tray_loop() {
        assert_eq!(route("show"), (true, vec!["show"]), "Show must not wait for the tray loop");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn the_grant_lines_are_acted_on_at_once_and_not_queued() {
        // Queuing them too would open System Settings a second time when the
        // tray loop got to them.
        for id in ["perm_accessibility", "perm_input_monitoring"] {
            assert_eq!(route(id), (true, vec![]), "{id}");
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn other_menu_items_are_only_queued() {
        for id in ["toggle_mute", "toggle_mood_mute", "github", "discord", "website", "exit"] {
            assert_eq!(route(id), (false, vec![id]), "{id} must reach the tray loop unchanged");
        }
    }

    #[test]
    fn fading_dims_alpha_and_leaves_color_channels_intact() {
        // Opaque red, then half-alpha green.
        let mut pixels = vec![255, 0, 0, 255, 0, 255, 0, 128];
        fade_alpha(&mut pixels);

        // RGB untouched: a dimmed icon must keep its colors, only its opacity
        // changes. Darkening RGB instead would look like a recolored icon.
        assert_eq!([pixels[0], pixels[1], pixels[2]], [255, 0, 0]);
        assert_eq!([pixels[4], pixels[5], pixels[6]], [0, 255, 0]);

        assert_eq!(pixels[3], (255.0 * MUTED_ICON_ALPHA).round() as u8);
        assert_eq!(pixels[7], (128.0 * MUTED_ICON_ALPHA).round() as u8);
    }

    #[test]
    fn fading_keeps_transparent_pixels_transparent() {
        // A transparent pixel gaining alpha would thicken the silhouette.
        let mut pixels = vec![0, 0, 0, 0];
        fade_alpha(&mut pixels);
        assert_eq!(pixels[3], 0);
    }

    #[test]
    fn fading_actually_reduces_opacity() {
        // Guards against the multiplier being set to 1.0 (or above), which
        // would make the muted icon indistinguishable from the normal one.
        assert!(MUTED_ICON_ALPHA > 0.0 && MUTED_ICON_ALPHA < 1.0);

        let mut pixels = vec![10, 20, 30, 255];
        fade_alpha(&mut pixels);
        assert!(pixels[3] < 255);
    }

    fn menu_in(language: Language) -> TrayMenu {
        tray_menu_state(&language.strings().tray, true, false)
    }

    /// The labels the tray showed before this migration, one per item, kept
    /// byte for byte: English is the language the app already ships in, so this
    /// is a move into the catalog and not a copy edit. A translator touching the
    /// English column fails here, next to the menu.
    #[test]
    fn the_menu_state_is_the_english_menu_users_already_had() {
        let menu = menu_in(Language::En);
        assert_eq!(menu.show, format!("Show {}", APP_NAME));
        assert_eq!(menu.mute_sounds, "Mute sounds");
        assert_eq!(menu.mute_mood, "Mute Mood");
        assert_eq!(menu.github, "GitHub Repository");
        assert_eq!(menu.discord, "Discord Community");
        assert_eq!(menu.website, "Official Website");
        assert_eq!(menu.exit, "Exit");
        assert!(!menu.sounds_muted, "sounds are on, so the mute entry is unchecked");
        assert!(!menu.mood_muted);
    }

    /// A language change has to reach every item, so no label may keep the
    /// English text. The brand is the exception the catalog allows: it is filled
    /// in at the call site, so `show` differs by its phrase, not by the name.
    #[test]
    fn another_language_relabels_every_item() {
        let english = menu_in(Language::En);
        for language in [Language::Zh, Language::Es] {
            let other = menu_in(language);
            for (key, en, translated) in [
                ("show", english.show.as_str(), other.show.as_str()),
                ("mute_sounds", english.mute_sounds, other.mute_sounds),
                ("mute_mood", english.mute_mood, other.mute_mood),
                ("github", english.github, other.github),
                ("discord", english.discord, other.discord),
                ("website", english.website, other.website),
                ("exit", english.exit, other.exit),
            ] {
                assert_ne!(en, translated, "tray.{key} still reads English in {}", language.tag());
            }
            assert!(
                other.show.contains(APP_NAME),
                "the brand has to survive the translation: {:?}",
                other.show
            );
        }
    }

    /// The mute entries carry their state in the checkmark. A label that followed
    /// it is the bug the held items were introduced to fix, so the same label has
    /// to come out for both states and only the checkmark may move.
    #[test]
    fn the_mute_labels_never_follow_the_sound_state() {
        let strings = &Language::En.strings().tray;
        let on = tray_menu_state(strings, true, false);
        let off = tray_menu_state(strings, false, true);

        assert_eq!(on.mute_sounds, off.mute_sounds);
        assert_eq!(on.mute_mood, off.mute_mood);
        assert_eq!((on.sounds_muted, on.mood_muted), (false, false));
        assert_eq!((off.sounds_muted, off.mood_muted), (true, true));
    }

    /// A relabel updates the items that exist and nothing else.
    ///
    /// The IDs `handle_tray_events` matches on belong to the `Menu` built in
    /// `new()`, and on Linux the glib tray object has to survive the update, so
    /// `update_menu` may not build a second menu. No runtime test can see that
    /// (it needs an event loop and a live menu), so this reads the source the
    /// same way `window_controller` checks its tray release.
    #[test]
    fn relabelling_reuses_the_menu_and_its_ids() {
        const SOURCE: &str = include_str!("tray.rs");
        let runtime = SOURCE.split("#[cfg(test)]").next().expect("the tests come last");
        let relabel = runtime
            .split("pub fn update_menu")
            .nth(1)
            .expect("update_menu must exist: it is the only path that relabels the tray")
            .split("pub fn handle_tray_events")
            .next()
            .expect("handle_tray_events follows update_menu");

        assert!(relabel.contains("set_text"), "a language change has to relabel the live items");
        for rebuilt in ["Menu::", "TrayIconBuilder", "MenuId::new"] {
            assert!(
                !relabel.contains(rebuilt),
                "update_menu must not build `{rebuilt}`: the ids and the tray object have to \
                 survive a relabel"
            );
        }
        for id in ["show", "toggle_mute", "toggle_mood_mute", "github", "discord", "website", "exit"] {
            let created = format!("MenuId::new(\"{id}\")");
            assert_eq!(
                runtime.matches(&created).count(),
                1,
                "`{id}` must be created once, in `new()`, and never re-created per relabel"
            );
        }
    }
}
