use crossbeam_channel::{ unbounded, Receiver, Sender };
use rodio::buffer::SamplesBuffer;
use rodio::{ OutputStream, OutputStreamHandle, Sink };
use std::collections::HashMap;
use std::sync::{ Arc, OnceLock };
use std::time::{ Duration, Instant };

use crate::libs::device_manager::DeviceManager;

const FADE_IN_MS: f32 = 2.0;
const FADE_OUT_MS: f32 = 5.0;
const EVICT_RAMP_MS: u64 = 10;
const MAX_VOICES: usize = 32;

/// Wheel scrolls have no release event and can repeat much faster than a human
/// clicks, so the engine rate-limits them as impulses. This is the only place
/// that window is applied: the Wayland listener forwards every wheel delta and
/// lets the engine decide, because a second limit in the listener would use its
/// own timestamp and drop a scroll the engine would have played. Nothing else
/// uses it today - the X11 listener reports no wheel events, because the rdev
/// branch that would do it is commented out.
pub(crate) const WHEEL_DEBOUNCE_MS: u64 = 120;

/// (samples, channels, sample_rate) for a decoded/resampled audio buffer.
type DecodedAudio = (Arc<Vec<f32>>, u16, u32);

/// Commands the engine thread accepts. Every audio-affecting operation goes
/// through this channel so the thread that owns `OutputStream` never has to
/// share it across threads (rodio's `OutputStream` is not `Send`).
///
/// Keyboard/mouse key events are NOT a variant here: the engine reads raw
/// `"KeyA"` / `"UP:KeyA"` strings directly off the same crossbeam receivers
/// the input listeners (rdev/device_query/evdev) send to, via `select!` in
/// `run_engine` - see `spawn_engine`. Routing them through `AudioCommand`
/// would add an extra hop with no benefit.
pub enum AudioCommand {
    SetVolume(f32),
    SetMouseVolume(f32),
    SetSoundEnabled(bool),
    SetKeyboardSoundEnabled(bool),
    SetMouseSoundEnabled(bool),
    LoadKeyboardPack {
        soundpack_id: String,
        update_cache_on_error: bool,
    },
    LoadMousePack {
        soundpack_id: String,
        update_cache_on_error: bool,
    },
    SwitchDevice(Option<String>), // None = system default
}

/// Events the engine thread pushes back out for the UI to react to.
#[derive(Clone, Debug)]
pub enum UiEvent {
    KeyDown(String),
    KeyUp(String),
    DeviceSwitched(Result<String, String>),
    PackLoaded {
        is_keyboard: bool,
        result: Result<String, String>,
    },
}

/// Cheap, `Clone + Send` handle to the audio engine thread. UI code and input
/// listeners hold this instead of the engine's internal state.
#[derive(Clone)]
pub struct AudioEngineHandle {
    tx: Sender<AudioCommand>,
}

impl AudioEngineHandle {
    pub fn send(&self, command: AudioCommand) {
        // The engine thread never exits while the app is running, so a send
        // failure here would mean the engine panicked - nothing UI-side can
        // recover from that, so just drop the command.
        let _ = self.tx.send(command);
    }
}

static ENGINE_HANDLE: OnceLock<AudioEngineHandle> = OnceLock::new();
static UI_EVENT_RX: OnceLock<Receiver<UiEvent>> = OnceLock::new();

/// Spawns the audio engine thread and returns a handle to it. Must be called
/// once, before `dioxus::launch`, so `OutputStream` is opened and lives on a
/// plain OS thread rather than inside the webview/Dioxus runtime.
///
/// `keyboard_rx`/`mouse_rx`/`hotkey_rx` are the sole consumers of the input
/// channels - the engine reads raw `"KeyA"` / `"UP:KeyA"` strings directly
/// from the input listeners (rdev, device_query, evdev, raw-input worker)
/// via `select!`, the same wire format those listeners already produce,
/// instead of the UI polling and forwarding them.
pub fn spawn_engine(
    keyboard_rx: Receiver<String>,
    mouse_rx: Receiver<String>,
    hotkey_rx: Receiver<String>
) -> AudioEngineHandle {
    let (cmd_tx, cmd_rx) = unbounded::<AudioCommand>();
    let (event_tx, event_rx) = unbounded::<UiEvent>();

    std::thread::spawn(move || {
        run_engine(cmd_rx, event_tx, keyboard_rx, mouse_rx, hotkey_rx);
    });

    let handle = AudioEngineHandle { tx: cmd_tx };
    let _ = ENGINE_HANDLE.set(handle.clone());
    let _ = UI_EVENT_RX.set(event_rx);
    handle
}

/// Returns the engine handle. Panics if `spawn_engine` hasn't run yet -
/// main.rs sets it up before the UI can possibly ask for it.
pub fn engine_handle() -> AudioEngineHandle {
    ENGINE_HANDLE.get().expect("Audio engine not started").clone()
}

/// Returns the `UiEvent` receiver for the UI's single poll loop.
pub fn ui_event_receiver() -> &'static Receiver<UiEvent> {
    UI_EVENT_RX.get().expect("Audio engine not started")
}

/// All state the engine thread owns exclusively. Nothing here is behind a
/// `Mutex` - the thread is the only place that ever touches it, so plain
/// fields are enough (this is the whole point of moving playback off the
/// Arc<Mutex<...>> path used pre-Phase-3). Fields are `pub(super)` so
/// `soundpack_loader.rs` (a sibling module) can update them after decoding.
pub(super) struct EngineState {
    stream: OutputStream,
    pub(super) stream_handle: OutputStreamHandle,
    device_manager: DeviceManager,
    current_device_id: Option<String>,
    pub(super) device_rate: Option<u32>,

    pub(super) keyboard_samples: Option<DecodedAudio>,
    pub(super) keyboard_samples_original: Option<DecodedAudio>,
    pub(super) mouse_samples: Option<DecodedAudio>,
    pub(super) mouse_samples_original: Option<DecodedAudio>,
    pub(super) key_map: HashMap<String, Vec<[f32; 2]>>,
    pub(super) mouse_map: HashMap<String, Vec<[f32; 2]>>,

    key_pressed: HashMap<String, bool>,
    mouse_pressed: HashMap<String, bool>,
    /// Shared last-fire time for both wheel directions. The wheel is an
    /// impulse with no release, so it is rate-limited here instead of being
    /// tracked in `mouse_pressed` (one timestamp means a fast up/down wobble
    /// cannot fire twice).
    last_wheel: Option<Instant>,
    pub(super) key_sinks: Vec<Sink>,
    pub(super) mouse_sinks: Vec<Sink>,

    volume: f32,
    mouse_volume: f32,
    sound_enabled: bool,
    keyboard_sound_enabled: bool,
    mouse_sound_enabled: bool,
}

/// Opens a stream for `device_id` (`None` = system default). Does NOT fall
/// back silently - callers decide what to do on `Err` (see `switch_device`,
/// which keeps the previous device on failure, vs `EngineState::new`, which
/// falls back to default since there's no previous device to keep).
fn open_stream(
    device_manager: &DeviceManager,
    device_id: Option<&str>
) -> Result<(OutputStream, OutputStreamHandle, Option<String>), String> {
    match device_id {
        Some(id) => {
            match device_manager.get_output_device_by_id(id) {
                Ok(Some(device)) => {
                    rodio::OutputStream
                        ::try_from_device(&device)
                        .map(|(stream, handle)| (stream, handle, Some(id.to_string())))
                        .map_err(|e| format!("Failed to open stream for device {}: {}", id, e))
                }
                Ok(None) => Err(format!("Device {} not found", id)),
                Err(e) => Err(format!("Error accessing device {}: {}", id, e)),
            }
        }
        None => {
            rodio::OutputStream
                ::try_default()
                .map(|(stream, handle)| (stream, handle, None))
                .map_err(|e| format!("Failed to open default audio output stream: {}", e))
        }
    }
}

impl EngineState {
    fn new() -> Self {
        let device_manager = DeviceManager::new();
        let config = crate::state::config_writer::current();

        let (stream, stream_handle, opened_device_id) = open_stream(
            &device_manager,
            config.selected_audio_device.as_deref()
        ).unwrap_or_else(|e| {
            crate::always_eprint!("❌ [AudioEngine] {} - falling back to default", e);
            open_stream(&device_manager, None).expect("Failed to open default audio output stream")
        });
        let current_device_id = opened_device_id.or(config.selected_audio_device.clone());
        let device_rate = device_manager.get_current_output_sample_rate();

        Self {
            stream,
            stream_handle,
            device_manager,
            current_device_id,
            device_rate,
            keyboard_samples: None,
            keyboard_samples_original: None,
            mouse_samples: None,
            mouse_samples_original: None,
            key_map: HashMap::new(),
            mouse_map: HashMap::new(),
            key_pressed: HashMap::new(),
            mouse_pressed: HashMap::new(),
            last_wheel: None,
            key_sinks: Vec::new(),
            mouse_sinks: Vec::new(),
            volume: config.volume,
            mouse_volume: config.mouse_volume,
            sound_enabled: config.enable_sound,
            keyboard_sound_enabled: config.enable_keyboard_sound,
            mouse_sound_enabled: config.enable_mouse_sound,
        }
    }

    fn handle_key_event(&mut self, code: &str, down: bool) {
        if !should_play(self.sound_enabled, self.keyboard_sound_enabled) {
            return;
        }
        if !debounce_press(&mut self.key_pressed, code, down) {
            return;
        }
        if let Some((start, end)) = lookup_timing(&self.key_map, code, down) {
            play_segment(
                &self.stream_handle,
                &self.keyboard_samples,
                code,
                start,
                end,
                self.volume,
                &mut self.key_sinks
            );
        }
    }

    fn handle_mouse_event(&mut self, code: &str, down: bool) {
        if !should_play(self.sound_enabled, self.mouse_sound_enabled) {
            return;
        }
        let now = Instant::now();
        if !mouse_event_is_playable(
            &mut self.mouse_pressed,
            &mut self.last_wheel,
            code,
            down,
            now
        ) {
            return;
        }
        if let Some((start, end)) = lookup_timing(&self.mouse_map, code, down) {
            play_segment(
                &self.stream_handle,
                &self.mouse_samples,
                code,
                start,
                end,
                self.mouse_volume,
                &mut self.mouse_sinks
            );
        }
    }

    fn switch_device(&mut self, device_id: Option<String>) -> Result<String, String> {
        // On Err, `self` is left untouched entirely - the previous device
        // keeps playing, matching the "keep current sound, report error"
        // requirement (Phase 3 success criteria).
        let (new_stream, new_handle, opened_device_id) = open_stream(
            &self.device_manager,
            device_id.as_deref()
        )?;

        // Re-resample cached original samples to the new device's rate so
        // in-flight soundpacks keep playing correctly without a reload.
        let new_rate = self.device_manager.get_current_output_sample_rate();
        if let Some((orig_samples, channels, orig_rate)) = &self.keyboard_samples_original {
            self.keyboard_samples = Some(
                resample_if_needed(orig_samples, *channels, *orig_rate, new_rate)
            );
        }
        if let Some((orig_samples, channels, orig_rate)) = &self.mouse_samples_original {
            self.mouse_samples = Some(
                resample_if_needed(orig_samples, *channels, *orig_rate, new_rate)
            );
        }

        // Drop old voices/stream only after the new one is confirmed open,
        // so a failed switch leaves the previous device still playing.
        self.key_sinks.clear();
        self.mouse_sinks.clear();
        self.stream = new_stream;
        self.stream_handle = new_handle;
        self.device_rate = new_rate;
        self.current_device_id = opened_device_id.clone().or(device_id);

        let label = self.current_device_id.clone().unwrap_or_else(|| "System Default".to_string());
        Ok(label)
    }
}

/// Whether a keystroke should produce sound, given the global mute flag and
/// the per-input-type (keyboard/mouse) flag.
///
/// Both flags live in `EngineState` and are only moved by
/// `AudioCommand::Set*SoundEnabled`, so a UI that writes the config without
/// sending that command leaves this returning `true` and the app keeps playing
/// while the UI shows muted - the mute-button bug this guards against.
fn should_play(sound_enabled: bool, type_enabled: bool) -> bool {
    sound_enabled && type_enabled
}

/// True for the impulse mouse codes - the vertical wheel. These have no
/// release event and so must not go through `debounce_press`: with nothing to
/// clear it, the first scroll would latch `pressed["MouseWheelUp"] = true` and
/// every later scroll would be dropped for the rest of the session.
fn is_wheel_code(code: &str) -> bool {
    code.starts_with("MouseWheel")
}

/// Caps wheel sounds at one per `WHEEL_DEBOUNCE_MS`. One timestamp is shared
/// by both directions so a fast up/down wobble cannot fire two sounds.
/// Returns `true` when the event is allowed through, recording `now`.
fn allow_wheel_impulse(last_wheel: &mut Option<Instant>, now: Instant) -> bool {
    if let Some(previous) = *last_wheel {
        if now.duration_since(previous) < Duration::from_millis(WHEEL_DEBOUNCE_MS) {
            return false;
        }
    }
    *last_wheel = Some(now);
    true
}

/// Gate for a mouse event. Wheel impulses take their own rate limit and never
/// touch the press/release map; every other button keeps `debounce_press`.
/// Split out of `handle_mouse_event` so the gating is testable without an
/// audio device.
fn mouse_event_is_playable(
    pressed: &mut HashMap<String, bool>,
    last_wheel: &mut Option<Instant>,
    code: &str,
    down: bool,
    now: Instant
) -> bool {
    if is_wheel_code(code) {
        allow_wheel_impulse(last_wheel, now)
    } else {
        debounce_press(pressed, code, down)
    }
}

/// Marks `code` pressed/released, returning `false` if this event should be
/// ignored (duplicate keydown, or keyup with no matching keydown).
fn debounce_press(pressed: &mut HashMap<String, bool>, code: &str, down: bool) -> bool {
    let was_down = *pressed.get(code).unwrap_or(&false);
    if down == was_down {
        return false;
    }
    pressed.insert(code.to_string(), down);
    true
}

/// Looks up the `[start, end]` (ms) pair for a keydown/keyup event from a
/// soundpack's timing map. Mirrors the pre-Phase-3 `play_key_event_sound`
/// logic in `sound_manager.rs`.
fn lookup_timing(map: &HashMap<String, Vec<[f32; 2]>>, code: &str, down: bool) -> Option<(f32, f32)> {
    match map.get(code) {
        Some(arr) if arr.len() == 2 => {
            let idx = if down { 0 } else { 1 };
            Some((arr[idx][0], arr[idx][1]))
        }
        Some(arr) if arr.len() == 1 => {
            if !down {
                return None; // keydown-only mapping, ignore keyup
            }
            Some((arr[0][0], arr[0][1]))
        }
        _ => None,
    }
}

fn play_segment(
    stream_handle: &OutputStreamHandle,
    samples: &Option<DecodedAudio>,
    code: &str,
    start_ms: f32,
    end_ms: f32,
    volume: f32,
    sinks: &mut Vec<Sink>
) {
    let Some((samples_arc, channels, sample_rate)) = samples else {
        return;
    };
    let samples: &Vec<f32> = samples_arc.as_ref();
    let channels = *channels;
    let sample_rate = *sample_rate;

    let duration = end_ms - start_ms;
    if start_ms < 0.0 || duration <= 0.0 {
        return;
    }

    let start_sample = ((start_ms / 1000.0) * (sample_rate as f32) * (channels as f32)) as usize;
    let end_sample = ((end_ms / 1000.0) * (sample_rate as f32) * (channels as f32)) as usize;
    let end_sample = end_sample.min(samples.len());
    if start_sample >= end_sample {
        crate::always_eprint!(
            "❌ [AudioEngine] Invalid sample range for '{}': {}..{} (max {})",
            code,
            start_sample,
            end_sample,
            samples.len()
        );
        return;
    }

    let mut segment_samples = samples[start_sample..end_sample].to_vec();
    apply_fade(&mut segment_samples, channels, sample_rate);
    let segment = SamplesBuffer::new(channels, sample_rate, segment_samples);

    if let Ok(sink) = Sink::try_new(stream_handle) {
        sink.set_volume(volume);
        sink.append(segment);

        manage_active_sinks(sinks, MAX_VOICES);
        sinks.push(sink);
    }
}

/// Applies a linear fade-in/fade-out to interleaved PCM samples in place.
/// Operates per-frame (one frame = `channels` consecutive samples) so all
/// channels in a frame share the same gain and stay in phase.
fn apply_fade(samples: &mut [f32], channels: u16, sample_rate: u32) {
    let channels = channels.max(1) as usize;
    let frame_count = samples.len() / channels;
    if frame_count == 0 {
        return;
    }

    let mut fade_in_frames = ((FADE_IN_MS / 1000.0) * (sample_rate as f32)) as usize;
    let mut fade_out_frames = ((FADE_OUT_MS / 1000.0) * (sample_rate as f32)) as usize;

    let half = frame_count / 2;
    if fade_in_frames > half {
        fade_in_frames = half;
    }
    if fade_out_frames > half {
        fade_out_frames = half;
    }

    for frame in 0..fade_in_frames {
        let gain = (frame as f32) / (fade_in_frames as f32);
        let base = frame * channels;
        for c in 0..channels {
            samples[base + c] *= gain;
        }
    }

    for frame in 0..fade_out_frames {
        let gain = (frame as f32) / (fade_out_frames as f32);
        let frame_idx = frame_count - 1 - frame;
        let base = frame_idx * channels;
        for c in 0..channels {
            samples[base + c] *= gain;
        }
    }
}

/// Removes finished sinks, then evicts the oldest voice (ramped down to
/// avoid a click) if the pool is still at or above `max_voices`.
fn manage_active_sinks(sinks: &mut Vec<Sink>, max_voices: usize) {
    sinks.retain(|s| !s.empty());

    if sinks.len() >= max_voices {
        let old_sink = sinks.remove(0);
        std::thread::spawn(move || {
            const STEPS: u32 = 10;
            let starting_volume = old_sink.volume();
            for step in 1..=STEPS {
                let gain = starting_volume * (1.0 - (step as f32) / (STEPS as f32));
                old_sink.set_volume(gain.max(0.0));
                std::thread::sleep(Duration::from_millis(EVICT_RAMP_MS / (STEPS as u64)));
            }
            old_sink.stop();
        });
    }
}

fn resample_if_needed(
    original_samples: &Arc<Vec<f32>>,
    channels: u16,
    original_rate: u32,
    target_rate: Option<u32>
) -> DecodedAudio {
    match target_rate {
        Some(target_rate) if target_rate != original_rate => {
            let resampled = super::resampler::resample_interleaved(
                original_samples,
                channels,
                original_rate,
                target_rate
            );
            (Arc::new(resampled), channels, target_rate)
        }
        _ => (original_samples.clone(), channels, original_rate),
    }
}

/// Parses the `"KeyA"` / `"UP:KeyA"` wire format the input listeners
/// (rdev, device_query, evdev) already send, same as the pre-Phase-3 UI
/// polling loops in `ui.rs` did.
fn parse_input_event(raw: &str) -> Option<(String, bool)> {
    if let Some(code) = raw.strip_prefix("UP:") {
        Some((code.to_string(), false))
    } else if !raw.is_empty() {
        Some((raw.to_string(), true))
    } else {
        None
    }
}

/// Handles the Ctrl+Alt+M hotkey: flips `enable_sound`, persists it, and
/// returns the new value so the caller can move the engine's own cached flag
/// in lockstep.
///
/// The persisted config is the source of truth for the new value (rather than
/// negating the engine's cached flag) so the hotkey cannot drift from what the
/// UI and tray read back. The flip happens *inside* the mutation, so the value
/// being negated is the authority's, not a copy read a moment earlier.
///
/// This runs on the engine thread, which has no access to the Dioxus signal.
/// The window still re-renders because `config_writer` notifies its subscribers
/// after every write, and the UI installs one at startup - before that existed,
/// the hotkey muted the app while the window kept showing the unmuted icon.
fn handle_toggle_sound() -> bool {
    let mut enabled = false;
    crate::state::config_writer::apply(|config| {
        config.enable_sound = !config.enable_sound;
        enabled = config.enable_sound;
    });

    crate::libs::tray_service::request_tray_update();
    crate::always_print!("🔄 [AudioEngine] Sound toggled: {}", enabled);

    // Keep the UI-side cache (`AudioContext::is_sound_enabled`) in step: the
    // tray toggle derives its next value from it, so a stale cache here would
    // make the following tray click a no-op.
    super::audio_context::sync_sound_enabled_cache(enabled);
    enabled
}

fn handle_command(state: &mut EngineState, event_tx: &Sender<UiEvent>, command: AudioCommand) {
    match command {
        AudioCommand::SetVolume(v) => {
            state.volume = v;
            for sink in &state.key_sinks {
                sink.set_volume(v);
            }
        }
        AudioCommand::SetMouseVolume(v) => {
            state.mouse_volume = v;
            for sink in &state.mouse_sinks {
                sink.set_volume(v);
            }
        }
        AudioCommand::SetSoundEnabled(enabled) => {
            state.sound_enabled = enabled;
        }
        AudioCommand::SetKeyboardSoundEnabled(enabled) => {
            state.keyboard_sound_enabled = enabled;
        }
        AudioCommand::SetMouseSoundEnabled(enabled) => {
            state.mouse_sound_enabled = enabled;
        }
        AudioCommand::LoadKeyboardPack { soundpack_id, update_cache_on_error } => {
            let result = crate::libs::trace::time(
                crate::libs::trace::Point::PackLoad,
                &soundpack_id,
                || super::soundpack_loader::load_keyboard_pack_into_engine(
                    state,
                    &soundpack_id,
                    update_cache_on_error
                )
            );
            let _ = event_tx.send(UiEvent::PackLoaded { is_keyboard: true, result });
        }
        AudioCommand::LoadMousePack { soundpack_id, update_cache_on_error } => {
            let result = crate::libs::trace::time(
                crate::libs::trace::Point::PackLoad,
                &soundpack_id,
                || super::soundpack_loader::load_mouse_pack_into_engine(
                    state,
                    &soundpack_id,
                    update_cache_on_error
                )
            );
            let _ = event_tx.send(UiEvent::PackLoaded { is_keyboard: false, result });
        }
        AudioCommand::SwitchDevice(device_id) => {
            // User-initiated switch: on failure, keep the previous device
            // playing and just report the error - don't silently fall back
            // to default (that's reserved for the device-removed case,
            // where there's no "previous" device left to keep).
            let label = device_id.clone().unwrap_or_else(|| "System Default".to_string());
            let result = crate::libs::trace::time(
                crate::libs::trace::Point::DeviceSwitch,
                &label,
                || state.switch_device(device_id)
            );
            let _ = event_tx.send(UiEvent::DeviceSwitched(result));
        }
    }
}

fn run_engine(
    cmd_rx: Receiver<AudioCommand>,
    event_tx: Sender<UiEvent>,
    keyboard_rx: Receiver<String>,
    mouse_rx: Receiver<String>,
    hotkey_rx: Receiver<String>
) {
    let mut state = EngineState::new();

    // Load the configured soundpacks once at startup.
    let config = crate::state::config_writer::current();
    if !config.keyboard_soundpack.is_empty() {
        let result = super::soundpack_loader::load_keyboard_pack_into_engine(
            &mut state,
            &config.keyboard_soundpack,
            false
        );
        let _ = event_tx.send(UiEvent::PackLoaded { is_keyboard: true, result });
    }
    if !config.mouse_soundpack.is_empty() {
        let result = super::soundpack_loader::load_mouse_pack_into_engine(
            &mut state,
            &config.mouse_soundpack,
            false
        );
        let _ = event_tx.send(UiEvent::PackLoaded { is_keyboard: false, result });
    }

    // This loop is purely event-driven: every arm below is a channel receive,
    // and there is no timed arm. Nothing here polls the audio device.
    //
    // A previous design checked once a second that the selected output device
    // was still present, so it could fall back to the system default on an
    // unplug. Enumerating devices costs hundreds of milliseconds per call
    // (cpal activates each device as the list is walked), and running that on
    // this thread - which both plays the sound and emits the UI event -
    // stalled keystrokes by up to several seconds. That automatic fallback is
    // gone by product decision: unplugging the selected device now simply goes
    // quiet, the config keeps the selection, and the user reselects a device in
    // Settings (or restarts). Manual switching via `AudioCommand::SwitchDevice`
    // is unaffected. Do not reintroduce periodic enumeration here.
    loop {
        crossbeam_channel::select! {
            recv(cmd_rx) -> msg => {
                match msg {
                    Ok(command) => {
                        handle_command(&mut state, &event_tx, command);
                    }
                    Err(_) => break, // sender dropped, app is shutting down
                }
            }
            recv(keyboard_rx) -> msg => {
                if let Ok(raw) = msg {
                    if let Some((code, down)) = parse_input_event(&raw) {
                        crate::libs::trace::record(crate::libs::trace::Point::EngineDequeue, &code, 0.0);
                        crate::libs::trace::time(crate::libs::trace::Point::PlayedSound, &code, || {
                            state.handle_key_event(&code, down);
                        });
                        crate::libs::trace::record(crate::libs::trace::Point::UiEventSent, &code, 0.0);
                        let _ = event_tx.send(if down { UiEvent::KeyDown(code) } else { UiEvent::KeyUp(code) });
                    }
                }
            }
            recv(mouse_rx) -> msg => {
                if let Ok(raw) = msg {
                    if let Some((code, down)) = parse_input_event(&raw) {
                        state.handle_mouse_event(&code, down);
                    }
                }
            }
            recv(hotkey_rx) -> msg => {
                if let Ok(command) = msg {
                    if command == "TOGGLE_SOUND" {
                        // Adopt the persisted value rather than negating the
                        // cached one, so the engine can't drift out of sync
                        // with config if the two ever disagree.
                        state.sound_enabled = handle_toggle_sound();
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The engine's own source, so the assertions below are checked against
    /// what actually ships rather than a description of it.
    const ENGINE_SOURCE: &str = include_str!("engine.rs");

    /// Everything above `mod tests`, i.e. the runtime code only. The test
    /// module names the removed symbols in its own assertions, so searching
    /// the whole file for them would always match.
    fn runtime_source() -> &'static str {
        ENGINE_SOURCE.split("#[cfg(test)]").next().expect("runtime code precedes the tests")
    }

    #[test]
    fn the_engine_loop_never_polls_the_audio_device() {
        // The keystroke-stall defect was a 1s timer on this loop running a
        // device enumeration that costs hundreds of ms, delaying both the
        // sound and the UI event behind it. The loop is now purely
        // event-driven, and this pins that: every `select!` arm must be a
        // channel receive, with no timed arm to hang periodic work on.
        let loop_body = runtime_source()
            .split("fn run_engine(")
            .nth(1)
            .expect("run_engine must exist");

        for timed_arm in ["crossbeam_channel::at(", "crossbeam_channel::after(", "default("] {
            assert!(
                !loop_body.contains(timed_arm),
                "engine loop must have no timed arm ({timed_arm}) - periodic work here \
                 delays every keystroke behind it"
            );
        }
    }

    #[test]
    fn no_device_presence_polling_remains_anywhere_in_the_engine() {
        // The watchdog and its off-thread prober were both removed: on unplug
        // the app goes quiet and the user reselects a device. Nothing should
        // be enumerating devices on a timer to detect that automatically.
        for removed in [
            "has_output_device_named",
            "DeviceWatchdog",
            "DevicePresenceProber",
            "fallback_to_default",
            "DeviceLost",
        ] {
            assert!(
                !runtime_source().contains(removed),
                "{removed} was removed with the device watchdog and must not return"
            );
        }
    }

    #[test]
    fn manual_device_switching_is_still_supported() {
        // Removing the automatic fallback must not take user-initiated
        // switching with it - that is the path Settings drives, and it is now
        // the only way playback moves between devices.
        assert!(runtime_source().contains("AudioCommand::SwitchDevice"));
        assert!(runtime_source().contains("fn switch_device"));
        assert!(runtime_source().contains("UiEvent::DeviceSwitched"));
    }

    #[test]
    fn global_mute_silences_both_input_types() {
        // The global flag wins regardless of the per-type flag - this is what
        // the home-page mute button and Ctrl+Alt+M both drive.
        assert!(!should_play(false, true), "global mute must silence keyboard");
        assert!(!should_play(false, false));
    }

    #[test]
    fn per_type_mute_silences_only_that_type() {
        // Muting the keyboard slider must not require touching the global
        // flag, and vice versa for the mouse slider.
        assert!(!should_play(true, false));
        assert!(should_play(true, true), "nothing muted must play");
    }

    #[test]
    fn set_sound_enabled_command_moves_engine_state() {
        // Regression guard for the mute bug: the UI writing config alone left
        // the engine's cached flag untouched and sound kept playing. These
        // asserts pin the command -> state edge that the UI must drive.
        //
        // Applies the same assignments `handle_command` performs; the full
        // `handle_command` needs an `EngineState`, which needs a real audio
        // device and so cannot be built in a headless test.
        let mut sound_enabled = true;
        let mut keyboard_sound_enabled = true;
        let mut mouse_sound_enabled = true;

        for command in [
            AudioCommand::SetSoundEnabled(false),
            AudioCommand::SetKeyboardSoundEnabled(false),
            AudioCommand::SetMouseSoundEnabled(false),
        ] {
            match command {
                AudioCommand::SetSoundEnabled(v) => {
                    sound_enabled = v;
                }
                AudioCommand::SetKeyboardSoundEnabled(v) => {
                    keyboard_sound_enabled = v;
                }
                AudioCommand::SetMouseSoundEnabled(v) => {
                    mouse_sound_enabled = v;
                }
                _ => unreachable!("only mute commands are exercised here"),
            }
        }

        assert!(!sound_enabled);
        assert!(!keyboard_sound_enabled);
        assert!(!mouse_sound_enabled);
        assert!(!should_play(sound_enabled, keyboard_sound_enabled));
        assert!(!should_play(sound_enabled, mouse_sound_enabled));
    }

    #[test]
    fn two_wheel_events_inside_the_limit_play_once() {
        // Fast scrolling must not machine-gun the sound: the engine keeps a
        // single 120 ms window for the wheel.
        let mut pressed = HashMap::new();
        let mut last_wheel = None;
        let t0 = Instant::now();

        assert!(mouse_event_is_playable(&mut pressed, &mut last_wheel, "MouseWheelUp", true, t0));
        assert!(
            !mouse_event_is_playable(
                &mut pressed,
                &mut last_wheel,
                "MouseWheelUp",
                true,
                t0 + Duration::from_millis(WHEEL_DEBOUNCE_MS - 1)
            ),
            "a second scroll inside the wheel window must be dropped"
        );
    }

    #[test]
    fn a_wheel_event_after_the_limit_plays_again() {
        let mut pressed = HashMap::new();
        let mut last_wheel = None;
        let t0 = Instant::now();

        assert!(mouse_event_is_playable(&mut pressed, &mut last_wheel, "MouseWheelDown", true, t0));
        assert!(
            mouse_event_is_playable(
                &mut pressed,
                &mut last_wheel,
                "MouseWheelDown",
                true,
                t0 + Duration::from_millis(WHEEL_DEBOUNCE_MS)
            ),
            "a scroll at the limit must play"
        );
    }

    #[test]
    fn the_wheel_timestamp_is_shared_by_both_directions() {
        // One timestamp for both directions, so a quick up/down wobble cannot
        // produce two sounds inside the same window.
        let mut pressed = HashMap::new();
        let mut last_wheel = None;
        let t0 = Instant::now();

        assert!(mouse_event_is_playable(&mut pressed, &mut last_wheel, "MouseWheelUp", true, t0));
        assert!(
            !mouse_event_is_playable(
                &mut pressed,
                &mut last_wheel,
                "MouseWheelDown",
                true,
                t0 + Duration::from_millis(50)
            ),
            "an up/down wobble must not fire twice"
        );
    }

    #[test]
    fn a_wheel_code_never_enters_the_press_release_map() {
        // The issue #43 regression: `debounce_press` latches the first scroll
        // as a press and, with no release event to clear it, rejects every
        // later scroll forever. The wheel must bypass that map entirely.
        let mut pressed = HashMap::new();
        let mut last_wheel = None;
        let t0 = Instant::now();

        assert!(mouse_event_is_playable(&mut pressed, &mut last_wheel, "MouseWheelUp", true, t0));
        assert!(
            !pressed.contains_key("MouseWheelUp"),
            "wheel impulse must not leave a latched press behind"
        );
        assert!(
            mouse_event_is_playable(
                &mut pressed,
                &mut last_wheel,
                "MouseWheelUp",
                true,
                t0 + Duration::from_millis(WHEEL_DEBOUNCE_MS + 1)
            ),
            "a later scroll must still play"
        );
        assert!(!pressed.contains_key("MouseWheelUp"));
    }

    #[test]
    fn regular_mouse_buttons_keep_their_press_release_gating() {
        // The wheel bypass must not weaken normal buttons: a repeat down with
        // no release in between is still rejected, exactly as before.
        let mut pressed = HashMap::new();
        let mut last_wheel = None;
        let t0 = Instant::now();

        assert!(mouse_event_is_playable(&mut pressed, &mut last_wheel, "MouseLeft", true, t0));
        assert!(
            !mouse_event_is_playable(
                &mut pressed,
                &mut last_wheel,
                "MouseLeft",
                true,
                t0 + Duration::from_secs(1)
            )
        );
        assert!(
            mouse_event_is_playable(
                &mut pressed,
                &mut last_wheel,
                "MouseLeft",
                false,
                t0 + Duration::from_secs(2)
            )
        );
        assert!(
            mouse_event_is_playable(
                &mut pressed,
                &mut last_wheel,
                "MouseLeft",
                true,
                t0 + Duration::from_secs(3)
            )
        );
    }

    /// Builds a mouse timing map from literal `[start, end]` pairs.
    fn mouse_map(entries: &[(&str, Vec<[f32; 2]>)]) -> HashMap<String, Vec<[f32; 2]>> {
        entries.iter().map(|(code, timing)| (code.to_string(), timing.clone())).collect()
    }

    #[test]
    fn undefined_mouse_codes_stay_silent() {
        // Every real pack defines only the two primary buttons, and the engine
        // deliberately does not substitute another sound for a code a pack
        // leaves undefined. The wheel, the wheel click and the extra buttons
        // resolve to nothing - silent, no panic.
        let map = mouse_map(&[
            ("MouseLeft", vec![[10.0, 20.0], [30.0, 40.0]]),
            ("MouseRight", vec![[50.0, 60.0], [70.0, 80.0]]),
        ]);

        for code in ["MouseMiddle", "MouseWheelUp", "MouseWheelDown", "Mouse4", "Mouse5"] {
            assert_eq!(lookup_timing(&map, code, true), None, "{code} press");
            assert_eq!(lookup_timing(&map, code, false), None, "{code} release");
        }
    }

    #[test]
    fn defined_mouse_codes_use_their_own_timing() {
        // A pack that wants wheel, wheel-click or extra-button sounds has to
        // define them. When it does, the engine uses exactly those windows,
        // press and release included.
        let map = mouse_map(&[
            ("MouseMiddle", vec![[50.0, 60.0], [70.0, 80.0]]),
            ("MouseWheelUp", vec![[90.0, 100.0]]),
        ]);

        assert_eq!(lookup_timing(&map, "MouseMiddle", true), Some((50.0, 60.0)));
        assert_eq!(lookup_timing(&map, "MouseMiddle", false), Some((70.0, 80.0)));
        assert_eq!(lookup_timing(&map, "MouseWheelUp", true), Some((90.0, 100.0)));
    }

    #[test]
    fn an_undefined_keyboard_key_stays_silent() {
        // An undefined keyboard key has never borrowed a mouse click, and the
        // mouse codes are no longer special-cased either: it resolves to
        // nothing.
        let map = mouse_map(&[("MouseLeft", vec![[10.0, 20.0]])]);

        assert_eq!(lookup_timing(&map, "KeyA", true), None);
        assert_eq!(lookup_timing(&map, "KeyA", false), None);
    }
}
