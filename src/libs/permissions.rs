//! The macOS privacy permissions MechvibesDX needs, read without prompting.
//!
//! Key capture needs two grants in System Settings > Privacy & Security:
//!
//! - **Accessibility**: used while the MechvibesDX window is focused.
//! - **Input Monitoring**: used while it is minimized or another app is in front.
//!
//! The tray menu shows both ("Accessibility: Granted" / "Grant Accessibility…")
//! so a silent app can be diagnosed from the menu bar. Reading the state here
//! never shows a system prompt; only the explicit "Grant …" menu items do that.
//!
//! The focused-window listener waits here for Accessibility and starts as soon
//! as it is granted (`wait_for_accessibility`), because Accessibility trust is
//! live, so that grant needs no restart. Input Monitoring is not: a process that
//! started without it cannot create its global event tap later, so the app has to
//! be relaunched after that grant, and nothing here waits for it. The tray lines
//! are kept current by `watch`, which runs on the main thread's dispatch queue
//! and so keeps working while the window is hidden, when the web view and the
//! tasks inside it can freeze (see `native_window`).
#![cfg(target_os = "macos")]

use block2::RcBlock;
use macos_accessibility_client::accessibility::application_is_trusted;
use objc2::msg_send;
use objc2::rc::{ Allocated, Retained };
use objc2::runtime::AnyClass;
use objc2::runtime::AnyObject;
use objc2_foundation::{ NSRange, NSString };
use std::cell::RefCell;
use std::ffi::c_void;
use std::time::Duration;

// `CGPreflightListenEventAccess` and `CGRequestListenEventAccess` (macOS 10.15+)
// are the Input Monitoring counterparts of the Accessibility trust check. The
// app's minimum macOS is 11, so they always exist.
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGPreflightListenEventAccess() -> bool;
    fn CGRequestListenEventAccess() -> bool;
}

// libdispatch, for running a check on the main thread without the web view.
#[link(name = "System", kind = "dylib")]
unsafe extern "C" {
    static _dispatch_main_q: c_void;
    fn dispatch_time(when: u64, delta_ns: i64) -> u64;
    fn dispatch_after_f(
        when: u64,
        queue: *const c_void,
        context: *mut c_void,
        work: extern "C" fn(*mut c_void)
    );
    fn pthread_main_np() -> i32;
}

/// `DISPATCH_TIME_NOW`.
const DISPATCH_TIME_NOW: u64 = 0;

// The AppKit constants that name the attributes of an attributed string.
#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {
    static NSForegroundColorAttributeName: *mut AnyObject;
    static NSFontAttributeName: *mut AnyObject;
}

/// The word that is picked out in green on a granted line.
const GRANTED: &str = "Granted";

/// How often a missing permission is re-checked, and how often the tray lines
/// are refreshed.
const CHECK_INTERVAL: Duration = Duration::from_secs(1);

/// `NSMenuItem.tag` values that mark the two permission lines, so they are found
/// by what they are and not by where they sit, which changes as soon as anything
/// is added above them. "MV" in the high bytes keeps them clear of any other
/// tag in the menu.
pub const ACCESSIBILITY_TAG: isize = 0x4D56_0001;
pub const INPUT_MONITORING_TAG: isize = 0x4D56_0002;

/// Deep links into the matching System Settings pane.
const ACCESSIBILITY_PANE: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";
const INPUT_MONITORING_PANE: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent";

/// Which of the two grants this process currently holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PermissionStatus {
    pub accessibility: bool,
    pub input_monitoring: bool,
}

/// Reads both grants without prompting.
pub fn current() -> PermissionStatus {
    PermissionStatus {
        accessibility: application_is_trusted(),
        input_monitoring: input_monitoring_granted(),
    }
}

fn input_monitoring_granted() -> bool {
    // SAFETY: a plain query with no arguments; it only reads the TCC state.
    unsafe { CGPreflightListenEventAccess() }
}

/// Blocks until `is_granted` returns true, calling `wait` between checks.
/// Returns whether it had to wait at all.
///
/// Both are parameters so the logic can be tested without sleeping or touching
/// the system's permissions.
pub fn wait_until_granted(mut is_granted: impl FnMut() -> bool, mut wait: impl FnMut()) -> bool {
    let mut waited = false;
    while !is_granted() {
        waited = true;
        wait();
    }
    waited
}

/// Blocks the calling thread until Accessibility is granted, checking once a
/// second without prompting. Returns at once if it already is.
///
/// Accessibility trust is live: a process that is granted it while running sees
/// it on the next check, so the focused-window listener can start without a
/// restart.
pub fn wait_for_accessibility() {
    wait_until_granted(application_is_trusted, || std::thread::sleep(CHECK_INTERVAL));
}

/// Remembers the last status seen. Returns the new status when it differs from
/// that one (and so should be acted on), and nothing when it is unchanged.
fn observe(last: &mut Option<PermissionStatus>, now: PermissionStatus) -> Option<PermissionStatus> {
    if *last == Some(now) {
        None
    } else {
        *last = Some(now);
        Some(now)
    }
}

struct Watcher {
    last: Option<PermissionStatus>,
    on_change: Box<dyn FnMut(PermissionStatus)>,
}

// Main thread only: the callback owns AppKit-facing menu items, which are not
// `Send`, and the checks run on the main queue.
thread_local! {
    static WATCHER: RefCell<Option<Watcher>> = const { RefCell::new(None) };
}

/// Calls `on_change` with the new status whenever Accessibility or Input
/// Monitoring changes, checking once a second from now on.
///
/// The check runs on the main thread's dispatch queue, which keeps being served
/// while the window is hidden. The tasks inside the web view do not: they can
/// freeze, and a poll living there would stop exactly when someone opens the
/// tray to find out why the app is silent. Must be called on the main thread;
/// anywhere else it logs and does nothing.
pub fn watch(on_change: impl FnMut(PermissionStatus) + 'static) {
    // SAFETY: a plain query with no arguments.
    if unsafe { pthread_main_np() } != 1 {
        crate::always_eprint!("❌ Permission watcher must be started on the main thread; not started");
        return;
    }
    WATCHER.with(|watcher| {
        *watcher.borrow_mut() = Some(Watcher {
            last: Some(current()),
            on_change: Box::new(on_change),
        });
    });
    schedule_next_check();
}

fn schedule_next_check() {
    // SAFETY: schedules a plain C function on the main queue; it takes no context.
    unsafe {
        let when = dispatch_time(DISPATCH_TIME_NOW, CHECK_INTERVAL.as_nanos() as i64);
        dispatch_after_f(when, &raw const _dispatch_main_q, std::ptr::null_mut(), check_now);
    }
}

extern "C" fn check_now(_: *mut c_void) {
    WATCHER.with(|watcher| {
        if let Some(watcher) = watcher.borrow_mut().as_mut() {
            if let Some(changed) = observe(&mut watcher.last, current()) {
                (watcher.on_change)(changed);
            }
        }
    });
    schedule_next_check();
}

/// The tray item text for one permission: a status line when granted, and the
/// action to take when it is not.
pub fn menu_text(name: &str, granted: bool) -> String {
    if granted { format!("{name}: Granted") } else { format!("Grant {name}…") }
}

/// A colour as 0.0-1.0 sRGB channels.
type Rgb = (f64, f64, f64);

const fn hex(r: u8, g: u8, b: u8) -> Rgb {
    (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0)
}

/// A colour with one value for light menus and one for dark.
struct Pair {
    light: Rgb,
    dark: Rgb,
}

// The three colours of the permission lines. No single colour is readable on
// both a light menu and a dark one, so each is a pair. Both halves clear 4.5:1
// (the WCAG minimum for body text) against a plausible menu background, which
// the system colours they replace did not: `systemGreenColor` measured 1.8:1 on
// a light menu, `systemRedColor` 2.9:1 and 2.5:1 on light and dark, and the grey
// label 2.6:1.
//
// The dark red is a lighter coral than a true red on purpose: red is a dark
// colour, and a saturated one cannot reach 4.5:1 on a dark menu.
const LABEL: Pair = Pair { light: hex(0x55, 0x55, 0x55), dark: hex(0xD2, 0xD2, 0xD2) };
const GRANTED_GREEN: Pair = Pair { light: hex(0x0F, 0x6B, 0x2A), dark: hex(0x5C, 0xE6, 0x7A) };
const MISSING_RED: Pair = Pair { light: hex(0xC4, 0x00, 0x1A), dark: hex(0xFF, 0xA8, 0xA0) };

fn srgb_color((r, g, b): Rgb) -> Option<Retained<AnyObject>> {
    let class = AnyClass::get(c"NSColor")?;
    // SAFETY: a class method taking four CGFloats and returning an NSColor.
    unsafe { msg_send![class, colorWithSRGBRed: r, green: g, blue: b, alpha: 1.0f64] }
}

/// Whether `appearance` is one of the dark ones ("NSAppearanceNameDarkAqua", the
/// vibrant dark, and the high-contrast dark variants all contain "Dark").
fn is_dark_appearance(appearance: *mut AnyObject) -> bool {
    if appearance.is_null() {
        return false;
    }
    // SAFETY: `appearance` is the live NSAppearance AppKit passed to the block.
    let name: Option<Retained<NSString>> = unsafe { msg_send![appearance, name] };
    name.is_some_and(|name| name.to_string().contains("Dark"))
}

/// A colour that is `pair.light` in a light menu and `pair.dark` in a dark one.
///
/// It is a dynamic `NSColor`: AppKit calls the block with the appearance it is
/// drawing in, every time it draws, so the colour is right for the menu it is
/// actually in and follows a switch between light and dark while the app runs,
/// with nothing here having to guess the appearance or watch for changes.
fn adaptive_color(pair: &'static Pair) -> Option<Retained<AnyObject>> {
    let class = AnyClass::get(c"NSColor")?;
    let provider = RcBlock::new(move |appearance: *mut AnyObject| -> *mut AnyObject {
        let rgb = if is_dark_appearance(appearance) { pair.dark } else { pair.light };
        match srgb_color(rgb) {
            // A block returns an autoreleased object, so ownership passes to the caller.
            Some(color) => Retained::autorelease_return(color),
            None => std::ptr::null_mut(),
        }
    });
    // SAFETY: a class method taking an optional name and a block; NSColor copies the block.
    unsafe { msg_send![class, colorWithName: std::ptr::null::<AnyObject>(), dynamicProvider: &*provider] }
}

/// The part of a granted line that is not "Granted".
fn label_color() -> Option<Retained<AnyObject>> {
    adaptive_color(&LABEL)
}

/// The word "Granted".
fn granted_color() -> Option<Retained<AnyObject>> {
    adaptive_color(&GRANTED_GREEN)
}

/// The whole "Grant …" line of a missing permission.
fn missing_color() -> Option<Retained<AnyObject>> {
    adaptive_color(&MISSING_RED)
}

/// The font every other menu item is drawn in.
///
/// An attributed title with no font set is drawn in AppKit's default text font
/// (Helvetica 12), not the menu font, so the menu font has to be asked for. Size
/// 0 means "the standard menu size", which keeps this in step with the OS.
fn menu_font() -> Option<Retained<AnyObject>> {
    let class = AnyClass::get(c"NSFont")?;
    // SAFETY: a class method taking a CGFloat and returning an NSFont.
    unsafe { msg_send![class, menuFontOfSize: 0.0f64] }
}

/// The UTF-16 `(location, length)` of the word "Granted" within the granted line
/// for `name`, which is how AppKit addresses a range of an attributed string.
pub fn granted_word_range(name: &str) -> (usize, usize) {
    let text = menu_text(name, true);
    let prefix = text.len() - GRANTED.len();
    (text[..prefix].encode_utf16().count(), GRANTED.encode_utf16().count())
}

/// Marks the menu item at `index` with `tag`, so that `style_menu_item` can find
/// it later wherever it has moved to. Call it right after inserting the item,
/// while its index is known.
///
/// Must run on the main thread, like every menu change.
pub fn tag_menu_item(ns_menu: *mut c_void, index: usize, tag: isize) {
    if ns_menu.is_null() {
        return;
    }
    let menu = ns_menu as *mut AnyObject;
    // SAFETY: `menu` is the live NSMenu owned by the tray; the index is checked
    // against the item count first.
    unsafe {
        let count: isize = msg_send![menu, numberOfItems];
        if index as isize >= count {
            return;
        }
        let item: *mut AnyObject = msg_send![menu, itemAtIndex: index as isize];
        if !item.is_null() {
            let _: () = msg_send![item, setTag: tag];
        }
    }
}

/// Styles the tray menu item marked with `tag` for `name` ("Accessibility" or
/// "Input Monitoring").
///
/// All text uses the standard menu font, the same as every other menu item.
///
/// - **Granted**: "Accessibility: Granted" with the label in the system's
///   secondary label colour (a dull grey that adapts to light and dark menus) and
///   only the word "Granted" in green.
/// - **Missing**: "Grant Accessibility…", entirely in red.
///
/// Neither state has a background, so both sit on the menu's own background.
///
/// `ns_menu` is the native `NSMenu` the tray shows. The menu library does not
/// expose its native items, so this reaches them through the menu, and finds the
/// one to style by its tag (see `tag_menu_item`), never by its position. It does
/// nothing if no item carries that tag or if any AppKit object cannot be built.
///
/// Must run on the main thread, like every menu change.
pub fn style_menu_item(ns_menu: *mut c_void, tag: isize, name: &str, granted: bool) {
    if ns_menu.is_null() {
        return;
    }
    let menu = ns_menu as *mut AnyObject;

    // SAFETY: `menu` is the live NSMenu owned by the tray, only AppKit objects
    // are touched, and every receiver is checked before use.
    unsafe {
        let index: isize = msg_send![menu, indexOfItemWithTag: tag];
        if index < 0 {
            return;
        }
        let item: *mut AnyObject = msg_send![menu, itemAtIndex: index];
        if item.is_null() {
            return;
        }

        // The base colour of the whole string; the word "Granted" is then picked
        // out on top of it.
        let base_color = if granted { label_color() } else { missing_color() };
        let label = menu_text(name, granted);
        let (Some(base_color), Some(font)) = (base_color, menu_font()) else {
            return;
        };

        let Some(dict_class) = AnyClass::get(c"NSMutableDictionary") else {
            return;
        };
        let attributes: Retained<AnyObject> = msg_send![dict_class, new];
        let _: () = msg_send![&*attributes, setObject: &*font, forKey: NSFontAttributeName];
        let _: () = msg_send![&*attributes, setObject: &*base_color, forKey: NSForegroundColorAttributeName];

        let Some(string_class) = AnyClass::get(c"NSMutableAttributedString") else {
            return;
        };
        let allocated: Allocated<AnyObject> = msg_send![string_class, alloc];
        let title: Retained<AnyObject> = msg_send![
            allocated,
            initWithString: &*NSString::from_str(&label),
            attributes: &*attributes
        ];

        if granted {
            if let Some(green) = granted_color() {
                let (location, length) = granted_word_range(name);
                let _: () = msg_send![
                    &*title,
                    addAttribute: NSForegroundColorAttributeName,
                    value: &*green,
                    range: NSRange::new(location, length)
                ];
            }
        }
        let _: () = msg_send![item, setAttributedTitle: &*title];
    }
}

/// Opens System Settings at the Accessibility pane.
pub fn open_accessibility_settings() {
    open_pane(ACCESSIBILITY_PANE);
}

/// Opens System Settings at the Input Monitoring pane.
///
/// Asking first is what lists MechvibesDX in that pane at all, so the user only
/// has to flip its switch. It shows the system prompt once; on later calls it
/// does nothing visible.
pub fn open_input_monitoring_settings() {
    // SAFETY: a plain request with no arguments.
    unsafe {
        CGRequestListenEventAccess();
    }
    open_pane(INPUT_MONITORING_PANE);
}

fn open_pane(url: &str) {
    if let Err(e) = open::that(url) {
        crate::always_eprint!("❌ Failed to open System Settings ({}): {}", url, e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_granted_permission_reads_as_a_status() {
        assert_eq!(menu_text("Accessibility", true), "Accessibility: Granted");
        assert_eq!(menu_text("Input Monitoring", true), "Input Monitoring: Granted");
    }

    #[test]
    fn a_missing_permission_reads_as_the_action_to_take() {
        assert_eq!(menu_text("Accessibility", false), "Grant Accessibility…");
        assert_eq!(menu_text("Input Monitoring", false), "Grant Input Monitoring…");
    }

    /// Reading the state must never block on or raise a system prompt, because
    /// the tray polls it. This links and runs both real queries; the values
    /// depend on the machine, so only that it returns is asserted.
    #[test]
    fn reading_the_state_returns_without_prompting() {
        let first = current();
        assert_eq!(first, current(), "two reads in a row must agree");
    }

    // ---- the AppKit styling, checked against a real native menu -------------

    use objc2::runtime::Sel;

    // Only the tests look at this one: production code never sets a background,
    // and the tests are what prove it.
    #[link(name = "AppKit", kind = "framework")]
    unsafe extern "C" {
        static NSBackgroundColorAttributeName: *mut AnyObject;
    }

    fn native_menu_with_item(title: &str) -> (Retained<AnyObject>, *mut AnyObject) {
        unsafe {
            let menu: Retained<AnyObject> = msg_send![AnyClass::get(c"NSMenu").unwrap(), new];
            let allocated: Allocated<AnyObject> = msg_send![AnyClass::get(c"NSMenuItem").unwrap(), alloc];
            let item: Retained<AnyObject> = msg_send![
                allocated,
                initWithTitle: &*NSString::from_str(title),
                action: None::<Sel>,
                keyEquivalent: &*NSString::from_str("")
            ];
            let _: () = msg_send![&*menu, addItem: &*item];
            tag_menu_item(Retained::as_ptr(&menu) as *mut c_void, 0, ACCESSIBILITY_TAG);
            let raw = Retained::as_ptr(&item) as *mut AnyObject;
            (menu, raw)
        }
    }

    type Rgb = (f64, f64, f64);

    /// What is drawn at one character of an item's attributed title.
    struct Drawn {
        text: String,
        foreground: Rgb,
        foreground_alpha: f64,
        /// Whether any background fill is set on the character.
        has_background: bool,
        font_name: String,
        font_size: f64,
    }

    /// The colour as it resolves right now, i.e. under this thread's current
    /// appearance, which is what makes a dynamic system colour differ between
    /// light and dark.
    fn srgba(color: &AnyObject) -> (Rgb, f64) {
        unsafe {
            let space: Retained<AnyObject> = msg_send![AnyClass::get(c"NSColorSpace").unwrap(), sRGBColorSpace];
            let converted: Retained<AnyObject> = msg_send![color, colorUsingColorSpace: &*space];
            let (mut r, mut g, mut b, mut a) = (0f64, 0f64, 0f64, 0f64);
            let _: () = msg_send![&*converted, getRed: &mut r, green: &mut g, blue: &mut b, alpha: &mut a];
            ((r, g, b), a)
        }
    }

    fn srgb(color: &AnyObject) -> Rgb {
        srgba(color).0
    }

    fn drawn_at(item: *mut AnyObject, index: usize) -> Drawn {
        unsafe {
            let attributed: Retained<AnyObject> = msg_send![item, attributedTitle];
            let text: Retained<NSString> = msg_send![&*attributed, string];
            let attribute = |key: *mut AnyObject| -> Option<Retained<AnyObject>> {
                msg_send![
                    &*attributed,
                    attribute: key,
                    atIndex: index,
                    effectiveRange: std::ptr::null_mut::<NSRange>()
                ]
            };
            let font = attribute(NSFontAttributeName).expect("a font on every character");
            let font_name: Retained<NSString> = msg_send![&*font, fontName];
            let font_size: f64 = msg_send![&*font, pointSize];
            let (foreground, foreground_alpha) = srgba(&attribute(NSForegroundColorAttributeName).expect("a text colour"));
            Drawn {
                text: text.to_string(),
                foreground,
                foreground_alpha,
                has_background: attribute(NSBackgroundColorAttributeName).is_some(),
                font_name: font_name.to_string(),
                font_size,
            }
        }
    }

    fn close(a: Rgb, b: Rgb) -> bool {
        (a.0 - b.0).abs() < 0.02 && (a.1 - b.1).abs() < 0.02 && (a.2 - b.2).abs() < 0.02
    }

    /// Neutral: the three channels (nearly) equal, i.e. some grey, black or white.
    fn is_neutral(c: Rgb) -> bool {
        (c.0 - c.1).abs() < 0.05 && (c.1 - c.2).abs() < 0.05
    }

    /// Runs `f` with this thread's appearance set to the named one, then puts it
    /// back. The setting is per thread, so this cannot affect other tests.
    fn with_appearance<T>(name: &str, f: impl FnOnce() -> T) -> T {
        unsafe {
            let class = AnyClass::get(c"NSAppearance").unwrap();
            let original: Option<Retained<AnyObject>> = msg_send![class, currentAppearance];
            let appearance: Retained<AnyObject> = msg_send![class, appearanceNamed: &*NSString::from_str(name)];
            let _: () = msg_send![class, setCurrentAppearance: &*appearance];
            let result = f();
            if let Some(original) = original {
                let _: () = msg_send![class, setCurrentAppearance: &*original];
            }
            result
        }
    }

    const LIGHT: &str = "NSAppearanceNameAqua";
    const DARK: &str = "NSAppearanceNameDarkAqua";

    /// Stand-ins for the menu background, which the OS makes translucent so its
    /// true value is not knowable here. Deliberately the harder end of each: a
    /// greyer light menu and a lighter dark menu both lower the contrast.
    const LIGHT_MENU: Rgb = (0.90, 0.90, 0.90);
    const DARK_MENU: Rgb = (0.30, 0.30, 0.30);

    /// WCAG contrast ratio of text `fg` (with alpha) drawn over an opaque `bg`.
    fn contrast(fg: Rgb, alpha: f64, bg: Rgb) -> f64 {
        fn linear(c: f64) -> f64 {
            if c <= 0.03928 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        }
        fn luminance(c: Rgb) -> f64 {
            0.2126 * linear(c.0) + 0.7152 * linear(c.1) + 0.0722 * linear(c.2)
        }
        let shown = (
            fg.0 * alpha + bg.0 * (1.0 - alpha),
            fg.1 * alpha + bg.1 * (1.0 - alpha),
            fg.2 * alpha + bg.2 * (1.0 - alpha),
        );
        let (a, b) = (luminance(shown), luminance(bg));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    /// The least contrast any of the three colours may have against either menu:
    /// the WCAG minimum for body text. The system colours they replace measure
    /// between 1.8:1 and 3.9:1 (see `the_system_colours_they_replace_fail_it`).
    const MIN_CONTRAST: f64 = 4.5;

    /// The colour of character `index` of `item`'s title as it resolves under
    /// the named appearance.
    fn resolved(item: *mut AnyObject, index: usize, appearance: &str) -> (Rgb, f64) {
        with_appearance(appearance, || {
            let drawn = drawn_at(item, index);
            (drawn.foreground, drawn.foreground_alpha)
        })
    }

    /// Contrast of character `index` against the stand-in menu of `appearance`.
    fn contrast_of(item: *mut AnyObject, index: usize, appearance: &str) -> f64 {
        let (rgb, alpha) = resolved(item, index, appearance);
        let menu = if appearance == DARK { DARK_MENU } else { LIGHT_MENU };
        contrast(rgb, alpha, menu)
    }

    /// The font and size of an ordinary menu item, i.e. what everything else in
    /// the menu is drawn in.
    fn menu_font_name_and_size() -> (String, f64) {
        unsafe {
            let font = menu_font().expect("the menu font");
            let name: Retained<NSString> = msg_send![&*font, fontName];
            let size: f64 = msg_send![&*font, pointSize];
            (name.to_string(), size)
        }
    }

    #[test]
    fn the_granted_word_is_found_by_its_utf16_range() {
        assert_eq!(granted_word_range("Accessibility"), ("Accessibility: ".len(), "Granted".len()));
        assert_eq!(granted_word_range("Input Monitoring"), ("Input Monitoring: ".len(), "Granted".len()));
    }

    /// Clearly green: the green channel well ahead of both others. Not tied to
    /// one shade, because the green differs between light and dark menus.
    fn is_green(c: Rgb) -> bool {
        c.1 > c.0 + 0.25 && c.1 > c.2 + 0.25
    }

    /// Clearly red: the red channel well ahead of both others. The dark menu's
    /// red is a lighter coral, so this does not demand a saturated red.
    fn is_red(c: Rgb) -> bool {
        c.0 > c.1 + 0.25 && c.0 > c.2 + 0.25
    }

    #[test]
    fn a_granted_line_has_a_grey_label_and_only_the_word_granted_in_green() {
        let (menu, item) = native_menu_with_item("placeholder");
        style_menu_item(Retained::as_ptr(&menu) as *mut c_void, ACCESSIBILITY_TAG, "Accessibility", true);

        let (start, _) = granted_word_range("Accessibility");
        let last = start + "Granted".len() - 1;
        assert_eq!(drawn_at(item, 0).text, "Accessibility: Granted");

        for appearance in [LIGHT, DARK] {
            let label = resolved(item, 0, appearance).0;
            let colon = resolved(item, start - 2, appearance).0;
            let space = resolved(item, start - 1, appearance).0;
            let first = resolved(item, start, appearance).0;
            let end = resolved(item, last, appearance).0;
            assert!(is_neutral(label), "{appearance}: the label is a grey, not a hue, was {label:?}");
            assert!(close(colon, label) && close(space, label), "{appearance}: the colon and space stay the label colour");
            assert!(is_green(first), "{appearance}: \"Granted\" should read as green, was {first:?}");
            assert!(close(first, end), "{appearance}: the whole word is one green, to its last letter");
            assert!(!close(label, first), "{appearance}: the label and the word are different colours");
        }
        assert!(!drawn_at(item, 0).has_background, "a granted line has no background fill");
        assert!(!drawn_at(item, start).has_background);
    }

    /// The reason for these colours: each has to be readable in a light menu and
    /// in a dark one, and no single fixed colour is both.
    #[test]
    fn every_permission_colour_is_readable_in_both_light_and_dark_menus() {
        let (menu, granted) = native_menu_with_item("placeholder");
        style_menu_item(Retained::as_ptr(&menu) as *mut c_void, ACCESSIBILITY_TAG, "Accessibility", true);
        let (menu, missing) = native_menu_with_item("placeholder");
        style_menu_item(Retained::as_ptr(&menu) as *mut c_void, ACCESSIBILITY_TAG, "Accessibility", false);
        let (start, _) = granted_word_range("Accessibility");

        for (what, item, index) in [("label", granted, 0), ("Granted", granted, start), ("Grant line", missing, 0)] {
            for appearance in [LIGHT, DARK] {
                let ratio = contrast_of(item, index, appearance);
                assert!(
                    ratio >= MIN_CONTRAST,
                    "{what} in a {appearance} menu: {ratio:.2}:1 (text {:?})",
                    resolved(item, index, appearance).0
                );
            }
            // It must actually adapt: darker text on the light menu than on the dark one.
            let light = resolved(item, index, LIGHT).0;
            let dark = resolved(item, index, DARK).0;
            let brightness = |c: Rgb| c.0 + c.1 + c.2;
            assert!(brightness(light) < brightness(dark), "{what}: should be darker on light ({light:?}) than on dark ({dark:?})");
        }
    }

    /// Control for the test above: the system colours these replaced must FAIL it
    /// in at least one appearance, or the threshold would prove nothing.
    #[test]
    fn the_system_colours_they_replace_fail_it() {
        for name in [c"systemGrayColor", c"secondaryLabelColor", c"systemGreenColor", c"systemRedColor"] {
            let selector = objc2::runtime::Sel::register(name);
            let color: Retained<AnyObject> = unsafe { msg_send![AnyClass::get(c"NSColor").unwrap(), performSelector: selector] };
            let worst = [(LIGHT, LIGHT_MENU), (DARK, DARK_MENU)]
                .into_iter()
                .map(|(appearance, menu)| {
                    let (rgb, alpha) = with_appearance(appearance, || srgba(&color));
                    contrast(rgb, alpha, menu)
                })
                .fold(f64::MAX, f64::min);
            assert!(worst < MIN_CONTRAST, "{name:?} measured {worst:.2}:1 at worst, so it should have failed");
        }
    }

    /// The reviewer's case: something added above the permission lines moves
    /// them down. Styling must follow the tag to the right item, not land on
    /// whatever now sits at the old index.
    #[test]
    fn styling_follows_the_tag_when_an_item_is_added_above() {
        let (menu, tagged) = native_menu_with_item("placeholder");
        let menu_ptr = Retained::as_ptr(&menu) as *mut c_void;

        let above = unsafe {
            let allocated: Allocated<AnyObject> = msg_send![AnyClass::get(c"NSMenuItem").unwrap(), alloc];
            let item: Retained<AnyObject> = msg_send![
                allocated,
                initWithTitle: &*NSString::from_str("added later"),
                action: None::<Sel>,
                keyEquivalent: &*NSString::from_str("")
            ];
            let _: () = msg_send![&*menu, insertItem: &*item, atIndex: 0isize];
            item
        };
        let index_of_tagged: isize = unsafe { msg_send![&*menu, indexOfItemWithTag: ACCESSIBILITY_TAG] };
        assert_eq!(index_of_tagged, 1, "the tagged item moved down one place");

        style_menu_item(menu_ptr, ACCESSIBILITY_TAG, "Accessibility", true);

        assert_eq!(drawn_at(tagged, 0).text, "Accessibility: Granted", "the tagged item was styled");
        let above_title: Option<Retained<AnyObject>> =
            unsafe { msg_send![Retained::as_ptr(&above) as *mut AnyObject, attributedTitle] };
        assert!(above_title.is_none(), "the item that took the old index was left alone");
    }

    #[test]
    fn the_two_lines_have_different_tags() {
        assert_ne!(ACCESSIBILITY_TAG, INPUT_MONITORING_TAG);
    }

    #[test]
    fn waiting_returns_at_once_when_already_granted() {
        let mut waits = 0;
        assert!(!wait_until_granted(|| true, || waits += 1));
        assert_eq!(waits, 0, "nothing to wait for");
    }

    #[test]
    fn waiting_keeps_checking_until_the_permission_is_granted() {
        let mut checks = 0;
        let mut waits = 0;
        let waited = wait_until_granted(
            || {
                checks += 1;
                checks > 3 // missing for three checks, granted on the fourth
            },
            || waits += 1
        );
        assert!(waited, "it had to wait");
        assert_eq!(waits, 3, "one wait after each failed check");
        assert_eq!(checks, 4, "and it stopped checking as soon as it was granted");
    }

    fn status(accessibility: bool, input_monitoring: bool) -> PermissionStatus {
        PermissionStatus { accessibility, input_monitoring }
    }

    #[test]
    fn a_change_is_reported_once_and_a_repeat_is_not() {
        let mut last = Some(status(false, false));
        assert_eq!(observe(&mut last, status(false, false)), None, "unchanged");
        assert_eq!(observe(&mut last, status(true, false)), Some(status(true, false)), "granted");
        assert_eq!(observe(&mut last, status(true, false)), None, "the same again");
        assert_eq!(observe(&mut last, status(true, true)), Some(status(true, true)), "the other one granted");
        assert_eq!(observe(&mut last, status(false, true)), Some(status(false, true)), "revoked");
    }

    #[test]
    fn the_first_reading_is_always_reported() {
        let mut last = None;
        assert_eq!(observe(&mut last, status(true, true)), Some(status(true, true)));
    }

    /// Control for the "no background" assertions: the check must be able to see
    /// a background when one is there, or "no background" would prove nothing.
    #[test]
    fn the_background_check_does_see_a_background_when_there_is_one() {
        let (_menu, item) = native_menu_with_item("placeholder");
        unsafe {
            let attributes: Retained<AnyObject> = msg_send![AnyClass::get(c"NSMutableDictionary").unwrap(), new];
            let red = missing_color().unwrap();
            let _: () = msg_send![&*attributes, setObject: &*red, forKey: NSBackgroundColorAttributeName];
            let _: () = msg_send![&*attributes, setObject: &*red, forKey: NSForegroundColorAttributeName];
            let font = menu_font().unwrap();
            let _: () = msg_send![&*attributes, setObject: &*font, forKey: NSFontAttributeName];
            let allocated: Allocated<AnyObject> = msg_send![AnyClass::get(c"NSAttributedString").unwrap(), alloc];
            let title: Retained<AnyObject> = msg_send![
                allocated,
                initWithString: &*NSString::from_str("with a background"),
                attributes: &*attributes
            ];
            let _: () = msg_send![item, setAttributedTitle: &*title];
        }
        assert!(drawn_at(item, 0).has_background);
    }

    #[test]
    fn a_missing_permission_is_plain_red_text_with_no_background() {
        let (menu, item) = native_menu_with_item("placeholder");
        style_menu_item(Retained::as_ptr(&menu) as *mut c_void, ACCESSIBILITY_TAG, "Input Monitoring", false);

        let first = drawn_at(item, 0);
        let last = drawn_at(item, "Grant Input Monitoring…".encode_utf16().count() - 1);
        assert_eq!(first.text, "Grant Input Monitoring…", "the text is the plain label, unpadded");
        for drawn in [&first, &last] {
            assert!(!drawn.has_background, "same plain background as a granted line");
        }
        for appearance in [LIGHT, DARK] {
            for index in [0, "Grant Input Monitoring…".encode_utf16().count() - 1] {
                let red = resolved(item, index, appearance).0;
                assert!(is_red(red), "{appearance}: the text should be red, was {red:?}");
            }
        }
    }

    /// Every part of both states is in the standard menu font at the standard
    /// size, never bold, so the lines sit in the menu like any other item.
    #[test]
    fn both_states_use_the_regular_menu_font_and_size() {
        let (expected_name, expected_size) = menu_font_name_and_size();
        assert!(!expected_name.to_lowercase().contains("bold"), "the menu font itself is not bold");

        for (name, granted) in [("Accessibility", true), ("Input Monitoring", true), ("Accessibility", false)] {
            let (menu, item) = native_menu_with_item("placeholder");
            style_menu_item(Retained::as_ptr(&menu) as *mut c_void, ACCESSIBILITY_TAG, name, granted);
            let (word_start, _) = granted_word_range(name);
            let points = if granted { vec![0, word_start] } else { vec![0, 3] };
            for index in points {
                let drawn = drawn_at(item, index);
                assert_eq!(drawn.font_name, expected_name, "{name} granted={granted} at {index}");
                assert_eq!(drawn.font_size, expected_size, "{name} granted={granted} at {index}");
            }
        }
    }

    #[test]
    fn styling_an_item_that_does_not_exist_does_nothing() {
        let (menu, _item) = native_menu_with_item("only item");
        // A tag nothing carries, and a null menu: neither may crash.
        style_menu_item(Retained::as_ptr(&menu) as *mut c_void, 0x1234, "Accessibility", true);
        style_menu_item(std::ptr::null_mut(), ACCESSIBILITY_TAG, "Accessibility", true);
        tag_menu_item(Retained::as_ptr(&menu) as *mut c_void, 9, INPUT_MONITORING_TAG);
        tag_menu_item(std::ptr::null_mut(), 0, INPUT_MONITORING_TAG);
    }

    #[test]
    fn the_panes_are_real_privacy_deep_links() {
        for pane in [ACCESSIBILITY_PANE, INPUT_MONITORING_PANE] {
            assert!(pane.starts_with("x-apple.systempreferences:com.apple.preference.security?Privacy_"));
        }
        assert_ne!(ACCESSIBILITY_PANE, INPUT_MONITORING_PANE);
    }
}
