// Event-driven App State Manager
use crate::state::soundpack::SoundpackCache;
use crate::{ debug_print, always_eprint };
use dioxus::prelude::*;
use once_cell::sync::OnceCell;
use std::sync::{ Arc, Mutex };

// Global app state for sharing between components
#[derive(Clone, Debug)]
pub struct AppState {
    pub optimized_cache: Arc<SoundpackCache>,
    pub last_updated: std::time::Instant,
}

impl PartialEq for AppState {
    fn eq(&self, other: &Self) -> bool {
        // Compare cache contents, ignoring timestamp for reactivity purposes
        Arc::ptr_eq(&self.optimized_cache, &other.optimized_cache)
    }
}

impl AppState {
    pub fn new() -> Self {
        debug_print!("🌍 Initializing global AppState...");
        Self {
            optimized_cache: Arc::new(SoundpackCache::load()),
            last_updated: std::time::Instant::now(),
        }
    }

    pub fn get_soundpacks(&self) -> Vec<crate::state::soundpack::SoundpackMetadata> {
        self.optimized_cache.soundpacks.values().cloned().collect()
    }
    pub fn refresh_cache(&mut self) {
        debug_print!("🔄 Refreshing soundpack cache...");
        let mut fresh_cache = SoundpackCache::load();
        fresh_cache.refresh_from_directory();
        fresh_cache.save();
        self.optimized_cache = Arc::new(fresh_cache);
        self.last_updated = std::time::Instant::now();
    }
}

// Global state instance
static GLOBAL_APP_STATE: OnceCell<Mutex<AppState>> = OnceCell::new();

/// The cached soundpack list, for code outside components (the tray menu).
pub fn soundpacks() -> Vec<crate::state::soundpack::SoundpackMetadata> {
    GLOBAL_APP_STATE.get()
        .and_then(|state| state.lock().ok())
        .map(|state| state.get_soundpacks())
        .unwrap_or_default()
}

// Simple hook for read-only access
pub fn use_app_state() -> AppState {
    let update_signal: Signal<u32> = use_context();

    let app_state = use_memo(move || {
        let _ = update_signal();
        // Subscribe to changes
        if let Some(global_state) = GLOBAL_APP_STATE.get() {
            if let Ok(state) = global_state.lock() {
                return state.clone();
            }
        }
        AppState::new()
    });

    let result = app_state.read().clone();
    result
}

// Hook to trigger state updates
pub fn use_state_trigger() -> Callback<()> {
    let mut update_signal: Signal<u32> = use_context();
    use_callback(move |_| {
        // Refresh cache and trigger UI update
        if let Some(global_state) = GLOBAL_APP_STATE.get() {
            if let Ok(mut state) = global_state.lock() {
                crate::always_print!("🔄 Triggering cache refresh...");
                state.refresh_cache();
            }
        }
        // Trigger UI update by incrementing the signal value
        let current_value = {
            let val = update_signal.read();
            *val
        };
        update_signal.set(current_value + 1);
    })
}

// Reload the current soundpacks from configuration
pub fn reload_current_soundpacks(audio_ctx: &crate::libs::audio::AudioContext) {
    let config = crate::state::config_writer::current();

    // Track if we need to update config (only for truly missing soundpacks)
    // Don't clear config for temporary loading failures to prevent infinite loops

    // Load keyboard soundpack (don't clear on error to prevent infinite reload loop)
    if !config.keyboard_soundpack.is_empty() {
        match
            crate::libs::audio::soundpack_loader::load_keyboard_soundpack_with_cache_control(
                audio_ctx,
                &config.keyboard_soundpack,
                false
            )
        {
            Ok(_) =>
                debug_print!(
                    "✅ Keyboard soundpack '{}' reloaded successfully",
                    config.keyboard_soundpack
                ),
            Err(e) => {
                // Log error but don't clear config to prevent infinite loop
                always_eprint!(
                    "❌ Failed to reload keyboard soundpack '{}': {}",
                    config.keyboard_soundpack,
                    e
                );
                always_eprint!("   Keeping current selection to prevent infinite reload loop");
            }
        }
    }

    // Load mouse soundpack (don't clear on error to prevent infinite reload loop)
    if !config.mouse_soundpack.is_empty() {
        match
            crate::libs::audio::soundpack_loader::load_mouse_soundpack_with_cache_control(
                audio_ctx,
                &config.mouse_soundpack,
                false
            )
        {
            Ok(_) =>
                debug_print!("✅ Mouse soundpack '{}' reloaded successfully", config.mouse_soundpack),
            Err(e) => {
                // Log error but don't clear config to prevent infinite loop
                always_eprint!(
                    "❌ Failed to reload mouse soundpack '{}': {}",
                    config.mouse_soundpack,
                    e
                );
                always_eprint!("   Keeping current selection to prevent infinite reload loop");
            }
        }
    }
}

// Initialize the app state - call this once at startup
pub fn init_app_state() {
    if GLOBAL_APP_STATE.get().is_none() {
        crate::always_print!("📝 Initializing global app state (mutex)...");
        let state = AppState::new();
        record_known_soundpacks(&state);
        let _ = GLOBAL_APP_STATE.set(Mutex::new(state));
    }
}

/// Gives every pack in the library an "added at" timestamp, so the lists can
/// sort by it and new arrivals can be told apart from the rest.
///
/// On the first run after upgrading this stamps the whole library at once and
/// marks it seen, so nobody opens the app to find every pack badged as new.
/// Packs that appear later - imported, or dropped into the folder by hand -
/// are stamped by the same path but keep their badge.
fn record_known_soundpacks(state: &AppState) {
    let folder_paths: Vec<String> = state.optimized_cache.soundpacks
        .values()
        .map(|pack| pack.folder_path.clone())
        .filter(|path| !path.is_empty())
        .collect();

    if folder_paths.is_empty() {
        return;
    }

    crate::state::config_writer::apply(|config| {
        let first_run = config.soundpack_added_at.is_empty();
        if first_run {
            crate::state::soundpack_library::backfill_existing(
                &mut config.soundpack_added_at,
                &mut config.soundpacks_seen,
                &folder_paths
            );
            return;
        }

        // Later runs: anything without a timestamp arrived since the last
        // launch, so it is stamped and left unseen to earn a badge.
        for folder_path in &folder_paths {
            crate::state::soundpack_library::mark_added(
                &mut config.soundpack_added_at,
                folder_path
            );
        }
    });
}

// Global update state
static GLOBAL_UPDATE_STATE: OnceCell<Mutex<Option<crate::utils::auto_updater::UpdateInfo>>> =
    OnceCell::new();

// Hook to get update info
pub fn use_update_info() -> Option<crate::utils::auto_updater::UpdateInfo> {
    let update_signal: Signal<u32> = use_context();
    // Trigger signal check
    let _ = update_signal();

    if let Some(global_update) = GLOBAL_UPDATE_STATE.get() {
        if let Ok(state) = global_update.lock() {
            return state.clone();
        }
    }
    None
}

// Function to set update info
pub fn set_update_info(update_info: Option<crate::utils::auto_updater::UpdateInfo>) {
    if let Some(global_update) = GLOBAL_UPDATE_STATE.get() {
        if let Ok(mut state) = global_update.lock() {
            *state = update_info;
        }
    }
    // Note: We need a way to trigger UI updates when this is called
    // This should be called from within Dioxus components that have access to the update signal
}

// Initialize update state - call this once at startup
pub fn init_update_state() {
    if GLOBAL_UPDATE_STATE.get().is_none() {
        crate::always_print!("📝 Initializing global update state...");
        let _ = GLOBAL_UPDATE_STATE.set(Mutex::new(None));

        // Load saved update info from config if available
        if let Some(saved_update) = crate::utils::auto_updater::get_saved_update_info() {
            crate::always_print!(
                "📦 Found saved update info: {} -> {}",
                saved_update.current_version,
                saved_update.latest_version
            );
            set_update_info(Some(saved_update));
        }
    }
}

// Hook to trigger update info changes (should be called from components)
pub fn use_update_info_setter() -> Callback<Option<crate::utils::auto_updater::UpdateInfo>> {
    let mut update_signal: Signal<u32> = use_context();
    use_callback(move |info: Option<crate::utils::auto_updater::UpdateInfo>| {
        set_update_info(info);
        // Trigger UI update
        let current_value = {
            let val = update_signal.read();
            *val
        };
        update_signal.set(current_value + 1);
    })
}
