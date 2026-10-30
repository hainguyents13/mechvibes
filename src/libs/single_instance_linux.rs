//! Stops a second copy of the app from running (Linux).
//!
//! Two copies means two input listeners and two audio engines, so every key
//! plays twice. It matters more now that a `mechvibes://` link starts the
//! app: the system launches a new copy for each link it opens, even while one
//! is running.
//!
//! The lock is an exclusive `flock` on a lock file, held for the whole run.
//! The kernel drops it when the process dies, however it dies, which is what
//! the named mutex does on Windows; checking and claiming are one call, so two
//! launches at once cannot both win. Next to it, a Unix socket is the
//! doorbell: the copy holding the lock listens, and a later copy connects and
//! rings it to bring the window up before exiting. Same API as the Windows
//! module, so `main()` treats both alike.
#![cfg(target_os = "linux")]

use std::collections::hash_map::DefaultHasher;
use std::fs::File;
use std::hash::{ Hash, Hasher };
use std::io::{ Read, Write };
use std::os::unix::io::AsRawFd;
use std::os::unix::net::{ UnixListener, UnixStream };
use std::path::PathBuf;
use std::sync::{ Mutex, OnceLock };
use std::time::Duration;

/// Written by a launch that wants the window.
const WAKE: u8 = b'w';

fn listener_slot() -> &'static Mutex<Option<UnixListener>> {
    static SLOT: OnceLock<Mutex<Option<UnixListener>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

/// Where the lock and the socket live, named after this copy of the app.
///
/// Keyed on the executable, like the Windows mutex, so a development build
/// and the installed app can run side by side. An AppImage runs from a new
/// mount point every launch, so it is keyed on the AppImage file instead. The
/// runtime folder is private to the user; where there is none, the app's own
/// data folder is, unlike the shared temp folder.
fn instance_paths() -> (PathBuf, PathBuf) {
    let exe = crate::libs::protocol
        ::appimage_file()
        .or_else(|| std::env::current_exe().ok())
        .map(|p| std::fs::canonicalize(&p).unwrap_or(p))
        .unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    exe.hash(&mut hasher);
    let name = format!("mechvibes-{:x}", hasher.finish());

    let dir = std::env
        ::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|dir| dir.is_dir())
        .or_else(|| crate::state::paths::data::config_json().parent().map(PathBuf::from))
        .unwrap_or_else(std::env::temp_dir);
    (dir.join(format!("{}.lock", name)), dir.join(format!("{}.sock", name)))
}

/// Held for the whole run. Dropping it releases the lock and removes the
/// socket.
pub struct InstanceGuard {
    _lock: Option<File>,
    socket: Option<PathBuf>,
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        if let Some(path) = &self.socket {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Claim the single instance, or `None` when another copy holds it.
pub fn acquire() -> Option<InstanceGuard> {
    let (lock_path, socket_path) = instance_paths();
    let lock = match File::options().create(true).write(true).truncate(false).open(&lock_path) {
        Ok(file) => file,
        Err(e) => {
            // Can't tell whether another copy exists, so let this one run:
            // a spurious refusal to start is worse than a possible double.
            crate::always_eprint!("⚠️  Could not open the single instance lock: {}", e);
            return Some(InstanceGuard { _lock: None, socket: None });
        }
    };
    // SAFETY: `lock` owns a valid descriptor for the duration of the call.
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return None;
    }

    // Holding the lock means no other copy is listening, so a socket file
    // still there is left over from a crash.
    let _ = std::fs::remove_file(&socket_path);
    let socket = match UnixListener::bind(&socket_path) {
        Ok(listener) => {
            if let Ok(mut slot) = listener_slot().lock() {
                *slot = Some(listener);
            }
            Some(socket_path)
        }
        Err(e) => {
            crate::always_eprint!("⚠️  Could not open the single instance socket: {}", e);
            None
        }
    };
    Some(InstanceGuard { _lock: Some(lock), socket })
}

/// Ask the running copy to show its window. Best effort, as on Windows.
pub fn signal_running_instance() {
    if let Ok(mut stream) = UnixStream::connect(instance_paths().1) {
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
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else {
                // Out of descriptors or similar: back off instead of spinning.
                std::thread::sleep(Duration::from_millis(200));
                continue;
            };
            // A caller that connects and then hangs must not hold up the
            // launches after it.
            let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
            let mut byte = [0u8; 1];
            if matches!(stream.read(&mut byte), Ok(1)) && byte[0] == WAKE {
                on_wake();
            }
        }
    });
}
