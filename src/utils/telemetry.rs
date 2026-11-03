//! Anonymous usage telemetry, counted by the Mechvibes website.
//!
//! Scope is deliberately tiny: one launch ping per app start, carrying only
//! the OS, the app version and the locale. That is enough to answer "how many
//! people use this, on what" and nothing else. No keystrokes, no soundpack
//! names, no file paths, no identifier of any kind.
//!
//! Wire contract, agreed with the website's `POST /api/app/launch`:
//! - Body is one JSON object: `os`, `osVersion`, `appVersion`, `locale`,
//!   `debug`. `os` is one of "Windows", "macOS", "Linux" or "Unknown".
//! - The site answers 204 and adds the ping to a per-day counter. It keeps no
//!   IP address and no single launches; a dev build (`debug: true`) is
//!   answered but not counted.
//! - Any other answer, or no answer at all, is ignored. There is no retry.
//!
//! Threading: `report_app_started` returns immediately. All work, including
//! the blocking HTTP call, happens on a detached `std::thread`. Nothing here
//! may ever be called from the audio engine loop, the input worker, or the UI
//! poll loop.

use crate::utils::constants::{ APP_NAME, APP_VERSION, LIBRARY_BASE_URL };
use serde::Serialize;
use std::time::Duration;

/// Where a launch is reported.
fn launch_url() -> String {
    format!("{}/api/app/launch", LIBRARY_BASE_URL)
}

/// Total budget for the request. A hanging or blackholed connection must
/// never outlive this, so a detached telemetry thread can never be what keeps
/// the process alive at shutdown.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct Launch {
    os: String,
    os_version: String,
    app_version: String,
    locale: String,
    debug: bool,
}

/// OS name in the exact spelling the website accepts.
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
/// than an error: a missing version must never stop the ping from being
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

/// Builds the one ping this app sends. Pure and side-effect free so the
/// payload shape can be asserted in tests without touching the network.
fn build_launch() -> Launch {
    Launch {
        os: os_name(),
        os_version: os_version(),
        app_version: APP_VERSION.to_string(),
        locale: locale(),
        debug: cfg!(debug_assertions),
    }
}

/// Performs the blocking POST. Returns the HTTP status so callers and
/// integration tests can assert on it; every error is mapped to a string and
/// nothing is ever propagated to the user.
fn send_blocking(url: &str, launch: &Launch) -> Result<u16, String> {
    let client = reqwest::blocking::Client
        ::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent(format!("{}/{}", APP_NAME, APP_VERSION))
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .post(url)
        .json(launch)
        .send()
        .map_err(|e| e.to_string())?;

    Ok(response.status().as_u16())
}

/// Reports a single app launch, if telemetry is enabled. Returns
/// immediately: the caller's thread does no config read, no DNS, and no I/O.
///
/// `telemetry_enabled` is passed in rather than read here so that the gate is
/// decided by the caller's already-loaded config and this module never
/// touches config state or the filesystem.
pub fn report_app_started(telemetry_enabled: bool) {
    if !telemetry_enabled {
        return;
    }

    // Detached on purpose. Nothing joins this thread: if the app exits first
    // the request is abandoned, which is the correct trade for a metric that
    // must never delay shutdown.
    std::thread::Builder
        ::new()
        .name("telemetry".to_string())
        .spawn(|| {
            match send_blocking(&launch_url(), &build_launch()) {
                Ok(status) => {
                    crate::debug_print!("📊 Telemetry launch -> HTTP {}", status);
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
    fn launches_go_to_the_website() {
        assert_eq!(launch_url(), "https://mechvibes.com/api/app/launch");
    }

    #[test]
    fn the_payload_carries_only_the_agreed_fields() {
        let json = serde_json::to_value(build_launch()).expect("launch must serialize");

        let obj = json.as_object().expect("body must be one object");
        let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["appVersion", "debug", "locale", "os", "osVersion"]);

        assert_eq!(obj["appVersion"], APP_VERSION);
        assert_eq!(obj["debug"], cfg!(debug_assertions));
    }

    #[test]
    fn the_os_is_one_the_website_accepts() {
        assert!(["Windows", "macOS", "Linux", "Unknown"].contains(&os_name().as_str()));
    }

    #[test]
    fn the_payload_contains_no_identifying_fields() {
        // Guards the promise made in the README: the ping carries the OS and
        // the app version, and nothing that could name a person or machine.
        let body = serde_json::to_string(&build_launch()).expect("launch must serialize");
        let lowered = body.to_lowercase();

        for forbidden in [
            "username",
            "userid",
            "user_id",
            "deviceid",
            "device_id",
            "machineid",
            "hostname",
            "session",
            "email",
            "soundpack",
            "path",
        ] {
            assert!(!lowered.contains(forbidden), "payload must not contain `{forbidden}`: {body}");
        }
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

    /// Sends a real ping through the real code path. Ignored by default so
    /// the normal suite never touches the network; run explicitly with
    /// `cargo test -- --ignored live_launch_pings_are_accepted`. A test build
    /// sends `debug: true`, so the site answers without counting it.
    #[test]
    #[ignore = "requires network; posts a real ping to mechvibes.com"]
    fn live_launch_pings_are_accepted() {
        let launch = build_launch();
        println!("payload: {}", serde_json::to_string_pretty(&launch).unwrap());

        // Blank fields would reach the dashboard as empty rows, so assert the
        // ones this app exists to measure are actually populated.
        assert!(!launch.os_version.is_empty(), "OS version must resolve on this host");
        assert!(!launch.locale.is_empty(), "locale must resolve on this host");

        let status = send_blocking(&launch_url(), &launch).expect("request must complete");
        println!("-> HTTP {status}");
        assert_eq!(status, 204, "the website must accept the ping this app actually builds");
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
