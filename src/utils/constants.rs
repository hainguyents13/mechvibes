/// Application constants used throughout the application
/// This file centralizes all application naming and branding constants

/// The display name of the application (with proper casing)
pub const APP_NAME: &str = "Mechvibes";

/// The display name with spaces for better readability
pub const APP_NAME_DISPLAY: &str = "Mechvibes";

/// The lowercase version for file names, URLs, etc.
#[allow(dead_code)]
pub const APP_NAME_LOWERCASE: &str = "mechvibes";

/// The identifier for the application (used in protocol registration, etc.)
///
/// Kept from the MechvibesDX days on purpose: macOS files privacy permissions
/// and the login item under the bundle identifier, so changing it would make
/// every upgraded Mac a different app. Users never see it.
#[allow(dead_code)]
pub const APP_IDENTIFIER: &str = "com.hainguyents13.mechvibesdx";

/// The protocol scheme for deep linking
pub const APP_PROTOCOL: &str = "mechvibes";

/// The library a `mechvibes://install/<id>` or `mechvibes://theme/<id>` link
/// installs from. Fixed at build time: the host is never taken from the link
/// itself.
pub const LIBRARY_BASE_URL: &str = "https://mechvibes.com";

/// Version of the application (should match Cargo.toml)
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// CSS ID prefix for custom styles
pub const CSS_ID_PREFIX: &str = "mechvibes";
