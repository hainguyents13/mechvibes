//! The one place `config.json` is written.
//!
//! # Why this module exists
//!
//! `AppConfig::save()` serialises the *whole* struct. Any writer that read a
//! config, did something slow, and then saved that copy silently reverted every
//! field another subsystem changed in between. That defect shipped three times
//! from three unrelated call sites (the startup update check, the long-lived
//! `UpdateService` snapshot, and a debounced volume write), and each was patched
//! on its own. Patching writers one at a time cannot work: the mistake is
//! available to every new writer by default.
//!
//! So the *shape* is what changed. `save()` is no longer reachable from outside
//! `crate::state`; the only way to write is [`apply`], which takes a mutation
//! and runs it against state this module owns. Holding an `AppConfig` across
//! time and writing it back is not a discouraged pattern here - there is no API
//! that accepts one, so it cannot be expressed.
//!
//! # What a mutation may assume
//!
//! The closure gets `&mut AppConfig` holding the newest known state, and should
//! touch only the fields it is responsible for. Two subsystems mutating
//! different fields compose: each runs against the result of the other, so
//! neither can revert the other's work. That is the merge semantics the old
//! read-modify-save pattern lacked.
//!
//! # Threading
//!
//! Writers live on the Dioxus UI thread, the audio engine thread, tokio tasks
//! and the tray callback. A `Mutex` authority rather than a writer thread is
//! deliberate: every existing writer is synchronous and several read the result
//! back immediately (the Ctrl+Alt+M handler needs the toggled value to move the
//! engine's own cached flag in the same call). A channel would make all of them
//! async and need a reply path per call site; the mutex gives the same
//! serialisation with none of that. Contention is nil in practice - writes are
//! user-driven, not per-keystroke.

use crate::state::config::AppConfig;
use std::sync::atomic::{ AtomicU64, Ordering };
use std::sync::{ Mutex, OnceLock };

/// The authoritative config. Loaded from disk once, on first use, and from then
/// on this - not the file - is what a mutation is applied to.
///
/// Re-reading the file before every mutation would also be correct, since this
/// process is its only writer, but it would put a synchronous parse on the UI
/// thread for every slider tick. Owning the state instead keeps the disk read
/// to one.
/// Nothing reachable from `AppConfig::load()` may call back into this module.
/// `load()` runs during the `get_or_init` below, so a re-entrant `current()`
/// would deadlock on the initialising `OnceLock`. The one thing `load()` calls
/// out to is `auto_startup::get_auto_startup_state`, which reads the Windows
/// registry (or, on macOS, the login-item service) and never touches the config - keep it that way.
static AUTHORITY: OnceLock<Mutex<AppConfig>> = OnceLock::new();

/// Bumped on every write that actually changed something.
///
/// This is how a mutation made off the UI thread reaches the window. A callback
/// cannot do it: the shared `Signal<AppConfig>` uses Dioxus's default
/// `UnsyncStorage` and is therefore not `Send`, so no closure registered here
/// could legally touch it. A counter can be read from anywhere, and the UI's
/// existing poll loop (`libs/ui.rs`, the one already draining `UiEvent`s)
/// compares it once per tick and republishes when it moved.
static GENERATION: AtomicU64 = AtomicU64::new(0);

fn authority() -> &'static Mutex<AppConfig> {
    AUTHORITY.get_or_init(|| Mutex::new(AppConfig::load()))
}

/// A snapshot of the current config.
///
/// Read-only by construction: writing it back is impossible, because no
/// function in this crate accepts an `AppConfig` to persist. Use it to read,
/// then use [`apply`] to change something.
pub fn current() -> AppConfig {
    // A poisoned lock means a mutation closure panicked mid-write. The config
    // is still a valid struct (the panic happened between field assignments at
    // worst), and refusing to serve settings would take the app down with it,
    // so the value is recovered rather than propagated.
    match authority().lock() {
        Ok(guard) => guard.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    }
}

/// Apply `mutate` to the authoritative config and persist the result.
///
/// The mutation sees the newest state, so a concurrent change from another
/// subsystem is already present and survives. Returns whether anything was
/// written.
///
/// A mutation that leaves the data unchanged is not persisted. Mount effects
/// re-assert the value they already hold on every navigation, and writing those
/// unconditionally turned navigation into a continuous rewrite of the file.
pub fn apply(mutate: impl FnOnce(&mut AppConfig)) -> bool {
    let changed = {
        let mut guard = match authority().lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        let before = guard.clone();
        mutate(&mut guard);

        let changed = !guard.data_equals(&before);
        if changed {
            guard.last_updated = chrono::Utc::now();
            if let Err(e) = guard.save() {
                crate::always_eprint!("❌ [config] Failed to save config: {}", e);
                // The change stays in memory for this session, but the user
                // has to know it will not be there next time.
                crate::utils::notice::report(
                    crate::utils::notice::NoticeKind::Error,
                    format!("Could not save your changes. {}", e),
                    None
                );
            }
        } else {
            // Undo a metadata-only edit so the in-memory struct cannot drift
            // from the file - `data_equals` ignores those fields, so they would
            // otherwise never be written.
            *guard = before;
        }

        changed
    };

    if changed {
        // Release ordering pairs with the `Acquire` load in `generation`: a
        // reader that sees the new number is guaranteed to see the write that
        // produced it, so it cannot republish a config older than the bump it
        // reacted to.
        GENERATION.fetch_add(1, Ordering::Release);
    }

    changed
}

/// How many times the config has changed since the process started.
///
/// The UI polls this and republishes [`current`] into the shared signal when it
/// moves, which is what lets a write made on the audio engine thread (Ctrl+Alt+M)
/// or in a tokio task re-render the window. Before this existed, the hotkey
/// muted the app while the window kept rendering the unmuted icon until some
/// unrelated UI write happened to republish.
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Acquire)
}

/// Forces the saved `config.json` (and the rename that put it there) onto disk.
///
/// `apply` writes through `save_json_to_file_atomically`, which writes a temp
/// file and renames it over the target but never syncs, so the new bytes can sit
/// in the OS cache for a while. That is fine for a slider, but not for the
/// start-at-login preferences: they are read on the very next launch, which is
/// often right after a logout or reboot.
///
/// - The file is synced on every OS. It is opened for write (without
///   truncating) because Windows' `FlushFileBuffers` needs a writable handle.
///   On macOS `File::sync_all` issues `F_FULLFSYNC`, which is what actually
///   reaches the disk.
/// - The parent directory is synced too, on Unix only, because that is where
///   the rename lives. A directory cannot be opened this way on Windows.
///
/// Best effort: a failure is logged, never fatal.
pub fn flush_to_disk() {
    let path = crate::state::paths::data::config_json();
    if let Err(e) = flush_path(&path) {
        crate::always_eprint!("❌ [config] Failed to flush {} to disk: {}", path.display(), e);
    }
}

fn flush_path(path: &std::path::Path) -> std::io::Result<()> {
    std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .and_then(|file| file.sync_all())
        .and_then(|_| sync_parent_dir(path))
}

#[cfg(unix)]
fn sync_parent_dir(path: &std::path::Path) -> std::io::Result<()> {
    match path.parent() {
        Some(dir) => std::fs::File::open(dir)?.sync_all(),
        None => Ok(()),
    }
}

#[cfg(not(unix))]
fn sync_parent_dir(_path: &std::path::Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flush_path_syncs_an_existing_file_without_changing_it() {
        let path = std::env::temp_dir().join(format!("mv-flush-test-{}.json", std::process::id()));
        std::fs::write(&path, b"{\"a\":1}").unwrap();
        assert!(flush_path(&path).is_ok());
        assert_eq!(std::fs::read(&path).unwrap(), b"{\"a\":1}");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn flush_path_reports_a_missing_file_instead_of_creating_it() {
        let path = std::env::temp_dir().join(format!("mv-flush-missing-{}.json", std::process::id()));
        assert!(flush_path(&path).is_err());
        assert!(!path.exists());
    }
    use crate::libs::theme::{ BuiltInTheme, Theme };

    /// The authority is process-global and backed by the real config file, so
    /// these tests drive an equivalent local one. What is being proven is the
    /// merge rule - a mutation runs against the newest state - which is a
    /// property of `apply`'s body, not of where the state is stored.
    struct Authority {
        config: AppConfig,
        writes: usize,
    }

    impl Authority {
        fn new() -> Self {
            Self { config: AppConfig::default(), writes: 0 }
        }

        /// Mirrors `apply`: mutate the live state, persist only a real change.
        fn apply(&mut self, mutate: impl FnOnce(&mut AppConfig)) -> bool {
            let before = self.config.clone();
            mutate(&mut self.config);
            let changed = !self.config.data_equals(&before);
            if changed {
                self.writes += 1;
            } else {
                self.config = before;
            }
            changed
        }
    }

    /// The composition property the whole refactor exists for: two subsystems
    /// mutating different fields must both survive, in either order, with no
    /// re-read discipline required of either caller.
    #[test]
    fn mutations_from_different_subsystems_compose() {
        let mut authority = Authority::new();

        // The UI thread changes the theme.
        authority.apply(|config| {
            config.theme = Theme::BuiltIn(BuiltInTheme::Dark);
        });
        // The audio engine thread mutes, knowing nothing about the theme.
        authority.apply(|config| {
            config.enable_sound = false;
        });
        // A tokio task records an update check, knowing nothing about either.
        authority.apply(|config| {
            config.auto_update.last_check = Some(1_700_000_000);
        });

        assert_eq!(
            authority.config.theme,
            Theme::BuiltIn(BuiltInTheme::Dark),
            "the UI's field must survive the two later writers"
        );
        assert!(!authority.config.enable_sound, "the engine's field must survive the tokio write");
        assert_eq!(
            authority.config.auto_update.last_check,
            Some(1_700_000_000),
            "and the tokio task's own field must be recorded"
        );
    }

    /// The startup-update-check shape: a writer that begins its work, is slow
    /// (a network round trip), and finishes after the user has changed
    /// something. Under the old read-modify-save API the pre-request struct was
    /// written back. A mutation cannot carry state across that gap, so the late
    /// write touches only its own field.
    #[test]
    fn a_slow_writer_finishing_late_does_not_revert_a_concurrent_change() {
        let mut authority = Authority::new();

        // The check starts here. All it may carry across the await is the value
        // it intends to write - there is no config to hold.
        let recorded_check_time = 1_700_000_000;

        // While the request is in flight the user drags the volume slider and
        // picks a theme.
        authority.apply(|config| {
            config.volume = 0.35;
        });
        authority.apply(|config| {
            config.theme = Theme::BuiltIn(BuiltInTheme::Dark);
        });

        // The request returns and the check writes its result.
        authority.apply(|config| {
            config.auto_update.last_check = Some(recorded_check_time);
        });

        assert_eq!(authority.config.volume, 0.35, "the volume set during the check must survive");
        assert_eq!(
            authority.config.theme,
            Theme::BuiltIn(BuiltInTheme::Dark),
            "and so must the theme"
        );
        assert_eq!(
            authority.config.auto_update.last_check,
            Some(recorded_check_time),
            "while the check still records its own result"
        );
    }

    /// The `UpdateService` shape: a writer that lives for the whole session and
    /// writes repeatedly. Its first write must not restore the state the app
    /// launched with, and neither must its tenth.
    #[test]
    fn a_long_lived_writer_does_not_restore_launch_state() {
        let mut authority = Authority::new();
        let launch_volume = authority.config.volume;

        // The service starts at launch. It captures no config.
        // The user immediately changes the volume.
        authority.apply(|config| {
            config.volume = 0.88;
        });
        assert_ne!(launch_volume, 0.88, "the test must actually move the value");

        // Every periodic tick for the rest of the session.
        for tick in 0..10 {
            authority.apply(|config| {
                config.auto_update.last_check = Some(1_700_000_000 + tick);
            });
            assert_eq!(
                authority.config.volume,
                0.88,
                "tick {tick} must not restore the launch-time volume"
            );
        }
    }

    /// The debounce shape: a deferred write that lands after the user has moved
    /// on. It may only carry the value it means to write, so whatever else
    /// changed meanwhile is untouched.
    #[test]
    fn a_deferred_write_landing_late_touches_only_its_own_field() {
        let mut authority = Authority::new();

        // Slider drag: the value is published now, and a deferred write of the
        // same value is scheduled.
        let debounced_volume = 0.2;
        authority.apply(|config| {
            config.volume = debounced_volume;
        });

        // The user navigates away and mutes before the timer fires.
        authority.apply(|config| {
            config.enable_sound = false;
        });

        // The deferred write finally lands.
        let wrote = authority.apply(|config| {
            config.volume = debounced_volume;
        });

        assert!(!wrote, "re-writing an already-current value must not touch the disk");
        assert!(!authority.config.enable_sound, "and must not revert the mute");
        assert_eq!(authority.config.volume, debounced_volume, "while the volume still holds");
    }

    /// Disk churn is a real regression risk: mount effects re-assert their
    /// current value on every navigation. Only genuine changes may reach the
    /// file.
    #[test]
    fn re_asserting_the_current_value_writes_nothing() {
        let mut authority = Authority::new();
        let current_volume = authority.config.volume;

        for _ in 0..20 {
            let wrote = authority.apply(|config| {
                config.volume = current_volume;
            });
            assert!(!wrote, "a no-op mutation must not be persisted");
        }

        assert_eq!(authority.writes, 0, "twenty navigations must produce zero writes");

        let wrote = authority.apply(|config| {
            config.volume = current_volume + 0.25;
        });
        assert!(wrote, "a real change must still be written");
        assert_eq!(authority.writes, 1, "exactly one write for one real change");
    }

    /// Serialises the tests that drive the *real* authority.
    ///
    /// They each snapshot the config, mutate it, and restore it. The authority
    /// is process-global and the test harness runs threads in parallel, so two
    /// of them overlapping would have one restore its snapshot over the other's
    /// assertions - a contention between the tests, not a defect in `apply`.
    static REAL_AUTHORITY_TESTS: Mutex<()> = Mutex::new(());

    fn lock_real_authority() -> std::sync::MutexGuard<'static, ()> {
        match REAL_AUTHORITY_TESTS.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    /// Drives the **real** `apply` from several OS threads at once - the real
    /// `Mutex` authority, the real atomic write, the real file - rather than
    /// the local model above.
    ///
    /// Every writer in the app lives on one of these: the Dioxus UI thread, the
    /// audio engine thread, a tokio task, or the tray callback. Each is a plain
    /// OS thread here mutating the one field that subsystem owns. Under the old
    /// read-modify-save pattern, whichever thread finished last would restore
    /// the fields the others had written; all of them must now survive.
    ///
    /// The config is captured before and restored after, so running the suite
    /// leaves the developer's settings exactly as they were.
    #[test]
    fn real_writers_on_different_threads_all_survive() {
        use std::sync::{ Arc, Barrier };

        let _serialised = lock_real_authority();
        let original = current();

        let threads = 8;
        let iterations = 40;
        let barrier = Arc::new(Barrier::new(threads));

        let handles: Vec<_> = (0..threads)
            .map(|id| {
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    // Start together so the writes genuinely overlap rather
                    // than queueing up behind one another.
                    barrier.wait();

                    for round in 0..iterations {
                        match id {
                            0 =>
                                apply(|config| {
                                    config.volume = 0.5;
                                }),
                            1 =>
                                apply(|config| {
                                    config.mouse_volume = 0.25;
                                }),
                            2 =>
                                apply(|config| {
                                    config.enable_sound = false;
                                }),
                            3 =>
                                apply(|config| {
                                    config.landscape_mode = true;
                                }),
                            4 =>
                                apply(|config| {
                                    config.auto_update.last_check = Some(1_700_000_000 + round);
                                }),
                            5 =>
                                apply(|config| {
                                    config.custom_css = "body{}".to_string();
                                }),
                            6 =>
                                apply(|config| {
                                    config.enable_volume_boost = true;
                                }),
                            _ =>
                                apply(|config| {
                                    config.start_minimized = true;
                                }),
                        };
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("no writer thread may panic");
        }

        let final_config = current();

        // Restore before asserting, so a failure still leaves the machine
        // clean.
        apply(|config| {
            *config = original;
        });

        assert_eq!(final_config.volume, 0.5, "the volume writer's field must survive");
        assert_eq!(final_config.mouse_volume, 0.25, "and the mouse volume writer's");
        assert!(!final_config.enable_sound, "and the mute writer's");
        assert!(final_config.landscape_mode, "and the layout writer's");
        assert_eq!(
            final_config.auto_update.last_check,
            Some(1_700_000_000 + (iterations - 1)),
            "and the update checker's"
        );
        assert_eq!(final_config.custom_css, "body{}", "and the customization writer's");
        assert!(final_config.enable_volume_boost, "and the volume-boost writer's");
        assert!(final_config.start_minimized, "and the startup writer's");
    }

    /// Every write goes through a temp file and a rename, so a reader that
    /// catches the file mid-save sees either the old document or the new one -
    /// never a truncated one. A truncated `config.json` is what routes the load
    /// path into the `.corrupt` rescue, costing the user every setting.
    #[test]
    fn a_concurrent_reader_never_observes_a_partial_document() {
        use std::sync::atomic::{ AtomicBool, Ordering as AtomicOrdering };
        use std::sync::Arc;

        let _serialised = lock_real_authority();
        let original = current();
        let path = crate::state::paths::data::config_json();
        let stop = Arc::new(AtomicBool::new(false));

        let reader = {
            let stop = Arc::clone(&stop);
            let path = path.clone();
            std::thread::spawn(move || {
                let mut partial_reads = 0usize;
                let mut successful_reads = 0usize;
                while !stop.load(AtomicOrdering::Relaxed) {
                    // A missing file is the instant between temp-write and
                    // rename on some platforms; only a *present but broken*
                    // document is the failure this guards against.
                    if let Ok(contents) = std::fs::read_to_string(&path) {
                        if crate::state::config::parse_lenient(&contents).is_ok() {
                            successful_reads += 1;
                        } else {
                            partial_reads += 1;
                        }
                    }
                }
                (successful_reads, partial_reads)
            })
        };

        for round in 0..200 {
            apply(|config| {
                config.auto_update.last_check = Some(1_700_000_000 + round);
            });
        }

        stop.store(true, AtomicOrdering::Relaxed);
        let (successful_reads, partial_reads) = reader.join().expect("reader must not panic");

        apply(|config| {
            *config = original;
        });

        assert!(successful_reads > 0, "the reader must actually have observed the file");
        assert_eq!(
            partial_reads,
            0,
            "a half-written config was observed {partial_reads} times - \
             the write is not atomic"
        );
    }

    /// A mutation that only bumps metadata is indistinguishable from a no-op to
    /// `data_equals`, so it must not leave the in-memory struct carrying a
    /// timestamp the file does not have.
    #[test]
    fn a_metadata_only_mutation_does_not_desync_memory_from_disk() {
        let mut authority = Authority::new();
        let before = authority.config.clone();

        let wrote = authority.apply(|config| {
            config.last_updated = chrono::Utc::now() + chrono::Duration::hours(1);
        });

        assert!(!wrote, "a metadata-only edit is not a change worth writing");
        assert_eq!(
            authority.config.last_updated,
            before.last_updated,
            "and must be rolled back rather than left only in memory"
        );
    }
}
