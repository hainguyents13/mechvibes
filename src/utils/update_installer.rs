//! Staging and installation of downloaded updates.
//!
//! Splits the "true auto-update" flow into pieces that can be tested without
//! a network or a running installer:
//!
//! * [`parse_sha256sums`] — pull one file's digest out of a `SHA256SUMS.txt`.
//! * [`verify_file_sha256`] — hash a file on disk and compare.
//! * [`StagedUpdate`] — a verified installer sitting in temp, remembered in
//!   config across restarts.
//! * [`launch_installer_and_exit`] (Windows) — the only part that needs a real
//!   machine to test.
//!
//! Every fallible step is expected to fail sometimes (no network, no
//! `SHA256SUMS.txt` in the release, antivirus eating the file). Callers treat
//! any error as "fall back to opening the browser", which is the behaviour
//! that existed before this module - the app must never be blocked by its own
//! updater.

use serde::{ Deserialize, Serialize };
use std::fmt;
use std::path::{ Path, PathBuf };

/// Errors from staging or installing an update.
///
/// Deliberately not merged with `UpdateError` in `auto_updater`: that one
/// describes *checking* (a network/API concern), this one describes handling
/// a file on disk. Keeping them apart stops "hash mismatch" from being
/// reported as a network error.
#[derive(Debug)]
pub enum StageError {
    /// Download failed, or the server answered with a non-success status.
    Download(String),
    /// The release has no `SHA256SUMS.txt`, or it has no line for our file.
    /// This is the expected state for releases published before Phase 6.
    ChecksumUnavailable(String),
    /// The file downloaded fine but its digest is not the expected one.
    /// Always treated as hostile: the file is deleted, never executed.
    ChecksumMismatch {
        expected: String,
        actual: String,
    },
    /// Filesystem trouble (temp dir not writable, rename failed, ...).
    Io(String),
    /// Staging or installing was attempted on a platform that has no silent
    /// installer story. Not a bug - the UI offers the Releases page instead.
    ///
    /// Only constructed off Windows, so a Windows-only build sees it as dead;
    /// the variant still has to exist there for the error type to be one type
    /// across platforms.
    #[cfg_attr(target_os = "windows", allow(dead_code))]
    Unsupported(String),
}

impl fmt::Display for StageError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            StageError::Download(msg) => write!(f, "Download failed: {}", msg),
            StageError::ChecksumUnavailable(msg) => write!(f, "Checksum unavailable: {}", msg),
            StageError::ChecksumMismatch { expected, actual } =>
                write!(f, "Checksum mismatch: expected {}, got {}", expected, actual),
            StageError::Io(msg) => write!(f, "I/O error: {}", msg),
            StageError::Unsupported(msg) => write!(f, "Unsupported: {}", msg),
        }
    }
}

impl std::error::Error for StageError {}

/// A verified installer waiting on disk, remembered across app restarts.
///
/// Persisted in `AppConfig::auto_update` so that choosing "Later" survives a
/// restart. The hash is stored alongside the path on purpose: the file is
/// re-verified before it is ever executed, so a staged installer that was
/// tampered with (or truncated by a crash) between sessions is caught rather
/// than run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StagedUpdate {
    pub version: String,
    pub installer_path: String,
    pub sha256: String,
}

impl StagedUpdate {
    /// Re-hashes the file and confirms it is still the version we expect.
    ///
    /// Returns the path only when everything lines up. Any mismatch, missing
    /// file, or version disagreement yields an error so the caller can discard
    /// the stale entry and start over.
    pub fn validate(&self, expected_version: &str) -> Result<PathBuf, StageError> {
        if self.version != expected_version {
            return Err(
                StageError::ChecksumUnavailable(
                    format!("Staged version {} is not the expected {}", self.version, expected_version)
                )
            );
        }

        let path = PathBuf::from(&self.installer_path);
        if !path.is_file() {
            return Err(StageError::Io(format!("Staged installer is gone: {}", self.installer_path)));
        }

        verify_file_sha256(&path, &self.sha256)?;
        Ok(path)
    }
}

/// Directory holding downloaded installers (`%TEMP%\mechvibes-updates`).
pub fn staging_dir() -> PathBuf {
    std::env::temp_dir().join("mechvibes-updates")
}

/// Extracts the digest for `filename` from `SHA256SUMS.txt` contents.
///
/// Accepts the format `sha256sum` and PowerShell's `Get-FileHash` pipeline
/// both produce: `<64 hex chars><whitespace>[*]<filename>`. The `*` marks
/// binary mode in coreutils output and is not part of the name. Matching is
/// done on the basename so a sums file listing `dist/Foo.exe` still resolves
/// `Foo.exe`, and the digest is lowercased so comparisons never hinge on the
/// generator's casing.
pub fn parse_sha256sums(contents: &str, filename: &str) -> Option<String> {
    let target = Path::new(filename).file_name()?.to_str()?.to_lowercase();

    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let mut parts = line.splitn(2, char::is_whitespace);
        let digest = parts.next()?.trim();
        let name = parts.next()?.trim().trim_start_matches('*').trim();

        // A digest is exactly 64 hex characters; anything else is a stray
        // line (a header, a note) rather than a checksum we should trust.
        if digest.len() != 64 || !digest.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }

        let entry_name = match Path::new(name).file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_lowercase(),
            None => {
                continue;
            }
        };

        if entry_name == target {
            return Some(digest.to_lowercase());
        }
    }

    None
}

/// Streams a file through SHA-256 and compares against `expected`.
///
/// Reads in chunks rather than slurping the whole installer (~50 MB) into
/// memory. Comparison is case-insensitive on the hex text.
pub fn verify_file_sha256(path: &Path, expected: &str) -> Result<(), StageError> {
    use sha2::{ Digest, Sha256 };
    use std::io::Read;

    let mut file = std::fs::File
        ::open(path)
        .map_err(|e| StageError::Io(format!("Cannot open {}: {}", path.display(), e)))?;

    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buf)
            .map_err(|e| StageError::Io(format!("Cannot read {}: {}", path.display(), e)))?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }

    let actual = hex_encode(&hasher.finalize());
    let expected_lower = expected.trim().to_lowercase();

    if actual == expected_lower {
        Ok(())
    } else {
        Err(StageError::ChecksumMismatch {
            expected: expected_lower,
            actual,
        })
    }
}

/// Lowercase hex, without pulling in a `hex` crate for eight lines.
fn hex_encode(bytes: &[u8]) -> String {
    use fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        // Writing to a String is infallible; the Result is discarded rather
        // than unwrapped so this cannot panic in a release path.
        let _ = write!(out, "{:02x}", byte);
    }
    out
}

/// Derives the `SHA256SUMS.txt` URL that sits beside an asset in the same
/// release. GitHub asset download URLs share a directory per release, so
/// swapping the last path segment is enough and avoids a second API call.
pub fn sha256sums_url_for(asset_url: &str) -> Option<String> {
    let cut = asset_url.rfind('/')?;
    Some(format!("{}/SHA256SUMS.txt", &asset_url[..cut]))
}

/// Filename portion of a download URL, used to look the asset up in the sums
/// file and to name the file on disk.
pub fn filename_from_url(url: &str) -> Option<String> {
    let name = url.rsplit('/').next()?;
    // Strip a query string if one ever appears; GitHub's asset URLs have none
    // today but a redirect could add one.
    let name = name.split('?').next()?;
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// Downloads `url` into the staging directory and verifies it against the
/// release's `SHA256SUMS.txt`.
///
/// Writes to `<name>.partial` first and renames only after the digest checks
/// out, so an interrupted download can never be mistaken for a finished one.
/// A file that is already staged and still hashes correctly is reused instead
/// of re-downloaded.
pub async fn download_and_verify(
    download_url: &str,
    version: &str,
    user_agent: &str
) -> Result<StagedUpdate, StageError> {
    let filename = filename_from_url(download_url).ok_or_else(||
        StageError::Download(format!("Cannot derive a filename from {}", download_url))
    )?;

    let dir = staging_dir();
    std::fs::create_dir_all(&dir).map_err(|e|
        StageError::Io(format!("Cannot create {}: {}", dir.display(), e))
    )?;

    let client = reqwest::Client::new();

    // Fetch the checksums first. Without them nothing gets executed, so
    // there is no point spending bandwidth on a ~50 MB installer we would
    // have to throw away.
    let sums_url = sha256sums_url_for(download_url).ok_or_else(||
        StageError::ChecksumUnavailable(format!("Cannot derive a sums URL from {}", download_url))
    )?;

    let sums_response = client
        .get(&sums_url)
        .header("User-Agent", user_agent)
        .send().await
        .map_err(|e| StageError::ChecksumUnavailable(e.to_string()))?;

    if !sums_response.status().is_success() {
        return Err(
            StageError::ChecksumUnavailable(
                format!("SHA256SUMS.txt: HTTP {}", sums_response.status())
            )
        );
    }

    let sums_body = sums_response
        .text().await
        .map_err(|e| StageError::ChecksumUnavailable(e.to_string()))?;

    let expected = parse_sha256sums(&sums_body, &filename).ok_or_else(||
        StageError::ChecksumUnavailable(format!("No entry for {} in SHA256SUMS.txt", filename))
    )?;

    let final_path = dir.join(&filename);

    // Already downloaded and still intact? Skip the transfer.
    if final_path.is_file() && verify_file_sha256(&final_path, &expected).is_ok() {
        return Ok(StagedUpdate {
            version: version.to_string(),
            installer_path: final_path.to_string_lossy().into_owned(),
            sha256: expected,
        });
    }

    let partial_path = dir.join(format!("{}.partial", filename));

    {
        use std::io::Write;

        let mut response = client
            .get(download_url)
            .header("User-Agent", user_agent)
            .send().await
            .map_err(|e| StageError::Download(e.to_string()))?;

        if !response.status().is_success() {
            return Err(StageError::Download(format!("HTTP {}", response.status())));
        }

        let mut file = std::fs::File
            ::create(&partial_path)
            .map_err(|e| StageError::Io(format!("Cannot create {}: {}", partial_path.display(), e)))?;

        // Streamed chunk by chunk via reqwest's own `chunk()` rather than
        // `bytes()`, so a ~50 MB installer never sits in memory in full, and
        // rather than `bytes_stream()`, which would mean taking on
        // futures-util as a direct dependency for one trait import.
        while
            let Some(chunk) = response
                .chunk().await
                .map_err(|e| StageError::Download(e.to_string()))?
        {
            file.write_all(&chunk).map_err(|e| StageError::Io(e.to_string()))?;
        }

        file.flush().map_err(|e| StageError::Io(e.to_string()))?;
    }

    // Verify while it is still named `.partial`, so a bad payload never even
    // briefly occupies the path the installer would be launched from.
    if let Err(e) = verify_file_sha256(&partial_path, &expected) {
        let _ = std::fs::remove_file(&partial_path);
        return Err(e);
    }

    std::fs::rename(&partial_path, &final_path).map_err(|e|
        StageError::Io(format!("Cannot rename staged installer: {}", e))
    )?;

    Ok(StagedUpdate {
        version: version.to_string(),
        installer_path: final_path.to_string_lossy().into_owned(),
        sha256: expected,
    })
}

/// Deletes a staged installer, ignoring "already gone".
pub fn discard_staged(staged: &StagedUpdate) {
    let path = PathBuf::from(&staged.installer_path);
    if path.exists() {
        let _ = std::fs::remove_file(&path);
    }
    let partial = PathBuf::from(format!("{}.partial", staged.installer_path));
    if partial.exists() {
        let _ = std::fs::remove_file(&partial);
    }
}

/// Arguments handed to the Inno Setup installer for an unattended upgrade.
///
/// `/NORESTART` matters: without it Inno may reboot the machine when it thinks
/// a locked file requires it.
///
/// `/RESTARTAPPLICATIONS` is deliberately NOT passed. It asks Restart Manager
/// to revive the apps it closed, but RM can only revive processes that called
/// the Windows `RegisterApplicationRestart` API, and this app never does - a
/// /LOG trace of a real silent install shows RM logging "Attempting to restart
/// applications" and nothing coming back. Passing it advertised a guarantee
/// that did not exist and invited the `.iss` to be written as though the
/// relaunch were covered. The relaunch comes from the silent-only `[Run]`
/// entry in `installer/windows/mechvibes-setup.iss` - one path, verified.
#[cfg(target_os = "windows")]
pub const INSTALLER_SILENT_ARGS: [&str; 3] = ["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"];

/// Spawns the installer detached and returns once it is running.
///
/// The installer must outlive this process - it is going to replace this very
/// executable. `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP` takes it out of
/// our console and process group so that our exit does not take it down with
/// us.
///
/// The caller is responsible for shutting the app down afterwards; this
/// function deliberately does not exit the process so that a spawn failure
/// stays recoverable (the UI falls back to the browser link).
#[cfg(target_os = "windows")]
pub fn spawn_installer_detached(installer_path: &Path) -> Result<(), StageError> {
    use std::os::windows::process::CommandExt;
    use std::process::{ Command, Stdio };

    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

    if !installer_path.is_file() {
        return Err(StageError::Io(format!("Installer missing: {}", installer_path.display())));
    }

    Command::new(installer_path)
        .args(INSTALLER_SILENT_ARGS)
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| StageError::Io(format!("Cannot start installer: {}", e)))?;

    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn spawn_installer_detached(_installer_path: &Path) -> Result<(), StageError> {
    Err(StageError::Unsupported("Silent install is only implemented on Windows".to_string()))
}

/// Whether this build can stage and run an installer at all.
///
/// Linux and macOS releases ship a `.deb` and an experimental tarball; neither
/// has an unattended in-place upgrade path, so those builds only ever link to
/// the Releases page.
pub const fn silent_update_supported() -> bool {
    cfg!(target_os = "windows")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// SHA-256 of the empty string, and of "hello world" - both well-known
    /// vectors, so a wrong hasher wiring is caught rather than a wrong
    /// expectation being baked in.
    const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    const HELLO_WORLD_SHA256: &str =
        "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";

    fn temp_file(name: &str, contents: &[u8]) -> PathBuf {
        let dir = std::env::temp_dir().join("mechvibes-update-tests");
        std::fs::create_dir_all(&dir).expect("test temp dir");
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).expect("create test file");
        f.write_all(contents).expect("write test file");
        path
    }

    /// Live check against the real v0.6.1 release, published before
    /// SHA256SUMS.txt existed. Confirms the app degrades to the manual
    /// download instead of running an unverified installer. Ignored by
    /// default so the suite stays offline-clean; run with `--ignored`.
    #[tokio::test]
    #[ignore = "requires network; hits the real GitHub release"]
    async fn a_release_without_checksums_fails_before_downloading() {
        let url =
            "https://github.com/hainguyents13/mechvibes/releases/download/v0.6.1/Mechvibes-0.6.1-Setup-x64.exe";
        let err = download_and_verify(url, "0.6.1", "mechvibes-test/0").await.expect_err(
            "v0.6.1 has no SHA256SUMS.txt"
        );
        assert!(
            matches!(err, StageError::ChecksumUnavailable(_)),
            "expected ChecksumUnavailable so the UI falls back to the browser, got {:?}",
            err
        );
    }

    #[test]
    fn verifies_a_file_whose_digest_matches() {
        let path = temp_file("match.bin", b"hello world");
        assert!(verify_file_sha256(&path, HELLO_WORLD_SHA256).is_ok());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn verification_is_case_insensitive_on_the_expected_digest() {
        let path = temp_file("case.bin", b"hello world");
        let upper = HELLO_WORLD_SHA256.to_uppercase();
        assert!(verify_file_sha256(&path, &upper).is_ok());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn rejects_a_file_whose_digest_differs() {
        let path = temp_file("mismatch.bin", b"hello world");
        let err = verify_file_sha256(&path, EMPTY_SHA256).expect_err("must reject wrong digest");
        match err {
            StageError::ChecksumMismatch { expected, actual } => {
                assert_eq!(expected, EMPTY_SHA256);
                assert_eq!(actual, HELLO_WORLD_SHA256);
            }
            other => panic!("expected ChecksumMismatch, got {:?}", other),
        }
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn reports_io_error_for_a_missing_file() {
        let path = std::env::temp_dir().join("mechvibes-update-tests/definitely-not-here.bin");
        let _ = std::fs::remove_file(&path);
        let err = verify_file_sha256(&path, EMPTY_SHA256).expect_err("must fail on missing file");
        assert!(matches!(err, StageError::Io(_)), "got {:?}", err);
    }

    #[test]
    fn parses_a_coreutils_style_sums_file() {
        let contents = format!(
            "{}  Mechvibes-0.7.0-Setup-x64.exe\n{}  mechvibes_0.7.0_amd64.deb\n",
            HELLO_WORLD_SHA256,
            EMPTY_SHA256
        );
        assert_eq!(
            parse_sha256sums(&contents, "Mechvibes-0.7.0-Setup-x64.exe"),
            Some(HELLO_WORLD_SHA256.to_string())
        );
        assert_eq!(
            parse_sha256sums(&contents, "mechvibes_0.7.0_amd64.deb"),
            Some(EMPTY_SHA256.to_string())
        );
    }

    #[test]
    fn parses_binary_mode_and_uppercase_digests() {
        // `sha256sum -b` prefixes the name with '*'; PowerShell emits
        // uppercase hex. Both must resolve to the same lowercase digest.
        let contents = format!("{} *Mechvibes-0.7.0-Setup-x64.exe\n", HELLO_WORLD_SHA256.to_uppercase());
        assert_eq!(
            parse_sha256sums(&contents, "Mechvibes-0.7.0-Setup-x64.exe"),
            Some(HELLO_WORLD_SHA256.to_string())
        );
    }

    #[test]
    fn matches_on_basename_when_the_sums_file_lists_a_path() {
        let contents = format!("{}  dist/Mechvibes-0.7.0-Setup-x64.exe\n", HELLO_WORLD_SHA256);
        assert_eq!(
            parse_sha256sums(&contents, "Mechvibes-0.7.0-Setup-x64.exe"),
            Some(HELLO_WORLD_SHA256.to_string())
        );
    }

    #[test]
    fn returns_none_when_the_file_has_no_entry() {
        // The v0.6.1 situation: a sums file exists but not for our asset.
        let contents = format!("{}  some-other-asset.deb\n", HELLO_WORLD_SHA256);
        assert_eq!(parse_sha256sums(&contents, "Mechvibes-0.7.0-Setup-x64.exe"), None);
    }

    #[test]
    fn ignores_comments_blank_lines_and_malformed_digests() {
        let contents = format!(
            "# generated by CI\n\nnot-a-digest  Mechvibes-0.7.0-Setup-x64.exe\n{}  Mechvibes-0.7.0-Setup-x64.exe\n",
            HELLO_WORLD_SHA256
        );
        assert_eq!(
            parse_sha256sums(&contents, "Mechvibes-0.7.0-Setup-x64.exe"),
            Some(HELLO_WORLD_SHA256.to_string())
        );
    }

    /// Byte-for-byte output of `sha256sum *` as the release job runs it,
    /// captured from a real shell rather than hand-written - the binary-mode
    /// `*` prefix it emits was a surprise worth pinning down in a test.
    #[test]
    fn parses_the_exact_output_the_release_job_produces() {
        let contents =
            "82a8f86fce67941686c364ddfa56423b39422a87e1336aa8d7d1ef18f4eca1c4 *Mechvibes-0.7.0-Setup-x64.exe\n\
             dae45e9c4c5641b0e8a58b6c3f0de0dd946b9ac47502cc45ef451f3119ebacd7 *mechvibes_0.7.0_amd64.deb\n";

        assert_eq!(
            parse_sha256sums(contents, "Mechvibes-0.7.0-Setup-x64.exe"),
            Some("82a8f86fce67941686c364ddfa56423b39422a87e1336aa8d7d1ef18f4eca1c4".to_string())
        );
        assert_eq!(
            parse_sha256sums(contents, "mechvibes_0.7.0_amd64.deb"),
            Some("dae45e9c4c5641b0e8a58b6c3f0de0dd946b9ac47502cc45ef451f3119ebacd7".to_string())
        );
    }

    #[test]
    fn returns_none_for_empty_contents() {
        assert_eq!(parse_sha256sums("", "anything.exe"), None);
    }

    #[test]
    fn derives_the_sums_url_from_an_asset_url() {
        assert_eq!(
            sha256sums_url_for(
                "https://github.com/o/r/releases/download/v0.7.0/Mechvibes-0.7.0-Setup-x64.exe"
            ),
            Some("https://github.com/o/r/releases/download/v0.7.0/SHA256SUMS.txt".to_string())
        );
    }

    #[test]
    fn extracts_the_filename_from_an_asset_url() {
        assert_eq!(
            filename_from_url(
                "https://github.com/o/r/releases/download/v0.7.0/Mechvibes-0.7.0-Setup-x64.exe"
            ),
            Some("Mechvibes-0.7.0-Setup-x64.exe".to_string())
        );
        assert_eq!(filename_from_url("https://example.com/"), None);
    }

    #[test]
    fn staged_update_validates_only_the_expected_version() {
        let path = temp_file("staged.bin", b"hello world");
        let staged = StagedUpdate {
            version: "0.7.0".to_string(),
            installer_path: path.to_string_lossy().into_owned(),
            sha256: HELLO_WORLD_SHA256.to_string(),
        };

        assert!(staged.validate("0.7.0").is_ok());
        // A staged 0.7.0 must not be used to "update" to 0.8.0.
        assert!(staged.validate("0.8.0").is_err());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn staged_update_rejects_a_tampered_file() {
        let path = temp_file("tampered.bin", b"hello world");
        let staged = StagedUpdate {
            version: "0.7.0".to_string(),
            installer_path: path.to_string_lossy().into_owned(),
            sha256: HELLO_WORLD_SHA256.to_string(),
        };
        assert!(staged.validate("0.7.0").is_ok());

        // Someone replaces the staged installer between sessions.
        std::fs::write(&path, b"malicious payload").expect("overwrite");
        let err = staged.validate("0.7.0").expect_err("tampered file must not validate");
        assert!(matches!(err, StageError::ChecksumMismatch { .. }), "got {:?}", err);
        let _ = std::fs::remove_file(path);
    }
}
