use serde::{ Deserialize, Serialize };
use std::collections::HashMap;
use std::sync::{ Arc, Mutex };
use std::sync::mpsc;
use std::thread;
use rodio::{ Decoder, OutputStream, OutputStreamHandle, Sink, Source };
use std::fs::File;
use std::io::BufReader;
use std::path::{ Path, PathBuf };

use crate::libs::device_manager::DeviceManager;
use crate::state::config::CustomMoodSound;

/// Commands for the single background thread that owns the shared ambiance
/// `OutputStream`. All sounds share one stream/handle (rather than each
/// played sound opening its own, as it once did) so a
/// device switch has exactly one stream to rebuild.
#[derive(Debug, PartialEq)]
enum AmbianceEngineCommand {
    Play {
        sound_id: String,
        path: PathBuf,
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
    /// Silences every sink without dropping it. Nothing resumes them in
    /// place: when sound becomes audible again every active sound is started
    /// with `Play`, which replaces its paused sink, so a sound switched on
    /// while silent starts too.
    PauseAll,
    /// `resume` carries the sounds that were actually audible before the
    /// switch (empty if paused/muted) - see `switch_ambiance_player_device`,
    /// which reads `AmbiancePlayerState` to build this since the engine
    /// thread only knows about `Sink`s, not playback intent.
    SwitchDevice {
        device_id: Option<String>,
        resume: Vec<(String, PathBuf, f32)>, // (sound_id, file, effective_volume)
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
            if !state.is_audible() {
                return Vec::new();
            }
            state
                .effective_volumes()
                .into_iter()
                .filter_map(|(sound_id, volume)| {
                    let path = state.get_sound_by_id(&sound_id)?.file_path();
                    Some((sound_id, path, volume))
                })
                .collect()
        })
        .unwrap_or_default();

    send_engine_command(AmbianceEngineCommand::SwitchDevice { device_id, resume });
}

/// Opens, decodes, and starts a looping `Sink` for the file at `path` on
/// `stream_handle`. Shared by `Play` and the `SwitchDevice` resume step so
/// both build sinks the same way. `path` comes from `AmbianceSound::file_path`.
fn build_ambiance_sink(
    stream_handle: &OutputStreamHandle,
    path: &Path,
    volume: f32
) -> Result<Sink, String> {
    let file = File::open(path).map_err(|e|
        format!("Failed to open audio file {}: {}", path.display(), e)
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
                AmbianceEngineCommand::Play { sound_id, path, volume } => {
                    if let Some(old) = sinks.remove(&sound_id) {
                        old.stop();
                    }

                    match build_ambiance_sink(&stream_handle, &path, volume) {
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

                            for (sound_id, path, volume) in resume {
                                match build_ambiance_sink(&stream_handle, &path, volume) {
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

// Send the commands one user action produced, in order
fn send_engine_commands(commands: Vec<AmbianceEngineCommand>) {
    for command in commands {
        send_engine_command(command);
    }
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AmbianceSound {
    pub id: String,
    pub name: String,
    pub description: String,
    /// A built-in sound's "assets/sounds/x.mp3", or, for one the user added,
    /// its file name in the Mood sounds folder. `file_path` resolves either.
    pub audio_url: String,
    pub icon: String, // Icon name for UI display
    /// Added by the user from a file of their own, so it can be removed.
    #[serde(default)]
    pub custom: bool,
}

impl AmbianceSound {
    /// A sound the user added, as the list shows it.
    pub fn from_custom(sound: &CustomMoodSound) -> Self {
        Self {
            id: sound.id.clone(),
            name: sound.name.clone(),
            description: "Your sound".to_string(),
            audio_url: sound.file_name.clone(),
            icon: "music".to_string(),
            custom: true,
        }
    }

    /// Where the sound's file is on disk.
    pub fn file_path(&self) -> PathBuf {
        if self.custom {
            crate::utils::mood_sounds::path_of(&self.audio_url)
        } else {
            // The web-style "assets/sounds/x.mp3" resolves beside the
            // executable on Windows/macOS but under /usr/lib on a Linux
            // install, where the process cwd is wherever the user launched from.
            crate::state::paths::bundled_assets::asset_path(&self.audio_url)
        }
    }

    /// The entry the config keeps for a sound the user added.
    fn to_custom(&self) -> Option<CustomMoodSound> {
        self.custom.then(|| CustomMoodSound {
            id: self.id.clone(),
            name: self.name.clone(),
            file_name: self.audio_url.clone(),
        })
    }
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
            sounds: Self::sounds_with(&config.ambiance_custom_sounds),
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
        let custom_sounds = self.custom_sounds();
        crate::state::config_writer::apply(move |config| {
            config.ambiance_active_sounds = active_sounds;
            config.ambiance_global_volume = global_volume;
            config.ambiance_is_muted = is_muted;
            config.ambiance_custom_sounds = custom_sounds;
            // Don't save is_playing - always start paused
        });
    }

    /// The built-in sounds followed by the ones the user added.
    fn sounds_with(custom: &[CustomMoodSound]) -> Vec<AmbianceSound> {
        let mut sounds = Self::get_builtin_sounds();
        sounds.extend(custom.iter().map(AmbianceSound::from_custom));
        sounds
    }

    /// The config entries of the sounds the user added, in list order.
    fn custom_sounds(&self) -> Vec<CustomMoodSound> {
        self.sounds.iter().filter_map(AmbianceSound::to_custom).collect()
    }

    /// Add a sound from the user's own file, switched on and, when sound is
    /// audible, playing right away. Returns false when that file is already
    /// in the list. Saves the change.
    pub fn add_custom_sound(&mut self, sound: CustomMoodSound) -> bool {
        let Some(commands) = self.add_custom_sound_commands(sound) else {
            return false;
        };
        send_engine_commands(commands);
        self.save_config();
        true
    }

    /// The state change behind `add_custom_sound`.
    fn add_custom_sound_commands(&mut self, sound: CustomMoodSound) -> Option<Vec<AmbianceEngineCommand>> {
        if self.get_sound_by_id(&sound.id).is_some() {
            return None;
        }
        let id = sound.id.clone();
        self.sounds.push(AmbianceSound::from_custom(&sound));
        Some(self.toggle_sound_commands(id))
    }

    /// Remove a sound the user added: stop it, drop it from the list, save,
    /// and delete its copy. A built-in sound is never removed.
    pub fn remove_custom_sound(&mut self, sound_id: &str) {
        let Some((commands, file_name)) = self.remove_custom_sound_commands(sound_id) else {
            return;
        };
        send_engine_commands(commands);
        self.save_config();
        crate::utils::mood_sounds::remove(&file_name);
    }

    /// The state change behind `remove_custom_sound`, with the file to delete.
    fn remove_custom_sound_commands(&mut self, sound_id: &str) -> Option<(Vec<AmbianceEngineCommand>, String)> {
        let index = self.sounds.iter().position(|sound| sound.id == sound_id && sound.custom)?;
        let sound = self.sounds.remove(index);
        let commands = if self.active_sounds.remove(sound_id).is_some() {
            vec![AmbianceEngineCommand::Stop(sound_id.to_string())]
        } else {
            Vec::new()
        };
        Some((commands, sound.audio_url))
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
                custom: false,
            },
            AmbianceSound {
                id: "forest".to_string(),
                name: "Forest".to_string(),
                description: "Birds chirping in the forest".to_string(),
                audio_url: "assets/sounds/forest.mp3".to_string(),
                icon: "tree-pine".to_string(),
                custom: false,
            },
            AmbianceSound {
                id: "ocean".to_string(),
                name: "Ocean Waves".to_string(),
                description: "Calming ocean waves".to_string(),
                audio_url: "assets/sounds/ocean.mp3".to_string(),
                icon: "waves".to_string(),
                custom: false,
            },
            AmbianceSound {
                id: "thunderstorm".to_string(),
                name: "Thunderstorm".to_string(),
                description: "Thunder and heavy rain".to_string(),
                audio_url: "assets/sounds/thunderstorm.mp3".to_string(),
                icon: "zap".to_string(),
                custom: false,
            },
            AmbianceSound {
                id: "fire".to_string(),
                name: "Campfire".to_string(),
                description: "Crackling campfire".to_string(),
                audio_url: "assets/sounds/fire.mp3".to_string(),
                icon: "flame".to_string(),
                custom: false,
            },
            AmbianceSound {
                id: "wind".to_string(),
                name: "Wind".to_string(),
                description: "Gentle wind through trees".to_string(),
                audio_url: "assets/sounds/wind.mp3".to_string(),
                icon: "wind".to_string(),
                custom: false,
            },
            AmbianceSound {
                id: "stream".to_string(),
                name: "Stream".to_string(),
                description: "Babbling brook".to_string(),
                audio_url: "assets/sounds/stream.mp3".to_string(),
                icon: "waves".to_string(),
                custom: false,
            },
            AmbianceSound {
                id: "river".to_string(),
                name: "River".to_string(),
                description: "Flowing river water".to_string(),
                audio_url: "assets/sounds/river.mp3".to_string(),
                icon: "waves".to_string(),
                custom: false,
            },
            AmbianceSound {
                id: "cricket".to_string(),
                name: "Night Crickets".to_string(),
                description: "Crickets and night sounds".to_string(),
                audio_url: "assets/sounds/cricket.mp3".to_string(),
                icon: "moon".to_string(),
                custom: false,
            },
            AmbianceSound {
                id: "chatter".to_string(),
                name: "Coffee Shop".to_string(),
                description: "Ambient coffee shop chatter".to_string(),
                audio_url: "assets/sounds/chatter.mp3".to_string(),
                icon: "coffee".to_string(),
                custom: false,
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

    /// Whether the active sounds should be heard right now.
    fn is_audible(&self) -> bool {
        self.is_playing && !self.is_muted
    }

    /// Toggle a sound on/off, tell the engine, and save the change.
    pub fn toggle_sound(&mut self, sound_id: String) {
        send_engine_commands(self.toggle_sound_commands(sound_id));
        self.save_config();
    }

    /// The state change behind `toggle_sound`, returning what the engine
    /// has to do. A sound switched off is always stopped, so a paused sink
    /// for it is dropped too. A sound switched on while nothing is audible
    /// only joins the list; it starts with the rest once sound comes back.
    fn toggle_sound_commands(&mut self, sound_id: String) -> Vec<AmbianceEngineCommand> {
        if self.active_sounds.remove(&sound_id).is_some() {
            return vec![AmbianceEngineCommand::Stop(sound_id)];
        }

        let volume = 0.5; // Default volume 50%
        self.active_sounds.insert(sound_id.clone(), volume);
        if !self.is_audible() {
            return Vec::new();
        }
        match self.get_sound_by_id(&sound_id) {
            Some(sound) =>
                vec![AmbianceEngineCommand::Play {
                    path: sound.file_path(),
                    volume: effective_volume(volume, self.global_volume),
                    sound_id,
                }],
            None => Vec::new(),
        }
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

    /// Toggle global mute, tell the engine, and save the change.
    pub fn toggle_mute(&mut self) {
        send_engine_commands(self.toggle_mute_commands());
        self.save_config();
    }

    /// The state change behind `toggle_mute`. Mute and the play button both
    /// silence the sounds, so the engine only changes when that adds up to a
    /// different result: unmuting while paused stays silent.
    fn toggle_mute_commands(&mut self) -> Vec<AmbianceEngineCommand> {
        let was_audible = self.is_audible();
        self.is_muted = !self.is_muted;
        self.audibility_commands(was_audible)
    }

    /// Toggle global play/pause state and tell the engine.
    ///
    /// Not saved: the app always starts paused.
    pub fn toggle_play_pause(&mut self) {
        send_engine_commands(self.toggle_play_pause_commands());
    }

    /// The state change behind `toggle_play_pause`, see `toggle_mute_commands`.
    fn toggle_play_pause_commands(&mut self) -> Vec<AmbianceEngineCommand> {
        let was_audible = self.is_audible();
        self.is_playing = !self.is_playing;
        self.audibility_commands(was_audible)
    }

    /// What the engine has to do after mute or play/pause changed. Becoming
    /// audible starts every active sound afresh rather than resuming paused
    /// sinks, since a sound switched on while silent has no sink yet.
    fn audibility_commands(&self, was_audible: bool) -> Vec<AmbianceEngineCommand> {
        match (was_audible, self.is_audible()) {
            (false, true) => self.start_commands(),
            (true, false) => vec![AmbianceEngineCommand::PauseAll],
            _ => Vec::new(),
        }
    }

    /// A `Play` for every active sound at its effective volume.
    fn start_commands(&self) -> Vec<AmbianceEngineCommand> {
        self.effective_volumes()
            .into_iter()
            .filter_map(|(sound_id, volume)| {
                let path = self.get_sound_by_id(&sound_id)?.file_path();
                Some(AmbianceEngineCommand::Play { sound_id, path, volume })
            })
            .collect()
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

    fn playing_state() -> AmbiancePlayerState {
        let mut state = AmbiancePlayerState::new();
        state.is_playing = true;
        state.global_volume = 0.8;
        state
    }

    fn play(sound_id: &str, volume: f32) -> AmbianceEngineCommand {
        AmbianceEngineCommand::Play {
            sound_id: sound_id.to_string(),
            path: crate::state::paths::bundled_assets::asset_path(&format!("assets/sounds/{}.mp3", sound_id)),
            volume,
        }
    }

    fn lofi() -> CustomMoodSound {
        CustomMoodSound {
            id: "custom-0123456789abcdef0123456789abcdef".to_string(),
            name: "Lofi".to_string(),
            file_name: "sound-0123456789abcdef0123456789abcdef.mp3".to_string(),
        }
    }

    #[test]
    fn an_added_sound_joins_the_list_on_and_plays_from_the_mood_sounds_folder() {
        let mut state = playing_state();

        let commands = state.add_custom_sound_commands(lofi()).expect("a new sound is added");

        assert_eq!(commands, vec![AmbianceEngineCommand::Play {
            sound_id: lofi().id,
            path: crate::utils::mood_sounds::path_of(&lofi().file_name),
            volume: 0.4,
        }]);
        assert!(state.is_sound_active(&lofi().id));
        assert_eq!(state.sounds.last().map(|sound| sound.name.as_str()), Some("Lofi"));
        assert_eq!(state.custom_sounds(), vec![lofi()], "the config gets the entry back as it came");
    }

    #[test]
    fn the_same_file_added_twice_stays_one_entry() {
        let mut state = playing_state();
        state.add_custom_sound_commands(lofi());
        assert_eq!(state.add_custom_sound_commands(lofi()), None);
        assert_eq!(state.custom_sounds().len(), 1);
    }

    #[test]
    fn an_added_sound_waits_while_paused() {
        let mut state = AmbiancePlayerState::new();
        assert_eq!(state.add_custom_sound_commands(lofi()), Some(vec![]));
        assert!(state.is_sound_active(&lofi().id), "it starts with the rest on play");
    }

    #[test]
    fn removing_an_added_sound_stops_it_and_hands_back_its_file() {
        let mut state = playing_state();
        state.add_custom_sound_commands(lofi());

        let (commands, file_name) = state.remove_custom_sound_commands(&lofi().id).unwrap();

        assert_eq!(commands, vec![AmbianceEngineCommand::Stop(lofi().id)]);
        assert_eq!(file_name, lofi().file_name);
        assert!(!state.is_sound_active(&lofi().id));
        assert!(state.custom_sounds().is_empty());
    }

    #[test]
    fn built_in_sounds_cannot_be_removed() {
        let mut state = playing_state();
        state.active_sounds.insert("rain".to_string(), 0.5);
        assert_eq!(state.remove_custom_sound_commands("rain"), None);
        assert!(state.get_sound_by_id("rain").is_some());
        assert!(state.is_sound_active("rain"));
    }

    #[test]
    fn sounds_from_the_config_follow_the_built_in_ones() {
        let sounds = AmbiancePlayerState::sounds_with(&[lofi()]);
        let builtin = AmbiancePlayerState::get_builtin_sounds();
        assert_eq!(sounds.len(), builtin.len() + 1);
        assert!(builtin.iter().all(|sound| !sound.custom));
        assert!(sounds.last().unwrap().custom);
    }

    #[test]
    fn switching_a_sound_on_plays_it_once_scaled_by_the_master() {
        let mut state = playing_state();

        let commands = state.toggle_sound_commands("rain".to_string());

        assert_eq!(commands, vec![play("rain", 0.4)]);
        assert_eq!(state.active_sounds.get("rain"), Some(&0.5));
    }

    #[test]
    fn switching_a_sound_off_stops_it_once() {
        let mut state = playing_state();
        state.active_sounds.insert("rain".to_string(), 0.7);

        let commands = state.toggle_sound_commands("rain".to_string());

        assert_eq!(commands, vec![AmbianceEngineCommand::Stop("rain".to_string())]);
        assert!(!state.is_sound_active("rain"));
    }

    #[test]
    fn a_sound_switched_on_while_muted_starts_on_unmute() {
        let mut state = playing_state();
        state.is_muted = true;

        assert_eq!(state.toggle_sound_commands("rain".to_string()), vec![]);
        assert_eq!(state.toggle_mute_commands(), vec![play("rain", 0.4)]);
        assert!(!state.is_muted);
    }

    #[test]
    fn muting_while_playing_pauses_once() {
        let mut state = playing_state();
        state.active_sounds.insert("rain".to_string(), 0.5);

        assert_eq!(state.toggle_mute_commands(), vec![AmbianceEngineCommand::PauseAll]);
        assert!(state.is_muted);
    }

    #[test]
    fn unmuting_while_paused_stays_silent() {
        let mut state = AmbiancePlayerState::new();
        state.active_sounds.insert("rain".to_string(), 0.5);
        state.is_muted = true;

        assert_eq!(state.toggle_mute_commands(), vec![]);
    }

    #[test]
    fn pressing_play_starts_each_active_sound_once() {
        let mut state = AmbiancePlayerState::new();
        state.global_volume = 0.8;
        state.active_sounds.insert("rain".to_string(), 0.5);

        assert_eq!(state.toggle_play_pause_commands(), vec![play("rain", 0.4)]);
        assert_eq!(state.toggle_play_pause_commands(), vec![AmbianceEngineCommand::PauseAll]);
    }

    #[test]
    fn pressing_play_while_muted_stays_silent() {
        let mut state = AmbiancePlayerState::new();
        state.active_sounds.insert("rain".to_string(), 0.5);
        state.is_muted = true;

        assert_eq!(state.toggle_play_pause_commands(), vec![]);
        assert!(state.is_playing);
    }
}
