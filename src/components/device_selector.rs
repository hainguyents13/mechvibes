use crate::libs::audio::{ AudioCommand, AudioContext };
use crate::libs::device_manager::{ DeviceInfo, DeviceManager };
use crate::libs::input_device_manager::{ InputDeviceInfo, InputDeviceManager, InputDeviceType };
use crate::utils::config::use_config;
use dioxus::prelude::*;
use lucide_dioxus::{ Headphones, Keyboard, Mouse, RefreshCw };
use std::sync::{ Arc, Mutex, OnceLock };

#[derive(Clone, PartialEq, Copy)]
pub enum DeviceType {
    AudioOutput,
    Keyboard,
    Mouse,
}

/// Input devices enumerated during this app session, kept outside the component
/// tree so they survive the unmount that happens on every tab switch.
/// Audio outputs already have an equivalent cache in `DeviceManager`.
static ENUMERATED_INPUT_DEVICES: OnceLock<Mutex<Vec<InputDeviceInfo>>> = OnceLock::new();

/// Whether devices have been enumerated at least once this session. Set on
/// first mount, so re-entering the page reuses that result rather than
/// enumerating again; only the refresh button re-runs enumeration.
static DEVICES_ENUMERATED: OnceLock<Mutex<bool>> = OnceLock::new();

fn input_device_cache() -> &'static Mutex<Vec<InputDeviceInfo>> {
    ENUMERATED_INPUT_DEVICES.get_or_init(|| Mutex::new(Vec::new()))
}

fn devices_enumerated_flag() -> &'static Mutex<bool> {
    DEVICES_ENUMERATED.get_or_init(|| Mutex::new(false))
}

/// Records that an enumeration has completed, so remounts can restore results.
fn mark_devices_enumerated() {
    if let Ok(mut flag) = devices_enumerated_flag().lock() {
        *flag = true;
    }
}

fn have_devices_been_enumerated() -> bool {
    devices_enumerated_flag().lock().map(|flag| *flag).unwrap_or(false)
}

fn store_input_devices(devices: &[InputDeviceInfo]) {
    if let Ok(mut cache) = input_device_cache().lock() {
        *cache = devices.to_vec();
    }
}

fn cached_input_devices() -> Vec<InputDeviceInfo> {
    input_device_cache().lock().map(|cache| cache.clone()).unwrap_or_default()
}

/// Tells the Windows input worker host to reload its enabled-device filter,
/// so toggling a keyboard/mouse here takes effect immediately instead of on
/// the next app start. No-op elsewhere: on Linux/macOS the listeners read
/// the filter themselves.
fn notify_input_filter_changed() {
    #[cfg(target_os = "windows")]
    crate::libs::input_worker_host::notify_config_changed();
}

#[derive(Props, Clone, PartialEq)]
pub struct DeviceSelectorProps {
    device_type: DeviceType,
    label: String,
    description: Option<String>,
}

/// The label for the device-type line under a device's name.
///
/// The enum's `Debug` output used to be rendered straight into the list, which
/// put an English layer token on a translated page (and broke the moment the
/// variant was renamed). The mapping lives here, where the catalog is, and the
/// device's name and id keep coming from the OS untranslated.
fn input_device_type_label(
    device_type: &InputDeviceType,
    strings: &'static crate::i18n::Strings,
) -> &'static str {
    match device_type {
        InputDeviceType::Keyboard => strings.devices.type_keyboard,
        InputDeviceType::Mouse => strings.devices.type_mouse,
        InputDeviceType::Unknown => strings.devices.type_unknown,
    }
}

#[component]
pub fn DeviceSelector(props: DeviceSelectorProps) -> Element {
    let (config, update_config) = use_config();
    // Read at render time, never inside a `use_memo`: this read of the config
    // signal is what re-renders the list when the language changes. The
    // count-dependent sentences below go through `plural` / `fill`.
    let strings = crate::i18n::use_strings();
    let audio_devices = use_signal(|| Vec::<DeviceInfo>::new());
    let input_devices = use_signal(|| Vec::<InputDeviceInfo>::new());
    let is_loading = use_signal(|| false);
    let has_loaded = use_signal(|| false); // Track if devices have been loaded at least once
    let error_message = use_signal(String::new);
    let device_status = use_signal(|| std::collections::HashMap::<String, bool>::new());

    // Get current selected/enabled devices
    let current_selection = use_memo(move || {
        let config = config();
        match props.device_type {
            DeviceType::AudioOutput => (config.selected_audio_device.clone(), Vec::<String>::new()),
            DeviceType::Keyboard => (None, config.enabled_keyboards.clone()),
            DeviceType::Mouse => (None, config.enabled_mice.clone()),
        }
    });

    // Whether the saved audio device is among the ones enumerated. A selection
    // that is missing (unplugged, or saved under an ID format that no longer
    // resolves) is shown as the system default, which is what is actually
    // playing in that case.
    let selection_is_present = use_memo(move || {
        let Some(selected) = current_selection().0 else {
            return true;
        };
        audio_devices()
            .iter()
            .any(|device| device.id == selected)
    });

    // Load devices from cache (audio) or fresh (input devices)
    let load_cached_devices = {
        let mut audio_devices = audio_devices.clone();
        let mut input_devices = input_devices.clone();
        let mut is_loading = is_loading.clone();
        let mut has_loaded = has_loaded.clone();
        let mut error_message = error_message.clone();
        let device_type = props.device_type;

        use_callback(move |_| {
            spawn(async move {
                crate::always_print!("📋 [DeviceSelector] Loading devices...");
                is_loading.set(true);
                error_message.set(String::new());

                match device_type {
                    DeviceType::AudioOutput => {
                        // Use cached audio devices to avoid enumeration interference
                        match DeviceManager::get_cached_output_devices() {
                            Ok(device_list) => {
                                crate::always_print!("✅ [DeviceSelector] Loaded {} cached audio devices", device_list.len());
                                audio_devices.set(device_list);
                                mark_devices_enumerated();
                                has_loaded.set(true);
                            }
                            Err(e) => {
                                crate::always_print!("❌ [DeviceSelector] Failed to load cached devices: {}", e);
                                // Only the reason is stored. The sentence around
                                // it is read from the catalog where it is drawn,
                                // so a `use_memo` cannot freeze it.
                                error_message.set(e);
                                mark_devices_enumerated();
                                has_loaded.set(true);
                            }
                        }
                    }
                    DeviceType::Keyboard | DeviceType::Mouse => {
                        // Input devices don't interfere with audio, load fresh
                        match InputDeviceManager::get_devices() {
                            Ok(device_list) => {
                                crate::always_print!("✅ [DeviceSelector] Loaded {} input devices", device_list.len());
                                store_input_devices(&device_list);
                                input_devices.set(device_list);
                                mark_devices_enumerated();
                                has_loaded.set(true);
                            }
                            Err(e) => {
                                crate::always_print!("❌ [DeviceSelector] Failed to load input devices: {}", e);
                                error_message.set(e);
                                mark_devices_enumerated();
                                has_loaded.set(true);
                            }
                        }
                    }
                }

                is_loading.set(false);
            });
        })
    };

    // Refresh device cache (re-enumerate)
    let refresh_device_cache = {
        let load_cached = load_cached_devices.clone();
        let device_type = props.device_type;

        use_callback(move |_| {
            spawn(async move {
                crate::always_print!("🔄 [DeviceSelector] User clicked refresh - re-enumerating devices...");
                match device_type {
                    DeviceType::AudioOutput => {
                        // Refresh audio device cache
                        match DeviceManager::refresh_cache() {
                            Ok(_) => {
                                crate::always_print!("✅ [DeviceSelector] Audio cache refreshed successfully");
                                // Reload from refreshed cache
                                load_cached.call(());
                            }
                            Err(e) => {
                                crate::always_print!("❌ [DeviceSelector] Failed to refresh audio cache: {}", e);
                            }
                        }
                    }
                    DeviceType::Keyboard | DeviceType::Mouse => {
                        // Input devices - just reload fresh
                        crate::always_print!("🔄 [DeviceSelector] Reloading input devices...");
                        load_cached.call(());
                    }
                }
            });
        })
    };

    // The list is populated on mount rather than waiting for the refresh
    // button, so opening Settings always shows devices.
    //
    // The cost this used to avoid is still avoided: enumeration itself is what
    // can interrupt playback (cpal activates each device as the list is
    // walked), and it runs at most once per session here. `initialize_cache`
    // fills the audio cache on the first call and every later mount is a plain
    // clone of it; input devices come from the session cache once enumerated.
    // Refresh remains the only path that re-enumerates.
    use_hook({
        let mut audio_devices = audio_devices;
        let mut input_devices = input_devices;
        let mut has_loaded = has_loaded;
        let load_cached = load_cached_devices;
        let device_type = props.device_type;

        move || {
            match device_type {
                DeviceType::AudioOutput => {
                    // Reading the cache is a plain clone; it only enumerates
                    // if this is the first read of the session.
                    if let Ok(device_list) = DeviceManager::get_cached_output_devices() {
                        audio_devices.set(device_list);
                        mark_devices_enumerated();
                        has_loaded.set(true);
                    }
                }
                DeviceType::Keyboard | DeviceType::Mouse => {
                    if have_devices_been_enumerated() {
                        input_devices.set(cached_input_devices());
                        has_loaded.set(true);
                    } else {
                        // No session cache yet, so enumerate once. Unlike the
                        // audio path this has no manager-level cache to fall
                        // back on.
                        load_cached.call(());
                    }
                }
            }
        }
    });

    // Test device status (only for audio devices)
    let test_device_status = {
        let mut device_status = device_status.clone();
        let device_type = props.device_type;

        use_callback(move |device_id: String| {
            spawn(async move {
                match device_type {
                    DeviceType::AudioOutput => {
                        let device_manager = DeviceManager::new();
                        let is_available = device_manager
                            .test_output_device(&device_id)
                            .unwrap_or(false);
                        device_status.with_mut(|status| {
                            status.insert(device_id, is_available);
                        });
                    }
                    DeviceType::Keyboard | DeviceType::Mouse => {
                        // Input devices are always considered available if enumerated
                        device_status.with_mut(|status| {
                            status.insert(device_id, true);
                        });
                    }
                }
            });
        })
    };

    // Handle device selection/toggling
    let audio_ctx: Arc<AudioContext> = use_context();
    let handle_device_action = {
        let update_config = update_config.clone();
        let device_type = props.device_type;
        let test_device_status = test_device_status.clone();
        let audio_ctx = audio_ctx.clone();

        use_callback(move |device_id: String| {
            match device_type {
                DeviceType::AudioOutput => {
                    // Test device before selecting
                    test_device_status.call(device_id.clone());

                    let device_id_clone = device_id.clone();
                    let switch_target = if device_id_clone == "default" {
                        None
                    } else {
                        Some(device_id_clone.clone())
                    };
                    update_config(
                        Box::new(move |config| {
                            config.selected_audio_device = switch_target.clone();
                        })
                    );

                    // Apply at runtime: the audio engine and ambiance
                    // player both switch immediately, not just on next
                    // startup.
                    let runtime_target = if device_id == "default" {
                        None
                    } else {
                        Some(device_id)
                    };
                    audio_ctx.send(AudioCommand::SwitchDevice(runtime_target.clone()));
                    crate::state::ambiance::switch_ambiance_player_device(runtime_target);
                }
                DeviceType::Keyboard => {
                    let device_id_clone = device_id.clone();
                    update_config(
                        Box::new(move |config| {
                            if config.enabled_keyboards.contains(&device_id_clone) {
                                config.enabled_keyboards.retain(|id| id != &device_id_clone);
                            } else {
                                config.enabled_keyboards.push(device_id_clone);
                            }
                        })
                    );
                    notify_input_filter_changed();
                }
                DeviceType::Mouse => {
                    let device_id_clone = device_id.clone();
                    update_config(
                        Box::new(move |config| {
                            if config.enabled_mice.contains(&device_id_clone) {
                                config.enabled_mice.retain(|id| id != &device_id_clone);
                            } else {
                                config.enabled_mice.push(device_id_clone);
                            }
                        })
                    );
                    notify_input_filter_changed();
                }
            }
        })
    };

    // The label the selector would put on the current selection. Nothing renders
    // it today (hence the underscore), but it is the one place that spells out
    // the three count-dependent sentences, so the grammar stays here and now
    // comes from the catalog: `All {}` takes the noun pair's plural member,
    // `{} {} Selected` takes it for the count. It is a plain render-time value
    // rather than a `use_memo`: a memo recomputes only when a signal read
    // *inside* it changes, and the catalog is read outside, so it would keep the
    // first language until the component unmounted.
    let _current_device_name = {
        let (selected_device, enabled_devices) = current_selection();

        // The noun as the sentence needs it: lower-case in Spanish, and the
        // plural form when the count is not one (Chinese writes both the same).
        let noun = |count: usize| -> &'static str {
            match props.device_type {
                DeviceType::Keyboard =>
                    crate::i18n::plural(
                        strings.devices.keyboard_one,
                        strings.devices.keyboard_other,
                        count,
                    ),
                DeviceType::Mouse =>
                    crate::i18n::plural(
                        strings.devices.mouse_one,
                        strings.devices.mouse_other,
                        count,
                    ),
                // Audio outputs do not use either count sentence.
                DeviceType::AudioOutput => "",
            }
        };

        match props.device_type {
            DeviceType::AudioOutput => {
                match selected_device {
                    None => strings.devices.system_default.to_string(),
                    Some(current_id) =>
                        audio_devices()
                            .iter()
                            .find(|d| d.id == current_id)
                            .map(|d| d.name.clone())
                            .unwrap_or_else(|| strings.devices.unknown_device.to_string()),
                }
            }
            DeviceType::Keyboard | DeviceType::Mouse => {
                let device_count = enabled_devices.len();
                if device_count == 0 {
                    // The noun's plural member, so English reads `All Mice`
                    // instead of the `All Mouses` an appended `s` produced.
                    crate::i18n::fill(strings.devices.all_devices, &[&noun(0)])
                } else {
                    crate::i18n::fill(
                        strings.devices.selected_summary,
                        &[&device_count, &noun(device_count)],
                    )
                }
            }
        }
    };

    // Get device status for display
    let show_error_status = use_memo(move || {
        if props.device_type == DeviceType::AudioOutput {
            let (selected_device, _) = current_selection();
            if let Some(current) = selected_device {
                if let Some(status) = device_status().get(&current) {
                    return !status;
                }
            }
        }
        false
    });

    // A catalog string, so it is read where it is drawn rather than memoized: a
    // `use_memo` would keep the language it first rendered in.
    let no_devices_message = match props.device_type {
        DeviceType::AudioOutput => strings.devices.no_audio_devices,
        DeviceType::Keyboard => strings.devices.no_keyboard_devices,
        DeviceType::Mouse => strings.devices.no_mouse_devices,
    };

    // Combined device list for audio (includes system default).
    //
    // Built at render time, not in a `use_memo`: two of its fields are catalog
    // strings, and a memo would have frozen the language they were first built
    // in. The ids and names of the hardware devices are the OS's and are not
    // translated; only the pseudo-device's own labels come from the catalog.
    let all_audio_devices = if props.device_type == DeviceType::AudioOutput {
        let mut devices = Vec::new();

        // The system default is a pseudo-device, and `"default"` is its id: the
        // sentinel stays untranslated because it is compared against config.
        devices.push((
            "default".to_string(),
            strings.devices.system_default.to_string(),
            strings.devices.system_default_hint.to_string(),
            true,
        ));

        // Add hardware devices
        for device in audio_devices().iter() {
            devices.push((
                device.id.clone(),
                device.name.clone(),
                // Hardware devices carry no sub-line; the badge is `Default`.
                String::new(),
                device.is_default,
            ));
        }

        devices
    } else {
        Vec::new()
    };

    // Helper function to render device icon
    let device_icon = move || {
        match props.device_type {
            DeviceType::AudioOutput =>
                rsx! {
                    Headphones { class: "w-4 h-4" }
                },
            DeviceType::Keyboard =>
                rsx! {
                    Keyboard { class: "w-4 h-4" }
                },
            DeviceType::Mouse =>
                rsx! {
                    Mouse { class: "w-4 h-4" }
                },
        }
    };

    rsx! {
        div { class: "space-y-2",
            // Label and description
            div { class: "flex items-center gap-2 text-sm font-bold text-base-content/80",
                {device_icon()}
                span { "{props.label}" }
                button {
                    class: "btn btn-ghost btn-xs",
                    onclick: move |_| refresh_device_cache.call(()),
                    disabled: is_loading(),
                    title: strings.devices.refresh_device_list,
                    if is_loading() {
                        RefreshCw { class: "w-3 h-3 animate-spin" }
                    } else {
                        RefreshCw { class: "w-3 h-3" }
                    }
                }
            }

            if let Some(desc) = &props.description {
                p { class: "text-xs text-base-content/60", "{desc}" }            }

            // Device list with radio buttons
            div { class: "bg-base-100 px-4 py-3 rounded-box space-y-2",
                match props.device_type {
                    DeviceType::AudioOutput => rsx! {
                        if audio_devices().is_empty() && !is_loading() {
                            div { class: "text-center text-base-content/50 py-8",
                                {device_icon()}
                                if !has_loaded() {
                                    div { class: "mt-2 text-sm", "{strings.devices.loading_devices}" }
                                } else {
                                    div { class: "mt-2 text-sm", "{no_devices_message}" }
                                }
                            }
                        } else {
                            div { class: "space-y-2",
                                // Unified device list (system default + hardware devices)
                                for (device_id, device_name, badge_text, is_default) in all_audio_devices.iter() {
                                    label {
                                        key: "{device_id}",
                                        class: "flex items-center gap-3 rounded-lg hover:bg-base-100 cursor-pointer transition-colors",
                                        input {
                                            r#type: "radio",
                                            name: "audio-device",
                                            class: "radio radio-xs radio-primary",
                                            // "System Default" also takes the
                                            // selection when the saved device
                                            // is not in the list - it was
                                            // unplugged, or its ID predates
                                            // name-based IDs. Leaving every
                                            // radio unchecked would show no
                                            // selection at all while playback
                                            // is in fact on the default.
                                            checked: if device_id == "default" {
                                                current_selection().0.is_none() || !selection_is_present()
                                            } else {
                                                current_selection().0.as_ref() == Some(device_id)
                                            },
                                            onchange: {
                                                let device_id_clone = device_id.clone();
                                                move |_| {
                                                    handle_device_action.call(device_id_clone.clone());
                                                }
                                            }
                                        }
                                        div { class: "flex items-center gap-2 flex-1",
                                            div { class: "flex-1 min-w-0",
                                                div { class: "text-xs font-medium flex items-center gap-2",
                                                    span { class: "line-clamp-1", "{device_name}" }
                                                    if device_id == "default" {
                                                        span { class: "badge badge-xs badge-outline", "{strings.devices.default_badge}" }
                                                    } else if *is_default && !badge_text.is_empty() {
                                                        span { class: "badge badge-xs badge-outline", "{badge_text}" }
                                                    }
                                                }
                                                if device_id == "default" {
                                                    div { class: "text-xs text-base-content/60", "{badge_text}" }
                                                } else {
                                                    div { class: "text-xs text-base-content/60",
                                                        {crate::i18n::fill(strings.devices.device_id, &[&device_id])}
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    },
                    DeviceType::Keyboard | DeviceType::Mouse => rsx! {
                        if input_devices().is_empty() && !is_loading() {
                            div { class: "text-center text-base-content/50 py-8",
                                {device_icon()}
                                if !has_loaded() {
                                    div { class: "mt-2 text-sm", "{strings.devices.loading_devices}" }
                                } else {
                                    div { class: "mt-2 text-sm", "{no_devices_message}" }
                                }
                            }
                        } else {
                            div { class: "space-y-2",
                                // Available input devices
                                for device in input_devices().iter() {
                                    label { 
                                        key: "{device.id}",
                                        class: "flex items-center gap-3 p-3 rounded-lg hover:bg-base-100 cursor-pointer transition-colors",
                                        input {
                                            r#type: "checkbox",
                                            class: "checkbox checkbox-primary",
                                            checked: current_selection().1.contains(&device.id),
                                            onchange: {
                                                let device_id = device.id.clone();
                                                move |_| {
                                                    handle_device_action.call(device_id.clone());
                                                }
                                            }
                                        }
                                        div { class: "flex items-center gap-2 flex-1",
                                            {device_icon()}
                                            div { class: "flex-1 min-w-0",
                                                div { class: "text-sm font-medium truncate", "{device.name}" }
                                                div { class: "text-xs text-base-content/60",
                                                    {input_device_type_label(&device.device_type, strings)}
                                                }
                                            }
                                            div { class: "badge badge-success badge-sm", "{strings.devices.available_badge}" }
                                        }
                                    }
                                }
                            }
                        }
                    },
                }
            }

            // Error message
            if !error_message().is_empty() {
                div { class: "text-xs text-error mt-2",
                    {crate::i18n::fill(
                        match props.device_type {
                            DeviceType::AudioOutput => strings.devices.load_audio_failed,
                            DeviceType::Keyboard | DeviceType::Mouse => strings.devices.load_input_failed,
                        },
                        &[&error_message()],
                    )}
                }
            }

            // Device status warning
            if show_error_status() {
                div { class: "alert alert-warning mt-2",
                    div { class: "text-sm",
                        "⚠️ {strings.devices.selection_unavailable}"
                    }
                }
            }

            // Linux-specific information
            if cfg!(target_os = "linux") && props.device_type == DeviceType::AudioOutput {
                div { class: "alert alert-info mt-2",
                    div { class: "text-xs",
                        if !has_loaded() {
                            "ℹ️ {strings.devices.linux_click_refresh}"
                        } else {
                            "ℹ️ {strings.devices.linux_refresh_warning}"
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn AudioOutputSelector() -> Element {
    // Read at render time so the selector's own label follows a language change.
    let strings = crate::i18n::use_strings();
    rsx! {
        DeviceSelector {
            device_type: DeviceType::AudioOutput,
            label: strings.devices.audio_output_label.to_string(),
            description: Some(strings.devices.audio_output_description.to_string()),
        }
    }
}

#[allow(dead_code)]
#[component]
pub fn KeyboardSelector() -> Element {
    let strings = crate::i18n::use_strings();
    rsx! {
        DeviceSelector {
            device_type: DeviceType::Keyboard,
            label: strings.devices.keyboard_devices_label.to_string(),
            description: Some(strings.devices.keyboard_devices_description.to_string()),
        }
    }
}

#[allow(dead_code)]
#[component]
pub fn MouseSelector() -> Element {
    let strings = crate::i18n::use_strings();
    rsx! {
        DeviceSelector {
            device_type: DeviceType::Mouse,
            label: strings.devices.mouse_devices_label.to_string(),
            description: Some(strings.devices.mouse_devices_description.to_string()),
        }
    }
}
