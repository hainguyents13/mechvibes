//! Stops a second copy of the app from running (Windows).
//!
//! Two UI processes means two sets of input listeners feeding two audio
//! engines, so every keystroke plays twice. That was true of the old rdev
//! listeners too, but the Raw Input worker makes it easier to hit: the
//! worker captures regardless of focus, so a forgotten second instance is
//! audible everywhere rather than only when that window is in the
//! background.
//!
//! Uses a named mutex rather than a lock file because Windows releases it
//! automatically when the process dies, however it dies - a lock file left
//! behind by a crash would keep the app from ever starting again.
#![cfg(target_os = "windows")]

use std::collections::hash_map::DefaultHasher;
use std::hash::{ Hash, Hasher };
use std::ptr::null_mut;

use winapi::um::errhandlingapi::GetLastError;
use winapi::um::handleapi::CloseHandle;
use winapi::um::synchapi::{ CreateEventW, CreateMutexW, OpenEventW, SetEvent, WaitForSingleObject };
use winapi::um::winnt::{ EVENT_MODIFY_STATE, HANDLE };

/// Session-local (`Local\`) so two different users logged into the same
/// machine each get their own instance, which is the behavior people expect
/// from a per-user tray app. A `Global\` name would let one user's running
/// copy block another's.
const MUTEX_PREFIX: &str = r"Local\Mechvibes-SingleInstance-";

/// Event a refused second instance sets to raise the running window. Shares
/// the mutex's session-local scope for the same reason: one user's second
/// launch must never surface another user's window.
const WAKE_EVENT_PREFIX: &str = r"Local\Mechvibes-ShowWindow-";

/// `ERROR_ALREADY_EXISTS`. Defined here rather than pulled from winapi's
/// `winerror` module, which this crate does not enable as a feature; the
/// value is fixed by the Win32 ABI.
const ERROR_ALREADY_EXISTS: u32 = 183;

/// `WAIT_OBJECT_0` - the wait returned because the event was signalled.
/// Same rationale as `ERROR_ALREADY_EXISTS`: fixed by the Win32 ABI.
const WAIT_OBJECT_0: u32 = 0;

/// `INFINITE` - wait without timeout. Defined here for the same reason as the
/// constants above: winapi's `winbase` feature is not enabled by this crate.
const INFINITE: u32 = 0xffff_ffff;

/// Builds a lock name unique to this executable, so only *the same build*
/// blocks itself. Two copies of the installed app still refuse to co-run,
/// which is the point; but a `dx serve` dev build and an installed release
/// live at different paths and can run side by side, which is what you want
/// while developing. A single shared name makes `dx serve` fail to start
/// whenever the installed app happens to be running, with an "already
/// running" message that points at the wrong thing entirely.
///
/// The path is hashed rather than embedded: a mutex name has a length limit
/// and treats backslashes as kernel-namespace separators, both of which a
/// raw path would trip over.
///
/// Canonicalized first so that the same executable reached by different
/// spellings - an 8.3 short name, a `subst` drive, a symlink - still hashes
/// to one name. Lowercasing alone only settles case, and two spellings of one
/// exe would each claim their own mutex and defeat the guard entirely.
/// Canonicalization can fail (the file was moved out from under us); the
/// uncanonicalized path is a fine fallback, since a missed alias is no worse
/// than the behavior before.
fn mutex_name() -> String {
    format!("{}{:x}", MUTEX_PREFIX, exe_hash())
}

/// Hash identifying this executable, shared by the instance mutex and the
/// wake-up event so both name the same "copy of the app".
fn exe_hash() -> u64 {
    let exe = std::env::current_exe()
        .map(|p| std::fs::canonicalize(&p).unwrap_or(p))
        .map(|p| p.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    let mut hasher = DefaultHasher::new();
    exe.hash(&mut hasher);
    hasher.finish()
}

/// Name of the event a refused instance sets to surface the running window.
/// Derived from the same exe hash as the mutex, so a second copy only ever
/// wakes the instance that actually turned it away.
fn wake_event_name() -> String {
    format!("{}{:x}", WAKE_EVENT_PREFIX, exe_hash())
}

/// Encode a name for the Win32 `*W` APIs.
fn wide(value: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(value).encode_wide().chain(std::iter::once(0)).collect()
}

/// Ask the already-running instance to show and focus its window.
///
/// Called by the copy that lost the single-instance race, just before it
/// exits. Without this the second launch is completely silent - the binary is
/// built with `#![windows_subsystem = "windows"]`, so even the "already
/// running" message on stderr goes nowhere, and clicking the icon again looks
/// like nothing happened.
///
/// Best-effort by design: if the event cannot be opened (the other instance
/// exited in between, or it predates this feature) the second copy still
/// exits quietly, which is the old behavior rather than a new failure.
pub fn signal_running_instance() {
    let name = wide(&wake_event_name());

    // SAFETY: `name` is a NUL-terminated wide string that outlives the call.
    // A null handle means "not found", which is handled rather than used.
    unsafe {
        let handle = OpenEventW(EVENT_MODIFY_STATE, 0, name.as_ptr());
        if handle.is_null() {
            return;
        }
        SetEvent(handle);
        CloseHandle(handle);
    }
}

/// Watch for a second launch and run `on_wake` each time one happens.
///
/// Spawns a detached thread blocking on the event rather than polling, so an
/// idle app costs nothing. The event is auto-reset, so each launch delivers
/// exactly one wake-up.
///
/// Only the instance holding the single-instance lock should call this.
pub fn listen_for_wake_requests(on_wake: impl Fn() + Send + 'static) {
    let name = wide(&wake_event_name());

    // SAFETY: `name` outlives the call. `CreateEventW` with a null security
    // descriptor and manual_reset = 0 yields an auto-reset event owned by this
    // process; a null return means the event could not be created, in which
    // case the thread is simply not started.
    let handle = unsafe { CreateEventW(null_mut(), 0, 0, name.as_ptr()) };
    if handle.is_null() {
        crate::always_eprint!("⚠️ Could not create the wake-up event; a second launch will not raise the window");
        return;
    }

    // The handle is moved onto the listener thread, which owns it for the
    // life of the process. Windows reclaims it on exit.
    let handle = WakeEvent(handle);

    std::thread::spawn(move || {
        let event = handle;
        loop {
            // SAFETY: the handle is valid for as long as this thread runs and
            // is not used from anywhere else.
            let result = unsafe { WaitForSingleObject(event.0, INFINITE) };
            if result != WAIT_OBJECT_0 {
                // Abandoned or failed wait: nothing left to listen to, so stop
                // rather than spin on an unusable handle.
                break;
            }
            on_wake();
        }
    });
}

/// Owns the wake-up event handle so it can cross the thread boundary.
struct WakeEvent(HANDLE);

// SAFETY: the handle is only touched by the listener thread that takes
// ownership of it; Windows handles are process-wide, so the move is sound.
unsafe impl Send for WakeEvent {}

/// Holds the instance lock for as long as it is alive. Windows frees the
/// underlying mutex on process exit even if this is never dropped (a crash,
/// a `TerminateProcess`), so a stuck lock cannot outlive the process.
pub struct InstanceGuard(HANDLE);

// The handle is only closed on drop and never used concurrently; Windows
// handles are process-wide, so moving one across threads is fine.
unsafe impl Send for InstanceGuard {}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

/// Claims the single-instance lock, returning `None` if another copy of the
/// app already holds it - in which case the caller should exit without
/// starting anything.
///
/// The input worker must NOT call this: it is a child of an instance that
/// already holds the lock, and would always be refused.
pub fn acquire() -> Option<InstanceGuard> {
    let name = wide(&mutex_name());

    let handle = unsafe { CreateMutexW(null_mut(), 0, name.as_ptr()) };

    if handle.is_null() {
        // Can't tell whether another instance exists, so let this one run:
        // a spurious refusal to start is worse than a possible double.
        return Some(InstanceGuard(null_mut()));
    }

    // The handle is returned even when the mutex already existed, so the
    // error code is what distinguishes "we created it" from "someone else
    // owns it" - and the handle still has to be closed either way.
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        unsafe {
            CloseHandle(handle);
        }
        return None;
    }

    Some(InstanceGuard(handle))
}
