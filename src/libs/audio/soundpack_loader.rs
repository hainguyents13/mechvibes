use crate::state::paths;
use crate::state::soundpack::SoundPack;
use crate::state::soundpack::{ SoundpackCache, SoundpackMetadata };
use std::sync::Arc;

use super::audio_context::AudioContext;
use super::engine::{ AudioCommand, EngineState };

/// Determine soundpack type based on the soundpack path
fn determine_soundpack_type(soundpack_id: &str) -> crate::state::soundpack::SoundpackType {
    if soundpack_id.starts_with("keyboard/") || soundpack_id.starts_with("keyboard\\") {
        crate::state::soundpack::SoundpackType::Keyboard
    } else if soundpack_id.starts_with("mouse/") || soundpack_id.starts_with("mouse\\") {
        crate::state::soundpack::SoundpackType::Mouse
    } else {
        // Default to keyboard for backwards compatibility
        crate::state::soundpack::SoundpackType::Keyboard
    }
}

/// Requests both keyboard and mouse soundpacks be (re)loaded from config.
/// Loading happens asynchronously on the engine thread - callers that need
/// to know the outcome should watch `UiEvent::PackLoaded` (see `ui.rs`)
/// rather than this function's return value, which only reflects whether
/// the request was sent.
pub fn load_soundpack(context: &AudioContext) -> Result<(), String> {
    let config = crate::state::config_writer::current();
    load_keyboard_soundpack(context, &config.keyboard_soundpack)?;
    load_mouse_soundpack(context, &config.mouse_soundpack)?;
    Ok(())
}

pub fn load_keyboard_soundpack(context: &AudioContext, soundpack_id: &str) -> Result<(), String> {
    load_keyboard_soundpack_with_cache_control(context, soundpack_id, true)
}

/// Sends a `LoadKeyboardPack` command to the engine thread. Errors from the
/// actual load (missing file, parse failure, etc.) surface later via
/// `UiEvent::PackLoaded`, not this function's return value.
pub fn load_keyboard_soundpack_with_cache_control(
    context: &AudioContext,
    soundpack_id: &str,
    update_cache_on_error: bool
) -> Result<(), String> {
    // An empty id means the config has no pack for this slot - a hand-edited
    // or freshly created config, not an error. Nothing to load, and nothing is
    // loaded yet either, so leave the engine alone and let the user pick.
    if soundpack_id.is_empty() {
        crate::always_print!("🎹 No keyboard soundpack selected");
        return Ok(());
    }

    crate::always_print!("🎹 Requesting keyboard soundpack load: {}", soundpack_id);
    context.send(AudioCommand::LoadKeyboardPack {
        soundpack_id: soundpack_id.to_string(),
        update_cache_on_error,
    });
    Ok(())
}

pub fn load_mouse_soundpack(context: &AudioContext, soundpack_id: &str) -> Result<(), String> {
    load_mouse_soundpack_with_cache_control(context, soundpack_id, true)
}

/// Sends a `LoadMousePack` command to the engine thread. See
/// `load_keyboard_soundpack_with_cache_control` for the async-result note.
pub fn load_mouse_soundpack_with_cache_control(
    context: &AudioContext,
    soundpack_id: &str,
    update_cache_on_error: bool
) -> Result<(), String> {
    // See the keyboard loader: an empty slot is tolerated, not unloaded.
    if soundpack_id.is_empty() {
        crate::always_print!("🖱️ No mouse soundpack selected");
        return Ok(());
    }

    crate::always_print!("🖱️ Requesting mouse soundpack load: {}", soundpack_id);
    context.send(AudioCommand::LoadMousePack {
        soundpack_id: soundpack_id.to_string(),
        update_cache_on_error,
    });
    Ok(())
}

/// (samples, channels, sample_rate) for a decoded/resampled audio buffer.
type DecodedAudio = (Vec<f32>, u16, u32);

/// Loads and decodes a soundpack's audio file, then resamples it to
/// `device_rate` if given. Returns `(original, resampled)`; `original` keeps
/// the file's native rate so a later device switch can re-resample without
/// re-reading from disk.
///
/// `device_rate` is passed in by the caller (from `AudioContext`'s cached
/// rate) rather than probed here - probing the device on every soundpack
/// load can briefly interrupt other audio on Linux/ALSA (see the
/// enumeration-avoidance note in `ui.rs`). `None` (rate unknown, or probe
/// failed at startup) means skip resampling and keep the file's native rate,
/// same as pre-resample baseline behavior - guessing a rate would risk
/// resampling twice (once here, once again by rodio realtime).
///
/// A "multi" pack's timings are moved to where each key's file lands in the
/// combined buffer, which is why `soundpack` is taken mutably.
fn load_audio_file(
    soundpack_path: &str,
    soundpack: &mut SoundPack,
    device_rate: Option<u32>
) -> Result<(DecodedAudio, DecodedAudio), String> {
    let (samples, channels, file_rate) = if soundpack.definition_method == "multi" {
        decode_multi_pack(soundpack_path, soundpack)?
    } else {
        let sound_file_path = soundpack.audio_file
            .as_ref()
            .map(|src| pack_file_path(soundpack_path, src))
            .ok_or_else(|| "No audio_file field in soundpack config".to_string())?;

        if !std::path::Path::new(&sound_file_path).exists() {
            return Err(format!("Sound file not found: {}", sound_file_path));
        }

        // Use Symphonia for audio loading instead of Rodio
        load_audio_with_symphonia(&sound_file_path).map_err(|e|
            format!("Failed to load audio: {}", e)
        )?
    };

    match device_rate {
        Some(device_rate) if device_rate != file_rate => {
            let start = std::time::Instant::now();
            let resampled = super::resampler::resample_interleaved(
                &samples,
                channels,
                file_rate,
                device_rate
            );
            crate::always_print!(
                "🔁 Resampled soundpack audio {}Hz -> {}Hz in {:.1}ms",
                file_rate,
                device_rate,
                start.elapsed().as_secs_f64() * 1000.0
            );
            let original = (samples, channels, file_rate);
            Ok((original, (resampled, channels, device_rate)))
        }
        _ => {
            // Either the device rate matches the file rate, or it's unknown
            // (probe failed) - either way, skip resampling and reuse the
            // same buffer for original/resampled (no need to clone: both
            // tuples describe identical audio at the file's native rate).
            let decoded = (samples, channels, file_rate);
            Ok((decoded.clone(), decoded))
        }
    }
}

/// A file named in a pack's config, as a path inside the pack's folder.
fn pack_file_path(soundpack_path: &str, file: &str) -> String {
    format!("{}/{}", soundpack_path, file.trim_start_matches("./"))
}

/// The file a key of a "multi" pack plays from: its own, or the pack's
/// `audio_file` when it names none.
fn multi_key_file<'a>(
    key_def: &'a crate::state::soundpack::KeyDefinition,
    soundpack: &'a SoundPack
) -> Option<&'a String> {
    key_def.audio_file.as_ref().or(soundpack.audio_file.as_ref())
}

/// Decode a "multi" pack, where each key names its own file, into one buffer.
///
/// The pack on disk is left as it is, so it stays a multi pack that the editor
/// can open again. Every file is decoded and appended, and each key's timing
/// is moved to where its file starts. A file that cannot be read is logged and
/// its keys stay silent; the rest of the pack still plays.
fn decode_multi_pack(soundpack_path: &str, soundpack: &mut SoundPack) -> Result<DecodedAudio, String> {
    let mut files: Vec<String> = soundpack.definitions
        .values()
        .filter_map(|key_def| multi_key_file(key_def, soundpack).cloned())
        .collect();
    files.sort();
    files.dedup();

    let mut decoded = Vec::new();
    for file in files {
        match load_audio_with_symphonia(&pack_file_path(soundpack_path, &file)) {
            Ok(audio) => decoded.push((file, audio)),
            Err(e) => crate::always_eprint!("⚠️  {}: could not load {}: {}", soundpack.name, file, e),
        }
    }

    let (combined, offsets) = combine_decoded(decoded).ok_or_else(||
        "None of the pack's audio files could be loaded".to_string()
    )?;
    shift_multi_timings(soundpack, &offsets);
    Ok(combined)
}

/// Append decoded files into one buffer in the first file's channel layout
/// and rate, returning the buffer and where each file starts, in ms.
fn combine_decoded(
    files: Vec<(String, DecodedAudio)>
) -> Option<(DecodedAudio, std::collections::HashMap<String, f32>)> {
    let (_, (_, channels, rate)) = files.first()?;
    let (channels, rate) = (*channels, *rate);

    let mut combined = Vec::new();
    let mut offsets = std::collections::HashMap::new();
    for (file, (samples, from_channels, from_rate)) in files {
        let frames_so_far = combined.len() / (channels.max(1) as usize);
        offsets.insert(file, ((frames_so_far as f64) * 1000.0 / (rate as f64)) as f32);
        combined.extend(
            crate::utils::config_converter::convert_audio_format(
                &samples,
                from_channels,
                from_rate,
                channels,
                rate
            )
        );
    }
    Some(((combined, channels, rate), offsets))
}

/// Move each key's timing by where its file starts in the combined buffer.
/// A key whose file did not load is dropped, so it stays silent instead of
/// playing a slice of some other file.
fn shift_multi_timings(soundpack: &mut SoundPack, offsets: &std::collections::HashMap<String, f32>) {
    let fallback = soundpack.audio_file.clone();
    soundpack.definitions.retain(|_, key_def| {
        let Some(offset) = key_def.audio_file
            .as_ref()
            .or(fallback.as_ref())
            .and_then(|file| offsets.get(file)) else {
            return false;
        };
        for segment in &mut key_def.timing {
            segment[0] += offset;
            segment[1] += offset;
        }
        true
    });
}

/// Load audio file using Symphonia for consistent duration detection
fn load_audio_with_symphonia(file_path: &str) -> Result<(Vec<f32>, u16, u32), String> {
    use symphonia::core::audio::{ AudioBufferRef, Signal };
    use symphonia::core::codecs::{ DecoderOptions, CODEC_TYPE_NULL };
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;
    use std::fs::File;

    // First, check if file exists and has content
    let metadata = std::fs
        ::metadata(file_path)
        .map_err(|e| format!("Failed to get file metadata: {}", e))?;
    if metadata.len() == 0 {
        return Err(format!("Audio file is empty: {}", file_path));
    }

    let file = File::open(file_path).map_err(|e| format!("Failed to open file: {}", e))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = std::path::Path::new(file_path).extension() {
        if let Some(ext_str) = extension.to_str() {
            hint.with_extension(ext_str);
        }
    }

    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();

    let probed = symphonia::default
        ::get_probe()
        .format(&hint, mss, &fmt_opts, &meta_opts)
        .map_err(|e| {
            format!(
                "Failed to probe format for '{}': {} (file size: {} bytes)",
                file_path,
                e,
                metadata.len()
            )
        })?;

    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or("No supported audio tracks found")?;

    let dec_opts: DecoderOptions = Default::default();
    let mut decoder = symphonia::default
        ::get_codecs()
        .make(&track.codec_params, &dec_opts)
        .map_err(|e| format!("Failed to create decoder: {}", e))?;

    let track_id = track.id;
    let mut samples = Vec::new();
    let mut sample_rate = 44100u32;
    let mut channels = 2u16;

    // Decode audio packets
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(_) => {
                break;
            } // End of stream
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                if samples.is_empty() {
                    // Get format info from first decoded buffer
                    sample_rate = decoded.spec().rate;
                    channels = decoded.spec().channels.count() as u16;
                } // Convert audio buffer to f32 samples
                match decoded {
                    AudioBufferRef::F32(buf) => {
                        if channels == 1 {
                            // Mono audio
                            for &sample in buf.chan(0) {
                                samples.push(sample);
                            }
                        } else {
                            // Stereo audio - interleave samples correctly
                            let left_chan = buf.chan(0);
                            let right_chan = if buf.spec().channels.count() > 1 {
                                buf.chan(1)
                            } else {
                                buf.chan(0)
                            };
                            for (left, right) in left_chan.iter().zip(right_chan.iter()) {
                                samples.push(*left);
                                samples.push(*right);
                            }
                        }
                    }
                    AudioBufferRef::S32(buf) => {
                        if channels == 1 {
                            for &sample in buf.chan(0) {
                                samples.push((sample as f32) / (i32::MAX as f32));
                            }
                        } else {
                            let left_chan = buf.chan(0);
                            let right_chan = if buf.spec().channels.count() > 1 {
                                buf.chan(1)
                            } else {
                                buf.chan(0)
                            };
                            for (left, right) in left_chan.iter().zip(right_chan.iter()) {
                                samples.push((*left as f32) / (i32::MAX as f32));
                                samples.push((*right as f32) / (i32::MAX as f32));
                            }
                        }
                    }
                    AudioBufferRef::S16(buf) => {
                        if channels == 1 {
                            for &sample in buf.chan(0) {
                                samples.push((sample as f32) / (i16::MAX as f32));
                            }
                        } else {
                            let left_chan = buf.chan(0);
                            let right_chan = if buf.spec().channels.count() > 1 {
                                buf.chan(1)
                            } else {
                                buf.chan(0)
                            };
                            for (left, right) in left_chan.iter().zip(right_chan.iter()) {
                                samples.push((*left as f32) / (i16::MAX as f32));
                                samples.push((*right as f32) / (i16::MAX as f32));
                            }
                        }
                    }
                    AudioBufferRef::U32(buf) => {
                        if channels == 1 {
                            for &sample in buf.chan(0) {
                                samples.push(
                                    ((sample as f32) - (u32::MAX as f32) / 2.0) /
                                        ((u32::MAX as f32) / 2.0)
                                );
                            }
                        } else {
                            let left_chan = buf.chan(0);
                            let right_chan = if buf.spec().channels.count() > 1 {
                                buf.chan(1)
                            } else {
                                buf.chan(0)
                            };
                            for (left, right) in left_chan.iter().zip(right_chan.iter()) {
                                samples.push(
                                    ((*left as f32) - (u32::MAX as f32) / 2.0) /
                                        ((u32::MAX as f32) / 2.0)
                                );
                                samples.push(
                                    ((*right as f32) - (u32::MAX as f32) / 2.0) /
                                        ((u32::MAX as f32) / 2.0)
                                );
                            }
                        }
                    }
                    AudioBufferRef::U16(buf) => {
                        if channels == 1 {
                            for &sample in buf.chan(0) {
                                samples.push(
                                    ((sample as f32) - (u16::MAX as f32) / 2.0) /
                                        ((u16::MAX as f32) / 2.0)
                                );
                            }
                        } else {
                            let left_chan = buf.chan(0);
                            let right_chan = if buf.spec().channels.count() > 1 {
                                buf.chan(1)
                            } else {
                                buf.chan(0)
                            };
                            for (left, right) in left_chan.iter().zip(right_chan.iter()) {
                                samples.push(
                                    ((*left as f32) - (u16::MAX as f32) / 2.0) /
                                        ((u16::MAX as f32) / 2.0)
                                );
                                samples.push(
                                    ((*right as f32) - (u16::MAX as f32) / 2.0) /
                                        ((u16::MAX as f32) / 2.0)
                                );
                            }
                        }
                    }
                    AudioBufferRef::U8(buf) => {
                        if channels == 1 {
                            for &sample in buf.chan(0) {
                                samples.push(((sample as f32) - 128.0) / 128.0);
                            }
                        } else {
                            let left_chan = buf.chan(0);
                            let right_chan = if buf.spec().channels.count() > 1 {
                                buf.chan(1)
                            } else {
                                buf.chan(0)
                            };
                            for (left, right) in left_chan.iter().zip(right_chan.iter()) {
                                samples.push(((*left as f32) - 128.0) / 128.0);
                                samples.push(((*right as f32) - 128.0) / 128.0);
                            }
                        }
                    }
                    AudioBufferRef::S8(buf) => {
                        if channels == 1 {
                            for &sample in buf.chan(0) {
                                samples.push((sample as f32) / (i8::MAX as f32));
                            }
                        } else {
                            let left_chan = buf.chan(0);
                            let right_chan = if buf.spec().channels.count() > 1 {
                                buf.chan(1)
                            } else {
                                buf.chan(0)
                            };
                            for (left, right) in left_chan.iter().zip(right_chan.iter()) {
                                samples.push((*left as f32) / (i8::MAX as f32));
                                samples.push((*right as f32) / (i8::MAX as f32));
                            }
                        }
                    }
                    AudioBufferRef::F64(buf) => {
                        if channels == 1 {
                            for &sample in buf.chan(0) {
                                samples.push(sample as f32);
                            }
                        } else {
                            let left_chan = buf.chan(0);
                            let right_chan = if buf.spec().channels.count() > 1 {
                                buf.chan(1)
                            } else {
                                buf.chan(0)
                            };
                            for (left, right) in left_chan.iter().zip(right_chan.iter()) {
                                samples.push(*left as f32);
                                samples.push(*right as f32);
                            }
                        }
                    }
                    AudioBufferRef::U24(buf) => {
                        if channels == 1 {
                            for &sample in buf.chan(0) {
                                let sample_f32 = ((sample.inner() as f32) - 8388608.0) / 8388608.0; // 2^23
                                samples.push(sample_f32);
                            }
                        } else {
                            let left_chan = buf.chan(0);
                            let right_chan = if buf.spec().channels.count() > 1 {
                                buf.chan(1)
                            } else {
                                buf.chan(0)
                            };
                            for (left, right) in left_chan.iter().zip(right_chan.iter()) {
                                let left_f32 = ((left.inner() as f32) - 8388608.0) / 8388608.0;
                                let right_f32 = ((right.inner() as f32) - 8388608.0) / 8388608.0;
                                samples.push(left_f32);
                                samples.push(right_f32);
                            }
                        }
                    }
                    AudioBufferRef::S24(buf) => {
                        if channels == 1 {
                            for &sample in buf.chan(0) {
                                let sample_f32 = (sample.inner() as f32) / 8388607.0; // 2^23 - 1
                                samples.push(sample_f32);
                            }
                        } else {
                            let left_chan = buf.chan(0);
                            let right_chan = if buf.spec().channels.count() > 1 {
                                buf.chan(1)
                            } else {
                                buf.chan(0)
                            };
                            for (left, right) in left_chan.iter().zip(right_chan.iter()) {
                                let left_f32 = (left.inner() as f32) / 8388607.0;
                                let right_f32 = (right.inner() as f32) / 8388607.0;
                                samples.push(left_f32);
                                samples.push(right_f32);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                crate::always_print!("⚠️ [DEBUG] Decode error (continuing): {}", e);
                continue;
            }
        }
    }

    if samples.is_empty() {
        return Err("No audio data decoded".to_string());
    }

    Ok((samples, channels, sample_rate))
}

/// Derives the `type/name` id the cache is keyed by from a pack's absolute
/// path, trying both soundpack roots.
///
/// Kept separate from the two roots themselves so the path logic can be tested
/// without a filesystem: `relative_soundpack_id` does the work.
fn soundpack_id_from_path(soundpack_path: &str) -> String {
    let roots = [
        crate::utils::path::get_soundpacks_dir_absolute(),
        crate::utils::path::get_custom_soundpacks_dir_absolute(),
    ];
    relative_soundpack_id(soundpack_path, &roots)
}

/// The id for `soundpack_path` relative to whichever of `roots` contains it.
///
/// Falls back to the last two path components rather than the folder name
/// alone: an id without its `keyboard/`/`mouse/` prefix does not match the key
/// the directory scanner uses, and inserting under it duplicates the pack in
/// the cache and in every list rendered from it.
fn relative_soundpack_id(soundpack_path: &str, roots: &[String]) -> String {
    let path = std::path::Path::new(soundpack_path);

    for root in roots {
        if let Ok(relative) = path.strip_prefix(root) {
            return relative.to_string_lossy().replace('\\', "/");
        }
    }

    let mut tail: Vec<&str> = path
        .components()
        .rev()
        .take(2)
        .filter_map(|component| component.as_os_str().to_str())
        .collect();
    tail.reverse();

    if tail.is_empty() { "unknown".to_string() } else { tail.join("/") }
}

fn create_soundpack_metadata(
    soundpack_path: &str,
    soundpack: &SoundPack
) -> Result<SoundpackMetadata, String> {
    // Extract the soundpack ID from the full path
    // e.g., "/path/to/soundpacks/keyboard/Apex by teia" -> "keyboard/Apex by teia"
    //
    // Both roots have to be tried. Packs live under either the bundled
    // directory or the custom one in app data, and the id has to come out as
    // `type/name` for either: the scanner keys the cache that way, so an id
    // that loses its `keyboard/` prefix here inserts a second entry for a pack
    // already in the cache and the UI lists it twice. That is why only
    // imported packs duplicated - bundled ones matched the first root and
    // never reached the fallback.
    let id = soundpack_id_from_path(soundpack_path);

    // Get file metadata
    let last_modified = match std::fs::metadata(soundpack_path) {
        Ok(metadata) =>
            metadata
                .modified()
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        Err(_) => 0,
    };

    Ok(SoundpackMetadata {
        id: id.clone(), // Use calculated relative path ID instead of config ID
        name: soundpack.name.clone(),
        author: soundpack.author.clone(),
        description: soundpack.description.clone(),
        version: soundpack.version.clone().unwrap_or_else(|| "1.0".to_string()),
        tags: soundpack.tags.clone().unwrap_or_default(),
        icon: {
            // Generate dynamic URL for icon instead of base64 conversion
            if let Some(icon_filename) = &soundpack.icon {
                let icon_path = format!("{}/{}", soundpack_path, icon_filename);
                if std::path::Path::new(&icon_path).exists() {
                    // Create dynamic URL that will be served by the asset handler
                    Some(format!("/soundpack-images/{}/{}", id, icon_filename))
                } else {
                    Some(String::new()) // Empty string if icon file not found
                }
            } else {
                Some(String::new()) // Empty string if no icon specified
            }
        },
        soundpack_type: soundpack.soundpack_type.clone(), // Include the mouse field
        folder_path: id, // Use the derived folder path for loading
        last_modified,
        last_accessed: std::time::SystemTime
            ::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(), // Add validation fields with default values
        config_version: Some(soundpack.config_version_num),
        is_valid_v2: true, // Assume valid since it loaded successfully
        validation_status: "valid".to_string(),
        can_be_converted: false,
        // Error tracking - None since we successfully created metadata
        last_error: None,
    })
}

fn create_key_mappings(
    soundpack: &SoundPack,
    _samples: &[f32]
) -> std::collections::HashMap<String, Vec<(f64, f64)>> {
    let mut key_mappings = std::collections::HashMap::new(); // For keyboard soundpacks, use the definitions field for keyboard mappings
    // For mouse soundpacks, return empty key mappings
    if soundpack.soundpack_type == crate::state::soundpack::SoundpackType::Keyboard {
        for (key, key_def) in &soundpack.definitions {
            // Convert KeyDefinition timing to Vec<(f64, f64)>
            let converted_mappings: Vec<(f64, f64)> = key_def.timing
                .iter()
                .map(|pair| (pair[0] as f64, pair[1] as f64))
                .collect();
            key_mappings.insert(key.clone(), converted_mappings);
        }
    }

    key_mappings
}

/// Translates a mouse pack's definition key into the button code the runtime
/// actually emits.
///
/// V2 packs name their buttons directly (`MouseLeft`) and pass through
/// untouched. The keyboard names are what Mechvibes V1 mouse packs carry: they
/// were authored in the keyboard editor against iohook codes 1/2/3, which the
/// V1 converter read through its keyboard table into `Escape`/`Digit1`/
/// `Digit2`. The converter no longer does that, but packs converted by an
/// earlier build are already on disk with those keys, so they are accepted
/// here rather than left permanently silent.
fn mouse_button_for_definition(definition: &str) -> &str {
    match definition {
        "Escape" => "MouseLeft",
        "Digit1" => "MouseRight",
        "Digit2" => "MouseMiddle",
        other => other,
    }
}

fn create_mouse_mappings(
    soundpack: &SoundPack,
    _samples: &[f32]
) -> std::collections::HashMap<String, Vec<(f64, f64)>> {
    let mut mouse_mappings = std::collections::HashMap::new(); // For mouse soundpacks, use the definitions field directly
    if soundpack.soundpack_type == crate::state::soundpack::SoundpackType::Mouse {
        // This is a mouse soundpack, use definitions field for mouse mappings
        for (button, key_def) in &soundpack.definitions {
            // Convert KeyDefinition timing to Vec<(f64, f64)>
            let converted_mappings: Vec<(f64, f64)> = key_def.timing
                .iter()
                .map(|pair| (pair[0] as f64, pair[1] as f64))
                .collect();
            mouse_mappings.insert(
                mouse_button_for_definition(button).to_string(),
                converted_mappings
            );
        }
    } else {
        // This is a keyboard soundpack, create default mouse mappings from keyboard sounds
        crate::always_print!(
            "🖱️ No mouse definitions found, creating default mouse mappings from keyboard sounds"
        );

        // Use common keyboard keys as fallback for mouse buttons
        let fallback_mappings = [
            ("MouseLeft", "Space"),
            ("MouseRight", "Enter"),
            ("MouseMiddle", "Tab"),
            ("MouseWheelUp", "ArrowUp"),
            ("MouseWheelDown", "ArrowDown"),
            ("Mouse4", "Backspace"),
            ("Mouse5", "Delete"),
            ("Mouse6", "Home"),
            ("Mouse7", "End"),
            ("Mouse8", "PageUp"),
        ];
        for (mouse_button, keyboard_key) in &fallback_mappings {
            if let Some(key_def) = soundpack.definitions.get(*keyboard_key) {
                let converted_mappings: Vec<(f64, f64)> = key_def.timing
                    .iter()
                    .map(|pair| (pair[0] as f64, pair[1] as f64))
                    .collect();
                mouse_mappings.insert(mouse_button.to_string(), converted_mappings);
            }
        }
    }

    mouse_mappings
}

/// Loads a keyboard soundpack directly into engine-owned state (Phase 3).
/// Mirrors `load_keyboard_soundpack_optimized` but writes to `EngineState`
/// fields instead of an `&AudioContext`, since the engine thread owns its
/// data as plain fields rather than `Arc<Mutex<...>>`.
pub(super) fn load_keyboard_pack_into_engine(
    state: &mut EngineState,
    soundpack_id: &str,
    update_cache_on_error: bool
) -> Result<String, String> {
    if soundpack_id.is_empty() {
        return Err("empty soundpack ID".to_string());
    }

    match load_keyboard_pack_into_engine_inner(state, soundpack_id) {
        Ok(name) => Ok(name),
        Err(e) => {
            if update_cache_on_error {
                capture_soundpack_loading_error(soundpack_id, &e);
            }
            Err(e)
        }
    }
}

fn load_keyboard_pack_into_engine_inner(
    state: &mut EngineState,
    soundpack_id: &str
) -> Result<String, String> {
    let soundpack_path = paths::soundpacks::soundpack_dir(soundpack_id);
    let config_path = paths::soundpacks::config_json(soundpack_id);
    let config_content = std::fs
        ::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config: {}", e))?;
    let mut soundpack: SoundPack = serde_json
        ::from_str(&config_content)
        .map_err(|e| format!("Failed to parse V2 soundpack config: {}", e))?;
    soundpack.soundpack_type = determine_soundpack_type(soundpack_id);

    if soundpack.soundpack_type != crate::state::soundpack::SoundpackType::Keyboard {
        return Err("This is a mouse soundpack, not a keyboard soundpack".to_string());
    }

    let (original, resampled) = load_audio_file(&soundpack_path, &mut soundpack, state.device_rate)?;
    let key_mappings = create_key_mappings(&soundpack, &resampled.0);

    let (audio_samples, channels, sample_rate) = resampled;
    state.keyboard_samples = Some((Arc::new(audio_samples), channels, sample_rate));
    let (orig_samples, orig_channels, orig_rate) = original;
    state.keyboard_samples_original = Some((Arc::new(orig_samples), orig_channels, orig_rate));

    state.key_map.clear();
    for (key, mappings) in key_mappings {
        let converted: Vec<[f32; 2]> = mappings
            .into_iter()
            .map(|(start, end)| [start as f32, end as f32])
            .collect();
        state.key_map.insert(key, converted);
    }
    state.key_sinks.clear();

    update_soundpack_cache(&soundpack_path, &soundpack, soundpack_id);
    crate::always_print!("✅ [Engine] Loaded keyboard soundpack: {}", soundpack.name);
    Ok(soundpack.name)
}

/// Loads a mouse soundpack directly into engine-owned state (Phase 3).
/// See `load_keyboard_pack_into_engine` for the rationale.
pub(super) fn load_mouse_pack_into_engine(
    state: &mut EngineState,
    soundpack_id: &str,
    update_cache_on_error: bool
) -> Result<String, String> {
    if soundpack_id.is_empty() {
        return Err("empty soundpack ID".to_string());
    }

    match load_mouse_pack_into_engine_inner(state, soundpack_id) {
        Ok(name) => Ok(name),
        Err(e) => {
            if update_cache_on_error {
                capture_soundpack_loading_error(soundpack_id, &e);
            }
            Err(e)
        }
    }
}

fn load_mouse_pack_into_engine_inner(
    state: &mut EngineState,
    soundpack_id: &str
) -> Result<String, String> {
    let soundpack_path = paths::soundpacks::soundpack_dir(soundpack_id);
    let config_path = paths::soundpacks::config_json(soundpack_id);
    let config_content = std::fs
        ::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config: {}", e))?;
    let mut soundpack: SoundPack = serde_json
        ::from_str(&config_content)
        .map_err(|e| format!("Failed to parse config: {}", e))?;
    soundpack.soundpack_type = determine_soundpack_type(soundpack_id);

    if soundpack.soundpack_type != crate::state::soundpack::SoundpackType::Mouse {
        return Err("This is a keyboard soundpack, not a mouse soundpack".to_string());
    }

    let (original, resampled) = load_audio_file(&soundpack_path, &mut soundpack, state.device_rate)?;
    let mouse_mappings = create_mouse_mappings(&soundpack, &resampled.0);

    let (audio_samples, channels, sample_rate) = resampled;
    state.mouse_samples = Some((Arc::new(audio_samples), channels, sample_rate));
    let (orig_samples, orig_channels, orig_rate) = original;
    state.mouse_samples_original = Some((Arc::new(orig_samples), orig_channels, orig_rate));

    state.mouse_map.clear();
    for (button, mappings) in mouse_mappings {
        let converted: Vec<[f32; 2]> = mappings
            .into_iter()
            .map(|(start, end)| [start as f32, end as f32])
            .collect();
        state.mouse_map.insert(button, converted);
    }
    state.mouse_sinks.clear();

    update_soundpack_cache(&soundpack_path, &soundpack, soundpack_id);
    crate::always_print!("✅ [Engine] Loaded mouse soundpack: {}", soundpack.name);
    Ok(soundpack.name)
}

/// Shared metadata-cache update used by both engine loaders (extracted from
/// the duplicated tail of `load_keyboard_soundpack_optimized`/
/// `load_mouse_soundpack_optimized`).
fn update_soundpack_cache(soundpack_path: &str, soundpack: &SoundPack, soundpack_id: &str) {
    let mut cache = SoundpackCache::load();
    match create_soundpack_metadata(soundpack_path, soundpack) {
        Ok(metadata) => {
            cache.add_soundpack(metadata);
        }
        Err(e) => {
            crate::always_print!("⚠️ Failed to create metadata for {}: {}", soundpack_id, e);
        }
    }
    cache.save();
}

/// Capture soundpack loading error and update the cache
fn capture_soundpack_loading_error(soundpack_id: &str, error: &str) {
    // Skip creating cache entries for empty soundpack IDs
    if soundpack_id.is_empty() {
        crate::always_print!("⚠️ Skipping cache entry for empty soundpack ID: {}", error);
        return;
    }

    crate::always_print!("📝 Capturing loading error for {}: {}", soundpack_id, error);

    let mut cache = SoundpackCache::load();

    // Check if we already have metadata for this soundpack
    if let Some(existing_metadata) = cache.soundpacks.get_mut(soundpack_id) {
        // Update existing metadata with error
        existing_metadata.last_error = Some(error.to_string());
        existing_metadata.validation_status = "loading_error".to_string();
    } else {
        // Create minimal metadata entry with error information
        let error_metadata = SoundpackMetadata {
            id: soundpack_id.to_string(),
            name: format!("Error: {}", soundpack_id),
            author: None,
            description: Some(format!("Loading failed: {}", error)),
            version: "unknown".to_string(),
            tags: vec!["error".to_string()],
            icon: None,
            soundpack_type: determine_soundpack_type(soundpack_id),
            folder_path: soundpack_id.to_string(), // Add folder_path for error entries
            last_modified: 0,
            last_accessed: std::time::SystemTime
                ::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            config_version: None,
            is_valid_v2: false,
            validation_status: "loading_error".to_string(),
            can_be_converted: false,
            last_error: Some(error.to_string()),
        };

        cache.soundpacks.insert(soundpack_id.to_string(), error_metadata);
    }

    cache.save();
    crate::always_print!("💾 Updated cache with error information for {}", soundpack_id);
}

#[cfg(test)]
mod tests {
    use super::{ mouse_button_for_definition, relative_soundpack_id };

    /// Joins with the running platform's separator. `Path::components` only
    /// splits on the native one, so a hard-coded `\` is a single component on
    /// Linux and the assertions below would be testing nothing there.
    fn native_path(parts: &[&str]) -> String {
        parts.join(std::path::MAIN_SEPARATOR_STR)
    }

    fn builtin_root() -> String {
        native_path(&["", "opt", "Mechvibes", "soundpacks"])
    }

    fn custom_root() -> String {
        native_path(&["", "home", "someone", ".local", "share", "mechvibes", "soundpacks"])
    }

    fn roots() -> Vec<String> {
        vec![builtin_root(), custom_root()]
    }

    #[test]
    fn an_imported_pack_gets_the_same_id_the_scanner_uses() {
        // Only the built-in root used to be tried, so a pack in app data fell
        // through to a bare folder name. The scanner keys the cache by
        // `mouse/Viper Mini`, so inserting `Viper Mini` alongside it left two
        // entries for one pack and the selector listed it twice. Built-in
        // packs matched the first root and never showed the bug.
        assert_eq!(
            relative_soundpack_id(
                &native_path(&[&custom_root(), "mouse", "Viper Mini"]),
                &roots()
            ),
            "mouse/Viper Mini"
        );
        assert_eq!(
            relative_soundpack_id(
                &native_path(&[&builtin_root(), "keyboard", "eg-oreo"]),
                &roots()
            ),
            "keyboard/eg-oreo"
        );
    }

    #[test]
    fn a_path_under_no_known_root_still_keeps_its_type_prefix() {
        // The fallback keeps two components rather than one, so even an
        // unexpected location cannot produce a prefix-less id.
        assert_eq!(
            relative_soundpack_id(
                &native_path(&["", "elsewhere", "mouse", "Model O"]),
                &roots()
            ),
            "mouse/Model O"
        );
    }

    #[test]
    fn v1_era_definition_keys_map_onto_real_mouse_buttons() {
        // Packs converted by an earlier build carry the keyboard names the old
        // converter produced from iohook codes 1/2/3. Left silent, they load
        // fine and never play - the whole of issue #38.
        assert_eq!(mouse_button_for_definition("Escape"), "MouseLeft");
        assert_eq!(mouse_button_for_definition("Digit1"), "MouseRight");
        assert_eq!(mouse_button_for_definition("Digit2"), "MouseMiddle");
    }

    #[test]
    fn v2_definition_keys_pass_through_unchanged() {
        for button in ["MouseLeft", "MouseRight", "MouseMiddle"] {
            assert_eq!(mouse_button_for_definition(button), button);
        }
    }

    fn multi_pack(json: &str) -> crate::state::soundpack::SoundPack {
        serde_json::from_str(json).expect("valid multi pack")
    }

    /// Two files of 100 ms and 50 ms at 1 kHz mono: the second starts at 100 ms.
    fn two_files() -> Vec<(String, super::DecodedAudio)> {
        vec![
            ("a.ogg".to_string(), (vec![0.1; 100], 1, 1000)),
            ("b.ogg".to_string(), (vec![0.2; 50], 1, 1000))
        ]
    }

    #[test]
    fn files_are_appended_and_each_one_records_where_it_starts() {
        let ((samples, channels, rate), offsets) = super::combine_decoded(two_files()).expect("combined");
        assert_eq!((samples.len(), channels, rate), (150, 1, 1000));
        assert_eq!(offsets["a.ogg"], 0.0);
        assert_eq!(offsets["b.ogg"], 100.0);
    }

    #[test]
    fn a_file_in_another_layout_takes_the_first_files_layout() {
        let files = vec![
            ("a.ogg".to_string(), (vec![0.1; 200], 2, 1000)),
            ("b.ogg".to_string(), (vec![0.2; 50], 1, 1000))
        ];
        let ((samples, channels, _), offsets) = super::combine_decoded(files).expect("combined");
        // 100 stereo frames, then 50 mono frames spread to stereo.
        assert_eq!((samples.len(), channels), (300, 2));
        assert_eq!(offsets["b.ogg"], 100.0);
    }

    #[test]
    fn nothing_to_combine_is_none() {
        assert!(super::combine_decoded(Vec::new()).is_none());
    }

    /// Timings in a multi pack are within each key's own file, so they move by
    /// where that file landed. Both segments move, so a key with a press and a
    /// release keeps both, and a key with one segment keeps one.
    #[test]
    fn each_key_moves_by_its_own_files_offset() {
        let mut pack = multi_pack(
            r#"{
  "id": "multi", "name": "Multi", "definition_method": "multi",
  "definitions": {
    "KeyA": { "audio_file": "a.ogg", "timing": [[0, 40]] },
    "KeyB": { "audio_file": "b.ogg", "timing": [[0, 20], [20, 50]] }
  }
}"#
        );
        let (_, offsets) = super::combine_decoded(two_files()).expect("combined");
        super::shift_multi_timings(&mut pack, &offsets);

        assert_eq!(pack.definitions["KeyA"].timing, vec![[0.0, 40.0]]);
        assert_eq!(pack.definitions["KeyB"].timing, vec![[100.0, 120.0], [120.0, 150.0]]);
    }

    #[test]
    fn a_key_without_its_own_file_uses_the_packs_file() {
        let mut pack = multi_pack(
            r#"{
  "id": "multi", "name": "Multi", "definition_method": "multi", "audio_file": "b.ogg",
  "definitions": { "Space": { "timing": [[5, 45]] } }
}"#
        );
        let (_, offsets) = super::combine_decoded(two_files()).expect("combined");
        super::shift_multi_timings(&mut pack, &offsets);

        assert_eq!(pack.definitions["Space"].timing, vec![[105.0, 145.0]]);
    }

    /// A key whose file did not load stays silent rather than playing a slice
    /// of whichever file happens to sit at that position.
    #[test]
    fn a_key_whose_file_did_not_load_is_dropped() {
        let mut pack = multi_pack(
            r#"{
  "id": "multi", "name": "Multi", "definition_method": "multi",
  "definitions": {
    "KeyA": { "audio_file": "a.ogg", "timing": [[0, 40]] },
    "KeyC": { "audio_file": "missing.ogg", "timing": [[0, 40]] }
  }
}"#
        );
        let (_, offsets) = super::combine_decoded(two_files()).expect("combined");
        super::shift_multi_timings(&mut pack, &offsets);

        assert!(pack.definitions.contains_key("KeyA"));
        assert!(!pack.definitions.contains_key("KeyC"));
    }

    fn write_silence_wav(path: &std::path::Path, sample_rate: u32, channels: u16, duration_ms: u32) {
        let spec = hound::WavSpec {
            channels,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).expect("create wav");
        for _ in 0..((sample_rate * duration_ms) / 1000) * (channels as u32) {
            writer.write_sample(0i16).expect("write sample");
        }
        writer.finalize().expect("finalize wav");
    }

    /// The whole path from disk: real files are decoded, combined and the
    /// timings moved, and the pack's config.json is not touched.
    #[test]
    fn a_multi_pack_on_disk_loads_without_being_rewritten() {
        let dir = std::env::temp_dir().join(format!("mechvibes-multi-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("pack dir");
        write_silence_wav(&dir.join("a.wav"), 44100, 2, 200);
        write_silence_wav(&dir.join("b.wav"), 22050, 1, 100);
        let config = r#"{
  "id": "multi", "name": "Multi", "definition_method": "multi",
  "definitions": {
    "KeyA": { "audio_file": "a.wav", "timing": [[0, 50]] },
    "KeyB": { "audio_file": "b.wav", "timing": [[10, 60]] }
  }
}"#;
        std::fs::write(dir.join("config.json"), config).expect("config");

        let mut pack = multi_pack(config);
        let (samples, channels, rate) = super::decode_multi_pack(dir.to_str().unwrap(), &mut pack).expect(
            "decode"
        );

        assert_eq!((channels, rate), (2, 44100));
        let ms = ((samples.len() as f64) / 2.0 / 44.1).round();
        assert_eq!(ms, 300.0, "200 ms of a.wav, then 100 ms of b.wav at the first file's rate");
        assert_eq!(pack.definitions["KeyB"].timing, vec![[210.0, 260.0]]);
        assert_eq!(std::fs::read_to_string(dir.join("config.json")).expect("config"), config);
    }
}
