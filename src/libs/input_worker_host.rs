//! UI-side half of the Windows input worker: spawns the worker process,
//! reads its event stream, and feeds the existing keyboard/mouse/hotkey
//! channels so the audio engine (`audio/engine.rs`) sees exactly what it saw
//! when rdev produced those events.
//!
//! Everything that needs `AppConfig` lives here rather than in the worker:
//! per-device filtering and the Ctrl+Alt+M hotkey. That keeps config in one
//! process and the pipe one-directional.
//!
//! If the worker cannot be kept alive (spawn failure, or it dies
//! `MAX_RESTARTS` times), this falls back to the old rdev + device_query
//! listeners: the focus bug comes back, but the app still makes sound.
#![cfg(target_os = "windows")]

use crossbeam_channel::Sender;
use std::io::{ BufRead, BufReader };
use std::os::windows::process::CommandExt;
use std::process::{ Child, Command, Stdio };
use std::sync::atomic::{ AtomicU64, Ordering };
use std::sync::Mutex;
use std::time::{ Duration, Instant };

use crate::libs::input_worker::WORKER_ARG;

/// `CREATE_NO_WINDOW` - keeps the worker from flashing a console window.
/// Not exposed by the winapi features this crate enables, and it is a stable
/// documented constant.
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// After this many failed starts in a row we stop trying and fall back.
const MAX_RESTARTS: u32 = 5;

/// A worker that stayed up at least this long counts as healthy, so the
/// restart backoff resets. Without this, a worker that runs fine for hours
/// and then dies once would be treated as the 2nd of 5 strikes forever.
const HEALTHY_UPTIME: Duration = Duration::from_secs(30);

/// Bumped whenever Settings changes `enabled_keyboards`/`enabled_mice`, so
/// the reader thread notices its cached filter is stale.
static CONFIG_GENERATION: AtomicU64 = AtomicU64::new(0);

/// The lists themselves, published alongside the generation so the reader
/// thread never has to go looking for them. Reading them from the config
/// authority costs a clone; the `AppConfig::load()` this replaced parsed JSON,
/// read the registry for the auto-start state and could rewrite the file - none
/// of which belongs on the thread draining the worker's pipe, because stalling
/// that reader back-pressures the pipe and, through it, the worker's WndProc.
static ENABLED_DEVICES: Mutex<Option<(Vec<String>, Vec<String>)>> = Mutex::new(None);

/// Call after `AppConfig`'s enabled-device lists change (from
/// `device_selector.rs`) to make filtering take effect immediately.
pub fn notify_config_changed() {
    let config = crate::state::config_writer::current();
    *ENABLED_DEVICES.lock().unwrap() = Some((config.enabled_keyboards, config.enabled_mice));
    // Published after the lists, so a reader that sees the new generation is
    // guaranteed to find the new lists behind it.
    CONFIG_GENERATION.fetch_add(1, Ordering::Release);
}

/// Cached copy of the enabled-device filter, refreshed only when
/// `CONFIG_GENERATION` moves - re-reading config per keystroke is what the
/// audio path was fixed away from in Phase 1/2.
struct DeviceFilter {
    generation: u64,
    enabled_keyboards: Vec<String>,
    enabled_mice: Vec<String>,
}

impl DeviceFilter {
    /// Reads config from disk. Called once when a worker starts, from the
    /// supervisor thread before the read loop begins - never from inside it.
    fn load() -> Self {
        let generation = CONFIG_GENERATION.load(Ordering::Acquire);
        let (enabled_keyboards, enabled_mice) = ENABLED_DEVICES.lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| {
                let config = crate::state::config_writer::current();
                (config.enabled_keyboards, config.enabled_mice)
            });
        Self { generation, enabled_keyboards, enabled_mice }
    }

    fn refresh_if_stale(&mut self) {
        let generation = CONFIG_GENERATION.load(Ordering::Acquire);
        if generation == self.generation {
            return;
        }
        if let Some((keyboards, mice)) = ENABLED_DEVICES.lock().unwrap().clone() {
            self.enabled_keyboards = keyboards;
            self.enabled_mice = mice;
        }
        self.generation = generation;
    }

    /// Empty list = all devices allowed, matching
    /// `InputDeviceManager::should_process_device`'s convention and what
    /// Settings persists by default (`config.rs` defaults both to empty).
    /// A device whose id the worker could not resolve (`-`) is allowed
    /// through: going silent on a lookup failure would be a worse failure
    /// mode than an unfilterable device making sound.
    fn allows(&self, kind: char, device_id: &str) -> bool {
        let enabled = match kind {
            'K' => &self.enabled_keyboards,
            _ => &self.enabled_mice,
        };
        enabled.is_empty() || device_id == "-" || enabled.iter().any(|id| id == device_id)
    }
}

/// Tracks Ctrl/Alt so Ctrl+Alt+M can be recognized, mirroring
/// `input_listener.rs`'s rdev hotkey logic.
#[derive(Default)]
struct HotkeyState {
    ctrl_pressed: bool,
    alt_pressed: bool,
}

impl HotkeyState {
    /// Returns true when this event was the hotkey itself and should not
    /// also be played as a normal key.
    fn observe(&mut self, code: &str, is_down: bool) -> bool {
        match code {
            "ControlLeft" | "ControlRight" => {
                self.ctrl_pressed = is_down;
            }
            "AltLeft" | "AltRight" => {
                self.alt_pressed = is_down;
            }
            "KeyM" if is_down && self.ctrl_pressed && self.alt_pressed => {
                return true;
            }
            _ => {}
        }
        false
    }
}

/// Updates the consecutive-failure count after one worker attempt.
///
/// A worker that stayed up for `HEALTHY_UPTIME` was clearly working, so its
/// eventual death starts the count over instead of adding to it. Without
/// that, a worker restarted once every few hours would still accumulate
/// strikes and eventually trip the permanent fallback.
fn next_failure_count(failures: u32, uptime: Option<Duration>) -> u32 {
    match uptime {
        Some(uptime) if uptime >= HEALTHY_UPTIME => 0,
        _ => failures + 1,
    }
}

/// How long to wait before starting the next worker: 1s, 2s, 4s, 8s, capped.
/// Keeps a crash-looping worker from spinning the CPU.
///
/// `failures` is 0 when the worker that just died had been up long enough to
/// count as healthy, and that case has to wait the *shortest* time, not the
/// longest - a healthy worker dying once is the "user killed it in Task
/// Manager" case, where input should come back immediately.
fn restart_delay(failures: u32) -> Duration {
    Duration::from_secs(1u64 << failures.saturating_sub(1).min(3))
}

/// Starts the worker supervision thread. Returns immediately; input starts
/// flowing once the worker is up.
///
/// `on_fallback` runs if the worker can't be sustained, and should start the
/// legacy rdev/device_query listeners. It is called at most once.
pub fn start_input_worker_host(
    keyboard_tx: Sender<String>,
    mouse_tx: Sender<String>,
    hotkey_tx: Sender<String>,
    on_fallback: Box<dyn FnOnce() + Send>
) {
    std::thread::spawn(move || {
        let mut failures: u32 = 0;

        loop {
            let started_at = Instant::now();

            match spawn_worker() {
                Ok(child) => {
                    let exit = pump_worker(child, &keyboard_tx, &mouse_tx, &hotkey_tx);

                    if exit == PumpExit::ChannelClosed {
                        // The audio engine is gone, so the app is shutting
                        // down. Restarting a worker to feed a dead channel
                        // would just spawn a process nobody reads.
                        return;
                    }

                    failures = next_failure_count(failures, Some(started_at.elapsed()));
                    crate::always_eprint!("⚠️ [InputWorker] Worker exited; restarting");
                }
                Err(e) => {
                    failures = next_failure_count(failures, None);
                    crate::always_eprint!("❌ [InputWorker] Failed to spawn worker: {}", e);
                }
            }

            if failures >= MAX_RESTARTS {
                crate::always_eprint!(
                    "❌ [InputWorker] Giving up after {} attempts - falling back to rdev (keyboard input will not work while the app window is focused)",
                    failures
                );
                // Safe to start rdev now: pump_worker only returns once the
                // worker process has been reaped, so nothing else can still
                // be feeding these channels and doubling every sound.
                on_fallback();
                return;
            }

            std::thread::sleep(restart_delay(failures));
        }
    });
}

/// Why `pump_worker` stopped reading, which is what decides whether a
/// restart makes sense.
#[derive(PartialEq)]
enum PumpExit {
    /// The worker's stdout closed - it exited on its own and should be
    /// restarted.
    WorkerGone,
    /// A local problem (pipe error, or stdout was not captured) ended the
    /// read. The worker was killed; a restart is still worth trying.
    ReaderFailed,
    /// The audio engine's receivers are gone, so there is nothing left to
    /// feed. The worker was killed and no restart should follow.
    ChannelClosed,
}

fn spawn_worker() -> std::io::Result<Child> {
    let exe = std::env::current_exe()?;

    Command::new(exe)
        .arg(WORKER_ARG)
        .stdin(Stdio::piped()) // never written to - it is the worker's EOF lifeline
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
}

/// Forwards the worker's stderr into this process's log. Without it, the
/// `RegisterRawInputDevices`/`CreateWindowExW` error behind a permanent
/// failure would be lost, leaving only "gave up, falling back" with no
/// indication of why - the difference between a diagnosable field report and
/// a dead end.
fn drain_worker_stderr(stderr: std::process::ChildStderr) {
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            crate::always_eprint!("[InputWorker] {}", line);
        }
    });
}

/// Reads the worker's event lines, forwarding each to the audio channels.
///
/// Always leaves the worker process dead and reaped before returning, on
/// every exit path. That matters twice over: an orphaned worker would linger
/// in Task Manager holding a global input registration, and if the caller
/// went on to start the rdev fallback while it still ran, every keystroke
/// would play twice.
fn pump_worker(
    mut child: Child,
    keyboard_tx: &Sender<String>,
    mouse_tx: &Sender<String>,
    hotkey_tx: &Sender<String>
) -> PumpExit {
    // Taken (not dropped) for as long as we want the worker alive: closing
    // this pipe is what trips the worker's stdin-EOF lifeline. `reap` below
    // drops it deliberately - note that `Child::wait` would normally close
    // it for us to avoid deadlocking, but it cannot once it has been moved
    // out here, so the drop has to be explicit.
    let stdin = child.stdin.take();

    if let Some(stderr) = child.stderr.take() {
        drain_worker_stderr(stderr);
    }

    let Some(stdout) = child.stdout.take() else {
        reap(&mut child, stdin);
        return PumpExit::ReaderFailed;
    };

    let mut filter = DeviceFilter::load();
    let mut hotkey = HotkeyState::default();
    let mut exit = PumpExit::WorkerGone;
    // Keys/buttons the engine currently believes are held. If the worker
    // dies while one is down, its release never arrives, and the engine's
    // debounce would then swallow the next press of that key as a duplicate
    // - leaving it silent until pressed twice. Releases are synthesized on
    // the way out to keep the two sides in agreement.
    let mut held: Vec<(char, String)> = Vec::new();

    for line in BufReader::new(stdout).lines() {
        let Ok(line) = line else {
            exit = PumpExit::ReaderFailed;
            break;
        };

        let Some(event) = parse_worker_line(&line) else {
            continue;
        };

        filter.refresh_if_stale();

        if event.kind == 'K' {
            // Hotkey tracking runs before the device filter on purpose: the
            // global mute toggle should work from any keyboard, even one the
            // user disabled for soundpack playback.
            if hotkey.observe(event.code, event.is_down) {
                let _ = hotkey_tx.send("TOGGLE_SOUND".to_string());
                continue;
            }
        }

        let Some(wire) = wire_for_event(&event, &filter, &mut held) else {
            continue;
        };

        if event.kind == 'K' {
            crate::libs::trace::record(crate::libs::trace::Point::WorkerSend, event.code, 0.0);
        }

        let sent = match event.kind {
            'K' => keyboard_tx.send(wire),
            _ => mouse_tx.send(wire),
        };
        if sent.is_err() {
            exit = PumpExit::ChannelClosed;
            break;
        }
    }

    if exit != PumpExit::ChannelClosed {
        for (kind, code) in held {
            let wire = format!("UP:{}", code);
            let _ = match kind {
                'K' => keyboard_tx.send(wire),
                _ => mouse_tx.send(wire),
            };
        }
    }

    reap(&mut child, stdin);
    exit
}

/// Makes sure the worker is gone: closes its lifeline, then kills it in case
/// it is wedged somewhere that ignores EOF, then waits so it does not linger
/// as a zombie.
fn reap(child: &mut Child, stdin: Option<std::process::ChildStdin>) {
    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();
}

struct WorkerEvent<'a> {
    kind: char,
    device_id: &'a str,
    code: &'a str,
    is_down: bool,
}

/// Decides what to put on the wire for one event, updating `held` to match.
/// `None` means the event is dropped and nothing is sent.
///
/// The device filter gates presses only. A release is forwarded whenever its
/// press was forwarded, whatever the filter says by then: disabling a keyboard
/// while one of its keys is held would otherwise swallow the release, and the
/// engine's debounce would treat the next press of that key as a duplicate and
/// stay silent. Keying the decision off `held` rather than off the filter also
/// stops a disabled device from leaking a lone keyup sound, which testing the
/// filter on the down alone would not.
///
/// A wheel scroll is a down with no release, so it is forwarded without being
/// recorded in `held`. Recording it would grow `held` by one entry per scroll
/// and, when the worker restarts, send releases for scrolls that have no
/// release.
fn wire_for_event(
    event: &WorkerEvent<'_>,
    filter: &DeviceFilter,
    held: &mut Vec<(char, String)>
) -> Option<String> {
    if event.is_down {
        if !filter.allows(event.kind, event.device_id) {
            return None;
        }
        let is_wheel = event.kind == 'M' && crate::libs::audio::engine::is_wheel_code(event.code);
        if !is_wheel {
            held.push((event.kind, event.code.to_string()));
        }
        return Some(event.code.to_string());
    }

    let is_held = |(kind, code): &(char, String)| *kind == event.kind && code == event.code;
    if !held.iter().any(is_held) {
        return None;
    }
    held.retain(|entry| !is_held(entry));
    Some(format!("UP:{}", event.code))
}

/// Parses one `K\t{device_id}\t{code}\t{down|up}` line. Returns `None` for
/// anything malformed so a garbled line can never be mistaken for input.
fn parse_worker_line(line: &str) -> Option<WorkerEvent<'_>> {
    let mut parts = line.split('\t');
    let kind = match parts.next()? {
        "K" => 'K',
        "M" => 'M',
        _ => {
            return None;
        }
    };
    let device_id = parts.next()?;
    let code = parts.next()?;
    let is_down = match parts.next()? {
        "down" => true,
        "up" => false,
        _ => {
            return None;
        }
    };
    if code.is_empty() || parts.next().is_some() {
        return None;
    }

    Some(WorkerEvent { kind, device_id, code, is_down })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_well_formed_lines() {
        let e = parse_worker_line("K\tabc123\tKeyA\tdown").unwrap();
        assert_eq!(e.kind, 'K');
        assert_eq!(e.device_id, "abc123");
        assert_eq!(e.code, "KeyA");
        assert!(e.is_down);

        let e = parse_worker_line("M\t-\tMouseLeft\tup").unwrap();
        assert_eq!(e.kind, 'M');
        assert_eq!(e.device_id, "-");
        assert!(!e.is_down);
    }

    #[test]
    fn rejects_malformed_lines() {
        for line in [
            "",
            "K",
            "K\tabc\tKeyA",
            "X\tabc\tKeyA\tdown", // unknown kind
            "K\tabc\tKeyA\tsideways", // unknown direction
            "K\tabc\t\tdown", // empty code
            "K\tabc\tKeyA\tdown\textra", // trailing field
        ] {
            assert!(parse_worker_line(line).is_none(), "should reject {:?}", line);
        }
    }

    #[test]
    fn empty_filter_allows_everything() {
        let filter = DeviceFilter {
            generation: 0,
            enabled_keyboards: Vec::new(),
            enabled_mice: Vec::new(),
        };
        assert!(filter.allows('K', "anything"));
        assert!(filter.allows('M', "anything"));
    }

    #[test]
    fn populated_filter_allows_only_listed_devices_and_unknown_ids() {
        let filter = DeviceFilter {
            generation: 0,
            enabled_keyboards: vec!["kb1".to_string()],
            enabled_mice: vec!["m1".to_string()],
        };
        assert!(filter.allows('K', "kb1"));
        assert!(!filter.allows('K', "kb2"));
        assert!(filter.allows('M', "m1"));
        assert!(!filter.allows('M', "m2"));
        // Unresolvable device id fails open rather than going silent.
        assert!(filter.allows('K', "-"));
    }

    #[test]
    fn healthy_uptime_clears_accumulated_failures() {
        assert_eq!(next_failure_count(3, Some(HEALTHY_UPTIME)), 0);
        assert_eq!(next_failure_count(MAX_RESTARTS - 1, Some(HEALTHY_UPTIME * 10)), 0);
    }

    #[test]
    fn short_lived_worker_accumulates_failures_until_fallback() {
        let mut failures = 0;
        for expected in 1..=MAX_RESTARTS {
            failures = next_failure_count(failures, Some(Duration::from_secs(1)));
            assert_eq!(failures, expected);
        }
        // Only now, after MAX_RESTARTS consecutive quick deaths, does the
        // supervisor give up and fall back to rdev.
        assert!(failures >= MAX_RESTARTS);
    }

    #[test]
    fn spawn_failure_counts_as_a_failure() {
        assert_eq!(next_failure_count(0, None), 1);
        assert_eq!(next_failure_count(2, None), 3);
    }

    #[test]
    fn healthy_worker_death_retries_immediately_rather_than_underflowing() {
        // A worker up longer than HEALTHY_UPTIME resets the count to 0, and
        // that 0 reaches the backoff. Computing `failures - 1` here panicked
        // in debug builds and wrapped to the 8s cap in release, which is the
        // opposite of what a healthy worker's first death deserves.
        let failures = next_failure_count(4, Some(HEALTHY_UPTIME));
        assert_eq!(failures, 0);
        assert_eq!(restart_delay(failures), Duration::from_secs(1));
    }

    #[test]
    fn restart_delay_doubles_then_caps() {
        assert_eq!(restart_delay(1), Duration::from_secs(1));
        assert_eq!(restart_delay(2), Duration::from_secs(2));
        assert_eq!(restart_delay(3), Duration::from_secs(4));
        assert_eq!(restart_delay(4), Duration::from_secs(8));
        // Capped, and never shifts past what a u64 can hold.
        assert_eq!(restart_delay(5), Duration::from_secs(8));
        assert_eq!(restart_delay(u32::MAX), Duration::from_secs(8));
    }

    fn filter_for(keyboards: Vec<&str>) -> DeviceFilter {
        DeviceFilter {
            generation: 0,
            enabled_keyboards: keyboards.into_iter().map(String::from).collect(),
            enabled_mice: Vec::new(),
        }
    }

    fn event<'a>(device_id: &'a str, code: &'a str, is_down: bool) -> WorkerEvent<'a> {
        WorkerEvent { kind: 'K', device_id, code, is_down }
    }

    #[test]
    fn release_survives_the_device_being_disabled_mid_hold() {
        let mut held = Vec::new();

        // Key goes down while the keyboard is still enabled.
        let allowed = filter_for(vec![]);
        assert_eq!(wire_for_event(&event("kb1", "KeyX", true), &allowed, &mut held).as_deref(), Some("KeyX"));

        // User disables that keyboard in Settings while the key is still held.
        let disabled = filter_for(vec!["kb2"]);
        assert_eq!(
            wire_for_event(&event("kb1", "KeyX", false), &disabled, &mut held).as_deref(),
            Some("UP:KeyX"),
            "release of an already-forwarded press must not be filtered away"
        );
        assert!(held.is_empty(), "release must clear the held entry");
    }

    #[test]
    fn disabled_device_sends_neither_press_nor_release() {
        let filter = filter_for(vec!["kb1"]);
        let mut held = Vec::new();

        assert!(wire_for_event(&event("kb2", "KeyX", true), &filter, &mut held).is_none());
        assert!(held.is_empty());
        // No matching press was forwarded, so the release is dropped too -
        // a lone keyup would otherwise reach the engine from a muted device.
        assert!(wire_for_event(&event("kb2", "KeyX", false), &filter, &mut held).is_none());
    }

    #[test]
    fn release_without_a_press_is_dropped() {
        let filter = filter_for(vec![]);
        let mut held = Vec::new();
        assert!(wire_for_event(&event("kb1", "KeyX", false), &filter, &mut held).is_none());
    }

    #[test]
    fn held_tracks_kind_and_code_independently() {
        let filter = filter_for(vec![]);
        let mut held = Vec::new();

        wire_for_event(&event("kb1", "KeyX", true), &filter, &mut held);
        let mouse_down = WorkerEvent { kind: 'M', device_id: "m1", code: "KeyX", is_down: true };
        wire_for_event(&mouse_down, &filter, &mut held);
        assert_eq!(held.len(), 2);

        // Releasing the mouse "KeyX" must not clear the keyboard's entry.
        let mouse_up = WorkerEvent { kind: 'M', device_id: "m1", code: "KeyX", is_down: false };
        assert_eq!(wire_for_event(&mouse_up, &filter, &mut held).as_deref(), Some("UP:KeyX"));
        assert_eq!(held, vec![('K', "KeyX".to_string())]);
    }

    #[test]
    fn wheel_scrolls_are_forwarded_but_never_held() {
        let filter = filter_for(vec![]);
        let mut held = Vec::new();

        for code in ["MouseWheelUp", "MouseWheelDown", "MouseWheelUp"] {
            let scroll = WorkerEvent { kind: 'M', device_id: "m1", code, is_down: true };
            assert_eq!(wire_for_event(&scroll, &filter, &mut held).as_deref(), Some(code));
        }
        // Nothing held means no release is synthesized for a scroll when the
        // worker restarts, and the list does not grow with every scroll.
        assert!(held.is_empty());
    }

    #[test]
    fn wheel_scrolls_still_respect_the_mouse_filter() {
        let filter = DeviceFilter {
            generation: 0,
            enabled_keyboards: Vec::new(),
            enabled_mice: vec!["m1".to_string()],
        };
        let mut held = Vec::new();
        let scroll = WorkerEvent { kind: 'M', device_id: "m2", code: "MouseWheelUp", is_down: true };
        assert!(wire_for_event(&scroll, &filter, &mut held).is_none());
    }

    #[test]
    fn hotkey_needs_both_modifiers_held() {
        let mut state = HotkeyState::default();
        assert!(!state.observe("KeyM", true));

        state.observe("ControlLeft", true);
        assert!(!state.observe("KeyM", true));

        state.observe("AltLeft", true);
        assert!(state.observe("KeyM", true));

        // Releasing a modifier disarms it again.
        state.observe("ControlLeft", false);
        assert!(!state.observe("KeyM", true));
    }
}
