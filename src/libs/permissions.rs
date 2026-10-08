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
#![cfg(target_os = "macos")]

use macos_accessibility_client::accessibility::application_is_trusted;
use objc2::msg_send;
use objc2::rc::{ Allocated, Retained };
use objc2::runtime::AnyClass;
use objc2::runtime::AnyObject;
use objc2_foundation::{ NSRange, NSString };
use std::ffi::c_void;

// `CGPreflightListenEventAccess` and `CGRequestListenEventAccess` (macOS 10.15+)
// are the Input Monitoring counterparts of the Accessibility trust check. The
// app's minimum macOS is 11, so they always exist.
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGPreflightListenEventAccess() -> bool;
    fn CGRequestListenEventAccess() -> bool;
}

// The AppKit constants that name the attributes of an attributed string.
#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {
    static NSForegroundColorAttributeName: *mut AnyObject;
    static NSFontAttributeName: *mut AnyObject;
}

/// Neon green (#39FF14), the colour of the word "Granted".
const NEON_GREEN: (f64, f64, f64) = (57.0 / 255.0, 255.0 / 255.0, 20.0 / 255.0);

/// The word that is picked out in neon green on a granted line.
const GRANTED: &str = "Granted";

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
        // SAFETY: a plain query with no arguments; it only reads the TCC state.
        input_monitoring: unsafe { CGPreflightListenEventAccess() },
    }
}

/// The tray item text for one permission: a status line when granted, and the
/// action to take when it is not.
pub fn menu_text(name: &str, granted: bool) -> String {
    if granted { format!("{name}: Granted") } else { format!("Grant {name}…") }
}

/// The dull, mid-tone grey for the part of a granted line that is not "Granted".
/// A fixed system grey rather than a label colour, so it reads the same in light
/// and dark menus.
fn system_gray_color() -> Option<Retained<AnyObject>> {
    let class = AnyClass::get(c"NSColor")?;
    // SAFETY: a class method with no arguments that returns an NSColor.
    unsafe { msg_send![class, systemGrayColor] }
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

fn system_red_color() -> Option<Retained<AnyObject>> {
    let class = AnyClass::get(c"NSColor")?;
    // SAFETY: a class method with no arguments that returns an NSColor.
    unsafe { msg_send![class, systemRedColor] }
}

fn srgb_color(r: f64, g: f64, b: f64) -> Option<Retained<AnyObject>> {
    let class = AnyClass::get(c"NSColor")?;
    // SAFETY: a class method taking four CGFloats and returning an NSColor.
    unsafe { msg_send![class, colorWithSRGBRed: r, green: g, blue: b, alpha: 1.0f64] }
}

/// The UTF-16 `(location, length)` of the word "Granted" within the granted line
/// for `name`, which is how AppKit addresses a range of an attributed string.
pub fn granted_word_range(name: &str) -> (usize, usize) {
    let text = menu_text(name, true);
    let prefix = text.len() - GRANTED.len();
    (text[..prefix].encode_utf16().count(), GRANTED.encode_utf16().count())
}

/// Styles one tray menu item for `name` ("Accessibility" or "Input Monitoring").
///
/// All text uses the standard menu font, the same as every other menu item.
///
/// - **Granted**: "Accessibility: Granted" with the label in dull grey and only
///   the word "Granted" in neon green.
/// - **Missing**: "Grant Accessibility…", entirely in red.
///
/// Neither state has a background, so both sit on the menu's own background.
///
/// `ns_menu` is the native `NSMenu` the tray shows and `index` the item's
/// position in it. The menu library does not expose its native items, so this
/// reaches them through the menu. It only touches a menu item that exists, and
/// does nothing if any AppKit object cannot be built.
///
/// Must run on the main thread, like every menu change.
pub fn style_menu_item(ns_menu: *mut c_void, index: usize, name: &str, granted: bool) {
    if ns_menu.is_null() {
        return;
    }
    let menu = ns_menu as *mut AnyObject;

    // SAFETY: `menu` is the live NSMenu owned by the tray, only AppKit objects
    // are touched, and every receiver is checked before use.
    unsafe {
        let count: isize = msg_send![menu, numberOfItems];
        if index as isize >= count {
            return;
        }
        let item: *mut AnyObject = msg_send![menu, itemAtIndex: index as isize];
        if item.is_null() {
            return;
        }

        // The base colour of the whole string; the word "Granted" is then picked
        // out on top of it.
        let base_color = if granted { system_gray_color() } else { system_red_color() };
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
            let (r, g, b) = NEON_GREEN;
            if let Some(neon) = srgb_color(r, g, b) {
                let (location, length) = granted_word_range(name);
                let _: () = msg_send![
                    &*title,
                    addAttribute: NSForegroundColorAttributeName,
                    value: &*neon,
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
            let raw = Retained::as_ptr(&item) as *mut AnyObject;
            (menu, raw)
        }
    }

    type Rgb = (f64, f64, f64);

    /// What is drawn at one character of an item's attributed title.
    struct Drawn {
        text: String,
        foreground: Rgb,
        /// Whether any background fill is set on the character.
        has_background: bool,
        font_name: String,
        font_size: f64,
    }

    fn srgb(color: &AnyObject) -> Rgb {
        unsafe {
            let space: Retained<AnyObject> = msg_send![AnyClass::get(c"NSColorSpace").unwrap(), sRGBColorSpace];
            let converted: Retained<AnyObject> = msg_send![color, colorUsingColorSpace: &*space];
            let (mut r, mut g, mut b, mut a) = (0f64, 0f64, 0f64, 0f64);
            let _: () = msg_send![&*converted, getRed: &mut r, green: &mut g, blue: &mut b, alpha: &mut a];
            (r, g, b)
        }
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
            Drawn {
                text: text.to_string(),
                foreground: srgb(&attribute(NSForegroundColorAttributeName).expect("a text colour")),
                has_background: attribute(NSBackgroundColorAttributeName).is_some(),
                font_name: font_name.to_string(),
                font_size,
            }
        }
    }

    fn close(a: Rgb, b: Rgb) -> bool {
        (a.0 - b.0).abs() < 0.02 && (a.1 - b.1).abs() < 0.02 && (a.2 - b.2).abs() < 0.02
    }

    /// A grey is a colour whose channels are (nearly) equal, neither black nor white.
    fn is_dull_grey(c: Rgb) -> bool {
        (c.0 - c.1).abs() < 0.05 && (c.1 - c.2).abs() < 0.05 && c.0 > 0.3 && c.0 < 0.75
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

    #[test]
    fn a_granted_line_has_a_grey_label_and_only_the_word_granted_in_neon_green() {
        let (menu, item) = native_menu_with_item("placeholder");
        style_menu_item(Retained::as_ptr(&menu) as *mut c_void, 0, "Accessibility", true);

        let (start, _) = granted_word_range("Accessibility");
        let label = drawn_at(item, 0);
        let space_before_word = drawn_at(item, start - 1);
        let word = drawn_at(item, start);
        let last_letter = drawn_at(item, start + "Granted".len() - 1);

        assert_eq!(label.text, "Accessibility: Granted");
        assert!(is_dull_grey(label.foreground), "the label should be dull grey, was {:?}", label.foreground);
        assert!(is_dull_grey(space_before_word.foreground), "the colon and space stay grey");
        assert!(close(word.foreground, NEON_GREEN), "\"Granted\" should be neon green, was {:?}", word.foreground);
        assert!(close(last_letter.foreground, NEON_GREEN), "the whole word is green, to its last letter");
        assert!(!label.has_background, "a granted line has no background fill");
        assert!(!word.has_background);
    }

    /// Control for the "no background" assertions: the check must be able to see
    /// a background when one is there, or "no background" would prove nothing.
    #[test]
    fn the_background_check_does_see_a_background_when_there_is_one() {
        let (_menu, item) = native_menu_with_item("placeholder");
        unsafe {
            let attributes: Retained<AnyObject> = msg_send![AnyClass::get(c"NSMutableDictionary").unwrap(), new];
            let red = system_red_color().unwrap();
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
        style_menu_item(Retained::as_ptr(&menu) as *mut c_void, 0, "Input Monitoring", false);

        let first = drawn_at(item, 0);
        let last = drawn_at(item, "Grant Input Monitoring…".encode_utf16().count() - 1);
        assert_eq!(first.text, "Grant Input Monitoring…", "the text is the plain label, unpadded");
        for drawn in [&first, &last] {
            let red = drawn.foreground;
            assert!(red.0 > 0.7 && red.1 < 0.4 && red.2 < 0.4, "the text should be red, was {red:?}");
            assert!(!drawn.has_background, "same plain background as a granted line");
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
            style_menu_item(Retained::as_ptr(&menu) as *mut c_void, 0, name, granted);
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
        // Out of range, and a null menu: neither may crash.
        style_menu_item(Retained::as_ptr(&menu) as *mut c_void, 7, "Accessibility", true);
        style_menu_item(std::ptr::null_mut(), 0, "Accessibility", true);
    }

    #[test]
    fn the_panes_are_real_privacy_deep_links() {
        for pane in [ACCESSIBILITY_PANE, INPUT_MONITORING_PANE] {
            assert!(pane.starts_with("x-apple.systempreferences:com.apple.preference.security?Privacy_"));
        }
        assert_ne!(ACCESSIBILITY_PANE, INPUT_MONITORING_PANE);
    }
}
