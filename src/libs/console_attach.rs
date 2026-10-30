//! Reattaches this process to the terminal that launched it (Windows).
//!
//! The binary is built `#![windows_subsystem = "windows"]` so the GUI does not
//! flash a console window. The cost is that `println!` from a run started in
//! PowerShell goes nowhere: the process starts with no console at all, so
//! headless mode would be completely silent - no status lines, no "soundpack
//! not found", nothing.
//!
//! `AttachConsole(ATTACH_PARENT_PROCESS)` borrows the parent's console when
//! there is one. It is deliberately *not* paired with `AllocConsole`: a
//! headless run started from Task Scheduler or a login item has no parent
//! console, and popping a new window there would defeat the point of the mode.
//! In that case output goes only to the in-memory log buffer, which every
//! logging macro already tees into.
#![cfg(target_os = "windows")]

use std::sync::Once;

/// Attaches at most once. A second `AttachConsole` on an already-attached
/// process fails with `ERROR_ACCESS_DENIED` and would reopen the std handles
/// for no reason.
static ATTACHED: Once = Once::new();

/// Attaches to the parent process's console, if it has one.
///
/// Safe to call when there is no parent console: the attach fails and nothing
/// is reopened, leaving output to the log buffer alone.
pub fn attach_to_parent_console() {
    ATTACHED.call_once(|| {
        // A process that already has usable standard output must be left
        // alone. Two cases reach this: a console subsystem build, and any run
        // whose stdout is a pipe or a file (`mechvibes --headless > log`,
        // or a test harness capturing output). Reopening `CONOUT$` there
        // would redirect the stream away from the pipe the caller is reading,
        // discarding output they asked for.
        if has_usable_stdout() {
            return;
        }

        // SAFETY: `AttachConsole` takes only a process id and touches no
        // memory we own. `ATTACH_PARENT_PROCESS` is the documented sentinel
        // for "the console of the process that launched me". A zero return
        // means there was nothing to attach to, which is handled rather than
        // treated as success.
        let attached = unsafe {
            winapi::um::wincon::AttachConsole(winapi::um::wincon::ATTACH_PARENT_PROCESS)
        };
        if attached == 0 {
            return;
        }

        reopen_std_handles();
    });
}

/// Whether this process already has somewhere for `println!` to go.
///
/// A GUI-subsystem process launched from Explorer has `STD_OUTPUT_HANDLE` as
/// null or invalid, which is the case the attach exists for. Anything else -
/// an inherited console, a pipe, a redirect to a file - is already working and
/// must not be touched.
fn has_usable_stdout() -> bool {
    // SAFETY: `GetStdHandle` only looks up a handle in the process's own
    // startup information and touches no memory we own.
    let handle = unsafe {
        winapi::um::processenv::GetStdHandle(winapi::um::winbase::STD_OUTPUT_HANDLE)
    };
    !handle.is_null() && handle != winapi::um::handleapi::INVALID_HANDLE_VALUE
}

/// Points this process's C runtime stdout/stderr at the newly attached
/// console.
///
/// Attaching alone is not enough. The CRT bound `stdout`/`stderr` to invalid
/// handles at startup, when the process had no console, and Rust's `println!`
/// writes through those - so without this the attach succeeds and the output
/// still goes nowhere. `freopen` rebinds them to the console device.
fn reopen_std_handles() {
    // SAFETY: both string literals are NUL-terminated and static. `freopen`
    // is given the CRT's own `stdout`/`stderr` streams; a failure returns
    // null and is ignored, leaving the stream as it was.
    unsafe {
        let conout = c"CONOUT$".as_ptr();
        let mode = c"w".as_ptr();
        libc_freopen(conout, mode, stdout_stream());
        libc_freopen(conout, mode, stderr_stream());
    }
}

// The MSVC CRT exposes stdout/stderr through accessor functions rather than
// as exported globals, so they cannot simply be declared `extern`.
unsafe extern "C" {
    #[link_name = "freopen"]
    fn libc_freopen(
        filename: *const std::ffi::c_char,
        mode: *const std::ffi::c_char,
        stream: *mut std::ffi::c_void
    ) -> *mut std::ffi::c_void;

    #[link_name = "__acrt_iob_func"]
    fn acrt_iob_func(index: u32) -> *mut std::ffi::c_void;
}

/// The CRT's `stdout` stream (index 1 in the `__acrt_iob` table).
fn stdout_stream() -> *mut std::ffi::c_void {
    // SAFETY: index 1 is `stdout` in the documented MSVC CRT stream table.
    unsafe { acrt_iob_func(1) }
}

/// The CRT's `stderr` stream (index 2).
fn stderr_stream() -> *mut std::ffi::c_void {
    // SAFETY: index 2 is `stderr` in the documented MSVC CRT stream table.
    unsafe { acrt_iob_func(2) }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A process that already has working stdout must be left strictly alone.
    ///
    /// This is not a hypothetical: an earlier version attached unconditionally
    /// and `freopen`'d `CONOUT$` over the test harness's capture pipe, which
    /// killed the whole `cargo test` run mid-output. The same mechanism would
    /// silently discard the output of `mechvibes --headless > log.txt`.
    #[test]
    fn a_process_with_working_stdout_is_left_alone() {
        assert!(
            has_usable_stdout(),
            "the test harness always provides stdout - if this fails the guard \
             below is not being exercised"
        );

        // Reaching the end proves the guard held: stdout still belongs to the
        // harness, and the lines below are still captured by it.
        attach_to_parent_console();
        println!("stdout still works after attach_to_parent_console()");
    }

    /// The `Once` guard means repeated calls cost nothing and cannot reopen
    /// the streams a second time.
    #[test]
    fn attaching_is_idempotent_and_never_panics() {
        attach_to_parent_console();
        attach_to_parent_console();
    }
}
