use cpal::traits::{ DeviceTrait, HostTrait };
use cpal::{ Device, Host };
use crate::i18n::catalog::ErrorStrings;
use crate::i18n::fill;
use std::sync::{ Mutex, OnceLock };

#[derive(Debug, Clone, PartialEq)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

/// Why a device list could not be produced.
///
/// The wording is split from the cause on purpose. `Display` is the stable
/// English description the log and the callers that only print keep using, and
/// it never follows the UI language. [`DeviceError::ui_message`] is the sentence
/// the screen shows, composed from the catalog the caller hands in: this layer
/// runs on the audio thread and cannot know which language is in effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceError {
    /// `cpal` refused to list the speakers. The `String` is its description.
    OutputDevices(String),
    /// `cpal` refused to list the microphones.
    InputDevices(String),
    /// `cpal` refused to enumerate, for the callers that do not tell the two
    /// directions apart.
    Devices(String),
    /// The process-wide cache was never built and could not be built now.
    CacheUnavailable,
}

impl DeviceError {
    /// The sentence for this failure in the language `errors` carries.
    ///
    /// The caller resolves the language and passes the `errors` zone; nothing
    /// here reads the config.
    pub fn ui_message(&self, errors: &ErrorStrings) -> String {
        match self {
            DeviceError::OutputDevices(cause) => fill(errors.output_devices_unavailable, &[cause]),
            DeviceError::InputDevices(cause) => fill(errors.input_devices_unavailable, &[cause]),
            DeviceError::Devices(cause) => fill(errors.devices_unavailable, &[cause]),
            DeviceError::CacheUnavailable => errors.device_cache_unavailable.to_string(),
        }
    }
}

impl std::fmt::Display for DeviceError {
    /// Always English: the catalog for it is named here, never read from config,
    /// so a log line or a test reads the same in every language.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.ui_message(&crate::i18n::Language::En.strings().errors))
    }
}

// Global device cache to avoid repeated enumeration
static CACHED_OUTPUT_DEVICES: OnceLock<Mutex<Vec<DeviceInfo>>> = OnceLock::new();
static CACHED_INPUT_DEVICES: OnceLock<Mutex<Vec<DeviceInfo>>> = OnceLock::new();

// ALSA error suppressor for Linux to silence expected enumeration errors
#[cfg(target_os = "linux")]
struct AlsaErrorSuppressor {
    _stderr_fd: std::os::fd::OwnedFd,
}

#[cfg(target_os = "linux")]
impl AlsaErrorSuppressor {
    fn new() -> Self {
        use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

        // Redirect stderr to /dev/null temporarily to suppress ALSA error messages
        // ALSA generates expected errors when probing invalid/misconfigured devices
        unsafe {
            let null_fd = libc::open(
                b"/dev/null\0".as_ptr() as *const libc::c_char,
                libc::O_WRONLY
            );
            let stderr_fd = libc::dup(libc::STDERR_FILENO);
            libc::dup2(null_fd, libc::STDERR_FILENO);
            libc::close(null_fd);

            Self {
                _stderr_fd: OwnedFd::from_raw_fd(stderr_fd),
            }
        }
    }
}

#[cfg(target_os = "linux")]
impl Drop for AlsaErrorSuppressor {
    fn drop(&mut self) {
        use std::os::fd::AsRawFd;

        // Restore original stderr when suppressor is dropped
        unsafe {
            libc::dup2(self._stderr_fd.as_raw_fd(), libc::STDERR_FILENO);
        }
    }
}

/// Builds the persisted identity for a device from its name.
///
/// Device IDs used to be `output_{index}`, where the index was the position in
/// the enumeration order. That is not an identity: unplugging one device
/// shifts every device after it down a slot, so a saved selection silently
/// resolved to a different device. Deriving the ID from the name instead keeps
/// a selection pointing at the same device across replugs.
///
/// cpal 0.15 exposes only `name()`; the `Device::id()` that would give a true
/// backend identity arrived in cpal 0.17, and reaching it means moving to
/// rodio 0.22, whose `OutputStream` API change lands on the audio engine. Two
/// devices sharing a name (a pair of identical headsets) therefore still
/// collide - accepted for now, see plans/260813-audio-device-follow-behavior.
///
/// The hash is FNV-1a rather than `DefaultHasher` on purpose: std makes no
/// stability guarantee across Rust versions, and an ID that changes under the
/// user would unpin their device on upgrade.
fn device_id_from_name(prefix: &str, name: &str) -> String {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET;
    for byte in name.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    format!("{}_name:{:016x}", prefix, hash)
}

/// Whether `device_id` is a legacy `output_{index}`/`input_{index}` ID, which
/// resolves by enumeration position and needs migrating to a name-based one.
pub fn is_legacy_index_device_id(device_id: &str) -> bool {
    for prefix in ["output_", "input_"] {
        if let Some(rest) = device_id.strip_prefix(prefix) {
            return rest.parse::<usize>().is_ok();
        }
    }
    false
}

pub struct DeviceManager {
    host: Host,
}

impl Clone for DeviceManager {
    fn clone(&self) -> Self {
        Self {
            host: cpal::default_host(),
        }
    }
}

#[allow(dead_code)]
impl DeviceManager {
    pub fn new() -> Self {
        Self {
            host: cpal::default_host(),
        }
    }

    /// Get all available audio output devices.
    ///
    /// Kept in its `String` shape because `DeviceSelector` still stores the
    /// reason and words the sentence where it draws it. Delegates to
    /// [`DeviceManager::get_output_devices_typed`], so nothing is duplicated;
    /// the selector migrates by calling that one.
    pub fn get_output_devices(&self) -> Result<Vec<DeviceInfo>, String> {
        self.get_output_devices_typed().map_err(|e| e.to_string())
    }

    pub fn get_output_devices_typed(&self) -> Result<Vec<DeviceInfo>, DeviceError> {
        crate::always_print!("🔍 [DeviceManager] Starting audio output device enumeration...");
        let mut devices = Vec::new();
        let default_device = self.host.default_output_device();
        let default_name = default_device
            .as_ref()
            .and_then(|d| d.name().ok())
            .unwrap_or_else(|| "Unknown".to_string());

        crate::always_print!("🔍 [DeviceManager] Default device: {}", default_name);

        // Suppress ALSA error messages on Linux during device enumeration
        // ALSA probes all possible devices and generates expected errors for invalid/misconfigured ones
        #[cfg(target_os = "linux")]
        let _alsa_suppressor = AlsaErrorSuppressor::new();

        crate::always_print!("🔍 [DeviceManager] Enumerating output devices via ALSA/cpal...");
        match self.host.output_devices() {
            Ok(device_iter) => {
                for (index, device) in device_iter.enumerate() {
                    if let Ok(name) = device.name() {
                        // Filter out low-level ALSA device aliases
                        // Only show user-friendly device names (default, pipewire, pulse, etc.)
                        #[cfg(target_os = "linux")]
                        {
                            // Skip low-level ALSA aliases (hw:, plughw:, dmix:, dsnoop:, etc.)
                            if name.starts_with("hw:")
                                || name.starts_with("plughw:")
                                || name.starts_with("dmix:")
                                || name.starts_with("dsnoop:")
                                || name.starts_with("front:")
                                || name.starts_with("surround")
                                || name.starts_with("iec958:")
                            {
                                crate::always_print!("🔍 [DeviceManager] Skipping low-level ALSA alias: {}", name);
                                continue;
                            }
                        }

                        let is_default =
                            Some(&name) ==
                            default_device
                                .as_ref()
                                .and_then(|d| d.name().ok())
                                .as_ref();

                        crate::always_print!("🔍 [DeviceManager] Found device #{}: {} {}",
                            index,
                            name,
                            if is_default { "(default)" } else { "" }
                        );

                        devices.push(DeviceInfo {
                            id: device_id_from_name("output", &name),
                            name: name.clone(),
                            is_default,
                        });
                    }
                }
                crate::always_print!("✅ [DeviceManager] Enumeration complete. Found {} devices", devices.len());
            }
            Err(e) => {
                crate::always_print!("❌ [DeviceManager] Failed to enumerate: {}", e);
                return Err(DeviceError::OutputDevices(e.to_string()));
            }
        }

        // Ensure we have at least the default device
        if devices.is_empty() {
            crate::always_print!("⚠️ [DeviceManager] No devices found, adding default fallback");
            devices.push(DeviceInfo {
                id: "output_default".to_string(),
                name: default_name,
                is_default: true,
            });
        }

        crate::always_print!("🔍 [DeviceManager] Returning {} total devices", devices.len());
        Ok(devices)
    }

    /// Get all available audio input devices.
    ///
    /// The input half of [`DeviceManager::get_output_devices`], kept in the same
    /// `String` shape for the same reason.
    pub fn get_input_devices(&self) -> Result<Vec<DeviceInfo>, String> {
        self.get_input_devices_typed().map_err(|e| e.to_string())
    }

    pub fn get_input_devices_typed(&self) -> Result<Vec<DeviceInfo>, DeviceError> {
        let mut devices = Vec::new();
        let default_device = self.host.default_input_device();
        let default_name = default_device
            .as_ref()
            .and_then(|d| d.name().ok())
            .unwrap_or_else(|| "Unknown".to_string());

        // Suppress ALSA error messages on Linux during device enumeration
        #[cfg(target_os = "linux")]
        let _alsa_suppressor = AlsaErrorSuppressor::new();

        match self.host.input_devices() {
            Ok(device_iter) => {
                for device in device_iter {
                    if let Ok(name) = device.name() {
                        // Filter out low-level ALSA device aliases
                        #[cfg(target_os = "linux")]
                        {
                            if name.starts_with("hw:")
                                || name.starts_with("plughw:")
                                || name.starts_with("dmix:")
                                || name.starts_with("dsnoop:")
                                || name.starts_with("front:")
                                || name.starts_with("surround")
                                || name.starts_with("iec958:")
                            {
                                continue;
                            }
                        }

                        let is_default =
                            Some(&name) ==
                            default_device
                                .as_ref()
                                .and_then(|d| d.name().ok())
                                .as_ref();

                        devices.push(DeviceInfo {
                            id: device_id_from_name("input", &name),
                            name: name.clone(),
                            is_default,
                        });
                    }
                }
            }
            Err(e) => {
                return Err(DeviceError::InputDevices(e.to_string()));
            }
        }

        // Ensure we have at least the default device
        if devices.is_empty() {
            devices.push(DeviceInfo {
                id: "input_default".to_string(),
                name: default_name,
                is_default: true,
            });
        }

        Ok(devices)
    }

    /// Get device by ID for output devices.
    ///
    /// The output half of [`DeviceManager::get_output_devices`]. The audio
    /// engine calls this one: it only prints the failure, so it takes the typed
    /// error and formats it as it is.
    pub fn get_output_device_by_id(&self, device_id: &str) -> Result<Option<Device>, DeviceError> {
        if device_id == "output_default" {
            return Ok(self.host.default_output_device());
        }

        // Suppress ALSA error messages on Linux
        #[cfg(target_os = "linux")]
        let _alsa_suppressor = AlsaErrorSuppressor::new();

        match self.host.output_devices() {
            Ok(device_iter) => {
                // Legacy `output_{index}` IDs still resolve by position so a
                // config written before name-based IDs keeps working until
                // `Config::load` migrates it.
                let legacy_index = device_id
                    .strip_prefix("output_")
                    .and_then(|rest| rest.parse::<usize>().ok());

                for (index, device) in device_iter.enumerate() {
                    if legacy_index == Some(index) {
                        return Ok(Some(device));
                    }
                    if let Ok(name) = device.name() {
                        if device_id_from_name("output", &name) == device_id {
                            return Ok(Some(device));
                        }
                    }
                }
            }
            Err(e) => {
                return Err(DeviceError::Devices(e.to_string()));
            }
        }

        Ok(None)
    }

    /// Get device by ID for input devices
    /// The input half of [`DeviceManager::get_output_device_by_id`].
    pub fn get_input_device_by_id(&self, device_id: &str) -> Result<Option<Device>, DeviceError> {
        if device_id == "input_default" {
            return Ok(self.host.default_input_device());
        }

        // Suppress ALSA error messages on Linux
        #[cfg(target_os = "linux")]
        let _alsa_suppressor = AlsaErrorSuppressor::new();

        match self.host.input_devices() {
            Ok(device_iter) => {
                // Legacy `input_{index}` IDs still resolve by position, as in
                // `get_output_device_by_id`.
                let legacy_index = device_id
                    .strip_prefix("input_")
                    .and_then(|rest| rest.parse::<usize>().ok());

                for (index, device) in device_iter.enumerate() {
                    if legacy_index == Some(index) {
                        return Ok(Some(device));
                    }
                    if let Ok(name) = device.name() {
                        if device_id_from_name("input", &name) == device_id {
                            return Ok(Some(device));
                        }
                    }
                }
            }
            Err(e) => {
                return Err(DeviceError::Devices(e.to_string()));
            }
        }

        Ok(None)
    }

    /// Test if a device is available and working
    pub fn test_output_device(&self, device_id: &str) -> Result<bool, String> {
        // Suppress ALSA error messages on Linux
        #[cfg(target_os = "linux")]
        let _alsa_suppressor = AlsaErrorSuppressor::new();

        match self.get_output_device_by_id(device_id).map_err(|e| e.to_string())? {
            Some(device) => {
                // Try to get supported configurations to test device availability
                match device.supported_output_configs() {
                    Ok(mut configs) => Ok(configs.next().is_some()),
                    Err(_) => Ok(false),
                }
            }
            None => Ok(false),
        }
    }

    /// Test if an input device is available and working
    pub fn test_input_device(&self, device_id: &str) -> Result<bool, String> {
        // Suppress ALSA error messages on Linux
        #[cfg(target_os = "linux")]
        let _alsa_suppressor = AlsaErrorSuppressor::new();

        match self.get_input_device_by_id(device_id).map_err(|e| e.to_string())? {
            Some(device) => {
                // Try to get supported configurations to test device availability
                match device.supported_input_configs() {
                    Ok(mut configs) => Ok(configs.next().is_some()),
                    Err(_) => Ok(false),
                }
            }
            None => Ok(false),
        }
    }

    /// Get the sample rate of the currently selected output device (or the
    /// system default if none selected/available). Returns `None` on any
    /// enumeration/config error - callers should treat that as "skip
    /// resampling, keep the file's native rate" (same as pre-resample
    /// baseline behavior), not assume a hardcoded rate: guessing wrong would
    /// make rodio's realtime resampler run on top of ours, which is worse
    /// than not resampling at all.
    pub fn get_current_output_sample_rate(&self) -> Option<u32> {
        let config = crate::state::config_writer::current();

        let device = match &config.selected_audio_device {
            Some(device_id) =>
                match self.get_output_device_by_id(device_id) {
                    Ok(Some(device)) => Some(device),
                    _ => self.host.default_output_device(),
                }
            None => self.host.default_output_device(),
        };

        device.and_then(|d| d.default_output_config().ok()).map(|c| c.sample_rate().0)
    }
}

impl Default for DeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

// Global device cache management functions
impl DeviceManager {
    /// Initialize device cache on app startup (enumerate once and cache)
    pub fn initialize_cache() {
        crate::always_print!("🔍 [DeviceCache] Initializing device cache on app startup...");
        let manager = DeviceManager::new();

        // Load output devices
        match manager.get_output_devices_typed() {
            Ok(devices) => {
                crate::always_print!("✅ [DeviceCache] Cached {} output devices", devices.len());
                CACHED_OUTPUT_DEVICES.get_or_init(|| Mutex::new(devices));
            }
            Err(e) => {
                crate::always_print!("❌ [DeviceCache] Failed to cache output devices: {}", e);
                CACHED_OUTPUT_DEVICES.get_or_init(|| Mutex::new(Vec::new()));
            }
        }

        // Load input devices
        match manager.get_input_devices_typed() {
            Ok(devices) => {
                crate::always_print!("✅ [DeviceCache] Cached {} input devices", devices.len());
                CACHED_INPUT_DEVICES.get_or_init(|| Mutex::new(devices));
            }
            Err(e) => {
                crate::always_print!("❌ [DeviceCache] Failed to cache input devices: {}", e);
                CACHED_INPUT_DEVICES.get_or_init(|| Mutex::new(Vec::new()));
            }
        }
    }

    /// Get cached output devices (no enumeration - instant).
    ///
    /// Kept in its `String` shape because `DeviceSelector` stores the reason;
    /// delegates to [`DeviceManager::get_cached_output_devices_typed`].
    pub fn get_cached_output_devices() -> Result<Vec<DeviceInfo>, String> {
        Self::get_cached_output_devices_typed().map_err(|e| e.to_string())
    }

    pub fn get_cached_output_devices_typed() -> Result<Vec<DeviceInfo>, DeviceError> {
        crate::always_print!("📋 [DeviceCache] Returning cached output devices...");
        if let Some(cache) = CACHED_OUTPUT_DEVICES.get() {
            if let Ok(devices) = cache.lock() {
                crate::always_print!("✅ [DeviceCache] Found {} cached output devices", devices.len());
                return Ok(devices.clone());
            }
        }

        crate::always_print!("⚠️ [DeviceCache] Cache not initialized, initializing now...");
        Self::initialize_cache();

        if let Some(cache) = CACHED_OUTPUT_DEVICES.get() {
            if let Ok(devices) = cache.lock() {
                return Ok(devices.clone());
            }
        }

        Err(DeviceError::CacheUnavailable)
    }

    /// Get cached input devices (no enumeration - instant)
    /// The input half of [`DeviceManager::get_cached_output_devices`].
    pub fn get_cached_input_devices() -> Result<Vec<DeviceInfo>, String> {
        Self::get_cached_input_devices_typed().map_err(|e| e.to_string())
    }

    /// No caller: nothing in the window reads the input cache (input devices are
    /// listed fresh). Kept beside the output pair so the two stay symmetrical,
    /// and allowed for the same reason its `String` twin is.
    #[allow(dead_code)]
    pub fn get_cached_input_devices_typed() -> Result<Vec<DeviceInfo>, DeviceError> {
        crate::always_print!("📋 [DeviceCache] Returning cached input devices...");
        if let Some(cache) = CACHED_INPUT_DEVICES.get() {
            if let Ok(devices) = cache.lock() {
                crate::always_print!("✅ [DeviceCache] Found {} cached input devices", devices.len());
                return Ok(devices.clone());
            }
        }

        crate::always_print!("⚠️ [DeviceCache] Cache not initialized, initializing now...");
        Self::initialize_cache();

        if let Some(cache) = CACHED_INPUT_DEVICES.get() {
            if let Ok(devices) = cache.lock() {
                return Ok(devices.clone());
            }
        }

        Err(DeviceError::CacheUnavailable)
    }

    /// Force refresh cache (re-enumerate and update)
    /// Kept in its `String` shape because `DeviceSelector` only logs this one;
    /// delegates to [`DeviceManager::refresh_cache_typed`].
    pub fn refresh_cache() -> Result<(), String> {
        Self::refresh_cache_typed().map_err(|e| e.to_string())
    }

    pub fn refresh_cache_typed() -> Result<(), DeviceError> {
        crate::always_print!("🔄 [DeviceCache] Force refreshing device cache...");
        let manager = DeviceManager::new();

        // Refresh output devices
        match manager.get_output_devices_typed() {
            Ok(devices) => {
                if let Some(cache) = CACHED_OUTPUT_DEVICES.get() {
                    if let Ok(mut cached) = cache.lock() {
                        *cached = devices;
                        crate::always_print!("✅ [DeviceCache] Refreshed output devices cache");
                    }
                }
            }
            Err(e) => {
                crate::always_print!("❌ [DeviceCache] Failed to refresh output devices: {}", e);
                return Err(e);
            }
        }

        // Refresh input devices
        match manager.get_input_devices_typed() {
            Ok(devices) => {
                if let Some(cache) = CACHED_INPUT_DEVICES.get() {
                    if let Ok(mut cached) = cache.lock() {
                        *cached = devices;
                        crate::always_print!("✅ [DeviceCache] Refreshed input devices cache");
                    }
                }
            }
            Err(e) => {
                crate::always_print!("❌ [DeviceCache] Failed to refresh input devices: {}", e);
                return Err(e);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole point of a name-based ID: a saved selection has to survive
    /// the device list changing around it. These are the exact hashes the
    /// shipped build writes into config, so a change to the hash function
    /// that would unpin every user's device fails here.
    #[test]
    fn device_id_is_stable_for_a_given_name() {
        assert_eq!(
            device_id_from_name("output", "Speakers (Realtek High Definition Audio)"),
            device_id_from_name("output", "Speakers (Realtek High Definition Audio)")
        );
        assert_eq!(device_id_from_name("output", "Headphones"), "output_name:0b3a976597860826");
    }

    #[test]
    fn device_id_differs_by_name_and_by_direction() {
        assert_ne!(
            device_id_from_name("output", "Headphones"),
            device_id_from_name("output", "Speakers")
        );
        // An output and an input sharing a name must not collide - some
        // headsets enumerate under the same string on both directions.
        assert_ne!(
            device_id_from_name("output", "Headset"),
            device_id_from_name("input", "Headset")
        );
    }

    #[test]
    fn legacy_index_ids_are_recognised() {
        assert!(is_legacy_index_device_id("output_0"));
        assert!(is_legacy_index_device_id("input_12"));

        // Name-based IDs and the system-default sentinel are not legacy, and
        // must not be rewritten by the migration.
        assert!(!is_legacy_index_device_id(&device_id_from_name("output", "Headphones")));
        assert!(!is_legacy_index_device_id("default"));
        assert!(!is_legacy_index_device_id("output_default"));
    }

    /// The descriptions are the contract: the log reads the same in every
    /// language, and only `ui_message` follows the catalog.
    #[test]
    fn a_description_is_english_whatever_language_the_catalog_is_in() {
        use crate::i18n::Language;

        let failures = [
            DeviceError::OutputDevices("ALSA error".to_string()),
            DeviceError::InputDevices("ALSA error".to_string()),
            DeviceError::Devices("ALSA error".to_string()),
            DeviceError::CacheUnavailable,
        ];
        for failure in &failures {
            let english = failure.to_string();
            assert_eq!(
                english,
                failure.ui_message(&Language::En.strings().errors),
                "the English catalog is the description"
            );
            for language in [Language::Zh, Language::Es] {
                assert_eq!(
                    failure.to_string(),
                    english,
                    "`{failure:?}` changed with {language:?}: a description must not follow the UI language"
                );
            }
        }
        assert_eq!(DeviceError::OutputDevices("ALSA error".to_string()).to_string(), "Failed to enumerate output devices: ALSA error");
        assert_eq!(DeviceError::CacheUnavailable.to_string(), "Failed to access device cache");
        assert_eq!(
            DeviceError::CacheUnavailable.ui_message(&Language::Es.strings().errors),
            "No se pudo acceder a la caché de dispositivos"
        );
    }
}
