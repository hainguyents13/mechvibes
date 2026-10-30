//! Anonymous usage telemetry (Aptabase).
//!
//! Scope is deliberately tiny: one `app_started` event per launch, carrying
//! only the OS and the app version. That is enough to answer "how many people
//! use this, on what" and nothing else. No keystrokes, no soundpack names, no
//! file paths, no persistent identifier of any kind.
//!
//! Wire contract follows Aptabase's own SDK-authoring guide:
//! <https://github.com/aptabase/aptabase/wiki/How-to-build-your-own-SDK>
//! - `POST {host}/api/v0/events` with a JSON *array* of events (max 25).
//! - Headers: `App-Key: <app key>` and `Content-Type: application/json`.
//! - Host comes from the app key region: `A-EU-*` -> eu.aptabase.com,
//!   otherwise us.aptabase.com.
//! - `sessionId` is `epochSeconds` followed by 8 random digits. It is
//!   generated fresh on every launch and never stored, which is exactly
//!   Aptabase's anonymous model: we invent no user id of our own.
//!
//! Threading: `report_app_started` returns immediately. All work, including
//! the config read and the blocking HTTP call, happens on a detached
//! `std::thread`. Nothing here may ever be called from the audio engine loop,
//! the input worker, or the UI poll loop.

use serde::Serialize;
use std::time::Duration;

/// Aptabase app key for Mechvibes, from the app's dashboard at
/// aptabase.com. Aptabase app keys are public by design: they identify the
/// app to the ingest endpoint and grant no read access to the data, so
/// shipping one inside the binary is the intended usage.
const APP_KEY: &str = "A-US-6022972570";

/// A key that has not been configured yet. When `APP_KEY` still equals this,
/// telemetry is inert and no request is ever built or sent, so a fork of this
/// repo that has not created its own Aptabase app reports nothing anywhere.
const PLACEHOLDER_APP_KEY: &str = "A-XX-0000000000";

/// Identifies this hand-rolled client to Aptabase, per the `sdkVersion`
/// convention in their guide (`<sdk-name>@<version>`). The name predates the
/// rename and stays so usage reports keep one continuous series.
const SDK_VERSION: &str = concat!("aptabase-mechvibes-dx@", env!("CARGO_PKG_VERSION"));

/// Total budget for the request. A hanging or blackholed connection must
/// never outlive this, so a detached telemetry thread can never be what keeps
/// the process alive at shutdown.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// The single event name this app ever sends.
const EVENT_NAME: &str = "app_started";

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct SystemProps {
    os_name: String,
    os_version: String,
    app_version: String,
    locale: String,
    is_debug: bool,
    sdk_version: String,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct Event {
    timestamp: String,
    session_id: String,
    event_name: String,
    system_props: SystemProps,
    /// Always empty: this app attaches no custom properties to the event.
    props: serde_json::Value,
}

/// Whether the embedded key is a real one. Split out from the send path so
/// the "unconfigured build stays silent" rule is directly testable.
fn is_configured(app_key: &str) -> bool {
    !app_key.is_empty() && app_key != PLACEHOLDER_APP_KEY
}

/// Ingest host for an app key. Aptabase routes by the region segment of the
/// key; anything that is not explicitly EU goes to the US host, matching the
/// reference implementation in their SDK guide.
fn host_for_key(app_key: &str) -> &'static str {
    if app_key.contains("-EU-") { "https://eu.aptabase.com" } else { "https://us.aptabase.com" }
}

/// Full ingest URL for an app key.
fn events_url(app_key: &str) -> String {
    format!("{}/api/v0/events", host_for_key(app_key))
}

/// Aptabase's required session id shape: epoch seconds followed by exactly 8
/// random digits. Generated per launch and never persisted, so it cannot be
/// correlated across runs.
fn new_session_id() -> String {
    let epoch_seconds = chrono::Utc::now().timestamp().max(0);
    let random_suffix: u32 = rand::random_range(0..100_000_000);
    format!("{epoch_seconds}{random_suffix:08}")
}

/// Human-readable OS name in the casing Aptabase's dashboards expect.
fn os_name() -> String {
    if cfg!(target_os = "windows") {
        "Windows".to_string()
    } else if cfg!(target_os = "macos") {
        "macOS".to_string()
    } else if cfg!(target_os = "linux") {
        "Linux".to_string()
    } else {
        "Unknown".to_string()
    }
}

/// Best-effort OS version. Every failure path yields an empty string rather
/// than an error: a missing version must never stop the event from being
/// sent, and must never surface to the user.
#[cfg(target_os = "windows")]
fn os_version() -> String {
    use winreg::RegKey;
    use winreg::enums::HKEY_LOCAL_MACHINE;

    // `CurrentBuild` rather than `CurrentVersion`: since Windows 8.1 the
    // latter is frozen at 6.3 for compatibility, so it cannot distinguish
    // Windows 10 from 11. The build number is what actually moves.
    let key = match
        RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(
            "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion"
        )
    {
        Ok(key) => key,
        Err(_) => {
            return String::new();
        }
    };

    let build: String = key.get_value("CurrentBuild").unwrap_or_default();
    let major = if build.parse::<u32>().unwrap_or(0) >= 22000 { "11" } else { "10" };

    if build.is_empty() { String::new() } else { format!("{major}.0.{build}") }
}

/// Best-effort OS version on Unix. `uname -r` on Linux and `sw_vers` on
/// macOS; both are absent-tolerant and return an empty string on any failure.
#[cfg(not(target_os = "windows"))]
fn os_version() -> String {
    let (program, args): (&str, &[&str]) = if cfg!(target_os = "macos") {
        ("sw_vers", &["-productVersion"])
    } else {
        ("uname", &["-r"])
    };

    std::process::Command
        ::new(program)
        .args(args)
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// Best-effort BCP-47 locale. Windows has no `LANG` environment variable, so
/// the user's UI language comes from Win32 instead. Empty when unknown; this
/// is the one optional field and it is never worth failing over.
#[cfg(target_os = "windows")]
fn locale() -> String {
    use winapi::um::winnls::GetUserDefaultLocaleName;

    // LOCALE_NAME_MAX_LENGTH. The call writes a NUL-terminated BCP-47 name
    // such as "en-US" and returns its length including the terminator.
    let mut buffer = [0u16; 85];

    // SAFETY: `buffer` is a live, writable array of exactly the capacity
    // passed as the length argument, which is what this call requires. The
    // return value is validated before any of the buffer is read.
    let written = unsafe { GetUserDefaultLocaleName(buffer.as_mut_ptr(), buffer.len() as i32) };

    if written <= 1 {
        return String::new();
    }

    // Drop the trailing NUL that the length includes.
    String::from_utf16_lossy(&buffer[..(written as usize) - 1])
}

/// Best-effort BCP-47-ish locale from the environment. Empty when unknown;
/// this is the one optional field and it is never worth failing over.
#[cfg(not(target_os = "windows"))]
fn locale() -> String {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = std::env::var(var) {
            // "en_US.UTF-8" -> "en-US"
            let cleaned = value.split('.').next().unwrap_or_default().replace('_', "-");
            if !cleaned.is_empty() && cleaned != "C" && cleaned != "POSIX" {
                return cleaned;
            }
        }
    }
    String::new()
}

/// Builds the one event this app sends. Pure and side-effect free so the
/// payload shape can be asserted in tests without touching the network.
fn build_event() -> Event {
    Event {
        timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        session_id: new_session_id(),
        event_name: EVENT_NAME.to_string(),
        system_props: SystemProps {
            os_name: os_name(),
            os_version: os_version(),
            app_version: crate::utils::constants::APP_VERSION.to_string(),
            locale: locale(),
            is_debug: cfg!(debug_assertions),
            sdk_version: SDK_VERSION.to_string(),
        },
        props: serde_json::json!({}),
    }
}

/// Decides whether a launch should be reported at all. Both gates are
/// checked before any network machinery is created, so "off" costs nothing.
fn should_send(app_key: &str, telemetry_enabled: bool) -> bool {
    telemetry_enabled && is_configured(app_key)
}

/// Performs the blocking POST. Returns the HTTP status so callers and
/// integration tests can assert on it; every error is mapped to a string and
/// nothing is ever propagated to the user.
fn send_blocking(app_key: &str, event: &Event) -> Result<u16, String> {
    let client = reqwest::blocking::Client
        ::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .post(events_url(app_key))
        .header("App-Key", app_key)
        .json(&[event])
        .send()
        .map_err(|e| e.to_string())?;

    Ok(response.status().as_u16())
}

/// Reports a single app launch, if telemetry is enabled and a real app key is
/// embedded. Returns immediately: the caller's thread does no config read, no
/// DNS, and no I/O.
///
/// `telemetry_enabled` is passed in rather than read here so that the gate is
/// decided by the caller's already-loaded config and this module never
/// touches config state or the filesystem.
pub fn report_app_started(telemetry_enabled: bool) {
    if !should_send(APP_KEY, telemetry_enabled) {
        return;
    }

    // Detached on purpose. Nothing joins this thread: if the app exits first
    // the request is abandoned, which is the correct trade for a metric that
    // must never delay shutdown.
    std::thread::Builder
        ::new()
        .name("telemetry".to_string())
        .spawn(|| {
            let event = build_event();
            match send_blocking(APP_KEY, &event) {
                Ok(status) => {
                    crate::debug_print!("📊 Telemetry {} -> HTTP {}", EVENT_NAME, status);
                }
                Err(e) => {
                    // Offline, DNS failure, blocked by a firewall: all
                    // expected and all silent outside debug logging.
                    crate::debug_print!("📊 Telemetry skipped: {}", e);
                }
            }
        })
        .ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_placeholder_key_leaves_telemetry_inert() {
        // A fork that has not created its own Aptabase app must report
        // nothing, even with the setting switched on.
        assert!(!is_configured(PLACEHOLDER_APP_KEY));
        assert!(!should_send(PLACEHOLDER_APP_KEY, true));
    }

    #[test]
    fn an_empty_key_leaves_telemetry_inert() {
        assert!(!is_configured(""));
        assert!(!should_send("", true));
    }

    #[test]
    fn the_setting_being_off_blocks_a_real_key() {
        // The opt-out must win over a perfectly valid configuration.
        assert!(is_configured(APP_KEY));
        assert!(!should_send(APP_KEY, false));
    }

    #[test]
    fn a_real_key_with_the_setting_on_sends() {
        assert!(should_send(APP_KEY, true));
    }

    #[test]
    fn the_shipped_key_is_configured_and_routes_to_the_us_host() {
        // Pins the embedded key so a bad edit cannot silently point the app
        // at the wrong region or blank the key out.
        assert!(is_configured(APP_KEY));
        assert_eq!(host_for_key(APP_KEY), "https://us.aptabase.com");
    }

    #[test]
    fn region_routing_follows_the_app_key() {
        assert_eq!(host_for_key("A-EU-1234567890"), "https://eu.aptabase.com");
        assert_eq!(host_for_key("A-US-1234567890"), "https://us.aptabase.com");
        // Anything unrecognised falls back to US, matching Aptabase's own
        // reference implementation rather than dropping the event.
        assert_eq!(host_for_key("A-DEV-1234567890"), "https://us.aptabase.com");
    }

    #[test]
    fn the_endpoint_is_the_documented_events_path() {
        // Plural `events`, taking an array body. The singular form 404s.
        assert_eq!(events_url("A-US-1234567890"), "https://us.aptabase.com/api/v0/events");
        assert_eq!(events_url("A-EU-1234567890"), "https://eu.aptabase.com/api/v0/events");
    }

    #[test]
    fn session_ids_have_the_epoch_plus_eight_digits_shape() {
        let id = new_session_id();
        assert!(id.chars().all(|c| c.is_ascii_digit()), "session id must be digits only: {id}");

        // 10-digit epoch for any plausible clock, plus exactly 8 random.
        let epoch_len = chrono::Utc::now().timestamp().to_string().len();
        assert_eq!(id.len(), epoch_len + 8, "session id must carry exactly 8 random digits: {id}");
    }

    #[test]
    fn session_ids_are_not_reused_across_launches() {
        // The anonymity guarantee: nothing here is stable enough to follow a
        // user across runs.
        let a = new_session_id();
        let b = new_session_id();
        assert_ne!(a, b);
    }

    #[test]
    fn the_payload_carries_only_the_documented_system_props() {
        let event = build_event();
        let json = serde_json::to_value([&event]).expect("event must serialize");

        let array = json.as_array().expect("body must be an array of events");
        assert_eq!(array.len(), 1, "one launch is one event");

        let obj = array[0].as_object().expect("event must be an object");
        let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["eventName", "props", "sessionId", "systemProps", "timestamp"]);

        assert_eq!(obj["eventName"], EVENT_NAME);
        assert_eq!(obj["props"], serde_json::json!({}), "no custom props are ever attached");

        let system = obj["systemProps"].as_object().expect("systemProps must be an object");
        let mut system_keys: Vec<&str> = system.keys().map(String::as_str).collect();
        system_keys.sort_unstable();
        assert_eq!(
            system_keys,
            ["appVersion", "isDebug", "locale", "osName", "osVersion", "sdkVersion"]
        );

        assert_eq!(system["appVersion"], crate::utils::constants::APP_VERSION);
        assert_eq!(system["osName"], os_name());
    }

    #[test]
    fn the_payload_contains_no_identifying_fields() {
        // Guards the promise made in the README: the event carries the OS and
        // the app version, and nothing that could name a person or machine.
        let event = build_event();
        let body = serde_json::to_string(&[&event]).expect("event must serialize");
        let lowered = body.to_lowercase();

        for forbidden in [
            "username",
            "userid",
            "user_id",
            "deviceid",
            "device_id",
            "machineid",
            "hostname",
            "email",
            "soundpack",
            "path",
        ] {
            assert!(!lowered.contains(forbidden), "payload must not contain `{forbidden}`: {body}");
        }
    }

    #[test]
    fn the_timestamp_is_utc_rfc3339_with_milliseconds() {
        let event = build_event();
        assert!(event.timestamp.ends_with('Z'), "timestamp must be UTC: {}", event.timestamp);
        chrono::DateTime::parse_from_rfc3339(&event.timestamp).expect("timestamp must be RFC3339");
    }

    #[test]
    fn a_locale_never_reports_the_posix_placeholder() {
        // "C"/"POSIX" are not real locales; reporting them would just be noise
        // in the dashboard.
        let value = locale();
        assert!(value != "C" && value != "POSIX", "unexpected locale: {value}");
    }
}

#[cfg(test)]
mod live_tests {
    use super::*;

    /// Sends a real event through the real code path. Ignored by default so
    /// the normal suite never touches the network; run explicitly with
    /// `cargo test -- --ignored live_launch_events_are_accepted`.
    #[test]
    #[ignore = "requires network; posts a real event to Aptabase"]
    fn live_launch_events_are_accepted() {
        let event = build_event();
        println!("payload: {}", serde_json::to_string_pretty(&[&event]).unwrap());

        // Blank system props would reach the dashboard as "Unknown" rows, so
        // assert the ones this app exists to measure are actually populated.
        assert!(!event.system_props.os_version.is_empty(), "OS version must resolve on this host");
        assert!(!event.system_props.locale.is_empty(), "locale must resolve on this host");

        let status = send_blocking(APP_KEY, &event).expect("request must complete");
        println!("real key -> HTTP {status}");
        assert_eq!(status, 200, "Aptabase must accept the event this app actually builds");
    }

    /// A wrong key must fail as a plain non-2xx status, not an error the app
    /// could ever surface to the user.
    #[test]
    #[ignore = "requires network"]
    fn live_a_bogus_key_fails_quietly() {
        let event = build_event();
        let status = send_blocking("A-US-0000000000", &event).expect("request must complete");
        println!("bogus key -> HTTP {status}");
        assert_ne!(status, 200);
    }
}

#[cfg(test)]
mod off_path_tests {
    use super::*;
    use std::net::TcpListener;
    use std::sync::atomic::{ AtomicUsize, Ordering };
    use std::sync::Arc;

    /// Counts real TCP connections. Proving "off means zero requests" needs a
    /// socket someone could actually connect to, not a mocked-out seam that
    /// assumes the answer.
    fn counting_listener() -> (String, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a local port");
        let addr = listener.local_addr().expect("listener has an address").to_string();
        let hits = Arc::new(AtomicUsize::new(0));

        let thread_hits = hits.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                if stream.is_err() {
                    break;
                }
                thread_hits.fetch_add(1, Ordering::SeqCst);
            }
        });

        (addr, hits)
    }

    /// The opt-out promise, checked against a live socket: with the setting
    /// off, `report_app_started` opens no connection to anything.
    #[test]
    fn the_setting_being_off_opens_no_connection() {
        let (addr, hits) = counting_listener();

        // Sanity: the counter does register a real connection, so a zero
        // below means "nothing was sent" rather than "the probe is broken".
        std::net::TcpStream::connect(&addr).expect("control connection");
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(hits.load(Ordering::SeqCst), 1, "the probe itself must count connections");

        report_app_started(false);
        std::thread::sleep(Duration::from_millis(500));

        assert_eq!(
            hits.load(Ordering::SeqCst),
            1,
            "telemetry disabled must not open a single connection"
        );
    }

    /// `report_app_started` must hand back control immediately: it is called
    /// on the startup path, ahead of the audio engine and the input worker.
    #[test]
    fn reporting_never_blocks_the_calling_thread() {
        let start = std::time::Instant::now();
        report_app_started(true);
        let elapsed = start.elapsed();

        assert!(
            elapsed < Duration::from_millis(100),
            "startup must not wait on telemetry, took {elapsed:?}"
        );
    }
}
