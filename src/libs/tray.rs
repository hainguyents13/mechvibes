use tray_icon::{
    Icon,
    menu::{ CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, MenuId },
    TrayIcon,
    TrayIconBuilder,
    TrayIconEvent,
};
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
    OpenGitHub,
    OpenDiscord,
    OpenWebsite,
}

/// The two macOS permission lines in the tray menu.
///
/// Each shows "X: Granted" (nothing to do) or a red "Grant X…" (clickable, opens
/// the matching System Settings pane). Owned by the permission watcher after the
/// menu is built, so the text can change in place when a permission is granted
/// or revoked.
#[cfg(target_os = "macos")]
struct PermissionItems {
    accessibility: MenuItem,
    input_monitoring: MenuItem,
    /// The native menu the tray shows. The menu library does not expose its
    /// native items, so the colours are applied through this, finding each line
    /// by its tag (see `permissions::style_menu_item`). It lives as long as the
    /// tray does.
    ns_menu: *mut std::ffi::c_void,
}

#[cfg(target_os = "macos")]
impl PermissionItems {
    fn new(
        status: crate::libs::permissions::PermissionStatus,
        ns_menu: *mut std::ffi::c_void
    ) -> Self {
        use crate::libs::permissions::menu_text;
        Self {
            accessibility: MenuItem::with_id(
                MenuId::new("perm_accessibility"),
                menu_text("Accessibility", status.accessibility),
                !status.accessibility,
                None
            ),
            input_monitoring: MenuItem::with_id(
                MenuId::new("perm_input_monitoring"),
                menu_text("Input Monitoring", status.input_monitoring),
                !status.input_monitoring,
                None
            ),
            ns_menu,
        }
    }

    /// Styles the two lines: a dull grey label with a green "Granted", or the
    /// whole line in red when the permission is missing.
    fn style(&self, status: crate::libs::permissions::PermissionStatus) {
        use crate::libs::permissions::{ style_menu_item, ACCESSIBILITY_TAG, INPUT_MONITORING_TAG };
        style_menu_item(self.ns_menu, ACCESSIBILITY_TAG, "Accessibility", status.accessibility);
        style_menu_item(self.ns_menu, INPUT_MONITORING_TAG, "Input Monitoring", status.input_monitoring);
    }

    fn apply(&self, status: crate::libs::permissions::PermissionStatus) {
        use crate::libs::permissions::menu_text;
        self.accessibility.set_text(menu_text("Accessibility", status.accessibility));
        self.accessibility.set_enabled(!status.accessibility);
        self.input_monitoring.set_text(menu_text("Input Monitoring", status.input_monitoring));
        self.input_monitoring.set_enabled(!status.input_monitoring);
        // Setting the text above replaces the title, so the colours go on after.
        self.style(status);
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

pub struct TrayManager {
    tray_icon: TrayIcon,
    /// Kept alive past menu construction so the checkmark can be toggled in
    /// place. Rebuilding the whole menu to change one item made the label flip
    /// between "Mute"/"Unmute"; holding the item lets the label stay fixed and
    /// the state show as a checkmark instead.
    mute_item: CheckMenuItem,
    icons: TrayIcons,
}

impl TrayManager {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Load current config to determine sound state
        let config = crate::state::config_writer::current();
        // `enable_sound` is the positive flag, so muted is its inverse. The
        // menu item is checked when muted.
        let muted = !config.enable_sound;

        // Create the tray menu with specific IDs
        let show_item = MenuItem::with_id(
            MenuId::new("show"),
            &format!("Show {}", APP_NAME),
            true,
            None
        );
        let separator1 = PredefinedMenuItem::separator();

        // Sound control section. Fixed label; the checkmark carries the state.
        let mute_item = CheckMenuItem::with_id(
            MenuId::new("toggle_mute"),
            "Mute sounds",
            true,
            muted,
            None
        );
        let separator2 = PredefinedMenuItem::separator();

        // External links section
        let github_item = MenuItem::with_id(MenuId::new("github"), "GitHub Repository", true, None);
        let discord_item = MenuItem::with_id(
            MenuId::new("discord"),
            "Discord Community",
            true,
            None
        );
        let website_item = MenuItem::with_id(
            MenuId::new("website"),
            "Official Website",
            true,
            None
        );
        let separator = PredefinedMenuItem::separator();

        let exit_item = MenuItem::with_id(MenuId::new("exit"), "Exit", true, None);

        // Create the menu with the items
        let menu = Menu::with_items(
            &[
                &show_item,
                &separator1,
                &mute_item,
                &separator2,
                &github_item,
                &discord_item,
                &website_item,
                &separator,
                &exit_item,
            ]
        )?;

        // macOS: show whether Accessibility and Input Monitoring are granted,
        // just after the mute item and its separator. Inserted rather than listed
        // above so the menu on Windows and Linux is built exactly as before, and
        // placed relative to the mute item so it stays right if items are added
        // above it.
        #[cfg(target_os = "macos")]
        {
            use crate::libs::permissions::{ self, ACCESSIBILITY_TAG, INPUT_MONITORING_TAG };
            use tray_icon::menu::ContextMenu;

            let ns_menu = menu.ns_menu() as *mut std::ffi::c_void;
            let status = permissions::current();
            let items = PermissionItems::new(status, ns_menu);

            let mute_position = menu
                .items()
                .iter()
                .position(|item| item.id() == mute_item.id())
                .ok_or("the mute item is missing from the tray menu")?;
            let first = mute_position + 2; // after the mute item and its separator
            menu.insert(&items.accessibility, first)?;
            menu.insert(&items.input_monitoring, first + 1)?;
            menu.insert(&PredefinedMenuItem::separator(), first + 2)?;
            // Tagged now, while their positions are known, so that everything
            // after this finds them by tag and never by position.
            permissions::tag_menu_item(ns_menu, first, ACCESSIBILITY_TAG);
            permissions::tag_menu_item(ns_menu, first + 1, INPUT_MONITORING_TAG);
            items.style(status);

            // Keep the lines current. This runs on the main dispatch queue, not
            // in the tray loop inside the web view, so it keeps working while the
            // window is hidden and that loop is frozen.
            permissions::watch(move |status| {
                items.apply(status);
                crate::always_print!(
                    "🔐 Tray permissions: Accessibility {}, Input Monitoring {}",
                    if status.accessibility { "granted" } else { "missing" },
                    if status.input_monitoring { "granted" } else { "missing" }
                );
            });
        }

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
            mute_item,
            icons,
        })
    }

    /// Syncs the tray checkmark and icon with the persisted sound state.
    ///
    /// Called after every path that can flip `enable_sound` (tray item, the
    /// Settings toggle, Reset to Defaults, and the Ctrl+Alt+M hotkey), so the
    /// config on disk stays the single source of truth for what the tray shows.
    pub fn update_menu(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let config = crate::state::config_writer::current();
        let enabled = config.enable_sound;

        // Mutate the existing item instead of rebuilding the menu: the label is
        // fixed, only the checkmark moves.
        self.mute_item.set_checked(!enabled);
        self.tray_icon.set_icon(Some(self.icons.for_sound_enabled(enabled)))?;

        crate::always_print!("🔄 Tray updated: sounds {}", if enabled { "on" } else { "muted" });

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
        for id in ["toggle_mute", "github", "discord", "website", "exit"] {
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
}
