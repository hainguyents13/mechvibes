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

use winapi::shared::minwindef::DWORD;
use winapi::um::errhandlingapi::GetLastError;
use winapi::um::handleapi::CloseHandle;
use winapi::um::minwinbase::SECURITY_ATTRIBUTES;
use winapi::um::processthreadsapi::{ GetCurrentProcess, OpenProcessToken };
use winapi::shared::sddl::{
    ConvertSidToStringSidW,
    ConvertStringSecurityDescriptorToSecurityDescriptorW,
    SDDL_REVISION_1,
};
use winapi::um::securitybaseapi::GetTokenInformation;
use winapi::um::synchapi::{ CreateEventW, CreateMutexW, OpenEventW, SetEvent, WaitForSingleObject };
use winapi::um::winbase::LocalFree;
use winapi::um::winnt::{
    TokenUser,
    EVENT_MODIFY_STATE,
    HANDLE,
    LPWSTR,
    PSECURITY_DESCRIPTOR,
    SYNCHRONIZE,
    TOKEN_QUERY,
    TOKEN_USER,
};

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

/// `ERROR_ACCESS_DENIED`. Same rationale as `ERROR_ALREADY_EXISTS`: fixed by
/// the Win32 ABI.
const ERROR_ACCESS_DENIED: u32 = 5;

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

    // A null return means the event could not be created, in which case the
    // thread is simply not started.
    let handle = create_wake_event(&name);
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

/// Create the auto-reset wake-up event, open to every copy of the app the
/// same person starts.
///
/// With the default security, an event made by a copy running as
/// administrator only lets administrators and the system in. A second copy
/// started normally is turned away by the instance lock, then cannot set the
/// event either: double-clicking the app shows nothing, and a library link or
/// theme file opened just then is never picked up. So the event gets its own
/// security, built by [`wake_event_sddl`].
///
/// Falls back to the default security if that cannot be built or used: a
/// window that only a copy with the same rights can raise is better than one
/// nothing can raise.
fn create_wake_event(name: &[u16]) -> HANDLE {
    if let Some(security) = current_user_sid().and_then(|sid| SecurityDescriptor::from_sddl(&wake_event_sddl(&sid))) {
        let mut attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as DWORD,
            lpSecurityDescriptor: security.0,
            bInheritHandle: 0,
        };
        // SAFETY: `name` and `attributes` outlive the call, and `security`
        // stays alive until after it; Windows copies the descriptor into the
        // event, so freeing it afterwards is fine.
        let handle = unsafe { CreateEventW(&mut attributes, 0, 0, name.as_ptr()) };
        if !handle.is_null() {
            return handle;
        }
    }
    // SAFETY: `name` outlives the call; a null security descriptor means the
    // default security of this process.
    unsafe { CreateEventW(null_mut(), 0, 0, name.as_ptr()) }
}

/// Security for the wake-up event, in SDDL, for the user with SID `user_sid`.
///
/// - The system and administrators get full access, as they would by default.
/// - The user who started the app may set the event and wait on it, and
///   nothing more. Being granted by SID, this holds whether that user's copy
///   runs as administrator or not, and lets no other user in.
/// - The medium integrity label keeps the default "no write up" rule, so a
///   low-integrity process (a sandboxed browser tab, say) still cannot set it.
fn wake_event_sddl(user_sid: &str) -> String {
    format!(
        "D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;0x{:x};;;{})S:(ML;;NW;;;ME)",
        EVENT_MODIFY_STATE | SYNCHRONIZE,
        user_sid
    )
}

/// The SID of the user running this process, as a string (`S-1-5-21-...`).
/// `None` if the process token cannot be read.
fn current_user_sid() -> Option<String> {
    // SAFETY: every out-pointer points at a live local; the token handle is
    // closed on every path once it was opened, and the SID string Windows
    // allocates is freed with `LocalFree` once copied.
    unsafe {
        let mut token: HANDLE = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return None;
        }

        // The first call only reports the size needed. The buffer is made of
        // u64s so it is aligned for the pointer inside `TOKEN_USER`.
        let mut needed: DWORD = 0;
        GetTokenInformation(token, TokenUser, null_mut(), 0, &mut needed);
        let mut buffer = vec![0u64; (needed as usize).div_ceil(8)];
        let read = GetTokenInformation(token, TokenUser, buffer.as_mut_ptr().cast(), needed, &mut needed);
        CloseHandle(token);
        if read == 0 || needed == 0 {
            return None;
        }

        let user = &*buffer.as_ptr().cast::<TOKEN_USER>();
        let mut sid_string: LPWSTR = null_mut();
        if ConvertSidToStringSidW(user.User.Sid, &mut sid_string) == 0 {
            return None;
        }
        let length = (0..).take_while(|&i| *sid_string.add(i) != 0).count();
        let sid = String::from_utf16_lossy(std::slice::from_raw_parts(sid_string, length));
        LocalFree(sid_string.cast());
        Some(sid)
    }
}

/// A security descriptor Windows allocated, freed with `LocalFree` on drop.
struct SecurityDescriptor(PSECURITY_DESCRIPTOR);

impl SecurityDescriptor {
    /// Build one from an SDDL string. `None` if Windows rejects the string.
    fn from_sddl(sddl: &str) -> Option<Self> {
        let sddl = wide(sddl);
        let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
        // SAFETY: `sddl` is NUL-terminated and outlives the call; on success
        // `descriptor` is a buffer this value now owns and frees.
        let built = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1 as DWORD,
                &mut descriptor,
                null_mut()
            )
        };
        (built != 0 && !descriptor.is_null()).then_some(SecurityDescriptor(descriptor))
    }
}

impl Drop for SecurityDescriptor {
    fn drop(&mut self) {
        // SAFETY: the pointer came from
        // `ConvertStringSecurityDescriptorToSecurityDescriptorW` and is freed
        // only here.
        unsafe {
            LocalFree(self.0.cast());
        }
    }
}

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

    // The error code is read straight after the call, before anything else
    // can overwrite it.
    let (handle, last_error) = unsafe {
        let handle = CreateMutexW(null_mut(), 0, name.as_ptr());
        (handle, GetLastError())
    };

    match mutex_outcome(handle.is_null(), last_error) {
        MutexOutcome::Claimed => Some(InstanceGuard(handle)),
        MutexOutcome::AlreadyRunning => {
            // A handle comes back even when the mutex already existed, and it
            // still has to be closed.
            if !handle.is_null() {
                unsafe {
                    CloseHandle(handle);
                }
            }
            None
        }
        MutexOutcome::Unknown => Some(InstanceGuard(null_mut())),
    }
}

/// What creating the instance mutex says about other copies of the app.
#[derive(Debug, PartialEq, Eq)]
enum MutexOutcome {
    /// This copy created the mutex and is the only one running.
    Claimed,
    /// Another copy holds the mutex, so this one should exit.
    AlreadyRunning,
    /// No mutex and no way to tell whether another copy exists.
    Unknown,
}

/// Reads the result of `CreateMutexW`: whether the handle was null, and the
/// error code right after the call.
///
/// A handle comes back even when the mutex already existed, so the error
/// code is what tells "we created it" from "someone else owns it".
///
/// A null handle with "access denied" also means another copy is running:
/// the mutex exists, but it belongs to a copy running as administrator, and
/// a copy without admin rights is not allowed to open it. Letting this copy
/// run would play every key twice.
///
/// Any other null result says nothing about other copies, so this one is
/// let through: a spurious refusal to start is worse than a possible double.
fn mutex_outcome(handle_is_null: bool, last_error: u32) -> MutexOutcome {
    match (handle_is_null, last_error) {
        (true, ERROR_ACCESS_DENIED) => MutexOutcome::AlreadyRunning,
        (true, _) => MutexOutcome::Unknown,
        (false, ERROR_ALREADY_EXISTS) => MutexOutcome::AlreadyRunning,
        (false, _) => MutexOutcome::Claimed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_mutex_is_claimed() {
        assert_eq!(mutex_outcome(false, 0), MutexOutcome::Claimed);
    }

    #[test]
    fn an_existing_mutex_means_another_copy_is_running() {
        assert_eq!(mutex_outcome(false, ERROR_ALREADY_EXISTS), MutexOutcome::AlreadyRunning);
    }

    /// A copy running as administrator owns a mutex that a copy without
    /// admin rights cannot open; that copy must still be turned away.
    #[test]
    fn access_denied_means_another_copy_is_running() {
        assert_eq!(mutex_outcome(true, ERROR_ACCESS_DENIED), MutexOutcome::AlreadyRunning);
    }

    /// Any other failure gives no answer, and this copy is let through.
    #[test]
    fn other_failures_let_this_copy_run() {
        assert_eq!(mutex_outcome(true, 0), MutexOutcome::Unknown);
        // ERROR_INVALID_HANDLE: the name is taken by another kind of object.
        assert_eq!(mutex_outcome(true, 6), MutexOutcome::Unknown);
    }

    /// "Access denied" only counts when no handle came back; with a handle
    /// the mutex was created and the stale error code means nothing.
    #[test]
    fn access_denied_with_a_handle_is_still_claimed() {
        assert_eq!(mutex_outcome(false, ERROR_ACCESS_DENIED), MutexOutcome::Claimed);
    }

    /// The user gets exactly "set" and "wait" (0x2 | 0x100000), the system
    /// and administrators keep full access, and the medium label blocks
    /// low-integrity writers.
    #[test]
    fn the_wake_event_lets_the_same_user_set_it_and_nothing_more() {
        assert_eq!(
            wake_event_sddl("S-1-5-21-1-2-3-1001"),
            "D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;0x100002;;;S-1-5-21-1-2-3-1001)S:(ML;;NW;;;ME)"
        );
    }

    #[test]
    fn the_wake_event_security_is_accepted_by_windows() {
        let sid = current_user_sid().expect("the process token names a user");
        assert!(sid.starts_with("S-1-"), "unexpected SID string: {sid}");
        assert!(SecurityDescriptor::from_sddl(&wake_event_sddl(&sid)).is_some());
    }

    /// A string Windows rejects gives no descriptor, which is what sends
    /// `create_wake_event` back to the default security.
    #[test]
    fn a_rejected_security_string_gives_no_descriptor() {
        assert!(SecurityDescriptor::from_sddl("D:(not an ace)").is_none());
    }

    /// Name for an event only this test run uses.
    fn test_event_name(tag: &str) -> Vec<u16> {
        wide(&format!(r"Local\Mechvibes-Test-{}-{}", std::process::id(), tag))
    }

    /// Read the event's DACL and integrity label back from Windows, as SDDL.
    fn event_security(handle: HANDLE) -> String {
        use winapi::shared::sddl::ConvertSecurityDescriptorToStringSecurityDescriptorW;
        use winapi::um::securitybaseapi::GetKernelObjectSecurity;
        use winapi::um::winnt::{ DACL_SECURITY_INFORMATION, LABEL_SECURITY_INFORMATION };

        let which = DACL_SECURITY_INFORMATION | LABEL_SECURITY_INFORMATION;
        unsafe {
            let mut needed: DWORD = 0;
            GetKernelObjectSecurity(handle, which, null_mut(), 0, &mut needed);
            let mut buffer = vec![0u64; (needed as usize).div_ceil(8)];
            assert_ne!(
                GetKernelObjectSecurity(handle, which, buffer.as_mut_ptr().cast(), needed, &mut needed),
                0,
                "reading the event's security failed"
            );
            let mut text: LPWSTR = null_mut();
            assert_ne!(
                ConvertSecurityDescriptorToStringSecurityDescriptorW(
                    buffer.as_mut_ptr().cast(),
                    SDDL_REVISION_1 as DWORD,
                    which,
                    &mut text,
                    null_mut()
                ),
                0
            );
            let length = (0..).take_while(|&i| *text.add(i) != 0).count();
            let sddl = String::from_utf16_lossy(std::slice::from_raw_parts(text, length));
            LocalFree(text.cast());
            sddl
        }
    }

    /// The event really carries the user grant and the medium label; the
    /// default security of an elevated copy has neither, which is what kept
    /// a normal second copy from raising its window.
    #[test]
    fn the_created_wake_event_carries_the_user_grant_and_medium_label() {
        let name = test_event_name("security");
        let handle = create_wake_event(&name);
        assert!(!handle.is_null(), "the event could not be created");

        let sddl = event_security(handle);
        let sid = current_user_sid().unwrap();
        unsafe {
            CloseHandle(handle);
        }
        assert!(sddl.contains(&format!("(A;;0x100002;;;{sid})")), "no grant for the user in {sddl}");
        assert!(sddl.contains("(ML;;NW;;;ME)"), "no medium label in {sddl}");
    }

    /// Opening the event the way a second copy does, with only the right to
    /// set it, works, and setting it wakes the listener's handle.
    #[test]
    fn a_second_copy_can_open_and_set_the_wake_event() {
        let name = test_event_name("open");
        let handle = create_wake_event(&name);
        assert!(!handle.is_null(), "the event could not be created");

        unsafe {
            let opened = OpenEventW(EVENT_MODIFY_STATE, 0, name.as_ptr());
            assert!(!opened.is_null(), "OpenEventW failed with {}", GetLastError());
            assert_ne!(SetEvent(opened), 0);
            CloseHandle(opened);

            assert_eq!(WaitForSingleObject(handle, 0), WAIT_OBJECT_0);
            CloseHandle(handle);
        }
    }
}
