//! macOS: asks CoreAudio.
//!
//! On macOS 14 and later, CoreAudio lists every process doing audio and says
//! for each one whether it is taking input right now. That looks at input
//! only, so this app playing to a headset that also has a microphone does not
//! count.
//!
//! Older macOS has no such list. There the detector falls back to asking each
//! device whether any process is running it, but only for input-only devices
//! (a built-in or USB microphone). A device with outputs too, like a headset,
//! is skipped: this app's own playback keeps it running, so it would read as
//! "in use" for as long as the app plays and the app would never unmute.
//!
//! Reading these properties needs no microphone permission: nothing here
//! opens a device or reads audio.

use std::ffi::c_void;

type AudioObjectId = u32;

#[repr(C)]
struct AudioObjectPropertyAddress {
    selector: u32,
    scope: u32,
    element: u32,
}

#[link(name = "CoreAudio", kind = "framework")]
unsafe extern "C" {
    fn AudioObjectGetPropertyDataSize(
        object: AudioObjectId,
        address: *const AudioObjectPropertyAddress,
        qualifier_size: u32,
        qualifier: *const c_void,
        data_size: *mut u32
    ) -> i32;

    fn AudioObjectGetPropertyData(
        object: AudioObjectId,
        address: *const AudioObjectPropertyAddress,
        qualifier_size: u32,
        qualifier: *const c_void,
        data_size: *mut u32,
        data: *mut c_void
    ) -> i32;
}

/// A CoreAudio four-character code.
const fn fourcc(code: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*code)
}

const SYSTEM_OBJECT: AudioObjectId = 1;
const ELEMENT_MAIN: u32 = 0;
const SCOPE_GLOBAL: u32 = fourcc(b"glob");
const SCOPE_INPUT: u32 = fourcc(b"inpt");
const SCOPE_OUTPUT: u32 = fourcc(b"outp");
/// kAudioHardwarePropertyProcessObjectList, macOS 14 and later.
const PROCESS_OBJECT_LIST: u32 = fourcc(b"prs#");
/// kAudioProcessPropertyIsRunningInput, macOS 14 and later.
const PROCESS_IS_RUNNING_INPUT: u32 = fourcc(b"piri");
/// kAudioHardwarePropertyDevices.
const DEVICES: u32 = fourcc(b"dev#");
/// kAudioDevicePropertyStreams.
const DEVICE_STREAMS: u32 = fourcc(b"stm#");
/// kAudioDevicePropertyDeviceIsRunningSomewhere.
const DEVICE_IS_RUNNING_SOMEWHERE: u32 = fourcc(b"gone");

pub(super) struct Detector;

impl Detector {
    pub(super) fn new() -> Self {
        Self
    }

    /// True when some process is taking input from a microphone right now.
    /// A failed call counts as "not recording".
    pub(super) fn microphone_in_use(&mut self) -> bool {
        match object_list(SYSTEM_OBJECT, PROCESS_OBJECT_LIST) {
            Some(processes) => processes.into_iter().any(|process| u32_property(process, PROCESS_IS_RUNNING_INPUT, SCOPE_GLOBAL).is_some_and(|running| running != 0)),
            // Before macOS 14 the process list is an unknown property.
            None => input_only_device_in_use(),
        }
    }
}

/// The fallback for macOS before 14: some input-only device is running.
fn input_only_device_in_use() -> bool {
    let Some(devices) = object_list(SYSTEM_OBJECT, DEVICES) else {
        return false;
    };
    devices.into_iter().any(|device| {
        let has_inputs = data_size(device, DEVICE_STREAMS, SCOPE_INPUT).is_some_and(|size| size > 0);
        let has_outputs = data_size(device, DEVICE_STREAMS, SCOPE_OUTPUT).is_none_or(|size| size > 0);
        has_inputs && !has_outputs && u32_property(device, DEVICE_IS_RUNNING_SOMEWHERE, SCOPE_GLOBAL).is_some_and(|running| running != 0)
    })
}

fn address(selector: u32, scope: u32) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress { selector, scope, element: ELEMENT_MAIN }
}

/// The size in bytes of a property's value, or `None` when the object does
/// not have it.
fn data_size(object: AudioObjectId, selector: u32, scope: u32) -> Option<u32> {
    let address = address(selector, scope);
    let mut size = 0u32;
    // SAFETY: `address` and `size` are valid for the call; no qualifier.
    let status = unsafe { AudioObjectGetPropertyDataSize(object, &address, 0, std::ptr::null(), &mut size) };
    (status == 0).then_some(size)
}

/// A property holding a list of object ids, in global scope.
fn object_list(object: AudioObjectId, selector: u32) -> Option<Vec<AudioObjectId>> {
    let size = data_size(object, selector, SCOPE_GLOBAL)?;
    let mut ids = vec![0 as AudioObjectId; (size as usize) / std::mem::size_of::<AudioObjectId>()];
    let mut size = (ids.len() * std::mem::size_of::<AudioObjectId>()) as u32;
    let address = address(selector, SCOPE_GLOBAL);
    // SAFETY: `ids` has room for `size` bytes, and CoreAudio writes at most
    // that many and reports how many it wrote.
    let status = unsafe {
        AudioObjectGetPropertyData(object, &address, 0, std::ptr::null(), &mut size, ids.as_mut_ptr().cast())
    };
    if status != 0 {
        return None;
    }
    ids.truncate((size as usize) / std::mem::size_of::<AudioObjectId>());
    Some(ids)
}

/// A property holding one `u32`.
fn u32_property(object: AudioObjectId, selector: u32, scope: u32) -> Option<u32> {
    let address = address(selector, scope);
    let mut value = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: `value` is a `u32` and `size` says so.
    let status = unsafe {
        AudioObjectGetPropertyData(object, &address, 0, std::ptr::null(), &mut size, (&mut value as *mut u32).cast())
    };
    (status == 0).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_character_codes_match_the_coreaudio_headers() {
        // Values from AudioHardware.h / AudioHardwareBase.h.
        assert_eq!(PROCESS_OBJECT_LIST, 0x7072_7323);
        assert_eq!(PROCESS_IS_RUNNING_INPUT, 0x7069_7269);
        assert_eq!(SCOPE_GLOBAL, 0x676c_6f62);
    }

    /// Runs against the real CoreAudio: the calls must succeed and answer,
    /// whatever the answer is on this machine.
    #[test]
    fn the_device_list_can_be_read() {
        assert!(object_list(SYSTEM_OBJECT, DEVICES).is_some());
        let _ = Detector::new().microphone_in_use();
    }
}
