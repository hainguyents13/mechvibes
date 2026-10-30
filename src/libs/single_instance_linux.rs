//! Stops a second copy of the app from running (Linux).
//!
//! Two copies means two input listeners and two audio engines, so every key
//! plays twice. It matters more now that a `mechvibes://` link starts the
//! app: the system launches a new copy for each link it opens, even while one
//! is running.
//!
//! A Unix socket in the user's runtime folder is both the lock and the
//! doorbell. The first copy listens on it. A later copy connects, rings it to
//! bring the first one's window up, and exits. A socket left behind by a crash
//! is harmless: nobody answers on it, so the next launch removes it and
//! listens itself. Same API as the Windows module, so `main()` treats both
//! alike.
#![cfg(target_os = "linux")]

use std::collections::hash_map::DefaultHasher;
use std::hash::{ Hash, Hasher };
use std::io::{ Read, Write };
use std::os::unix::net::{ UnixListener, UnixStream };
use std::path::PathBuf;
use std::sync::{ Mutex, OnceLock };

/// Written by a launch that wants the window. A bare connection, like the
/// probe in `acquire`, rings nothing.
const WAKE: u8 = b'w';

fn listener_slot() -> &'static Mutex<Option<UnixListener>> {
    static SLOT: OnceLock<Mutex<Option<UnixListener>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

/// The socket for this copy of the app. Keyed on the executable, like the
/// Windows mutex, so a development build and the installed app can run side
/// by side. An AppImage runs from a new mount point every launch, so it is
/// keyed on the AppImage file itself instead.
fn socket_path() -> PathBuf {
    let exe = std::env
        ::var_os("APPIMAGE")
        .map(PathBuf::from)
        .or_else(|| std::env::current_exe().ok())
        .map(|p| std::fs::canonicalize(&p).unwrap_or(p))
        .unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    exe.hash(&mut hasher);

    // The runtime folder is private to the user. The temp folder is the
    // fallback where there is none.
    let dir = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    dir.join(format!("mechvibes-{:x}.sock", hasher.finish()))
}

/// Held for the whole run; removes the socket when the app exits normally.
pub struct InstanceGuard(Option<PathBuf>);

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        if let Some(path) = &self.0 {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Claim the single instance, or `None` when another copy is running.
pub fn acquire() -> Option<InstanceGuard> {
    let path = socket_path();
    if UnixStream::connect(&path).is_ok() {
        return None;
    }
    // Nobody answered, so any socket file there is left over from a crash.
    let _ = std::fs::remove_file(&path);
    match UnixListener::bind(&path) {
        Ok(listener) => {
            if let Ok(mut slot) = listener_slot().lock() {
                *slot = Some(listener);
            }
            Some(InstanceGuard(Some(path)))
        }
        Err(e) => {
            // Can't tell whether another copy exists, so let this one run:
            // a spurious refusal to start is worse than a possible double.
            crate::always_eprint!("⚠️  Could not claim the single instance socket: {}", e);
            Some(InstanceGuard(None))
        }
    }
}

/// Ask the running copy to show its window. Best effort, as on Windows.
pub fn signal_running_instance() {
    if let Ok(mut stream) = UnixStream::connect(socket_path()) {
        let _ = stream.write_all(&[WAKE]);
    }
}

/// Run `on_wake` each time a later launch rings. Only the copy holding the
/// instance should call this.
pub fn listen_for_wake_requests(on_wake: impl Fn() + Send + 'static) {
    let Some(listener) = listener_slot().lock().ok().and_then(|mut slot| slot.take()) else {
        return;
    };
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let mut byte = [0u8; 1];
            if matches!(stream.read(&mut byte), Ok(1)) && byte[0] == WAKE {
                on_wake();
            }
        }
    });
}
