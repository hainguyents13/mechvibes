//! Windows: asks the audio stack directly. A capture endpoint with an active
//! audio session means some app is recording from it right now, which is the
//! same signal that lights the microphone icon in the taskbar. The
//! `CapabilityAccessManager` registry key that tracks microphone use was tried
//! first, but current Windows builds no longer update its `LastUsedTimeStop`
//! values while an app records, so it cannot tell a call in progress from one
//! that ended weeks ago.

use windows::Win32::Media::Audio::{
    eCapture,
    AudioSessionStateActive,
    IAudioSessionManager2,
    IMMDevice,
    IMMDeviceEnumerator,
    MMDeviceEnumerator,
    DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::{ CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED };

pub(super) struct Detector {
    enumerator: Option<IMMDeviceEnumerator>,
}

impl Detector {
    /// Joins this thread to the multithreaded COM apartment and creates the
    /// device enumerator once. If that fails the detector reports the
    /// microphone as free, so the app just keeps playing.
    pub(super) fn new() -> Self {
        // SAFETY: plain COM calls on the watcher's own thread, which stays in
        // the apartment for its whole life.
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

    /// True when any active capture device has an active audio session, i.e.
    /// some app is recording from a microphone right now. A failed call
    /// counts as "not recording".
    pub(super) fn microphone_in_use(&mut self) -> bool {
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
    // SAFETY: COM calls on a device handed out by the enumerator above, on the
    // same thread.
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
