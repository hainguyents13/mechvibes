//! Input worker process entry point (Windows only).
//!
//! The same executable, launched with `--input-worker`, runs this instead
//! of the Dioxus UI. It owns the Raw Input registration (`rawinput_listener`)
//! and streams every key/button transition to its parent over stdout, one
//! line per event.
//!
//! It exists as a separate process because Windows only allows one Raw Input
//! target window per device class per process, and tao/wry claims that slot
//! in the UI process - see `rawinput_listener.rs` module docs. This process
//! never builds a webview, so nothing competes for the registration.
//!
//! ## Wire format (worker -> host, one line per event)
//!
//! ```text
//! K\t{device_id}\t{code}\t{down|up}    keyboard
//! M\t{device_id}\t{code}\t{down|up}    mouse
//! ```
//!
//! `device_id` is the hash `InputDeviceManager` computes for that physical
//! device, or `-` when the event has no source device. `code` is the same
//! string the rdev/evdev listeners produce (`"KeyA"`, `"MouseLeft"`), so the
//! audio engine's parser is untouched. Text rather than a binary encoding so
//! the stream can be read by eye when debugging: run the worker from a
//! terminal and watch the lines.
//!
//! Raw Input reports a NULL `hDevice` for keys injected by software
//! (`SendInput`: input methods, on-screen keyboards, AutoHotkey), while a real
//! keystroke always carries the handle of its keyboard. So a `K` line with `-`
//! is an injected key. The host decides whether it plays
//! (`input_worker_host.rs`, the "on-screen keyboards" setting).
//!
//! Deliberately NOT done here: filtering and hotkey detection. Config
//! lives in the UI process; keeping the only copy there avoids syncing it
//! across the pipe (see `input_worker_host.rs`).
//!
//! ## Lifetime
//!
//! The worker exits when its stdin reaches EOF, which Windows delivers as
//! soon as the parent's pipe handle closes - on clean exit *and* on crash.
//! That is the whole orphan-prevention mechanism; no Job Object needed.
#![cfg(target_os = "windows")]

use std::io::{ BufWriter, Read, Write };

use crate::libs::rawinput_listener::{ run_rawinput_loop, EventKind, RawInputEvent };

/// Command-line flag that selects this mode. Checked in `main()` before any
/// other initialization.
pub const WORKER_ARG: &str = "--input-worker";

/// Runs the worker until stdin closes or the message loop dies. Never
/// returns to normal app startup - callers should exit the process after.
pub fn run() {
    spawn_stdin_lifeline();

    let mut out = BufWriter::new(std::io::stdout());

    let result = run_rawinput_loop(
        Box::new(move |event: RawInputEvent| {
            let kind = match event.kind {
                EventKind::Keyboard => 'K',
                EventKind::Mouse => 'M',
            };
            let device_id = event.device_id.as_deref().unwrap_or("-");
            let direction = if event.is_down { "down" } else { "up" };

            // Flushed per event on purpose: a BufWriter that only flushes
            // when full would hold keystrokes back until the next burst,
            // which is exactly the latency this architecture exists to
            // remove. The write is a few dozen bytes into a pipe.
            if
                writeln!(out, "{}\t{}\t{}\t{}", kind, device_id, event.code, direction).is_err() ||
                out.flush().is_err()
            {
                // Parent went away mid-write; nothing left to serve.
                std::process::exit(0);
            }
        })
    );

    if let Err(e) = result {
        eprintln!("[worker] Raw Input listener failed: {}", e);
        std::process::exit(1);
    }
}

/// Exits the process when the parent's end of our stdin pipe closes.
///
/// This is the orphan guard: if the UI process exits or crashes, Windows
/// closes its handles, our stdin hits EOF, and we go away with it. Runs on
/// its own thread because the Raw Input message loop owns the main one.
///
/// Note when debugging by hand: launching the worker without giving it a
/// live stdin (e.g. `Start-Process` with stdout redirected but no stdin
/// pipe) makes it exit immediately, because that stdin reports EOF at once.
/// That is the guard working, not a crash - to watch its output, spawn it
/// with stdin piped and hold the handle open, the way the host does.
fn spawn_stdin_lifeline() {
    std::thread::spawn(|| {
        let mut stdin = std::io::stdin();
        let mut buf = [0u8; 64];
        loop {
            match stdin.read(&mut buf) {
                Ok(0) => break, // EOF - parent is gone
                Ok(_) => {} // host currently sends nothing; ignore and keep waiting
                Err(_) => break,
            }
        }
        std::process::exit(0);
    });
}
