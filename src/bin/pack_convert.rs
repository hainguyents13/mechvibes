//! Headless sound pack converter.
//!
//! `pack-convert <in.zip> <out.zip> [--mouse] [--author <name>]` runs the same conversion the app
//! performs when it loads an imported pack (`utils::soundpack`): a v1 config goes
//! through `convert_v1_to_v2`, and a v2 pack passes through as it is, whether it
//! plays from one file ("single") or one file per key ("multi"). The result is
//! zipped with `config.json` at the archive root.
//!
//! The converter modules are compiled into this binary with `#[path]` instead of
//! being linked from the app crate, so none of the Dioxus / windowing / input
//! code is built into or started by this tool. Only the modules below are shared:
//! the converter, the validator and the resampler, none of which touch the UI.
//!
//! stdout carries exactly one line of JSON on success. The converter's progress
//! logging goes to stderr and only when `PACK_CONVERT_VERBOSE` is set.
#![allow(dead_code)]

/// Replacement for the app's logging macros. The converter calls these for
/// progress output; here they go to stderr (opt-in) so stdout stays machine
/// readable.
#[macro_export]
macro_rules! always_print {
    ($($arg:tt)*) => {
        if std::env::var_os("PACK_CONVERT_VERBOSE").is_some() {
            eprintln!($($arg)*);
        }
    };
}

#[macro_export]
macro_rules! always_eprint {
    ($($arg:tt)*) => {
        if std::env::var_os("PACK_CONVERT_VERBOSE").is_some() {
            eprintln!($($arg)*);
        }
    };
}

// Declared at the crate root, where `#[path]` is resolved from `src/bin`, a
// folder that exists. Inside `mod utils { }` it would be resolved from
// `src/bin/utils`, which only Windows tolerates: Linux and macOS cannot step
// out of a folder that is not there with `..`.
#[path = "../utils/config_converter.rs"]
pub mod config_converter;
#[path = "../utils/soundpack_validator.rs"]
pub mod soundpack_validator;
#[path = "../libs/audio/resampler.rs"]
pub mod resampler;

/// Stands in for `utils::path`, which in the app pulls in the app data
/// directory resolution. The converter and validator only need this one. It
/// sits at the root because the converter reaches it as `super::path`.
pub mod path {
    pub fn read_file_contents(path: &str) -> Result<String, String> {
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read file '{}': {}", path, e))
    }
}

/// The app's module paths, which the shared files use to reach each other.
mod utils {
    pub use crate::{config_converter, path, soundpack_validator};
}

mod libs {
    pub mod audio {
        pub use crate::resampler;
    }
}

use serde_json::{json, Value};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use utils::config_converter::convert_v1_to_v2;
use utils::soundpack_validator::{validate_soundpack_config, SoundpackValidationStatus};

/// Upper bound on what one archive may expand to. Inputs are untrusted downloads.
const MAX_UNPACKED_BYTES: u64 = 1 << 30;

const USAGE: &str = "usage: pack-convert <in.zip> <out.zip> [--mouse] [--author <name>]";

fn main() {
    match run() {
        Ok(summary) => println!("{}", summary),
        Err(message) => {
            eprintln!("pack-convert: {}", message);
            std::process::exit(1);
        }
    }
}

fn run() -> Result<Value, String> {
    let mut positional = Vec::new();
    let mut is_mouse = false;
    let mut author: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--mouse" => is_mouse = true,
            "--author" => author = Some(args.next().ok_or("--author needs a value")?),
            _ => positional.push(arg),
        }
    }
    let [input, output] = positional.as_slice() else {
        return Err(USAGE.to_string());
    };

    let work = ScratchDir::new()?;
    extract(Path::new(input), work.path())?;
    let config_path = find_config(work.path())?;
    let pack_dir = config_path.parent().ok_or("config.json has no parent folder")?.to_path_buf();
    strip_bom(&config_path)?;

    let summary = convert(&config_path, is_mouse, author.as_deref())?;
    pack(&config_path, &pack_dir, Path::new(output))?;
    Ok(summary)
}

/// Run the app's conversion pipeline on `config_path` and describe what happened.
fn convert(
    config_path: &Path,
    is_mouse: bool,
    author: Option<&str>
) -> Result<Value, String> {
    let config_str = config_path.to_str().ok_or("path is not valid UTF-8")?;

    let validation = validate_soundpack_config(config_str);
    let (input_version, method) = match validation.status {
        SoundpackValidationStatus::VersionOneNeedsConversion => {
            let before = read_json(config_path)?;
            let method = before
                .get("key_define_type")
                .and_then(Value::as_str)
                .unwrap_or("single")
                .to_string();
            convert_v1_to_v2(config_str, config_str, None, is_mouse)
                .map_err(|e| format!("v1 to v2 conversion failed: {}", e))?;
            (1, method)
        }
        SoundpackValidationStatus::Valid => {
            let before = read_json(config_path)?;
            let method = before
                .get("definition_method")
                .and_then(Value::as_str)
                .unwrap_or("single")
                .to_string();
            (2, method)
        }
        _ => return Err(format!("not a convertible sound pack: {}", validation.message)),
    };

    // V1 configs have no author, so the converter leaves it out and the app's
    // validator then calls the result incomplete. The caller supplies it here.
    if let Some(author) = author {
        let mut config = read_json(config_path)?;
        if config.get("author").is_none() {
            config["author"] = Value::String(author.to_string());
            let text = serde_json::to_string_pretty(&config).map_err(|e| format!("cannot serialise config.json: {}", e))?;
            fs::write(config_path, text).map_err(|e| format!("cannot write config.json: {}", e))?;
        }
    }

    // Never emit a pack the app itself would refuse to load. A missing author is
    // the one tolerated gap: the importer writes it when it publishes the pack.
    let revalidation = validate_soundpack_config(config_str);
    let only_author_missing = matches!(
        &revalidation.status,
        SoundpackValidationStatus::MissingRequiredFields(fields) if fields.as_slice() == ["author"]
    );
    if only_author_missing {
        eprintln!("pack-convert: warning: output has no author; pass --author to set one");
    } else if revalidation.status != SoundpackValidationStatus::Valid {
        return Err(format!("converted pack failed validation: {}", revalidation.message));
    }

    let after = read_json(config_path)?;
    let files = audio_files(&after);
    // A single pack plays from its `audio_file`. A multi pack may have none,
    // since each key names its own, so it reports null here and lists every
    // file under `audio_files`.
    let audio = after.get("audio_file").and_then(Value::as_str);
    if files.is_empty() || (method != "multi" && audio.is_none()) {
        return Err("converted config has no audio_file".to_string());
    }
    let keys = after
        .get("definitions")
        .or_else(|| after.get("defs"))
        .and_then(Value::as_object)
        .map_or(0, |d| d.len());
    Ok(json!({
        "input_version": input_version,
        "method": method,
        "output_audio": audio,
        "audio_files": files,
        "keys": keys,
    }))
}

/// Every audio file a config plays from: its `audio_file`, and for a multi
/// pack the file each key names. Sorted, without repeats.
fn audio_files(config: &Value) -> Vec<&str> {
    let mut files: Vec<&str> = config.get("audio_file").and_then(Value::as_str).into_iter().collect();
    if let Some(definitions) = config.get("definitions").and_then(Value::as_object) {
        files.extend(definitions.values().filter_map(|d| d.get("audio_file").and_then(Value::as_str)));
    }
    files.sort_unstable();
    files.dedup();
    files
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("cannot read config.json: {}", e))?;
    serde_json::from_str(&text).map_err(|e| format!("invalid config.json: {}", e))
}

/// Legacy configs written by Windows editors sometimes start with a UTF-8 BOM,
/// which `serde_json` rejects.
fn strip_bom(config_path: &Path) -> Result<(), String> {
    let bytes = fs::read(config_path).map_err(|e| format!("cannot read config.json: {}", e))?;
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        fs::write(config_path, rest).map_err(|e| format!("cannot rewrite config.json: {}", e))?;
    }
    Ok(())
}

/// A relative path made only of ordinary components, or `None` for anything that
/// could escape the extraction folder or that is archive noise.
fn safe_relative(entry_name: &str) -> Option<PathBuf> {
    let normalised = entry_name.replace('\\', "/");
    let mut out = PathBuf::new();
    for part in Path::new(&normalised).components() {
        match part {
            Component::Normal(p) => {
                if p == "__MACOSX" {
                    return None;
                }
                out.push(p);
            }
            Component::CurDir => {}
            _ => return None,
        }
    }
    let file = out.file_name()?.to_str()?;
    if file == ".DS_Store" || file.starts_with("._") {
        return None;
    }
    Some(out)
}

fn extract(zip_path: &Path, dest: &Path) -> Result<(), String> {
    let file = File::open(zip_path).map_err(|e| format!("cannot open {}: {}", zip_path.display(), e))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("not a valid zip: {}", e))?;

    let declared: u64 = (0..archive.len())
        .filter_map(|i| archive.by_index_raw(i).ok().map(|f| f.size()))
        .sum();
    if declared > MAX_UNPACKED_BYTES {
        return Err(format!("archive expands to {} bytes, over the {} limit", declared, MAX_UNPACKED_BYTES));
    }

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| format!("cannot read zip entry {}: {}", i, e))?;
        if entry.is_dir() {
            continue;
        }
        let Some(relative) = safe_relative(entry.name()) else {
            continue;
        };
        let target = dest.join(&relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("cannot create folder: {}", e))?;
        }
        let mut out = File::create(&target).map_err(|e| format!("cannot create {}: {}", target.display(), e))?;
        io::copy(&mut entry.by_ref().take(MAX_UNPACKED_BYTES), &mut out)
            .map_err(|e| format!("cannot extract {}: {}", relative.display(), e))?;
    }
    Ok(())
}

/// The shallowest `config.json`; ties go to the lexicographically first path so
/// the choice is deterministic.
fn find_config(root: &Path) -> Result<PathBuf, String> {
    let mut found: Vec<PathBuf> = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).map_err(|e| format!("cannot list {}: {}", dir.display(), e))? {
            let path = entry.map_err(|e| format!("cannot list folder: {}", e))?.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().is_some_and(|n| n.eq_ignore_ascii_case("config.json")) {
                found.push(path);
            }
        }
    }
    found.sort_by_key(|p| (p.components().count(), p.clone()));
    found.into_iter().next().ok_or_else(|| "no config.json found in the archive".to_string())
}

/// Write the output zip: `config.json` at the root, plus the audio files and
/// icon the converted config references. For a multi pack that is the file
/// each key names, as well as the pack's own `audio_file` if it has one.
fn pack(config_path: &Path, pack_dir: &Path, output: &Path) -> Result<(), String> {
    let config = read_json(config_path)?;
    let mut members: Vec<(String, PathBuf)> = vec![("config.json".to_string(), config_path.to_path_buf())];

    // Every audio file has to be there; the icon is optional.
    let icon = config.get("icon").and_then(Value::as_str);
    for (name, required) in audio_files(&config).into_iter().map(|n| (n, true)).chain(icon.map(|n| (n, false))) {
        let relative = safe_relative(name.trim_start_matches("./"))
            .ok_or_else(|| format!("'{}' is not a safe relative path", name))?;
        let source = pack_dir.join(&relative);
        if source.is_file() {
            members.push((relative.to_string_lossy().replace('\\', "/"), source));
        } else if required {
            return Err(format!("audio file '{}' is missing from the pack", name));
        }
    }

    let out_file = File::create(output).map_err(|e| format!("cannot create {}: {}", output.display(), e))?;
    let mut writer = zip::ZipWriter::new(out_file);
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, source) in members {
        writer.start_file(name, options).map_err(|e| format!("cannot write zip: {}", e))?;
        let mut reader = File::open(&source).map_err(|e| format!("cannot read {}: {}", source.display(), e))?;
        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer).map_err(|e| format!("cannot read {}: {}", source.display(), e))?;
        writer.write_all(&buffer).map_err(|e| format!("cannot write zip: {}", e))?;
    }
    writer.finish().map_err(|e| format!("cannot finish zip: {}", e))?;
    Ok(())
}

/// A temp folder removed on drop, so failed runs leave nothing behind.
struct ScratchDir(PathBuf);

impl ScratchDir {
    fn new() -> Result<Self, String> {
        let path = std::env::temp_dir().join(format!("pack-convert-{}-{}", std::process::id(), uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).map_err(|e| format!("cannot create temp folder: {}", e))?;
        Ok(Self(path))
    }
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pack-convert-test-{}-{}", tag, uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    fn write_wav(path: &Path) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 8000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).expect("wav");
        for i in 0..800 {
            writer.write_sample((i % 100) as i16).expect("sample");
        }
        writer.finalize().expect("finalize");
    }

    fn zip_dir(zip_path: &Path, files: &[(&str, &Path)]) {
        let mut writer = zip::ZipWriter::new(File::create(zip_path).expect("zip"));
        let options = zip::write::SimpleFileOptions::default();
        for (name, source) in files {
            writer.start_file(*name, options).expect("entry");
            writer.write_all(&fs::read(source).expect("read")).expect("write");
        }
        writer.finish().expect("finish");
    }

    #[test]
    fn unsafe_and_noise_entries_are_dropped() {
        assert_eq!(safe_relative("a/b.txt"), Some(PathBuf::from("a").join("b.txt")));
        assert_eq!(safe_relative(r"a\b.txt"), Some(PathBuf::from("a").join("b.txt")));
        assert_eq!(safe_relative("../evil.txt"), None);
        assert_eq!(safe_relative("/abs.txt"), None);
        assert_eq!(safe_relative("__MACOSX/pack/config.json"), None);
        assert_eq!(safe_relative("pack/.DS_Store"), None);
        assert_eq!(safe_relative("pack/._sound.ogg"), None);
    }

    #[test]
    fn the_shallowest_config_wins() {
        let root = scratch("find");
        fs::create_dir_all(root.join("a/b")).expect("dirs");
        fs::write(root.join("a/b/config.json"), "{}").expect("deep");
        fs::write(root.join("a/config.json"), "{}").expect("shallow");
        assert_eq!(find_config(&root).expect("found"), root.join("a/config.json"));
    }

    #[test]
    fn a_v1_single_pack_in_a_subfolder_converts_to_a_root_level_v2_zip() {
        let dir = scratch("e2e");
        let wav = dir.join("sound.wav");
        write_wav(&wav);
        let config = dir.join("config.json");
        fs::write(
            &config,
            r#"{"id":"x","name":"T","key_define_type":"single","sound":"sound.wav","defines":{"1":[0,50],"30":[50,100]}}"#,
        )
        .expect("config");
        let input = dir.join("in.zip");
        zip_dir(&input, &[("pack/config.json", &config), ("pack/sound.wav", &wav)]);

        let work = ScratchDir::new().expect("work");
        extract(&input, work.path()).expect("extract");
        let config_path = find_config(work.path()).expect("config");
        let pack_dir = config_path.parent().expect("parent").to_path_buf();
        let summary = convert(&config_path, false, Some("Tester")).expect("convert");
        assert_eq!(summary["input_version"], 1);
        assert_eq!(summary["method"], "single");
        assert_eq!(summary["output_audio"], "sound.wav");

        let output = dir.join("out.zip");
        pack(&config_path, &pack_dir, &output).expect("pack");
        let mut archive = zip::ZipArchive::new(File::open(&output).expect("open")).expect("zip");
        let mut names: Vec<String> = (0..archive.len()).map(|i| archive.by_index(i).expect("entry").name().to_string()).collect();
        names.sort();
        assert_eq!(names, ["config.json", "sound.wav"]);
        let mut text = String::new();
        archive.by_name("config.json").expect("config").read_to_string(&mut text).expect("read");
        let out: Value = serde_json::from_str(&text).expect("json");
        assert_eq!(out["config_version"], "2");
        assert_eq!(out["author"], "Tester");
    }

    #[test]
    fn a_config_that_is_not_a_pack_is_refused() {
        let dir = scratch("refuse");
        let config = dir.join("config.json");
        fs::write(&config, r#"{"name":"nothing"}"#).expect("config");
        assert!(convert(&config, false, None).is_err());
    }

    /// A v2 multi pack is left as it is, as the app now plays it, and the zip
    /// carries the file every key names.
    #[test]
    fn a_v2_multi_pack_passes_through_with_every_keys_file() {
        let dir = scratch("multi");
        let a = dir.join("a.wav");
        let b = dir.join("b.wav");
        write_wav(&a);
        write_wav(&b);
        let config = dir.join("config.json");
        let original = r#"{"id":"m","name":"M","author":"A","config_version":"2","definition_method":"multi","definitions":{"KeyA":{"audio_file":"a.wav","timing":[[0,50]]},"KeyB":{"audio_file":"b.wav","timing":[[0,50]]}}}"#;
        fs::write(&config, original).expect("config");

        let summary = convert(&config, false, None).expect("convert");
        assert_eq!(summary["input_version"], 2);
        assert_eq!(summary["method"], "multi");
        assert_eq!(summary["audio_files"], json!(["a.wav", "b.wav"]));

        let output = dir.join("out.zip");
        pack(&config, &dir, &output).expect("pack");
        let mut archive = zip::ZipArchive::new(File::open(&output).expect("open")).expect("zip");
        let mut names: Vec<String> = (0..archive.len()).map(|i| archive.by_index(i).expect("entry").name().to_string()).collect();
        names.sort();
        assert_eq!(names, ["a.wav", "b.wav", "config.json"]);
        let mut text = String::new();
        archive.by_name("config.json").expect("config").read_to_string(&mut text).expect("read");
        let out: Value = serde_json::from_str(&text).expect("json");
        assert_eq!(out["definition_method"], "multi");
        assert_eq!(out["definitions"]["KeyB"]["audio_file"], "b.wav");
    }
}
