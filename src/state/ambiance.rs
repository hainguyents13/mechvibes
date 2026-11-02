use serde::{ Deserialize, Serialize };
use std::collections::HashMap;
use std::sync::{ Arc, Mutex };
use std::sync::mpsc;
use std::thread;
use rodio::{ Decoder, OutputStream, OutputStreamHandle, Sink, Source };
use std::fs::File;
use std::io::BufReader;

use crate::libs::device_manager::DeviceManager;

/// Commands for the single background thread that owns the shared ambiance
/// `OutputStream`. All sounds share one stream/handle (rather than each
/// `play_ambiance_sound` call opening its own, as before Phase 3) so a
/// device switch has exactly one stream to rebuild.
enum AmbianceEngineCommand {
    Play {
        sound_id: String,
        audio_url: String,
        volume: f32,
    },
    Stop(String),
    SetVolume {
        sound_id: String,
        volume: f32,
    },
    /// Effective volume per sound, sent when the master slider moves. Each
    /// sound keeps its own level relative to the others, so this carries one
    /// value per sink rather than a single value for all of them.
    SetVolumes(Vec<(String, f32)>),
    PauseAll,
    ResumeAll,
    /// `resume` carries the sounds that were actually audible before the
    /// switch (empty if paused/muted) - see `switch_ambiance_player_device`,
    /// which reads `AmbiancePlayerState` to build this since the engine
    /// thread only knows about `Sink`s, not playback intent.
    SwitchDevice {
        device_id: Option<String>,
        resume: Vec<(String, String, f32)>, // (sound_id, audio_url, effective_volume)
    },
}

static AMBIANCE_ENGINE_TX: std::sync::OnceLock<
    Mutex<Option<mpsc::Sender<AmbianceEngineCommand>>>
> = std::sync::OnceLock::new();

// Global ambiance player state
static GLOBAL_AMBIANCE_PLAYER_STATE: std::sync::OnceLock<Arc<Mutex<AmbiancePlayerState>>> = std::sync::OnceLock::new();

fn send_engine_command(command: AmbianceEngineCommand) {
    if let Some(tx_slot) = AMBIANCE_ENGINE_TX.get() {
        if let Some(tx) = tx_slot.lock().unwrap().as_ref() {
            let _ = tx.send(command);
        }
    }
}

/// Switches the ambiance player's shared output device at runtime (`None` =
/// system default). Sounds that were actually audible (playing, not paused,
/// not muted) before the switch are re-started on the new stream - existing
/// `Sink`s are bound to the old `OutputStreamHandle` and can't just move
/// over. No-ops if the engine hasn't started yet.
pub fn switch_ambiance_player_device(device_id: Option<String>) {
    let resume = get_global_ambiance_player_state_copy()
        .map(|state| {
            if !state.is_playing || state.is_muted {
                return Vec::new();
            }
            state
                .effective_volumes()
                .into_iter()
                .filter_map(|(sound_id, volume)| {
                    let audio_url = state.get_sound_by_id(&sound_id)?.audio_url.clone();
                    Some((sound_id, audio_url, volume))
                })
                .collect()
        })
        .unwrap_or_default();

    send_engine_command(AmbianceEngineCommand::SwitchDevice { device_id, resume });
}

/// Opens, decodes, and starts a looping `Sink` for `audio_url` on
/// `stream_handle`. Shared by `Play` and the `SwitchDevice` resume step so
/// both build sinks the same way.
fn build_ambiance_sink(
    stream_handle: &OutputStreamHandle,
    audio_url: &str,
    volume: f32
) -> Result<Sink, String> {
    // `audio_url` is the web-style "assets/sounds/x.mp3" string the sound list
    // stores; the resolver turns it into the real on-disk location, which is
    // beside the executable on Windows/macOS but under /usr/lib on a Linux
    // install where the process cwd is wherever the user launched from.
    let full_path = crate::state::paths::bundled_assets::asset_path(audio_url);
    let file = File::open(&full_path).map_err(|e|
        format!("Failed to open audio file {}: {}", full_path.display(), e)
    )?;
    let decoder = Decoder::new(BufReader::new(file)).map_err(|e|
        format!("Failed to decode audio: {}", e)
    )?;
    let sink = Sink::try_new(stream_handle).map_err(|e|
        format!("Failed to create audio sink: {}", e)
    )?;
    sink.set_volume(volume.clamp(0.0, 1.0));
    sink.append(decoder.repeat_infinite());
    Ok(sink)
}

fn open_stream(device_id: Option<&str>) -> Result<(OutputStream, OutputStreamHandle), String> {
    if let Some(id) = device_id {
        let device_manager = DeviceManager::new();
        if let Ok(Some(device)) = device_manager.get_output_device_by_id(id) {
            if let Ok(pair) = rodio::OutputStream::try_from_device(&device) {
                return Ok(pair);
            }
        }
        crate::always_eprint!("❌ [Ambiance] Failed to open device {}, falling back to default", id);
    }
    rodio::OutputStream
        ::try_default()
        .map_err(|e| format!("Failed to create audio output stream: {}", e))
}

// Initialize global ambiance player
pub fn initialize_global_ambiance_player() {
    let (tx, rx) = mpsc::channel::<AmbianceEngineCommand>();
    AMBIANCE_ENGINE_TX.get_or_init(|| Mutex::new(Some(tx)));

    // Initialize global state
    let _state_ref = GLOBAL_AMBIANCE_PLAYER_STATE.get_or_init(|| {
        Arc::new(Mutex::new(AmbiancePlayerState::initialize()))
    });

    // One thread owns the shared OutputStream for its whole lifetime -
    // OutputStream (via cpal::Stream) is not Send, so it must be created on
    // and never leave this thread; SwitchDevice rebuilds it in place here.
    thread::spawn(move || {
        let initial_device = crate::state::config_writer::current().selected_audio_device;
        let (mut stream, mut stream_handle) = match open_stream(initial_device.as_deref()) {
            Ok(pair) => pair,
            Err(e) => {
                crate::always_eprint!("❌ [Ambiance] Failed to open audio stream: {}", e);
                return;
            }
        };
        let mut sinks: HashMap<String, Sink> = HashMap::new();

        while let Ok(command) = rx.recv() {
            match command {
                AmbianceEngineCommand::Play { sound_id, audio_url, volume } => {
                    if let Some(old) = sinks.remove(&sound_id) {
                        old.stop();
                    }

                    match build_ambiance_sink(&stream_handle, &audio_url, volume) {
                        Ok(sink) => {
                            sinks.insert(sound_id.clone(), sink);
                            crate::always_print!("🎵 [Ambiance] Started playing sound: {}", sound_id);
                        }
                        Err(e) => crate::always_eprint!("❌ [Ambiance] Failed to play sound {}: {}", sound_id, e),
                    }
                }
                AmbianceEngineCommand::Stop(sound_id) => {
                    if let Some(sink) = sinks.remove(&sound_id) {
                        sink.stop();
                        crate::always_print!("🔇 [Ambiance] Stopped sound: {}", sound_id);
                    }
                }
                AmbianceEngineCommand::SetVolume { sound_id, volume } => {
                    if let Some(sink) = sinks.get(&sound_id) {
                        sink.set_volume(volume.clamp(0.0, 1.0));
                    }
                }
                AmbianceEngineCommand::SetVolumes(volumes) => {
                    for (sound_id, volume) in volumes {
                        if let Some(sink) = sinks.get(&sound_id) {
                            sink.set_volume(volume.clamp(0.0, 1.0));
                        }
                    }
                }
                AmbianceEngineCommand::PauseAll => {
                    for sink in sinks.values() {
                        sink.pause();
                    }
                }
                AmbianceEngineCommand::ResumeAll => {
                    for sink in sinks.values() {
                        sink.play();
                    }
                }
                AmbianceEngineCommand::SwitchDevice { device_id, resume } => {
                    match open_stream(device_id.as_deref()) {
                        Ok((new_stream, new_handle)) => {
                            // Sinks are bound to the stream_handle they were
                            // created from and can't move to the new stream,
                            // so drop them and rebuild fresh ones on the new
                            // handle for whatever was actually audible.
                            for sink in sinks.values() {
                                sink.stop();
                            }
                            sinks.clear();
                            stream = new_stream;
                            stream_handle = new_handle;

                            for (sound_id, audio_url, volume) in resume {
                                match build_ambiance_sink(&stream_handle, &audio_url, volume) {
                                    Ok(sink) => {
                                        sinks.insert(sound_id, sink);
                                    }
                                    Err(e) =>
                                        crate::always_eprint!(
                                            "❌ [Ambiance] Failed to resume sound {} after device switch: {}",
                                            sound_id,
                                            e
                                        ),
                                }
                            }
                            crate::always_print!("🎵 [Ambiance] Switched output device");
                        }
                        Err(e) => crate::always_eprint!("❌ [Ambiance] Failed to switch device: {}", e),
                    }
                }
            }
        }

        // Keep `stream` alive for the thread's lifetime.
        let _ = &stream;
    });

    crate::always_print!("🎵 Global ambiance player initialized");
}

// Initialize global ambiance player state (call from main component)
pub fn initialize_global_ambiance_player_state() {
    let state_ref = GLOBAL_AMBIANCE_PLAYER_STATE.get_or_init(|| {
        Arc::new(Mutex::new(AmbiancePlayerState::initialize()))
    });

    // Force initialization
    let _state_lock = state_ref.lock().unwrap();
}

// Update global ambiance player state
pub fn update_global_ambiance_player_state<F>(f: F) where F: FnOnce(&mut AmbiancePlayerState) {
    if let Some(state_ref) = GLOBAL_AMBIANCE_PLAYER_STATE.get() {
        if let Ok(mut state_lock) = state_ref.lock() {
            f(&mut *state_lock);
        }
    }
}

// Get a copy of global ambiance player state
pub fn get_global_ambiance_player_state_copy() -> Option<AmbiancePlayerState> {
    GLOBAL_AMBIANCE_PLAYER_STATE.get().and_then(|state_ref| {
        state_ref
            .lock()
            .ok()
            .map(|state| state.clone())
    })
}

// Play a sound
pub fn play_ambiance_sound(sound_id: String, audio_url: String, volume: f32) -> Result<(), String> {
    send_engine_command(AmbianceEngineCommand::Play { sound_id, audio_url, volume });
    Ok(())
}

// Stop a sound
pub fn stop_ambiance_sound(sound_id: &str) -> Result<(), String> {
    send_engine_command(AmbianceEngineCommand::Stop(sound_id.to_string()));
    Ok(())
}

// Pause all sounds
pub fn pause_all_ambiance_sounds() -> Result<(), String> {
    send_engine_command(AmbianceEngineCommand::PauseAll);
    crate::always_print!("⏸️ Paused all ambiance sounds");
    Ok(())
}

// Resume all sounds
pub fn resume_all_ambiance_sounds() -> Result<(), String> {
    send_engine_command(AmbianceEngineCommand::ResumeAll);
    crate::always_print!("▶️ Resumed all ambiance sounds");
    Ok(())
}

// Set sound volume
pub fn set_ambiance_sound_volume(sound_id: &str, volume: f32) -> Result<(), String> {
    send_engine_command(AmbianceEngineCommand::SetVolume {
        sound_id: sound_id.to_string(),
        volume,
    });
    Ok(())
}

// Set the effective volume of several sounds at once
fn set_ambiance_sound_volumes(volumes: Vec<(String, f32)>) {
    send_engine_command(AmbianceEngineCommand::SetVolumes(volumes));
}

/// What a sound actually plays at: its own slider scaled by the master
/// slider, so the master changes the overall level without flattening the mix.
fn effective_volume(sound_volume: f32, global_volume: f32) -> f32 {
    (sound_volume * global_volume).clamp(0.0, 1.0)
}

// Set global mute for all sounds
pub fn set_global_ambiance_mute(muted: bool) -> Result<(), String> {
    if muted {
        send_engine_command(AmbianceEngineCommand::PauseAll);
    } else {
        send_engine_command(AmbianceEngineCommand::ResumeAll);
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AmbianceSound {
    pub id: String,
    pub name: String,
    pub description: String,
    pub audio_url: String,
    pub icon: String, // Icon name for UI display
}

#[derive(Debug, Clone)]
pub struct AmbiancePlayerState {
    pub sounds: Vec<AmbianceSound>,
    pub active_sounds: HashMap<String, f32>, // sound_id -> individual volume (0.0 to 1.0)
    pub global_volume: f32, // 0.0 to 1.0 - global multiplier for all sounds
    pub is_muted: bool,
    pub is_playing: bool, // Global play/pause state for all ambiance sounds
}

impl Default for AmbiancePlayerState {
    fn default() -> Self {
        Self {
            sounds: Self::get_builtin_sounds(),
            active_sounds: HashMap::new(),
            global_volume: 0.5,
            is_muted: false,
            is_playing: false, // Default to paused, user needs to click play to start
        }
    }
}

impl AmbiancePlayerState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Initialize from config
    pub fn initialize() -> Self {
        let config = crate::state::config_writer::current();

        Self {
            sounds: Self::get_builtin_sounds(),
            active_sounds: config.ambiance_active_sounds.clone(),
            global_volume: config.ambiance_global_volume,
            is_muted: config.ambiance_is_muted,
            is_playing: false, // Always start paused, don't persist play state
        }
    }

    /// Save current state to config.
    ///
    /// Returns nothing: the write goes through `config_writer`, which reports a
    /// failure to persist on stderr itself. Every caller already discarded the
    /// old `Result`, so surfacing one here only invited `let _ =`.
    pub fn save_config(&self) {
        let active_sounds = self.active_sounds.clone();
        let global_volume = self.global_volume;
        let is_muted = self.is_muted;
        crate::state::config_writer::apply(move |config| {
            config.ambiance_active_sounds = active_sounds;
            config.ambiance_global_volume = global_volume;
            config.ambiance_is_muted = is_muted;
            // Don't save is_playing - always start paused
        });
    }

    /// Get built-in ambiance sounds (using local assets)
    pub fn get_builtin_sounds() -> Vec<AmbianceSound> {
        vec![
            AmbianceSound {
                id: "rain".to_string(),
                name: "Rain".to_string(),
                description: "Gentle rainfall".to_string(),
                audio_url: "assets/sounds/rain.mp3".to_string(),
                icon: "cloud-rain".to_string(),
            },
            AmbianceSound {
                id: "forest".to_string(),
                name: "Forest".to_string(),
                description: "Birds chirping in the forest".to_string(),
                audio_url: "assets/sounds/forest.mp3".to_string(),
                icon: "tree-pine".to_string(),
            },
            AmbianceSound {
                id: "ocean".to_string(),
                name: "Ocean Waves".to_string(),
                description: "Calming ocean waves".to_string(),
                audio_url: "assets/sounds/ocean.mp3".to_string(),
                icon: "waves".to_string(),
            },
            AmbianceSound {
                id: "thunderstorm".to_string(),
                name: "Thunderstorm".to_string(),
                description: "Thunder and heavy rain".to_string(),
                audio_url: "assets/sounds/thunderstorm.mp3".to_string(),
                icon: "zap".to_string(),
            },
            AmbianceSound {
                id: "fire".to_string(),
                name: "Campfire".to_string(),
                description: "Crackling campfire".to_string(),
                audio_url: "assets/sounds/fire.mp3".to_string(),
                icon: "flame".to_string(),
            },
            AmbianceSound {
                id: "wind".to_string(),
                name: "Wind".to_string(),
                description: "Gentle wind through trees".to_string(),
                audio_url: "assets/sounds/wind.mp3".to_string(),
                icon: "wind".to_string(),
            },
            AmbianceSound {
                id: "stream".to_string(),
                name: "Stream".to_string(),
                description: "Babbling brook".to_string(),
                audio_url: "assets/sounds/stream.mp3".to_string(),
                icon: "waves".to_string(),
            },
            AmbianceSound {
                id: "river".to_string(),
                name: "River".to_string(),
                description: "Flowing river water".to_string(),
                audio_url: "assets/sounds/river.mp3".to_string(),
                icon: "waves".to_string(),
            },
            AmbianceSound {
                id: "cricket".to_string(),
                name: "Night Crickets".to_string(),
                description: "Crickets and night sounds".to_string(),
                audio_url: "assets/sounds/cricket.mp3".to_string(),
                icon: "moon".to_string(),
            },
            AmbianceSound {
                id: "chatter".to_string(),
                name: "Coffee Shop".to_string(),
                description: "Ambient coffee shop chatter".to_string(),
                audio_url: "assets/sounds/chatter.mp3".to_string(),
                icon: "coffee".to_string(),
            }
        ]
    }

    /// Get sound by ID
    pub fn get_sound_by_id(&self, sound_id: &str) -> Option<&AmbianceSound> {
        self.sounds.iter().find(|sound| sound.id == sound_id)
    }

    /// Check if a sound is currently active (playing)
    pub fn is_sound_active(&self, sound_id: &str) -> bool {
        self.active_sounds.contains_key(sound_id)
    }

    /// Get volume for a specific sound
    pub fn get_sound_volume(&self, sound_id: &str) -> f32 {
        self.active_sounds.get(sound_id).copied().unwrap_or(0.5)
    }

    /// Toggle a sound on/off
    pub fn toggle_sound(&mut self, sound_id: String) {
        if self.active_sounds.contains_key(&sound_id) {
            // Stop the sound
            self.active_sounds.remove(&sound_id);
            let _ = stop_ambiance_sound(&sound_id);
        } else {
            // Start the sound
            self.active_sounds.insert(sound_id.clone(), 0.5); // Default volume 50%
            if self.is_playing && !self.is_muted {
                if let Some(sound) = self.get_sound_by_id(&sound_id) {
                    let effective_volume = 0.5 * self.global_volume;
                    let _ = play_ambiance_sound(
                        sound_id.clone(),
                        sound.audio_url.clone(),
                        effective_volume
                    );
                }
            }
        }
        self.save_config();
    }

    /// The effective volume of every active sound, see `effective_volume`.
    pub fn effective_volumes(&self) -> Vec<(String, f32)> {
        self.active_sounds
            .iter()
            .map(|(sound_id, &volume)| (sound_id.clone(), effective_volume(volume, self.global_volume)))
            .collect()
    }

    /// Set volume for a specific sound.
    ///
    /// Does not write the config: a slider calls this on every input event
    /// during a drag, so the caller saves once the drag settles.
    pub fn set_sound_volume(&mut self, sound_id: String, volume: f32) {
        let clamped_volume = volume.clamp(0.0, 1.0);
        if self.active_sounds.contains_key(&sound_id) {
            self.active_sounds.insert(sound_id.clone(), clamped_volume);

            // Update audio player volume if sound is playing
            if self.is_playing && !self.is_muted {
                let _ = set_ambiance_sound_volume(
                    &sound_id,
                    effective_volume(clamped_volume, self.global_volume)
                );
            }
        }
    }

    /// Set global volume (0.0 to 1.0), which scales every active sound.
    ///
    /// Does not write the config, for the same reason as `set_sound_volume`.
    pub fn set_global_volume(&mut self, volume: f32) {
        self.global_volume = volume.clamp(0.0, 1.0);
        set_ambiance_sound_volumes(self.effective_volumes());
    }

    /// Toggle global mute
    pub fn toggle_mute(&mut self) {
        self.is_muted = !self.is_muted;

        // Update audio player mute state
        let _ = set_global_ambiance_mute(self.is_muted);

        self.save_config();
    }

    /// Toggle global play/pause state
    pub fn toggle_play_pause(&mut self) {
        self.is_playing = !self.is_playing;

        if self.is_playing {
            // Start playing all active sounds
            self.start_all_active_sounds();
            let _ = resume_all_ambiance_sounds();
        } else {
            // Pause all sounds
            let _ = pause_all_ambiance_sounds();
        }

        // Don't save to config - play state is not persistent
    }

    /// Start all active sounds when play is pressed
    fn start_all_active_sounds(&self) {
        if !self.is_playing || self.is_muted {
            return;
        }

        for (sound_id, volume) in self.effective_volumes() {
            if let Some(sound) = self.get_sound_by_id(&sound_id) {
                let _ = play_ambiance_sound(sound_id.clone(), sound.audio_url.clone(), volume);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_volume_scales_the_sound_by_the_master() {
        assert_eq!(effective_volume(0.8, 0.5), 0.4);
        assert_eq!(effective_volume(0.4, 1.0), 0.4);
        assert_eq!(effective_volume(0.6, 0.0), 0.0);
    }

    #[test]
    fn effective_volume_stays_in_range() {
        assert_eq!(effective_volume(1.5, 1.0), 1.0);
        assert_eq!(effective_volume(-0.2, 1.0), 0.0);
    }

    #[test]
    fn master_volume_keeps_the_mix_between_sounds() {
        let mut state = AmbiancePlayerState::new();
        state.active_sounds.insert("rain".to_string(), 0.8);
        state.active_sounds.insert("fire".to_string(), 0.4);
        state.global_volume = 0.5;

        let mut volumes = state.effective_volumes();
        volumes.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(volumes, vec![("fire".to_string(), 0.2), ("rain".to_string(), 0.4)]);
    }
}
