//! Mutes the app while another app is using a microphone, when the user has
//! switched "Mute while the microphone is in use" on.
//!
//! Each platform has its own detector, in the submodule of the same name:
//! Windows asks for active capture sessions, macOS asks CoreAudio which
//! processes are taking input, and Linux asks the PulseAudio or PipeWire
//! server for recording streams. Each one answers "not in use" when it cannot
//! tell, so a detector that fails leaves the app playing.
//!
//! The mute is runtime-only. It never writes `enable_sound`: the engine plays
//! only when the user's own mute is off *and* the microphone mute is off, so
//! when a call ends the user's Ctrl+Alt+M choice is exactly what it was.

use std::time::Duration;

use crate::libs::audio::engine::{ AudioCommand, AudioEngineHandle };

const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Whether the engine should be muted right now.
fn wanted_mic_mute(setting_on: bool, mic_in_use: impl FnOnce() -> bool) -> bool {
    // The detector only runs while the setting is on, so leaving the switch
    // off costs nothing.
    setting_on && mic_in_use()
}

/// The command to send for a new `wanted` value, or `None` when it matches
/// what the engine was last told. The engine hears about changes only.
fn command_for_change(last_sent: bool, wanted: bool) -> Option<AudioCommand> {
    (wanted != last_sent).then_some(AudioCommand::SetMicMuted(wanted))
}

/// Starts the background thread that keeps the engine's microphone mute in
/// step with the setting and the microphone. One check per second.
pub fn spawn_watcher(engine: AudioEngineHandle) {
    std::thread::spawn(move || {
        let mut detector = Detector::new();
        let mut last_sent = false;
        loop {
            let setting_on = crate::state::config_writer::current().mute_while_mic_in_use;
            let wanted = wanted_mic_mute(setting_on, || detector.microphone_in_use());
            if let Some(command) = command_for_change(last_sent, wanted) {
                crate::always_print!(
                    "🎙️ [MicActivity] {}",
                    if wanted { "Microphone in use, muting" } else { "Unmuting" }
                );
                engine.send(command);
                last_sent = wanted;
            }
            std::thread::sleep(POLL_INTERVAL);
        }
    });
}

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use self::windows::Detector;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use self::macos::Detector;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use self::linux::Detector;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_switch_off_never_mutes_and_never_asks_the_microphone() {
        let mut asked = false;
        assert!(
            !wanted_mic_mute(false, || {
                asked = true;
                true
            })
        );
        assert!(!asked, "with the switch off the detector must not run");
    }

    #[test]
    fn the_switch_on_mutes_only_while_the_microphone_is_in_use() {
        assert!(wanted_mic_mute(true, || true));
        assert!(!wanted_mic_mute(true, || false));
    }

    #[test]
    fn the_engine_hears_only_about_changes() {
        assert!(command_for_change(false, false).is_none());
        assert!(command_for_change(true, true).is_none());
        assert!(matches!(command_for_change(false, true), Some(AudioCommand::SetMicMuted(true))));
        assert!(matches!(command_for_change(true, false), Some(AudioCommand::SetMicMuted(false))));
    }

    /// Needs a real microphone, so it only runs on demand:
    /// `cargo test mic_activity -- --ignored --nocapture` while another app
    /// records, and again after it stops.
    #[test]
    #[ignore]
    fn print_whether_the_microphone_is_in_use_now() {
        let started = std::time::Instant::now();
        let in_use = Detector::new().microphone_in_use();
        println!("microphone in use: {in_use} ({:?})", started.elapsed());
    }
}
