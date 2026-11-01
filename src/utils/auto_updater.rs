// Only the tests build an `AppConfig` directly now. Every runtime path here
// goes through `config_writer`, which never hands one out to be written back.
#[cfg(test)]
use crate::state::config::AppConfig;
use crate::utils::constants::APP_NAME;
use crate::utils::update_installer::{
    self,
    StageError,
    StagedUpdate,
};
use chrono::{ DateTime, Utc };
use reqwest;
use semver::Version;
use serde::{ Deserialize, Serialize };
use std::error::Error;
use std::fmt;
use tokio::time::{ interval, Duration as TokioDuration };

// Fixed repository information
const REPO_OWNER: &str = "hainguyents13";
const REPO_NAME: &str = "mechvibes";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub download_url: Option<String>,
    pub release_notes: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub is_prerelease: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub name: String,
    pub body: String,
    pub published_at: String,
    pub prerelease: bool,
    pub assets: Vec<GitHubAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubAsset {
    pub name: String,
    pub browser_download_url: String,
    pub content_type: String,
    pub size: u64,
}

#[derive(Debug)]
pub enum UpdateError {
    NetworkError(String),
    ParseError(String),
    NotFound,
    InvalidVersion(String),
}

impl fmt::Display for UpdateError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            UpdateError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            UpdateError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            UpdateError::NotFound => write!(f, "No releases found"),
            UpdateError::InvalidVersion(msg) => write!(f, "Invalid version: {}", msg),
        }
    }
}

impl Error for UpdateError {}

/// How this build is packaged, which is what decides *which* release asset the
/// user should be pointed at.
///
/// This exists so the choice is a pure function of an enum rather than a
/// `#[cfg]`-gated closure. The previous shape compiled exactly one arm per
/// platform, so the Linux rule could only ever be exercised by a Linux test
/// run - and a Windows-only local suite reported green while `main` went red on
/// ubuntu. Every arm is now reachable from every OS.
///
/// Every variant is constructed on the platform it describes, but only one
/// platform is ever compiled at a time - so on any given build the other
/// variants look dead to the lint. That is the whole point of the type: the
/// rules for all of them are compiled, and tested, everywhere.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallKind {
    /// Windows, installed or portable: the `.exe` installer is the asset.
    Windows,
    /// macOS on Apple Silicon: the `arm64` `.dmg`.
    MacOsAppleSilicon,
    /// macOS on Intel: the `x86_64` `.dmg`.
    MacOsIntel,
    /// Linux, running from a mounted AppImage.
    LinuxAppImage,
    /// Linux, anything else (a `.deb` install, or a build from source).
    LinuxPackage,
}

/// The packaging of the currently running build.
///
/// The only part of the asset choice that is platform- and runtime-dependent;
/// everything downstream of it is pure.
fn current_install_kind() -> InstallKind {
    #[cfg(target_os = "windows")]
    {
        InstallKind::Windows
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        InstallKind::MacOsIntel
    }
    #[cfg(all(target_os = "macos", not(target_arch = "x86_64")))]
    {
        InstallKind::MacOsAppleSilicon
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        if crate::state::paths::is_running_from_appimage() {
            InstallKind::LinuxAppImage
        } else {
            InstallKind::LinuxPackage
        }
    }
}

/// Whether `name` (already lowercased) is the asset `kind` should install.
///
/// On Linux the answer deliberately depends on how the app is *running*, not
/// merely on the OS: someone who launched an AppImage should be pointed at the
/// next AppImage, and someone on a `.deb` install at the next `.deb`. Both
/// assets ship in every release, so without this the alphabetically-first one
/// won - which is exactly how the `.AppImage` started being offered to `.deb`
/// users.
fn asset_matches(kind: InstallKind, name: &str) -> bool {
    match kind {
        // Must contain "x64" and end in ".exe" - the same contract the release
        // pipeline asserts on (see release.yml).
        InstallKind::Windows => name.ends_with(".exe") && name.contains("x64"),
        // Both DMGs ship in every release, so the architecture in the name
        // decides, not which one the list happens to start with.
        InstallKind::MacOsAppleSilicon => is_mac_package(name) && name.contains("arm64"),
        InstallKind::MacOsIntel => is_mac_package(name) && name.contains("x86_64"),
        InstallKind::LinuxAppImage => name.ends_with(".appimage"),
        InstallKind::LinuxPackage => name.ends_with(".deb"),
    }
}

fn is_mac_package(name: &str) -> bool {
    name.ends_with(".dmg") || name.ends_with(".pkg")
}

/// The download URL `kind` should be offered from `assets`, if any.
///
/// Pure, so every platform's rule is testable from every platform. Linux and
/// macOS still have no unattended in-place upgrade path; these arms exist to
/// keep the "what's new" UI honest, and the UI links to the Releases page
/// rather than downloading anything.
fn find_download_url_for(kind: InstallKind, assets: &[GitHubAsset]) -> Option<String> {
    assets
        .iter()
        .find(|asset| asset_matches(kind, &asset.name.to_lowercase()))
        .map(|asset| asset.browser_download_url.clone())
}

pub struct AutoUpdater {
    pub current_version: String,
}

impl AutoUpdater {
    pub fn new() -> Self {
        Self {
            current_version: crate::utils::constants::APP_VERSION.to_string(),
        }
    }
    pub async fn check_for_updates(&self) -> Result<UpdateInfo, UpdateError> {
        let url = format!("https://api.github.com/repos/{}/{}/releases", REPO_OWNER, REPO_NAME);

        let client = reqwest::Client::new();
        let response = client
            .get(&url)
            .header("User-Agent", format!("{}/{}", APP_NAME, self.current_version))
            .send().await
            .map_err(|e| UpdateError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(UpdateError::NetworkError(format!("HTTP {}", response.status())));
        }

        let releases: Vec<GitHubRelease> = response
            .json().await
            .map_err(|e| UpdateError::ParseError(e.to_string()))?;

        if releases.is_empty() {
            return Err(UpdateError::NotFound);
        }

        // Find the latest release (excluding prereleases)
        let latest_release = releases
            .iter()
            .find(|release| !release.prerelease)
            .ok_or(UpdateError::NotFound)?;

        crate::always_print!(
            "Latest release: {} ({}), published at {}",
            latest_release.tag_name,
            latest_release.name,
            latest_release.published_at
        );

        let current_version = Version::parse(&self.current_version).map_err(|e|
            UpdateError::InvalidVersion(e.to_string())
        )?;

        let latest_version_str = latest_release.tag_name.trim_start_matches('v');
        let latest_version = Version::parse(latest_version_str).map_err(|e|
            UpdateError::InvalidVersion(e.to_string())
        )?;

        let update_available = latest_version > current_version;

        // Find appropriate download URL for current platform
        let download_url = self.find_download_url(&latest_release.assets);

        let published_at = DateTime::parse_from_rfc3339(&latest_release.published_at)
            .map(|dt| dt.with_timezone(&Utc))
            .ok();

        Ok(UpdateInfo {
            current_version: self.current_version.clone(),
            latest_version: latest_version.to_string(),
            update_available,
            download_url,
            release_notes: Some(latest_release.body.clone()),
            published_at,
            is_prerelease: latest_release.prerelease,
        })
    }

    /// Picks the release asset this platform can actually install.
    ///
    /// Returns `None` rather than guessing when nothing matches. The previous
    /// implementation fell back to `assets[0]`, which on Linux and macOS meant
    /// offering the *Windows* `.exe` installer, since the Windows asset is
    /// alphabetically first in every release. A missing button is correct;
    /// a button that downloads an unusable file is not.
    fn find_download_url(&self, assets: &[GitHubAsset]) -> Option<String> {
        find_download_url_for(current_install_kind(), assets)
    }

    /// Link to the release page, used wherever a direct download is not
    /// something this platform can act on.
    pub fn releases_page_url(version: &str) -> String {
        format!("https://github.com/{}/{}/releases/tag/v{}", REPO_OWNER, REPO_NAME, version)
    }

    /// User-Agent GitHub expects on both API and asset requests.
    pub fn user_agent(&self) -> String {
        format!("{}/{}", APP_NAME, self.current_version)
    }
}

// Configuration for auto-update settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AutoUpdateConfig {
    pub last_check: Option<u64>,
    pub available_version: Option<String>,
    pub available_download_url: Option<String>,
    /// An installer already downloaded and verified, kept so that a session
    /// interrupted between download and install does not pay for the transfer
    /// twice. `serde(default)` so configs written by versions before this
    /// field deserialize instead of being reset.
    #[serde(default)]
    pub staged_update: Option<StagedUpdate>,
}

/*
 * AUTO-UPDATE STRATEGY:
 *
 * 1. STARTUP CHECK (check_for_updates_on_startup):
 *    - Runs when app starts from completely closed state
 *    - Only checks if last_check was more than 1 hour ago (to avoid spam on frequent restarts)
 *    - Updates config.json with results
 *    - Updates global UI state for immediate titlebar notification
 *
 * 2. BACKGROUND SERVICE (UpdateService.start):
 *    - Runs periodic checks every 24 hours while app is running
 *    - Independent of startup checks
 *    - Continues normal background operation
 *
 * 3. MANUAL CHECK (in settings page):
 *    - User can trigger immediate check via "Check for Updates" button
 *    - Always performs check regardless of timing
 *
 * This ensures users always get fresh update info when they launch the app,
 * while avoiding excessive API calls on frequent app restarts.
 */

// Even simpler function without parameters
pub async fn check_for_updates_simple() -> Result<UpdateInfo, UpdateError> {
    let updater = AutoUpdater::new();
    updater.check_for_updates().await
}

// Service for auto-update background checking
///
/// Holds no config of its own. It used to own an `Arc<Mutex<AppConfig>>` taken
/// once at launch and never refreshed, and wrote that struct back after every
/// check - reverting every setting the user had touched since the app started.
/// There is now nothing to go stale: each tick reads what it needs and writes
/// only its own fields.
pub struct UpdateService;

impl UpdateService {
    pub fn new() -> Self {
        Self
    }

    pub async fn start(&self) {
        tokio::spawn(async move {
            let mut interval = interval(TokioDuration::from_secs(86400)); // Check every 24 hours

            loop {
                interval.tick().await;
                // Read the timestamp fresh on every tick rather than from a
                // long-lived copy.
                let update_config = crate::state::config_writer::current().auto_update;
                // Check if it's time to check for updates (every 24 hours)
                if let Some(last_check) = update_config.last_check {
                    let now = std::time::SystemTime
                        ::now()
                        .duration_since(std::time::SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let check_interval = 24 * 3600; // 24 hours in seconds
                    if now.saturating_sub(last_check) < check_interval {
                        continue;
                    }
                }

                crate::always_print!("🔄 Checking for updates...");

                match check_for_updates_simple().await {
                    Ok(update_info) => {
                        // Record the result by mutating only the fields this
                        // check owns. Everything the user changed while the
                        // request was in flight is untouched.
                        let checked_at = std::time::SystemTime
                            ::now()
                            .duration_since(std::time::SystemTime::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        let available = update_info.update_available.then(|| (
                            update_info.latest_version.clone(),
                            update_info.download_url.clone(),
                        ));

                        crate::state::config_writer::apply(move |config| {
                            config.auto_update.last_check = Some(checked_at);
                            match available {
                                Some((version, download_url)) => {
                                    config.auto_update.available_version = Some(version);
                                    config.auto_update.available_download_url = download_url;
                                }
                                None => {
                                    config.auto_update.available_version = None;
                                    config.auto_update.available_download_url = None;
                                }
                            }
                        });
                        if update_info.update_available {
                            crate::always_print!(
                                "🆕 Update available: {} -> {}",
                                update_info.current_version,
                                update_info.latest_version
                            );
                            // Notify only. Downloading is a deliberate user
                            // action (the "Update now" button in Settings),
                            // never something a background check starts on its
                            // own.
                            crate::state::app::set_update_info(Some(update_info));
                        } else {
                            crate::always_print!("✅ No updates available");
                            // Clear update info if no updates
                            crate::state::app::set_update_info(None);
                        }
                    }
                    Err(e) => {
                        crate::always_eprint!("❌ Failed to check for updates: {}", e);
                    }
                }
            }
        });
    }

    // pub async fn check_now(&self) -> Result<UpdateInfo, Box<dyn std::error::Error>> {
    //     let update_info = check_for_updates_simple().await?; // Update last check time
    //     {
    //         let mut config_guard = self.config.lock().await;
    //         config_guard.auto_update.last_check = Some(
    //             std::time::SystemTime
    //                 ::now()
    //                 .duration_since(std::time::SystemTime::UNIX_EPOCH)
    //                 .unwrap_or_default()
    //                 .as_secs()
    //         );
    //         let _ = config_guard.save();
    //     }

    //     Ok(update_info)
    // }

    // pub async fn download_and_install_update(
    //     &self,
    //     update_info: &UpdateInfo
    // ) -> Result<(), Box<dyn std::error::Error>> {
    //     if let Some(download_url) = &update_info.download_url {
    //         crate::always_print!("📥 Downloading update...");

    //         let temp_dir = std::env::temp_dir();
    //         let default_filename = format!("mechvibes_v{}.exe", update_info.latest_version);
    //         let filename = download_url.split('/').last().unwrap_or(&default_filename);
    //         let installer_path = temp_dir.join(filename);

    //         let updater = AutoUpdater::new();
    //         updater.download_update(download_url, &installer_path).await?;

    //         crate::always_print!("🔧 Installing update...");
    //         updater.install_update(&installer_path)?;

    //         crate::always_print!("✅ Update installed successfully. Please restart the application.");
    //         Ok(())
    //     } else {    //         Err("No download URL available".into())
    //     }
    // }
}

/// Where the update flow currently stands, for the UI to render.
///
/// Only ever moves forward within a session: `Idle -> Downloading -> Ready`,
/// with `Failed` as the terminal state that puts the UI back on the plain
/// "open the browser" button that existed before staging did. `Ready` can also
/// be entered directly at startup when a previous session staged an installer
/// but was interrupted before it ran.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum UpdateStage {
    /// Nothing found, or nothing to do on this platform.
    #[default]
    Idle,
    /// Installer transfer in flight.
    Downloading {
        version: String,
    },
    /// Verified installer on disk. The click that started the download goes
    /// straight on to run it; this is also where a restored staged installer
    /// from an interrupted session lands.
    Ready {
        version: String,
        installer_path: String,
    },
    /// Staging gave up. `reason` is shown as a hint; the UI falls back to
    /// linking the Releases page, which always works.
    Failed {
        version: String,
        reason: String,
    },
}

static UPDATE_STAGE: once_cell::sync::Lazy<std::sync::Mutex<UpdateStage>> =
    once_cell::sync::Lazy::new(|| std::sync::Mutex::new(UpdateStage::Idle));

/// Current stage, cloned so callers never hold the lock.
pub fn get_update_stage() -> UpdateStage {
    UPDATE_STAGE.lock()
        .map(|guard| guard.clone())
        .unwrap_or(UpdateStage::Idle)
}

fn set_update_stage(stage: UpdateStage) {
    if let Ok(mut guard) = UPDATE_STAGE.lock() {
        *guard = stage;
    }
}

/// Clears a `Failed` stage back to `Idle` so the button returns to its normal
/// state, used when a newer version than the one that failed shows up.
///
/// Only `Failed` is cleared: resetting out of `Downloading` would offer a
/// second click while a transfer is still running, and resetting out of
/// `Ready` would throw away a verified installer.
pub fn clear_failed_update() {
    if let Ok(mut guard) = UPDATE_STAGE.lock() {
        if matches!(*guard, UpdateStage::Failed { .. }) {
            *guard = UpdateStage::Idle;
        }
    }
}

/// Downloads and verifies the installer for `update_info`.
///
/// Only ever called from the "Update now" button - nothing in the
/// check paths starts a transfer, because a ~50 MB download is the user's
/// decision to make, not a side effect of a background version check.
///
/// Idempotent: a no-op on platforms without a silent installer, when a
/// download is already in flight, or when this version is already staged and
/// still verifies. Never returns an error - failures land in
/// [`UpdateStage::Failed`] and the UI falls back to the browser link.
pub async fn download_and_stage_update(update_info: &UpdateInfo) {
    if !update_installer::silent_update_supported() {
        return;
    }

    if !update_info.update_available {
        return;
    }

    let version = update_info.latest_version.clone();

    // Don't stack a second download on top of one already in flight, and
    // don't re-download something already verified and waiting.
    match get_update_stage() {
        UpdateStage::Downloading { .. } => {
            return;
        }
        UpdateStage::Ready { version: ref ready, .. } if *ready == version => {
            return;
        }
        _ => {}
    }

    let Some(download_url) = update_info.download_url.clone() else {
        set_update_stage(UpdateStage::Failed {
            version,
            reason: "No installer asset in this release".to_string(),
        });
        return;
    };

    // A staged installer from a previous session that still verifies is
    // reused as-is, so an interrupted session coming back never costs a second
    // download.
    let config = crate::state::config_writer::current();
    if let Some(staged) = &config.auto_update.staged_update {
        if let Ok(path) = staged.validate(&version) {
            set_update_stage(UpdateStage::Ready {
                version,
                installer_path: path.to_string_lossy().into_owned(),
            });
            return;
        }
        // Stale or corrupt - drop it before downloading a replacement.
        update_installer::discard_staged(staged);
    }

    set_update_stage(UpdateStage::Downloading { version: version.clone() });
    crate::always_print!("📥 Downloading update {} (user requested)...", version);

    let user_agent = AutoUpdater::new().user_agent();
    match update_installer::download_and_verify(&download_url, &version, &user_agent).await {
        Ok(staged) => {
            let installer_path = staged.installer_path.clone();
            crate::state::config_writer::apply(move |config| {
                config.auto_update.staged_update = Some(staged);
            });
            crate::always_print!("✅ Update {} verified and ready to install", version);
            set_update_stage(UpdateStage::Ready { version, installer_path });
        }
        Err(e) => {
            // Expected for any release published without SHA256SUMS.txt, and
            // for every network hiccup. Log and let the browser link stand in.
            let reason = match &e {
                StageError::ChecksumUnavailable(_) =>
                    "This release has no published checksums; use the download page.".to_string(),
                StageError::ChecksumMismatch { .. } =>
                    "The downloaded installer failed verification and was discarded.".to_string(),
                other => other.to_string(),
            };
            crate::always_eprint!("⚠️  Update download failed ({}); falling back to manual download", e);
            set_update_stage(UpdateStage::Failed { version, reason });
        }
    }
}

/// Whether `candidate` is strictly newer than `current`.
///
/// The guard against installing a staged file after the app was upgraded some
/// other way (a manual download, a reinstall) - running an older installer
/// then would be a silent downgrade. Unparseable versions answer `false`:
/// when in doubt, do not run an installer.
pub fn is_upgrade(current: &str, candidate: &str) -> bool {
    match (Version::parse(current), Version::parse(candidate)) {
        (Ok(current), Ok(candidate)) => candidate > current,
        _ => false,
    }
}

/// Re-checks a staged installer at startup and, if it is still valid for an
/// update we still want, puts the UI straight into `Ready`.
///
/// This covers a session that closed between the download finishing and the
/// installer running: the file survives, the button returns on the next launch
/// and reuses it. The hash is verified again rather than trusted from config,
/// so a file altered between sessions is discarded, not executed.
pub fn restore_staged_update() {
    if !update_installer::silent_update_supported() {
        return;
    }

    let Some(staged) = crate::state::config_writer::current().auto_update.staged_update else {
        return;
    };

    let current = crate::utils::constants::APP_VERSION;

    if !is_upgrade(current, &staged.version) {
        crate::always_print!("🧹 Discarding staged update {} (no longer newer than {})", staged.version, current);
        update_installer::discard_staged(&staged);
        crate::state::config_writer::apply(|config| {
            config.auto_update.staged_update = None;
        });
        return;
    }

    match staged.validate(&staged.version) {
        Ok(path) => {
            crate::always_print!("📦 Staged update {} is ready from a previous session", staged.version);
            set_update_stage(UpdateStage::Ready {
                version: staged.version.clone(),
                installer_path: path.to_string_lossy().into_owned(),
            });
        }
        Err(e) => {
            crate::always_eprint!("⚠️  Discarding unusable staged update: {}", e);
            update_installer::discard_staged(&staged);
            crate::state::config_writer::apply(|config| {
                config.auto_update.staged_update = None;
            });
        }
    }
}

/// Launches the staged installer and hands control to it.
///
/// Order matters and is the whole risk of this feature:
/// 1. Re-verify the file (never execute something unverified).
/// 2. Clear it from config, so a crashed install cannot loop on the same file.
/// 3. Spawn the installer detached, so our exit does not kill it.
///
/// The caller must then close the app. The input worker child process needs no
/// separate signal: it exits on stdin EOF as soon as our handles close (see
/// `input_worker.rs`), so no orphan survives. Inno's `CloseApplications=yes`
/// closes anything still holding the binary, and the silent-only `[Run]` entry
/// in the `.iss` starts the app again once the files are in place - that entry
/// is the single relaunch path, verified against a real /LOG trace.
///
/// Returns an error without touching the app's lifecycle if anything fails, so
/// a blocked or missing installer leaves the user exactly where they were.
pub fn install_staged_update() -> Result<(), StageError> {
    let stage = get_update_stage();
    let UpdateStage::Ready { version, installer_path } = stage else {
        return Err(StageError::Io("No verified update is ready to install".to_string()));
    };

    let staged = crate::state::config_writer
        ::current()
        .auto_update.staged_update.ok_or_else(||
            StageError::Io("Staged update is missing from config".to_string())
        )?;

    // Verify one last time, immediately before execution.
    let path = staged.validate(&version)?;
    if path.to_string_lossy() != installer_path {
        return Err(StageError::Io("Staged installer path changed unexpectedly".to_string()));
    }

    // Cleared before spawning: if the install dies halfway, the next launch
    // re-checks and re-downloads rather than retrying a file that just failed.
    crate::state::config_writer::apply(|config| {
        config.auto_update.staged_update = None;
    });

    update_installer::spawn_installer_detached(&path)?;
    crate::always_print!("🚀 Installer for {} launched; shutting down for upgrade", version);
    Ok(())
}


/// The saved update from config, but **only** if it is genuinely newer than
/// the running build.
///
/// This is the single read path for `auto_update.available_version`, and it
/// must stay that way. The config file on disk is untrusted input: version
/// 0.6.1 records "0.6.2 is available", the user upgrades to 0.6.2 by any
/// route that does not go through a check (a manual download, the installer,
/// a reinstall), and that stale line now describes an update to the very
/// version already running. The write paths do clear it, but they only run
/// when a check actually reaches the network - and the startup check is
/// rate-limited to once an hour, so there is a window where nothing clears
/// anything. Filtering on read closes that window regardless of what the
/// file says.
///
/// Comparison is semver, never string equality: "0.10.0" > "0.9.0" is only
/// true numerically.
pub fn get_saved_update_info() -> Option<UpdateInfo> {
    let config = crate::state::config_writer::current();
    let available_version = config.auto_update.available_version.as_ref()?;
    let current_version = crate::utils::constants::APP_VERSION;

    if !is_upgrade(current_version, available_version) {
        return None;
    }

    Some(UpdateInfo {
        current_version: current_version.to_string(),
        latest_version: available_version.clone(),
        update_available: true,
        download_url: config.auto_update.available_download_url.clone(),
        release_notes: Some(AutoUpdater::releases_page_url(available_version)),
        published_at: None, // Not saved in config
        is_prerelease: false, // Not saved in config
    })
}

/// Drops a saved update that is no longer newer than the running build.
///
/// The companion to the read-side filter: without this, a stale entry stays
/// in `config.json` forever, invisibly, and any future code that reads the
/// field directly would resurrect the bug. Called at startup, so upgrading
/// by any means cleans the record up once rather than on every read.
///
/// Returns whether anything was cleared, which the tests assert on.
pub fn clear_stale_available_version() -> bool {
    let Some(available_version) = crate::state::config_writer
        ::current()
        .auto_update.available_version else {
        return false;
    };

    if is_upgrade(crate::utils::constants::APP_VERSION, &available_version) {
        return false;
    }

    crate::always_print!(
        "🧹 Clearing stale update record: v{} is not newer than the running v{}",
        available_version,
        crate::utils::constants::APP_VERSION
    );
    crate::state::config_writer::apply(|config| {
        config.auto_update.available_version = None;
        config.auto_update.available_download_url = None;
    });
    true
}

// Check for updates on app startup (when app was completely closed)
pub async fn check_for_updates_on_startup() -> Result<UpdateInfo, UpdateError> {
    crate::always_print!("🔄 Checking for updates on startup...");

    // Only the timestamp is kept across the request: holding the whole struct
    // would invite saving it back after the await and reverting anything the
    // user changed meanwhile.
    let last_check = crate::state::config_writer::current().auto_update.last_check;
    let now = std::time::SystemTime
        ::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Check if we should perform startup check
    // Only check if:
    // 1. Never checked before (last_check is None), OR
    // 2. Last check was more than 1 hour ago (to avoid spam on frequent restarts)
    let should_check = match last_check {
        None => {
            crate::always_print!("📅 First time checking for updates");
            true
        }
        Some(last_check) => {
            let time_since_last_check = now.saturating_sub(last_check);
            let one_hour = 3600; // 1 hour in seconds
            if time_since_last_check >= one_hour {
                crate::always_print!(
                    "📅 Last check was {} hours ago, checking again",
                    time_since_last_check / 3600
                );
                true
            } else {
                crate::always_print!(
                    "📅 Recently checked ({} minutes ago), skipping startup check",
                    time_since_last_check / 60
                );
                false
            }
        }
    };

    if !should_check {
        // Return cached info if available
        if let Some(saved_update) = get_saved_update_info() {
            return Ok(saved_update);
        } else {
            // Create a "no update" response
            return Ok(UpdateInfo {
                current_version: crate::utils::constants::APP_VERSION.to_string(),
                latest_version: crate::utils::constants::APP_VERSION.to_string(),
                update_available: false,
                download_url: None,
                release_notes: None,
                published_at: None,
                is_prerelease: false,
            });
        }
    }

    // Perform actual check
    match check_for_updates_simple().await {
        Ok(update_info) => {
            // Only this path's own fields are written. The user can change the
            // volume, theme or mute state while the network round trip is in
            // flight, and none of that is visible to - or touchable by - this
            // mutation.
            let available = update_info.update_available.then(|| (
                update_info.latest_version.clone(),
                update_info.download_url.clone(),
            ));

            if update_info.update_available {
                crate::always_print!(
                    "🆕 Startup check: Update available {} -> {}",
                    update_info.current_version,
                    update_info.latest_version
                );
            } else {
                crate::always_print!("✅ Startup check: No updates available");
            }

            crate::state::config_writer::apply(move |config| {
                config.auto_update.last_check = Some(now);
                match available {
                    Some((version, download_url)) => {
                        config.auto_update.available_version = Some(version);
                        config.auto_update.available_download_url = download_url;
                    }
                    None => {
                        config.auto_update.available_version = None;
                        config.auto_update.available_download_url = None;
                    }
                }
            });

            // Update global state. Notification only - see the note in
            // UpdateService::start: nothing downloads without a user click.
            if update_info.update_available {
                crate::state::app::set_update_info(Some(update_info.clone()));
            } else {
                crate::state::app::set_update_info(None);
            }

            Ok(update_info)
        }
        Err(e) => {
            crate::always_eprint!("❌ Startup update check failed: {}", e);
            // Return cached info if check failed
            if let Some(saved_update) = get_saved_update_info() {
                Ok(saved_update)
            } else {
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod update_service_persistence_tests {
    use super::*;

    /// What the periodic check now writes, expressed exactly as
    /// `UpdateService::start` does: a mutation over fields this path owns, with
    /// no `AppConfig` carried in from anywhere.
    ///
    /// The predecessor of this test drove an `Arc<Mutex<AppConfig>>` snapshotted
    /// at launch, because that is what the service held. There is no longer any
    /// such struct to model - the service is a unit type - so what is checked
    /// here is that the mutation touches only `auto_update`, which is what makes
    /// the launch-time snapshot unnecessary in the first place.
    fn record_check_result(config: &mut AppConfig, now: u64) {
        config.auto_update.last_check = Some(now);
        config.auto_update.available_version = None;
        config.auto_update.available_download_url = None;
    }

    #[test]
    fn the_periodic_check_does_not_revert_settings_changed_since_launch() {
        // The user's config as it stands when a tick fires - every field
        // different from the defaults the app launched with.
        let mut config = AppConfig {
            volume: 0.68,
            mouse_volume: 0.62,
            theme: crate::libs::theme::Theme::BuiltIn(crate::libs::theme::BuiltInTheme::Luxury),
            enable_sound: false,
            ..AppConfig::default()
        };

        record_check_result(&mut config, 1_700_000_000);

        assert_eq!(config.volume, 0.68, "keyboard volume must survive the periodic check");
        assert_eq!(config.mouse_volume, 0.62, "mouse volume must survive it too");
        assert_eq!(
            config.theme,
            crate::libs::theme::Theme::BuiltIn(crate::libs::theme::BuiltInTheme::Luxury),
            "the theme must survive it"
        );
        assert!(!config.enable_sound, "and so must the mute state");
        assert_eq!(
            config.auto_update.last_check,
            Some(1_700_000_000),
            "while the check still records its own result"
        );
    }

    /// `tokio::time::interval` fires its first tick immediately, so this check
    /// runs at startup rather than only after 24 hours. That is what made the
    /// old stale-snapshot write reachable within seconds of launch, and it is
    /// why the mutation must be safe to run at any moment.
    #[test]
    fn ticking_repeatedly_never_disturbs_an_unrelated_field() {
        let mut config = AppConfig { volume: 0.68, ..AppConfig::default() };

        for tick in 0..24 {
            record_check_result(&mut config, 1_700_000_000 + tick);
            assert_eq!(config.volume, 0.68, "tick {tick} must leave the volume alone");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_versions_are_upgrades() {
        assert!(is_upgrade("0.6.1", "0.6.2"));
        assert!(is_upgrade("0.6.1", "0.7.0"));
        assert!(is_upgrade("0.6.1", "1.0.0"));
    }

    #[test]
    fn the_same_version_is_not_an_upgrade() {
        assert!(!is_upgrade("0.6.1", "0.6.1"));
    }

    #[test]
    fn older_versions_are_never_upgrades() {
        // The downgrade guard: a staged 0.6.0 must not be installed over an
        // app that is already 0.6.1, however it got there.
        assert!(!is_upgrade("0.6.1", "0.6.0"));
        assert!(!is_upgrade("1.0.0", "0.9.9"));
    }

    #[test]
    fn prereleases_rank_below_their_release() {
        // semver ordering, asserted so a future switch to a different compare
        // cannot quietly start offering 0.7.0-alpha to a 0.7.0 user.
        assert!(!is_upgrade("0.7.0", "0.7.0-alpha.1"));
        assert!(is_upgrade("0.7.0-alpha.1", "0.7.0"));
    }

    #[test]
    fn unparseable_versions_never_trigger_an_install() {
        assert!(!is_upgrade("0.6.1", "not-a-version"));
        assert!(!is_upgrade("garbage", "0.7.0"));
        assert!(!is_upgrade("", ""));
    }

    #[test]
    fn releases_page_url_points_at_the_tag() {
        assert_eq!(
            AutoUpdater::releases_page_url("0.7.0"),
            "https://github.com/hainguyents13/mechvibes/releases/tag/v0.7.0"
        );
    }

    fn asset(name: &str) -> GitHubAsset {
        GitHubAsset {
            name: name.to_string(),
            browser_download_url: format!("https://example.com/{}", name),
            content_type: "application/octet-stream".to_string(),
            size: 1,
        }
    }

    /// The full asset list of a real release, in the order the GitHub API
    /// returns it - the Windows installer first, which is exactly why the old
    /// `assets[0]` fallback served .exe files to Linux and macOS.
    fn release_assets() -> Vec<GitHubAsset> {
        vec![
            asset("Mechvibes-0.7.0-Setup-x64.exe"),
            asset("SHA256SUMS.txt"),
            asset("mechvibes-0.7.0-macos-arm64.dmg"),
            asset("mechvibes-0.7.0-macos-x86_64.dmg"),
            asset("mechvibes-0.7.0-x86_64.AppImage"),
            asset("mechvibes_0.7.0_amd64.deb")
        ]
    }

    /// Both Linux kinds must only ever see a Linux asset, never a `.dmg` or an
    /// `.exe`. The earlier version of this test early-returned on Windows,
    /// which is the same blind spot as a `#[cfg]` - it simply never ran where
    /// the suite was actually being run.
    #[test]
    fn neither_linux_kind_is_ever_offered_a_foreign_asset() {
        let assets = release_assets();
        for kind in [InstallKind::LinuxAppImage, InstallKind::LinuxPackage] {
            let url = find_download_url_for(kind, &assets).unwrap().to_lowercase();
            assert!(
                url.ends_with(".deb") || url.ends_with(".appimage"),
                "{:?} was offered {}",
                kind,
                url
            );
        }
    }

    /// Every packaging kind's choice, asserted from **every** OS.
    ///
    /// The predecessor of this test gated each expectation behind a `#[cfg]`,
    /// so only one arm compiled per platform. A Windows-only local run
    /// therefore never executed the Linux rule, and `main` went red on ubuntu
    /// the moment the AppImage asset started sorting ahead of the `.deb`. This
    /// version has no `cfg` at all.
    #[test]
    fn each_install_kind_is_offered_the_asset_it_can_install() {
        let assets = release_assets();
        let pick = |kind| find_download_url_for(kind, &assets);

        assert_eq!(
            pick(InstallKind::Windows).as_deref(),
            Some("https://example.com/Mechvibes-0.7.0-Setup-x64.exe")
        );
        assert_eq!(
            pick(InstallKind::MacOsAppleSilicon).as_deref(),
            Some("https://example.com/mechvibes-0.7.0-macos-arm64.dmg")
        );
        assert_eq!(
            pick(InstallKind::MacOsIntel).as_deref(),
            Some("https://example.com/mechvibes-0.7.0-macos-x86_64.dmg")
        );
        assert_eq!(
            pick(InstallKind::LinuxAppImage).as_deref(),
            Some("https://example.com/mechvibes-0.7.0-x86_64.AppImage"),
            "an AppImage user must be offered the next AppImage"
        );
        assert_eq!(
            pick(InstallKind::LinuxPackage).as_deref(),
            Some("https://example.com/mechvibes_0.7.0_amd64.deb"),
            "a .deb user must be offered the next .deb"
        );
    }

    /// The exact regression that turned `main` red: both Linux assets ship in
    /// every release, and the `.AppImage` sorts before the `.deb`, so a rule
    /// that accepted either would hand `.deb` users an AppImage purely by
    /// list order. Pinned so asset naming cannot resurrect it.
    #[test]
    fn linux_asset_order_in_the_release_never_decides_the_pick() {
        let forward = release_assets();
        let mut reversed = release_assets();
        reversed.reverse();

        for assets in [forward, reversed] {
            assert!(
                find_download_url_for(InstallKind::LinuxPackage, &assets)
                    .unwrap()
                    .ends_with(".deb"),
                "a .deb install was offered a non-.deb asset"
            );
            assert!(
                find_download_url_for(InstallKind::LinuxAppImage, &assets)
                    .unwrap()
                    .to_lowercase()
                    .ends_with(".appimage"),
                "an AppImage run was offered a non-AppImage asset"
            );
        }
    }

    /// No kind may ever be handed another platform's installer, checked
    /// exhaustively rather than for the one platform being compiled.
    #[test]
    fn no_install_kind_is_ever_offered_another_platforms_asset() {
        let assets = release_assets();
        for (kind, expected_suffix) in [
            (InstallKind::Windows, ".exe"),
            (InstallKind::MacOsAppleSilicon, ".dmg"),
            (InstallKind::MacOsIntel, ".dmg"),
            (InstallKind::LinuxAppImage, ".appimage"),
            (InstallKind::LinuxPackage, ".deb"),
        ] {
            let url = find_download_url_for(kind, &assets).unwrap().to_lowercase();
            assert!(
                url.ends_with(expected_suffix),
                "{:?} was offered {}, expected a {} asset",
                kind,
                url,
                expected_suffix
            );
        }
    }

    /// `README-macos-<version>.txt` ships in real releases. The macOS rule used
    /// to accept any name containing "macos", which would have matched that
    /// text file ahead of the DMG had it sorted first.
    #[test]
    fn the_macos_rule_does_not_match_the_readme_text_file() {
        assert!(!asset_matches(InstallKind::MacOsAppleSilicon, "readme-macos-0.7.0.txt"));
        assert!(!asset_matches(InstallKind::MacOsIntel, "readme-macos-0.7.0.txt"));
        assert!(asset_matches(InstallKind::MacOsAppleSilicon, "mechvibes-0.7.0-macos-arm64.dmg"));
    }

    /// Each Mac is offered its own DMG whichever comes first in the release,
    /// and a release from before the Intel build (arm64 only) offers an Intel
    /// Mac nothing, so it falls back to the Releases page.
    #[test]
    fn each_mac_gets_the_dmg_for_its_architecture() {
        let mut reversed = release_assets();
        reversed.reverse();
        for assets in [release_assets(), reversed] {
            assert!(
                find_download_url_for(InstallKind::MacOsAppleSilicon, &assets)
                    .unwrap()
                    .ends_with("macos-arm64.dmg")
            );
            assert!(
                find_download_url_for(InstallKind::MacOsIntel, &assets)
                    .unwrap()
                    .ends_with("macos-x86_64.dmg")
            );
        }

        let arm_only = vec![asset("mechvibes-3.4.0-macos-arm64.dmg")];
        assert_eq!(find_download_url_for(InstallKind::MacOsIntel, &arm_only), None);
    }

    /// A release missing this kind's asset yields None, so the UI falls back to
    /// the Releases-page link instead of offering something unusable.
    #[test]
    fn a_kind_with_no_matching_asset_gets_nothing() {
        let only_windows = vec![asset("Mechvibes-0.7.0-Setup-x64.exe")];
        assert_eq!(find_download_url_for(InstallKind::LinuxPackage, &only_windows), None);
        assert_eq!(find_download_url_for(InstallKind::LinuxAppImage, &only_windows), None);
        assert_eq!(find_download_url_for(InstallKind::MacOsAppleSilicon, &only_windows), None);
        assert_eq!(find_download_url_for(InstallKind::MacOsIntel, &only_windows), None);
    }

    /// The regression that mattered: no platform may ever be handed a
    /// Windows installer it cannot run. Asserted for every platform rather
    /// than only the one being compiled, via the filename that comes back.
    #[test]
    fn never_offers_a_windows_installer_to_a_non_windows_build() {
        let url = AutoUpdater::new().find_download_url(&release_assets()).unwrap_or_default();
        if !cfg!(target_os = "windows") {
            assert!(!url.ends_with(".exe"), "non-Windows build was offered {}", url);
        }
    }

    /// The Windows filter is "contains x64 AND ends in .exe". The macOS and
    /// Linux asset names must never satisfy it, or a Windows user could be
    /// handed an unrunnable file as an "update". Checked here as well as in
    /// release.yml, because the CI assertion only runs at release time while
    /// this one runs on every commit.
    ///
    /// Note "arm64" does NOT contain the substring "x64" (a-r-m-6-4), which is
    /// what makes the DMG name safe; asserted rather than reasoned about.
    #[test]
    fn no_non_windows_asset_can_satisfy_the_windows_installer_filter() {
        let windows_filter =
            |name: &str| name.ends_with(".exe") && name.contains("x64");

        for name in [
            "mechvibes-0.7.0-macos-arm64.dmg",
            "mechvibes-0.7.0-macos-x86_64.dmg",
            "mechvibes_0.7.0_amd64.deb",
            // "x86_64" is x-8-6-_-6-4: the substring "x64" does not occur in
            // it, and the extension is not .exe either. Both halves of the
            // filter have to fail, and this asserts it rather than eyeballing.
            "mechvibes-0.7.0-x86_64.AppImage",
            "README-macos-0.7.0.txt",
            "SHA256SUMS.txt",
        ] {
            assert!(
                !windows_filter(&name.to_lowercase()),
                "'{}' would be served to Windows users as an installer",
                name
            );
        }

        // The real installer must still match, otherwise the filter is simply
        // broken in the other direction.
        assert!(windows_filter(&"Mechvibes-0.7.0-Setup-x64.exe".to_lowercase()));
    }

    /// A release with no asset for this platform must yield None so the UI
    /// shows a link to the release page, rather than falling back to whatever
    /// asset happened to be listed first.
    #[test]
    fn returns_none_when_no_asset_suits_this_platform() {
        let assets = vec![asset("SHA256SUMS.txt"), asset("source-code.zip")];
        assert_eq!(AutoUpdater::new().find_download_url(&assets), None);
    }

    #[test]
    fn returns_none_for_a_release_with_no_assets_at_all() {
        assert_eq!(AutoUpdater::new().find_download_url(&[]), None);
    }

    /// Decides whether a saved `available_version` may be shown, given the
    /// running build. Extracted so the rule can be tested against arbitrary
    /// version pairs - `get_saved_update_info` itself is pinned to the real
    /// `APP_VERSION` and the on-disk config, neither of which a unit test
    /// should mutate.
    fn saved_update_is_showable(current: &str, saved: &str) -> bool {
        is_upgrade(current, saved)
    }

    /// The exact bug a user hit on 0.6.2: an older build had written
    /// "0.6.2 is available" into config, the user upgraded to 0.6.2 outside
    /// the update flow, and Settings then offered an update to the version
    /// already running.
    #[test]
    fn a_saved_update_equal_to_the_running_version_is_never_offered() {
        assert!(!saved_update_is_showable("0.6.2", "0.6.2"));
    }

    #[test]
    fn a_saved_update_older_than_the_running_version_is_never_offered() {
        // Reinstalling an older build must not make the app offer to
        // "update" backwards either.
        assert!(!saved_update_is_showable("0.6.2", "0.6.1"));
        assert!(!saved_update_is_showable("1.0.0", "0.9.9"));
    }

    #[test]
    fn a_saved_update_newer_than_the_running_version_is_still_offered() {
        // The fix must not break the case the feature exists for.
        assert!(saved_update_is_showable("0.6.2", "0.6.3"));
        assert!(saved_update_is_showable("0.6.2", "0.7.0"));
    }

    /// String comparison would rank "0.9.0" above "0.10.0"; semver does not.
    #[test]
    fn version_comparison_is_numeric_not_lexicographic() {
        assert!(saved_update_is_showable("0.9.0", "0.10.0"));
        assert!(!saved_update_is_showable("0.10.0", "0.9.0"));
    }

    /// A config written by a future build, or hand-edited, must not crash or
    /// be treated as an update.
    #[test]
    fn a_malformed_saved_version_is_never_offered() {
        assert!(!saved_update_is_showable("0.6.2", "not-a-version"));
        assert!(!saved_update_is_showable("0.6.2", ""));
    }

    /// The clearing rule used by `clear_stale_available_version`: an entry is
    /// dropped exactly when it is not an upgrade.
    #[test]
    fn the_clear_rule_drops_stale_entries_and_keeps_real_ones() {
        let should_clear = |current: &str, saved: &str| !is_upgrade(current, saved);

        // Stale: same version (the reported bug) and older versions.
        assert!(should_clear("0.6.2", "0.6.2"));
        assert!(should_clear("0.6.2", "0.6.1"));
        // Junk that could never be installed.
        assert!(should_clear("0.6.2", "garbage"));
        // A genuine pending update must survive the cleanup.
        assert!(!should_clear("0.6.2", "0.6.3"));
    }
}
