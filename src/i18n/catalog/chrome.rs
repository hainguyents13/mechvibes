//! The application shell: the window title bar, the header, the volume slider,
//! the app-info panel and the small pieces every page is built from.
//!
//! `header.rs` has no text of its own (it only injects CSS) and `logo.rs` draws
//! the "Mechvibes" wordmark from a constant: a brand is not a translation, so
//! neither contributes a key here.
//!
//! Punctuation around a value stays at the call site, not in the catalog: the
//! space after `Volume`, the `%` after the number and the `:` after a label are
//! layout, and an editor trimming trailing whitespace must not be able to change
//! what the user sees.
//!
//! `zone!` takes one entry per line, so the three languages of a long sentence
//! sit side by side on a wide line rather than wrapped onto three.

zone! {
    /// See the module comment.
    ChromeStrings {
        // Title bar. `Minimize`, `Hide to tray` and `Quit` are tooltips, and the
        // bar has no room to spare: keep these to one short word or phrase.
        new_version_available: "New version {} available!", "新版本 {} 可用！", "¡Nueva versión {} disponible!",
        minimize: "Minimize", "最小化", "Minimizar",
        hide_to_tray: "Hide to tray", "隐藏到托盘", "Ocultar en la bandeja",
        quit: "Quit", "退出", "Salir",

        // Volume slider.
        volume: "Volume", "音量", "Volumen",
        mute: "Mute", "静音", "Silenciar",
        unmute: "Unmute", "取消静音", "Reactivar sonido",

        // The app-info panel. `OS` and `Arch` are abbreviations, not identifiers:
        // each language shortens them its own way, so they are translated too.
        application_paths: "Application Paths", "应用路径", "Rutas de la aplicación",
        executable: "Executable", "可执行文件", "Ejecutable",
        working_dir: "Working Dir", "工作目录", "Directorio de trabajo",
        file_system_status: "File System Status", "文件系统状态", "Estado del sistema de archivos",
        system_info: "System info", "系统信息", "Información del sistema",
        os: "OS", "操作系统", "SO",
        arch: "Arch", "架构", "Arquitectura",
        open_app_directory: "Open app directory", "打开应用目录", "Abrir la carpeta de la app",

        // The Debug section's log viewer.
        debug_log_blurb: "Recent activity from the app, kept in memory only. Installed builds have no console, so this is where log lines go. Nothing is written to disk and nothing leaves your device unless you press Export.", "应用最近的活动记录，仅保存在内存中。安装版没有控制台，日志就出现在这里。除非你点击导出，否则不会写入磁盘，也不会离开你的设备。", "Actividad reciente de la app, solo en memoria. Las versiones instaladas no tienen consola, así que aquí van las líneas de registro. Nada se escribe en disco y nada sale de tu equipo salvo que pulses Exportar.",
        no_log_lines: "No log lines captured yet.", "尚未捕获到日志。", "Todavía no se ha capturado ninguna línea de registro.",
        export_logs: "Export logs", "导出日志", "Exportar registros",
        // Two counts and no noun that inflects for number in any of the three
        // languages, so this is one template with two placeholders rather than a
        // `_one` / `_other` pair.
        log_counts: "{} shown, {} kept", "已显示 {} 条，共保留 {} 条", "{} visibles, {} en memoria",
        log_saved: "Log file saved.", "日志文件已保存。", "Archivo de registro guardado.",
        log_save_failed: "Could not save the log file: {}", "无法保存日志文件：{}", "No se pudo guardar el archivo de registro: {}",
        verbose_logging: "Verbose logging", "详细日志", "Registro detallado",
        verbose_logging_description: "Adds per-keystroke timing lines, useful for latency reports. Key identities are always masked. Resets to off every time the app starts.", "增加每次按键的耗时记录，便于分析延迟问题。按键标识始终会被遮蔽。每次启动应用后都会恢复为关闭状态。", "Añade tiempos por pulsación, útiles para informes de latencia. Las teclas siempre van enmascaradas. Vuelve a desactivarse cada vez que arranca la app.",

        // The notice toasts.
        show_file: "Show file", "显示文件", "Mostrar archivo",

        // The image field. The dialog action it shares with other screens
        // (`pick_an_image`) lives in `common`.
        paste_image_link: "Paste an image link", "粘贴图片链接", "Pega un enlace de imagen",
        images_filter: "Images", "图片", "Imágenes",
        image_unusable: "Could not use this image. Try another one.", "无法使用这张图片，请换一张。", "No se pudo usar esta imagen. Prueba con otra.",
        image_hint: "Pick an image, or paste a web link", "选择一张图片，或粘贴网页链接", "Elige una imagen o pega un enlace web",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// English is the language the app already shipped in, so these labels are a
    /// contract: this migration re-words nothing on screen. A translator editing
    /// the English column by accident fails here, next to the key, instead of in
    /// a screenshot nobody compares.
    #[test]
    fn english_keeps_the_words_the_shell_already_had() {
        let expected = [
            ("minimize", EN.minimize, "Minimize"),
            ("hide_to_tray", EN.hide_to_tray, "Hide to tray"),
            ("quit", EN.quit, "Quit"),
            ("volume", EN.volume, "Volume"),
            ("mute", EN.mute, "Mute"),
            ("unmute", EN.unmute, "Unmute"),
            ("application_paths", EN.application_paths, "Application Paths"),
            ("executable", EN.executable, "Executable"),
            ("working_dir", EN.working_dir, "Working Dir"),
            ("file_system_status", EN.file_system_status, "File System Status"),
            ("system_info", EN.system_info, "System info"),
            ("os", EN.os, "OS"),
            ("arch", EN.arch, "Arch"),
            ("open_app_directory", EN.open_app_directory, "Open app directory"),
            ("no_log_lines", EN.no_log_lines, "No log lines captured yet."),
            ("export_logs", EN.export_logs, "Export logs"),
            ("log_saved", EN.log_saved, "Log file saved."),
            ("verbose_logging", EN.verbose_logging, "Verbose logging"),
            ("show_file", EN.show_file, "Show file"),
            ("paste_image_link", EN.paste_image_link, "Paste an image link"),
            ("images_filter", EN.images_filter, "Images"),
            ("image_unusable", EN.image_unusable, "Could not use this image. Try another one."),
            ("image_hint", EN.image_hint, "Pick an image, or paste a web link"),
        ];
        for (key, actual, shipped) in expected {
            assert_eq!(actual, shipped, "chrome.{key} changed the text users already had");
        }
    }

    /// The two templates carry their placeholder in a sentence the shell shows
    /// with a value filled in; a reworded English column that drops the `{}` would
    /// silently stop showing the version or the counts.
    #[test]
    fn english_templates_keep_their_placeholders() {
        assert_eq!(EN.new_version_available, "New version {} available!");
        assert_eq!(EN.log_counts, "{} shown, {} kept");
        assert_eq!(EN.log_save_failed, "Could not save the log file: {}");
    }
}
