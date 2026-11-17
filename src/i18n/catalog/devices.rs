//! The device selectors: audio output, keyboards and mice, including the
//! count-dependent sentences.
//!
//! **Three grammar sites live here**, and none of them is a `format!` at the
//! call site any more:
//!
//! - `All {}s` built that English plural by appending an `s` to the noun, so it
//!   spelled "All Mouses" for the mouse list. The noun now comes from a
//!   `_one` / `_other` pair and the `s` is gone: English reads "All Keyboards"
//!   and "All Mice".
//! - `Mouse` / `Mice` is that pair (`mouse_one` / `mouse_other`). Chinese writes
//!   the same glyphs in both members and still goes through
//!   [`crate::i18n::plural`].
//! - `{} {} Selected` was one English template with two values spliced in. It is
//!   still one template, and the noun it takes is the pair's form for that
//!   count.
//!
//! **The standalone label and the noun inside a sentence are different keys.**
//! Spanish capitalises the first ("Teclado", under the device name) and not the
//! second ("2 teclados seleccionados"); Chinese capitalises neither. One key
//! plus `to_lowercase()` at the call site is what breaks the next language
//! added, so they stay separate entries.
//!
//! Device names and IDs come from the OS and are never translated, and the
//! `"default"` sentinel is an id, not a label: it keeps its spelling.

zone! {
    /// See the module comment.
    DevicesStrings {
        // ----- The selectors' own labels and descriptions -----
        audio_output_label: "Audio Output Device", "音频输出设备", "Dispositivo de salida de audio",
        audio_output_description: "Select the audio device for soundpack playback", "选择用于音效包播放的音频设备", "Elige el dispositivo de audio para reproducir paquetes de sonido",
        keyboard_devices_label: "Keyboard Devices", "键盘设备", "Dispositivos de teclado",
        keyboard_devices_description: "Select which keyboards should generate sound effects", "选择哪些键盘应产生音效", "Elige qué teclados deben generar efectos de sonido",
        mouse_devices_label: "Mouse Devices", "鼠标设备", "Dispositivos de ratón",
        mouse_devices_description: "Select which mice should generate sound effects", "选择哪些鼠标应产生音效", "Elige qué ratones deben generar efectos de sonido",

        // ----- The list itself -----
        refresh_device_list: "Refresh device list", "刷新设备列表", "Actualizar la lista de dispositivos",
        loading_devices: "Loading devices...", "正在加载设备…", "Cargando dispositivos…",
        system_default: "System Default", "系统默认", "Predeterminado del sistema",
        system_default_hint: "Use system default audio device", "使用系统默认音频设备", "Usar el dispositivo de audio predeterminado del sistema",
        unknown_device: "Unknown Device", "未知设备", "Dispositivo desconocido",
        default_badge: "Default", "默认", "Predeterminado",
        available_badge: "Available", "可用", "Disponible",
        device_id: "Device ID: {}", "设备 ID：{}", "ID del dispositivo: {}",
        no_audio_devices: "No audio devices found", "未找到音频设备", "No se encontraron dispositivos de audio",
        no_keyboard_devices: "No keyboard devices found", "未找到键盘设备", "No se encontraron dispositivos de teclado",
        no_mouse_devices: "No mouse devices found", "未找到鼠标设备", "No se encontraron dispositivos de ratón",
        // The `⚠️` / `ℹ️` that prefix these on screen stay at the call site.
        selection_unavailable: "Selected device may not be available. Audio may not work properly.", "所选设备可能不可用。音频可能无法正常工作。", "Es posible que el dispositivo seleccionado no esté disponible. El audio podría no funcionar bien.",
        linux_click_refresh: "Linux: Click refresh to load available devices. This may briefly interrupt audio playback due to ALSA device enumeration.", "Linux：点击刷新以加载可用设备。由于 ALSA 设备枚举，音频播放可能会短暂中断。", "Linux: pulsa actualizar para cargar los dispositivos disponibles. La reproducción de audio puede interrumpirse brevemente por la enumeración de dispositivos de ALSA.",
        linux_refresh_warning: "Linux: Refresh button will briefly interrupt audio playback (ALSA limitation).", "Linux：刷新按钮会短暂中断音频播放（ALSA 的限制）。", "Linux: el botón de actualizar interrumpirá brevemente la reproducción de audio (limitación de ALSA).",

        // ----- Errors the selector shows in place -----
        // Only the sentence is catalogued: the reason inside it comes from the
        // device layers and still describes itself in English (see the Errors
        // zone, which will own those strings).
        load_audio_failed: "Failed to load audio devices: {}", "加载音频设备失败：{}", "No se pudieron cargar los dispositivos de audio: {}",
        load_input_failed: "Failed to load input devices: {}", "加载输入设备失败：{}", "No se pudieron cargar los dispositivos de entrada: {}",

        // ----- The grammar: nouns in a sentence, lower-case in Spanish -----
        keyboard_one: "Keyboard", "键盘", "teclado",
        keyboard_other: "Keyboards", "键盘", "teclados",
        mouse_one: "Mouse", "鼠标", "ratón",
        mouse_other: "Mice", "鼠标", "ratones",
        all_devices: "All {}", "全部{}", "Todos los {}",
        // A count and a noun, and the adjective in the Spanish column would have
        // to agree with the noun's number, which one template cannot do. The
        // Spanish wording is a label instead ("Selección: 2 teclados"), so the
        // template holds the count and the noun and nothing that inflects.
        selected_summary: "{} {} Selected", "已选择 {} 个{}", "Selección: {} {}",

        // ----- The device-type line under a device's name: a stand-alone label -----
        type_keyboard: "Keyboard", "键盘", "Teclado",
        type_mouse: "Mouse", "鼠标", "Ratón",
        type_unknown: "Unknown", "未知", "Desconocido",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{ fill, plural };

    /// English is the language the app already shipped in, so the text of every
    /// key here is a contract. The one exception is the `All {}s` sentence,
    /// which is spelled out in
    /// `the_all_sentence_takes_the_nouns_plural_form` below.
    #[test]
    fn english_keeps_the_text_the_selector_already_had() {
        let expected = [
            ("audio_output_label", EN.audio_output_label, "Audio Output Device"),
            (
                "audio_output_description",
                EN.audio_output_description,
                "Select the audio device for soundpack playback",
            ),
            ("keyboard_devices_label", EN.keyboard_devices_label, "Keyboard Devices"),
            (
                "keyboard_devices_description",
                EN.keyboard_devices_description,
                "Select which keyboards should generate sound effects",
            ),
            ("mouse_devices_label", EN.mouse_devices_label, "Mouse Devices"),
            (
                "mouse_devices_description",
                EN.mouse_devices_description,
                "Select which mice should generate sound effects",
            ),
            ("refresh_device_list", EN.refresh_device_list, "Refresh device list"),
            ("loading_devices", EN.loading_devices, "Loading devices..."),
            ("system_default", EN.system_default, "System Default"),
            ("system_default_hint", EN.system_default_hint, "Use system default audio device"),
            ("unknown_device", EN.unknown_device, "Unknown Device"),
            ("default_badge", EN.default_badge, "Default"),
            ("available_badge", EN.available_badge, "Available"),
            ("device_id", EN.device_id, "Device ID: {}"),
            ("no_audio_devices", EN.no_audio_devices, "No audio devices found"),
            ("no_keyboard_devices", EN.no_keyboard_devices, "No keyboard devices found"),
            ("no_mouse_devices", EN.no_mouse_devices, "No mouse devices found"),
            (
                "selection_unavailable",
                EN.selection_unavailable,
                "Selected device may not be available. Audio may not work properly.",
            ),
            (
                "linux_click_refresh",
                EN.linux_click_refresh,
                "Linux: Click refresh to load available devices. This may briefly interrupt audio playback due to ALSA device enumeration.",
            ),
            (
                "linux_refresh_warning",
                EN.linux_refresh_warning,
                "Linux: Refresh button will briefly interrupt audio playback (ALSA limitation).",
            ),
            ("load_audio_failed", EN.load_audio_failed, "Failed to load audio devices: {}"),
            ("load_input_failed", EN.load_input_failed, "Failed to load input devices: {}"),
            ("keyboard_one", EN.keyboard_one, "Keyboard"),
            ("keyboard_other", EN.keyboard_other, "Keyboards"),
            ("mouse_one", EN.mouse_one, "Mouse"),
            ("mouse_other", EN.mouse_other, "Mice"),
            ("selected_summary", EN.selected_summary, "{} {} Selected"),
            ("type_keyboard", EN.type_keyboard, "Keyboard"),
            ("type_mouse", EN.type_mouse, "Mouse"),
            ("type_unknown", EN.type_unknown, "Unknown"),
        ];
        for (key, actual, shipped) in expected {
            assert_eq!(actual, shipped, "devices.{key} changed text users already had");
        }
    }

    /// The noun pair is what replaces the `s` that used to be appended to the
    /// noun at the call site. Chinese does not inflect for number and says so
    /// here, so a later translator cannot "fix" it by inventing a plural.
    #[test]
    fn the_mouse_noun_is_a_real_plural() {
        assert_eq!(plural(EN.keyboard_one, EN.keyboard_other, 1), "Keyboard");
        assert_eq!(plural(EN.keyboard_one, EN.keyboard_other, 2), "Keyboards");
        assert_eq!(plural(EN.mouse_one, EN.mouse_other, 1), "Mouse");
        assert_eq!(plural(EN.mouse_one, EN.mouse_other, 2), "Mice");
        assert!(!EN.mouse_other.ends_with("ses"), "`Mouses` is not the plural of `Mouse`");
        assert_eq!(ZH.keyboard_one, ZH.keyboard_other, "Chinese does not inflect for number");
        assert_eq!(ZH.mouse_one, ZH.mouse_other, "Chinese does not inflect for number");
    }

    /// `All {}s` is impossible to translate as a suffix, so the sentence takes
    /// the pair's plural member. `All Mouses` is what the old call site produced
    /// for the mouse list; the pair spells it `Mice`.
    #[test]
    fn the_all_sentence_takes_the_nouns_plural_form() {
        for (language, strings, keyboards, mice) in
            [("en", &EN, "All Keyboards", "All Mice"), ("es", &ES, "Todos los teclados", "Todos los ratones")]
        {
            let all_keyboards = fill(strings.all_devices, &[&plural(strings.keyboard_one, strings.keyboard_other, 0)]);
            let all_mice = fill(strings.all_devices, &[&plural(strings.mouse_one, strings.mouse_other, 0)]);
            assert_eq!(all_keyboards, keyboards, "{language} All sentence for keyboards");
            assert_eq!(all_mice, mice, "{language} All sentence for mice");
        }
    }

    /// The count sentence and its noun agree in English, and the Spanish column
    /// is worded around the adjective's number so the count does not have to
    /// agree with it.
    #[test]
    fn the_selected_sentence_agrees_with_its_noun() {
        let one = fill(EN.selected_summary, &[&1, &plural(EN.keyboard_one, EN.keyboard_other, 1)]);
        let two = fill(EN.selected_summary, &[&2, &plural(EN.mouse_one, EN.mouse_other, 2)]);
        assert_eq!(one, "1 Keyboard Selected");
        assert_eq!(two, "2 Mice Selected");
        let spanish_one = fill(ES.selected_summary, &[&1, &plural(ES.mouse_one, ES.mouse_other, 1)]);
        let spanish_two = fill(ES.selected_summary, &[&2, &plural(ES.mouse_one, ES.mouse_other, 2)]);
        assert_eq!(spanish_one, "Selección: 1 ratón");
        assert_eq!(spanish_two, "Selección: 2 ratones");
    }

    /// Lesson from the previous attempt: the position of the word decides the
    /// key. Spanish capitalises a stand-alone label and not a noun inside a
    /// sentence; Chinese capitalises neither.
    #[test]
    fn a_label_and_a_noun_in_a_sentence_are_two_keys() {
        assert_eq!(ES.type_keyboard, "Teclado");
        assert_eq!(ES.keyboard_one, "teclado");
        assert_eq!(ES.type_mouse, "Ratón");
        assert_eq!(ES.mouse_one, "ratón");
        assert_eq!(ZH.type_keyboard, ZH.keyboard_one);
        assert_eq!(ZH.type_mouse, ZH.mouse_one);
    }
}
