//! The Settings page: the page header, its seven `Collapse` sections and the
//! footer.
//!
//! The selectors the Devices section renders (audio output, keyboards, mice)
//! keep their copy in [`super::devices`], because they are separate components
//! with their own counts and plurals.
//!
//! **The tray noun is two keys, not one.** `TRAY_NAME` used to be a constant
//! that read `menu bar` on macOS and `tray` elsewhere, and it is spliced into two
//! sentences ("Start hidden in the {}", "Click the {} icon"). The platform picks
//! the word; the language picks the spelling, so both words are catalog entries
//! and the call site chooses by `cfg!(target_os = "macos")`.
//!
//! Emoji, the space around the heart in the footer and the `❌` in front of an
//! error stay at the call site: the catalog holds words, and an editor trimming
//! a trailing space must not be able to change what the user sees.

zone! {
    /// Settings page copy.
    SettingsStrings {
        // ----- The Language section (shipped with the selector) -----
        language_section_title: "Language", "语言", "Idioma",
        language_system: "System", "跟随系统", "Sistema",

        // ----- Page header -----
        page_title: "Settings", "设置", "Ajustes",
        page_subtitle: "Config your {} experience.", "配置你的 {} 体验。", "Configura tu experiencia con {}.",

        // ----- The section titles -----
        section_general: "General", "通用", "General",
        section_devices: "Devices", "设备", "Dispositivos",
        section_updates: "Updates", "更新", "Actualizaciones",
        section_privacy: "Privacy", "隐私", "Privacidad",
        section_app_info: "App info", "应用信息", "Información de la app",
        section_debug: "Debug", "调试", "Depuración",
        section_danger_zone: "Danger zone", "危险区域", "Zona de peligro",

        // ----- The tray noun, chosen by platform at the call site -----
        tray_noun_menu_bar: "menu bar", "菜单栏", "barra de menús",
        tray_noun_tray: "tray", "托盘", "bandeja",

        // ----- General: the togglers -----
        volume_boost_title: "Volume boost (200% max)", "音量增强（最高 200%）", "Aumento de volumen (máx. 200%)",
        volume_boost_description: "Allow volume sliders to go up to 200%. May cause audio distortion at high levels.", "允许音量滑块最高到 200%。高音量时可能导致音频失真。", "Permite que los controles de volumen lleguen al 200%. Puede causar distorsión a volúmenes altos.",
        mute_mic_title: "Mute while the microphone is in use", "麦克风使用期间静音", "Silenciar mientras se usa el micrófono",
        mute_mic_description: "{} goes quiet during calls and recordings, and comes back when your microphone is free.", "通话和录音期间 {} 会静音，麦克风空闲后自动恢复。", "{} se silencia durante llamadas y grabaciones, y vuelve cuando tu micrófono queda libre.",
        mute_mood_title: "Mute Mood with your keyboard and mouse", "键盘和鼠标操作时静音 Mood", "Silenciar Mood con tu teclado y ratón",
        // The product name and the shortcut are part of the sentence: they read
        // the same in every language, and splitting them out would leave a
        // translator guessing at word order.
        mute_mood_description: "When you mute Mechvibes from the tray, with Ctrl+Alt+M or during a call, the Mood sounds go quiet too.", "当你从托盘、按 Ctrl+Alt+M 或在通话中让 Mechvibes 静音时，Mood 的声音也会静音。", "Cuando silencias Mechvibes desde la bandeja, con Ctrl+Alt+M o durante una llamada, los sonidos de Mood también se silencian.",
        auto_start_title: "Start at login", "登录时启动", "Iniciar al iniciar sesión",
        auto_start_description: "Automatically start {} when you log in", "登录时自动启动 {}", "Iniciar {} automáticamente al iniciar sesión",
        start_minimized_title: "Start minimized to tray", "启动时最小化到托盘", "Iniciar minimizado en la bandeja",
        start_minimized_description: "When starting at login, open minimized to the system tray", "登录启动时最小化到系统托盘", "Al iniciar sesión, abrir minimizado en la bandeja del sistema",
        start_hidden_title: "Start hidden in the {}", "启动时隐藏到{}", "Iniciar oculto en la {}",
        start_hidden_description: "Every time {} starts, it plays sounds in the background with its window hidden. Click the {} icon or open {} again to show it.", "每次启动 {} 时，它都会在窗口隐藏的情况下于后台播放音效。点击{}图标或再次打开 {} 即可显示窗口。", "Cada vez que {} se inicia, reproduce sonidos en segundo plano con la ventana oculta. Haz clic en el icono de la {} o abre {} de nuevo para mostrarla.",
        floating_dock_title: "Floating dock", "浮动停靠栏", "Barra flotante",
        floating_dock_description: "Show the bottom bar as a floating pill instead of a full-width bar.", "将底栏显示为浮动的胶囊，而不是全宽条。", "Muestra la barra inferior como una píldora flotante en lugar de una barra de ancho completo.",

        // ----- Devices: the hint under the selector -----
        devices_hint: "Device changes apply immediately — no restart needed. If audio ever stops working (e.g. after unplugging a device), restarting the app can help.", "设备更改会立即生效，无需重启。如果音频停止工作（例如拔掉设备后），重启应用可能有帮助。", "Los cambios de dispositivo se aplican al instante — no hace falta reiniciar. Si el audio deja de funcionar (p. ej. tras desconectar un dispositivo), reiniciar la app puede ayudar.",

        // ----- Updates -----
        updates_silent_hint: "Update checking runs every 24 hours. Nothing is downloaded until you choose to install.", "每 24 小时检查一次更新。在你选择安装之前不会下载任何内容。", "La búsqueda de actualizaciones se ejecuta cada 24 horas. No se descarga nada hasta que elijas instalarlo.",
        updates_manual_hint: "Update checking runs every 24 hours in the background. Updates are installed manually from the releases page.", "每 24 小时在后台检查一次更新。更新需从发布页面手动安装。", "La búsqueda de actualizaciones se ejecuta cada 24 horas en segundo plano. Las actualizaciones se instalan a mano desde la página de versiones.",
        checking: "Checking...", "正在检查…", "Comprobando…",
        check_for_updates: "Check for Updates", "检查更新", "Buscar actualizaciones",
        last_checked: "Last checked: {}", "上次检查：{}", "Última comprobación: {}",
        never_checked: "Never checked", "从未检查", "Nunca comprobado",
        // The error text itself comes from the updater and stays in English for
        // now; this is only the sentence that introduces it.
        update_check_failed: "Failed to check for updates: {}", "检查更新失败：{}", "No se pudo comprobar si hay actualizaciones: {}",
        update_available: "Update available: v{}", "有可用更新：v{}", "Actualización disponible: v{}",
        view_release_notes: "View release notes", "查看发行说明", "Ver notas de la versión",
        up_to_date: "You're running the latest version (v{})", "你正在使用最新版本（v{}）", "Estás usando la última versión (v{})",
        current_version: "Current version: v{}", "当前版本：v{}", "Versión actual: v{}",

        // ----- Privacy -----
        telemetry_title: "Share anonymous usage stats", "共享匿名使用统计", "Compartir estadísticas de uso anónimas",
        telemetry_description: "Sends one anonymous ping when the app starts, containing only your OS and app version. Never your keystrokes or any personal data.", "应用启动时发送一次匿名 ping，仅包含你的操作系统和应用版本。绝不包含你的按键或个人数据。", "Envía un ping anónimo al iniciar la app, solo con tu sistema operativo y la versión de la app. Nunca tus pulsaciones ni datos personales.",

        // ----- Danger zone -----
        danger_zone_body: "Reset all settings to their default values. This action cannot be undone.", "将所有设置恢复为默认值。此操作无法撤销。", "Restablece todos los ajustes a sus valores predeterminados. Esta acción no se puede deshacer.",
        reset_to_defaults: "Reset to Defaults", "恢复默认值", "Restablecer valores predeterminados",

        // ----- Footer -----
        // The heart icon and the links sit between these words, so the sentence
        // is split into four keys. Each piece is a word, and the single spaces
        // that join them belong to the call site.
        made_with: "Made with", "用爱制作", "Hecho con",
        made_by: "by", "由", "por",
        footer_and: "and", "和", "y",
        footer_contributors: "these awesome people", "这些了不起的贡献者", "estas personas increíbles",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// English is the language the app already shipped in, so the text of every
    /// key here is a contract: this slice moves it, it does not re-word it. A
    /// translator editing the English column by accident fails here, next to the
    /// key, instead of in a screenshot nobody compares.
    #[test]
    fn english_keeps_the_text_the_page_already_had() {
        let expected = [
            ("page_title", EN.page_title, "Settings"),
            ("page_subtitle", EN.page_subtitle, "Config your {} experience."),
            ("section_general", EN.section_general, "General"),
            ("section_devices", EN.section_devices, "Devices"),
            ("section_updates", EN.section_updates, "Updates"),
            ("section_privacy", EN.section_privacy, "Privacy"),
            ("section_app_info", EN.section_app_info, "App info"),
            ("section_debug", EN.section_debug, "Debug"),
            ("section_danger_zone", EN.section_danger_zone, "Danger zone"),
            ("tray_noun_menu_bar", EN.tray_noun_menu_bar, "menu bar"),
            ("tray_noun_tray", EN.tray_noun_tray, "tray"),
            ("volume_boost_title", EN.volume_boost_title, "Volume boost (200% max)"),
            (
                "volume_boost_description",
                EN.volume_boost_description,
                "Allow volume sliders to go up to 200%. May cause audio distortion at high levels.",
            ),
            ("mute_mic_title", EN.mute_mic_title, "Mute while the microphone is in use"),
            (
                "mute_mic_description",
                EN.mute_mic_description,
                "{} goes quiet during calls and recordings, and comes back when your microphone is free.",
            ),
            ("mute_mood_title", EN.mute_mood_title, "Mute Mood with your keyboard and mouse"),
            (
                "mute_mood_description",
                EN.mute_mood_description,
                "When you mute Mechvibes from the tray, with Ctrl+Alt+M or during a call, the Mood sounds go quiet too.",
            ),
            ("auto_start_title", EN.auto_start_title, "Start at login"),
            ("auto_start_description", EN.auto_start_description, "Automatically start {} when you log in"),
            ("start_minimized_title", EN.start_minimized_title, "Start minimized to tray"),
            (
                "start_minimized_description",
                EN.start_minimized_description,
                "When starting at login, open minimized to the system tray",
            ),
            ("start_hidden_title", EN.start_hidden_title, "Start hidden in the {}"),
            ("floating_dock_title", EN.floating_dock_title, "Floating dock"),
            (
                "floating_dock_description",
                EN.floating_dock_description,
                "Show the bottom bar as a floating pill instead of a full-width bar.",
            ),
            (
                "devices_hint",
                EN.devices_hint,
                "Device changes apply immediately — no restart needed. If audio ever stops working (e.g. after unplugging a device), restarting the app can help.",
            ),
            (
                "updates_silent_hint",
                EN.updates_silent_hint,
                "Update checking runs every 24 hours. Nothing is downloaded until you choose to install.",
            ),
            (
                "updates_manual_hint",
                EN.updates_manual_hint,
                "Update checking runs every 24 hours in the background. Updates are installed manually from the releases page.",
            ),
            ("checking", EN.checking, "Checking..."),
            ("check_for_updates", EN.check_for_updates, "Check for Updates"),
            ("last_checked", EN.last_checked, "Last checked: {}"),
            ("never_checked", EN.never_checked, "Never checked"),
            ("update_check_failed", EN.update_check_failed, "Failed to check for updates: {}"),
            ("update_available", EN.update_available, "Update available: v{}"),
            ("view_release_notes", EN.view_release_notes, "View release notes"),
            ("up_to_date", EN.up_to_date, "You're running the latest version (v{})"),
            ("current_version", EN.current_version, "Current version: v{}"),
            ("telemetry_title", EN.telemetry_title, "Share anonymous usage stats"),
            (
                "telemetry_description",
                EN.telemetry_description,
                "Sends one anonymous ping when the app starts, containing only your OS and app version. Never your keystrokes or any personal data.",
            ),
            (
                "danger_zone_body",
                EN.danger_zone_body,
                "Reset all settings to their default values. This action cannot be undone.",
            ),
            ("reset_to_defaults", EN.reset_to_defaults, "Reset to Defaults"),
            ("made_with", EN.made_with, "Made with"),
            ("made_by", EN.made_by, "by"),
            ("footer_and", EN.footer_and, "and"),
            ("footer_contributors", EN.footer_contributors, "these awesome people"),
        ];
        for (key, actual, shipped) in expected {
            assert_eq!(actual, shipped, "settings.{key} changed text users already had");
        }
    }

    /// The templates carry values the page fills in; an English column that
    /// drops a `{}` would silently stop showing the app name, the tray noun or
    /// the version.
    #[test]
    fn english_templates_keep_their_placeholders() {
        for (key, template, placeholders) in [
            ("page_subtitle", EN.page_subtitle, 1),
            ("mute_mic_description", EN.mute_mic_description, 1),
            ("auto_start_description", EN.auto_start_description, 1),
            ("start_hidden_title", EN.start_hidden_title, 1),
            ("start_hidden_description", EN.start_hidden_description, 3),
            ("update_check_failed", EN.update_check_failed, 1),
            ("update_available", EN.update_available, 1),
            ("last_checked", EN.last_checked, 1),
            ("up_to_date", EN.up_to_date, 1),
            ("current_version", EN.current_version, 1),
        ] {
            assert_eq!(
                template.matches("{}").count(),
                placeholders,
                "settings.{key} needs {placeholders} placeholder(s): {template:?}"
            );
        }
        assert_eq!(
            EN.start_hidden_description,
            "Every time {} starts, it plays sounds in the background with its window hidden. Click the {} icon or open {} again to show it.",
        );
    }

    /// Spanish is the language where the two platform words disagree on gender
    /// with the article in front of them, so the sentences that splice the noun
    /// in must read correctly for both (`la bandeja`, `la barra de menús`).
    #[test]
    fn the_tray_noun_fits_both_sentences() {
        for noun in [ES.tray_noun_tray, ES.tray_noun_menu_bar] {
            let title = crate::i18n::fill(ES.start_hidden_title, &[&noun]);
            assert!(title.contains(noun), "Spanish start_hidden_title drops {noun}: {title}");
            let description = crate::i18n::fill(
                ES.start_hidden_description,
                &[&"Mechvibes", &noun, &"Mechvibes"],
            );
            assert!(
                description.contains(noun),
                "Spanish start_hidden_description drops {noun}: {description}"
            );
        }
    }
}
