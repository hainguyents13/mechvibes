//! Mutes the app while another app is using a microphone, when the user has
//! switched "Mute while the microphone is in use" on (Windows only for now).
//!
//! The detector asks the Windows audio stack directly: a capture endpoint
//! with an active audio session means some app is recording from it right
//! now, which is the same signal that lights the microphone icon in the
//! taskbar. The `CapabilityAccessManager` registry key that tracks
//! microphone use was tried first, but current Windows builds no longer
//! update its `LastUsedTimeStop` values while an app records, so it cannot
//! tell a call in progress from one that ended weeks ago.
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
        let detector = Detector::new();
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

use windows_impl::Detector;

mod windows_impl {
    use windows::Win32::Media::Audio::{
        eCapture,
        AudioSessionStateActive,
        IAudioSessionManager2,
        IMMDevice,
        IMMDeviceEnumerator,
        MMDeviceEnumerator,
        DEVICE_STATE_ACTIVE,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance,
        CoInitializeEx,
        CLSCTX_ALL,
        COINIT_MULTITHREADED,
    };

    pub(super) struct Detector {
        enumerator: Option<IMMDeviceEnumerator>,
    }

    impl Detector {
        /// Joins this thread to the multithreaded COM apartment and creates
        /// the device enumerator once. If that fails the detector reports the
        /// microphone as free, so the app just keeps playing.
        pub(super) fn new() -> Self {
            // SAFETY: plain COM calls on the watcher's own thread, which
            // stays in the apartment for its whole life.
            let enumerator = unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
            };
            match enumerator {
                Ok(enumerator) => Self { enumerator: Some(enumerator) },
                Err(e) => {
                    crate::always_eprint!("❌ [MicActivity] Could not open the audio device list: {}", e);
                    Self { enumerator: None }
                }
            }
        }

        /// True when any active capture device has an active audio session,
        /// i.e. some app is recording from a microphone right now. A failed
        /// call counts as "not recording".
        pub(super) fn microphone_in_use(&self) -> bool {
            let Some(enumerator) = &self.enumerator else {
                return false;
            };
            // SAFETY: COM calls on an enumerator created on this thread.
            unsafe {
                let Ok(devices) = enumerator.EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE) else {
                    return false;
                };
                let count = devices.GetCount().unwrap_or(0);
                (0..count).any(|index| {
                    devices
                        .Item(index)
                        .is_ok_and(|device| device_is_recording(&device))
                })
            }
        }
    }

    /// Whether any audio session on this capture device is active.
    fn device_is_recording(device: &IMMDevice) -> bool {
        // SAFETY: COM calls on a device handed out by the enumerator above,
        // on the same thread.
        unsafe {
            let Ok(manager) = device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else {
                return false;
            };
            let Ok(sessions) = manager.GetSessionEnumerator() else {
                return false;
            };
            let count = sessions.GetCount().unwrap_or(0);
            (0..count).any(|index| {
                sessions
                    .GetSession(index)
                    .and_then(|session| session.GetState())
                    .is_ok_and(|state| state == AudioSessionStateActive)
            })
        }
    }
}

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
