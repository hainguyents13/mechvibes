//! Text that reaches the screen from outside the component tree: the notice
//! templates, the messages the audio and device layers surface, the failures
//! adding a sound to Mood can raise, the storage helpers' causes, and the toasts
//! a theme file's save reports.
//!
//! The layers that produce these failures have no catalog in hand — a sound
//! pack load runs on the engine thread, a device list comes from `cpal`, and a
//! notice can be reported before the window exists. So none of them decides
//! the language: each keeps a **stable English `Display`** for the log and
//! exposes a typed failure, and the sentence below is filled in by whoever has
//! the catalog (the render site, or the site that reports the notice).
//!
//! That is why every key here is a full sentence rather than a fragment: the
//! caller supplies the cause (or the path), and the whole wording stays in the
//! catalog where a translator can move the placeholder around.

zone! {
    /// See the module comment.
    ErrorStrings {
        // ----- Notices reported by `utils::notice` -----
        // A change that could not be written, with the save error as the cause.
        could_not_save_your_changes: "Could not save your changes. {}", "无法保存你的更改。{}", "No se pudieron guardar tus cambios. {}",
        // The same shape for the saved themes list, which reports on its own.
        could_not_save_your_theme: "Could not save your theme. {}", "无法保存你的主题。{}", "No se pudo guardar tu tema. {}",
        // A list that was damaged and moved aside; `{}` is where the copy went.
        colors_list_damaged: "Your colors list was damaged. A copy was kept at {}.", "你的颜色列表已损坏。副本保留在 {}。", "Tu lista de colores estaba dañada. Se guardó una copia en {}.",
        // A list that could not be read at all this session.
        colors_list_read_only: "Could not read your colors list, so nothing in it was changed. Restart Mechvibes to try again.", "无法读取你的颜色列表，因此其中的内容未被更改。请重启 Mechvibes 再试一次。", "No se pudo leer tu lista de colores, así que no se cambió nada en ella. Reinicia Mechvibes para volver a intentarlo.",
        saved_themes_damaged: "Your saved themes were damaged. A copy was kept at {}.", "你保存的主题已损坏。副本保留在 {}。", "Tus temas guardados estaban dañados. Se guardó una copia en {}.",
        saved_themes_read_only: "Could not open your saved themes, so theme changes will not be saved for now. Restart Mechvibes to try again.", "无法打开你保存的主题，因此主题更改暂时无法保存。请重启 Mechvibes 再试一次。", "No se pudieron abrir tus temas guardados, así que los cambios de tema no se guardarán por ahora. Reinicia Mechvibes para volver a intentarlo.",
        saved_themes_from_newer_app: "Your saved themes come from a newer Mechvibes. Update Mechvibes to change them.", "你保存的主题来自更新版本的 Mechvibes。请更新 Mechvibes 后再修改。", "Tus temas guardados vienen de una versión más reciente de Mechvibes. Actualiza Mechvibes para cambiarlos.",

        // ----- The colors list itself (`state::themes`) -----
        theme_name_required: "Give it a name first.", "请先给它起个名字。", "Primero ponle un nombre.",
        theme_name_taken: "You already have one with this name. Pick another name.", "已经有一个同名的了。请换一个名字。", "Ya tienes uno con este nombre. Elige otro nombre.",
        theme_not_found: "Theme not found", "未找到主题", "Tema no encontrado",
        data_directory_unavailable: "Failed to create data directory: {}", "无法创建数据目录：{}", "No se pudo crear el directorio de datos: {}",

        // ----- Audio devices (`libs::device_manager`) -----
        // `{}` is `cpal`'s own description of the failure.
        output_devices_unavailable: "Failed to enumerate output devices: {}", "无法枚举输出设备：{}", "No se pudieron enumerar los dispositivos de salida: {}",
        input_devices_unavailable: "Failed to enumerate input devices: {}", "无法枚举输入设备：{}", "No se pudieron enumerar los dispositivos de entrada: {}",
        // Used where the caller does not say which direction failed.
        devices_unavailable: "Failed to enumerate devices: {}", "无法枚举设备：{}", "No se pudieron enumerar los dispositivos: {}",
        device_cache_unavailable: "Failed to access device cache", "无法访问设备缓存", "No se pudo acceder a la caché de dispositivos",

        // ----- Raw input devices (`libs::input_device_manager`) -----
        // These come from the Win32 raw input API, so there is no cause to append.
        raw_input_count_unavailable: "Failed to get raw input device count", "无法获取原始输入设备的数量", "No se pudo obtener la cantidad de dispositivos de entrada sin procesar",
        raw_input_list_unavailable: "Failed to get raw input device list", "无法获取原始输入设备列表", "No se pudo obtener la lista de dispositivos de entrada sin procesar",
        device_name_size_unavailable: "Failed to get device name size", "无法获取设备名称的长度", "No se pudo obtener la longitud del nombre del dispositivo",
        device_name_unavailable: "Failed to get device name", "无法获取设备名称", "No se pudo obtener el nombre del dispositivo",
        device_info_unavailable: "Failed to get device info", "无法获取设备信息", "No se pudo obtener la información del dispositivo",
        device_name_retrieval_failed: "Could not retrieve device name", "无法检索设备名称", "No se pudo recuperar el nombre del dispositivo",
        friendly_name_unsupported: "Not supported on non-Windows platforms", "非 Windows 平台不支持此操作", "No es compatible con plataformas que no sean Windows",

        // ----- Sound packs (`libs::audio::soundpack_loader`) -----
        // `{}` is the underlying cause, or the path, as noted per key.
        soundpack_id_empty: "empty soundpack ID", "音效包 ID 为空", "ID de paquete de sonido vacío",
        soundpack_config_unreadable: "Failed to read config: {}", "无法读取配置：{}", "No se pudo leer la configuración: {}",
        soundpack_config_invalid: "Failed to parse config: {}", "无法解析配置：{}", "No se pudo interpretar la configuración: {}",
        // Kept apart from the key above so the English text stays as it is:
        // the keyboard loader names the config version, the mouse one does not.
        soundpack_config_invalid_v2: "Failed to parse V2 soundpack config: {}", "无法解析 V2 音效包配置：{}", "No se pudo interpretar la configuración V2 del paquete de sonido: {}",
        soundpack_is_mouse: "This is a mouse soundpack, not a keyboard soundpack", "这是鼠标音效包，不是键盘音效包", "Este es un paquete de sonido de ratón, no de teclado",
        soundpack_is_keyboard: "This is a keyboard soundpack, not a mouse soundpack", "这是键盘音效包，不是鼠标音效包", "Este es un paquete de sonido de teclado, no de ratón",
        soundpack_audio_field_missing: "No audio_file field in soundpack config", "音效包配置中没有 audio_file 字段", "La configuración del paquete de sonido no incluye el campo audio_file",
        soundpack_audio_file_missing: "Sound file not found: {}", "未找到音频文件：{}", "No se encontró el archivo de audio: {}",
        soundpack_audio_unreadable: "Failed to load audio: {}", "无法加载音频：{}", "No se pudo cargar el audio: {}",
        soundpack_no_named_audio_file: "The pack names no audio file", "该音效包没有指定音频文件", "El paquete no indica ningún archivo de audio",
        soundpack_no_audio_loadable: "None of the pack's audio files could be loaded", "该音效包的音频文件都无法加载", "No se pudo cargar ninguno de los archivos de audio del paquete",

        // ----- Importing a pack (`utils::soundpack_validator`) -----
        // What the import dialog says when a ZIP is refused. The validator has no
        // catalog in hand — it also runs inside the headless `pack-convert` — so it
        // raises a typed failure and the dialog words it with these.
        soundpack_zip_missing: "File does not exist", "文件不存在", "El archivo no existe",
        soundpack_zip_not_archive: "File must be a ZIP archive", "文件必须是 ZIP 压缩包", "El archivo debe ser un paquete ZIP",
        soundpack_zip_open_failed: "Cannot open file: {}", "无法打开文件：{}", "No se pudo abrir el archivo: {}",
        soundpack_zip_invalid: "Invalid ZIP file: {}", "无效的 ZIP 文件：{}", "Archivo ZIP no válido: {}",
        soundpack_zip_empty: "ZIP file is empty", "ZIP 文件为空", "El archivo ZIP está vacío",
        // The archive's entries: a name the target systems cannot store, one name
        // twice, and the config the installer will read. `{}` is the entry name or
        // `zip`'s description of the failure.
        soundpack_entry_unreadable: "Failed to read archive entry: {}", "无法读取压缩包条目：{}", "No se pudo leer la entrada del paquete: {}",
        soundpack_archive_entry_unreadable: "Cannot read file in archive: {}", "无法读取压缩包中的文件：{}", "No se pudo leer el archivo dentro del paquete: {}",
        soundpack_entry_name_unusable: "The ZIP contains a file with a name that cannot be used: '{}'", "ZIP 中包含文件名无法使用的文件：'{}'", "El ZIP contiene un archivo con un nombre que no se puede usar: '{}'",
        soundpack_entry_name_repeated: "The ZIP has more than one file named '{}'. Every file in a sound pack needs a name of its own.", "ZIP 中有多个名为 '{}' 的文件。音效包中的每个文件都需要有各自的名称。", "El ZIP tiene más de un archivo llamado '{}'. Cada archivo de un paquete de sonido necesita un nombre propio.",
        soundpack_config_not_found: "No config.json found in ZIP file", "ZIP 文件中未找到 config.json", "No se encontró config.json en el archivo ZIP",
        soundpack_config_read_failed: "Failed to read config.json: {}", "无法读取 config.json：{}", "No se pudo leer config.json: {}",
        soundpack_config_too_large: "The sound pack's config.json is too large.", "该音效包的 config.json 过大。", "El config.json del paquete de sonido es demasiado grande.",
        // The pack as a whole: the ZIP the structure check opens, its audio, and the
        // temp config that check validates. `{}` is the OS or `zip` cause.
        soundpack_archive_open_failed: "Cannot open ZIP file: {}", "无法打开 ZIP 文件：{}", "No se pudo abrir el archivo ZIP: {}",
        soundpack_archive_invalid: "Invalid ZIP archive: {}", "无效的 ZIP 压缩包：{}", "Paquete ZIP no válido: {}",
        soundpack_no_audio_files: "No audio files found in soundpack", "音效包中未找到音频文件", "No se encontraron archivos de audio en el paquete de sonido",
        soundpack_temp_config_failed: "Cannot write temp config: {}", "无法写入临时配置：{}", "No se pudo escribir la configuración temporal: {}",
        // `{}` is the report's own sentence (the two keys below it) or the list of
        // field names the config is missing.
        soundpack_structure_invalid: "Invalid soundpack structure: {}", "音效包结构无效：{}", "Estructura del paquete de sonido no válida: {}",
        soundpack_fields_missing: "Missing required fields: {}", "缺少必填字段：{}", "Faltan campos obligatorios: {}",
        soundpack_version_unsupported: "Invalid or unsupported soundpack version", "音效包版本无效或不受支持", "Versión del paquete de sonido no válida o no compatible",
        soundpack_requires_newer_app: "This soundpack requires a newer version of Mechvibes", "此音效包需要更新版本的 Mechvibes", "Este paquete de sonido requiere una versión más reciente de Mechvibes",
        // The report's `InvalidStructure` payload: the file it could not read, or
        // one entry of the config it could not use. `{}` is the cause, or the key
        // name and its index in the definitions map.
        soundpack_config_file_unreadable: "Cannot read config file: {}", "无法读取配置文件：{}", "No se pudo leer el archivo de configuración: {}",
        soundpack_config_json_invalid: "Invalid JSON: {}", "无效的 JSON：{}", "JSON no válido: {}",
        soundpack_issue_timing_expected: "Invalid definitions entry for '{}': expected timing array", "“{}”的定义条目无效：应为 timing 数组", "Entrada de definitions no válida para '{}': se esperaba un arreglo timing",
        soundpack_issue_timing_pair: "Invalid timing array for '{}[{}]': expected [start, end]", "“{}[{}]”的 timing 数组无效：应为 [start, end]", "Arreglo timing no válido para '{}[{}]': se esperaba [start, end]",
        soundpack_issue_timing_entry: "Invalid timing entry for '{}[{}]': expected array", "“{}[{}]”的 timing 条目无效：应为数组", "Entrada timing no válida para '{}[{}]': se esperaba un arreglo",
        soundpack_issue_definitions_not_object: "definitions field should be an object", "definitions 字段应为对象", "El campo definitions debe ser un objeto",
        soundpack_issue_mouse_not_boolean: "mouse field should be boolean", "mouse 字段应为布尔值", "El campo mouse debe ser booleano",

        // ----- Mood's own sounds (`utils::mood_sounds`) -----
        // The failures that carry a `{}` take the picked file's name or the OS
        // cause; the others are whole sentences.
        mood_sound_pick_supported_file: "Pick an MP3, WAV, OGG, FLAC or M4A file.", "请选择 MP3、WAV、OGG、FLAC 或 M4A 文件。", "Elige un archivo MP3, WAV, OGG, FLAC o M4A.",
        mood_sound_open_failed: "Could not open the file: {}", "无法打开该文件：{}", "No se pudo abrir el archivo: {}",
        // Also the sentence for a picked *theme* file that cannot be opened
        // (`utils::theme_share::open`): one sentence, one key, two pickers.
        mood_sound_read_failed: "Could not read the file: {}", "无法读取该文件：{}", "No se pudo leer el archivo: {}",
        mood_sound_save_failed: "Could not save the sound: {}", "无法保存该音效：{}", "No se pudo guardar el sonido: {}",
        mood_sound_open_sound_failed: "Could not open the sound: {}", "无法打开该音效：{}", "No se pudo abrir el sonido: {}",
        mood_sound_not_playable: "This file is not a sound Mechvibes can play.", "该文件不是 Mechvibes 能播放的音效。", "Este archivo no es un sonido que Mechvibes pueda reproducir.",
        mood_sound_empty: "This sound file is empty.", "该音效文件是空的。", "Este archivo de sonido está vacío.",
        // A list name, not a failure: what the list shows when the picked file
        // has nothing printable in its name. Stored in config as it is, so the
        // module reads the English column at add time (see `mood_sounds`).
        mood_sound_fallback_name: "My sound", "我的音效", "Mi sonido",

        // ----- The storage helpers behind a save (`utils::path`, `utils::data`) -----
        // `{}` is the folder and the OS cause. The same sentence the path helper
        // has always produced, so a caller that only prints it reads it unchanged.
        directory_create_failed: "Failed to create directory '{}': {}", "无法创建目录 '{}'：{}", "No se pudo crear el directorio '{}': {}",
        // The four ways writing a JSON file can fail; `{}` is the path and the
        // OS or `serde_json` cause, as the sentence needs it.
        json_serialize_failed: "Failed to serialize data: {}", "无法序列化数据：{}", "No se pudieron serializar los datos: {}",
        file_write_failed: "Failed to write file '{}': {}", "无法写入文件 '{}'：{}", "No se pudo escribir el archivo '{}': {}",
        file_replace_failed: "Failed to replace file '{}': {}", "无法替换文件 '{}'：{}", "No se pudo reemplazar el archivo '{}': {}",

        // ----- Sharing a theme file (`components::theme_share`, `utils::theme_share`) -----
        // These are raised by a click rather than a render, so the module that
        // raises them reads the choice as the file is saved or imported.
        nothing_to_undo: "Nothing to undo. The look changed since.", "没有可撤销的操作。外观已经改变。", "No hay nada que deshacer. La apariencia cambió desde entonces.",
        saving_stopped_unexpectedly: "Saving stopped unexpectedly.", "保存意外中断。", "El guardado se detuvo inesperadamente.",
        importing_stopped_unexpectedly: "Importing stopped unexpectedly.", "导入意外中断。", "La importación se detuvo inesperadamente.",
        theme_file_saved: "Saved. Your friend can double-click the file to use it.", "已保存。对方双击该文件即可使用。", "Guardado. Tu amigo puede hacer doble clic en el archivo para usarlo.",
        // `{}` is the OS cause of a write to the `.mvtheme` file.
        could_not_save_the_file: "Could not save the file: {}", "无法保存该文件：{}", "No se pudo guardar el archivo: {}",
        could_not_save_the_theme: "Could not save the theme.", "无法保存该主题。", "No se pudo guardar el tema.",
        // ----- Gathering the look into a shareable file (`utils::theme_share`) -----
        // `{}` is what the file itself is or names: the image, the address it
        // was downloaded from, or the name and extension of the file Share
        // would write over.
        share_theme_name_required: "Give the theme a name first.", "请先为主题起个名字。", "Primero ponle un nombre al tema.",
        share_theme_missing: "The selected theme is missing. Pick a theme and try again.", "所选主题已不存在。请选择一个主题再试。", "El tema seleccionado no existe. Elige un tema y vuelve a intentarlo.",
        share_image_not_an_image: "'{}' is not a PNG, JPEG, GIF, WebP or BMP image.", "“{}”不是 PNG、JPEG、GIF、WebP 或 BMP 图片。", "'{}' no es una imagen PNG, JPEG, GIF, WebP ni BMP.",
        share_image_unreadable: "Could not read the image '{}'.", "无法读取图片“{}”。", "No se pudo leer la imagen '{}'.",
        share_image_unreadable_retry: "Could not read the image '{}'. Pick it again and retry.", "无法读取图片“{}”。请重新选择后再试。", "No se pudo leer la imagen '{}'. Vuelve a elegirla e inténtalo de nuevo.",
        share_image_unreadable_cause: "Could not read the image '{}': {}", "无法读取图片“{}”：{}", "No se pudo leer la imagen '{}': {}",
        // Two keys for two sentences: the one about a picked file quotes the
        // path, the one about a download names the address as it is.
        share_image_too_large: "The image '{}' is larger than 10 MB. Use a smaller one.", "图片“{}”超过 10 MB。请使用更小的图片。", "La imagen '{}' supera los 10 MB. Usa una más pequeña.",
        share_image_url_too_large: "The image {} is larger than 10 MB. Use a smaller one.", "图片 {} 超过 10 MB。请使用更小的图片。", "La imagen {} supera los 10 MB. Usa una más pequeña.",
        share_image_download_failed: "Could not download the image {}. Check your connection, or pick the image from your computer.", "无法下载图片 {}。请检查网络连接，或从电脑中选择该图片。", "No se pudo descargar la imagen {}. Revisa tu conexión o elige la imagen de tu computadora.",
        share_file_exists: "There is already a file called \"{}.{}\". Share again and pick another name.", "已存在名为“{}.{}”的文件。请重新分享并选择另一个名称。", "Ya existe un archivo llamado \"{}.{}\". Comparte de nuevo y elige otro nombre.",
        // `{}` is the cause: a `zip` or `serde_json` description, or the name of
        // the image file that could not be written.
        could_not_write_the_theme: "Could not write the theme: {}", "无法写入主题：{}", "No se pudo escribir el tema: {}",
        could_not_save_theme_images: "Could not save the theme's images. {}", "无法保存主题的图片。{}", "No se pudieron guardar las imágenes del tema. {}",

        // ----- Reading a theme file (`utils::theme_share`) -----
        // Every one of these is shown in the import dialog, so it is a full
        // sentence the file's own reader does not have to assemble.
        theme_file_not_a_theme: "This is not a Mechvibes theme file.", "这不是 Mechvibes 主题文件。", "Este no es un archivo de tema de Mechvibes.",
        theme_file_too_large: "This theme file is too large.", "此主题文件过大。", "Este archivo de tema es demasiado grande.",
        theme_file_from_newer_app: "This theme was made with a newer Mechvibes. Update the app to use it.", "此主题来自更新版本的 Mechvibes。请更新应用后再使用。", "Este tema viene de una versión más reciente de Mechvibes. Actualiza la app para usarlo.",
        theme_file_damaged: "This theme file is damaged.", "此主题文件已损坏。", "Este archivo de tema está dañado.",
        theme_file_image_damaged: "This theme file is damaged: an image is missing or unreadable.", "此主题文件已损坏：缺少图片或无法读取。", "Este archivo de tema está dañado: falta una imagen o no se puede leer.",
        theme_file_no_colors: "This theme has no colors this app can use.", "此主题没有本应用可用的颜色。", "Este tema no tiene colores que esta app pueda usar.",
        theme_file_color_unusable: "This theme file has a color this app cannot use.", "此主题文件包含本应用无法使用的颜色。", "Este archivo de tema tiene un color que esta app no puede usar.",

        // ----- A saved theme the list refuses to write (`state::saved_themes`) -----
        // Three sentence forms for one idea, kept apart on purpose: the saved
        // theme's own colors (“Its”), the colors copied into the list (“These”),
        // and the file just read ("This theme", above).
        theme_colors_unusable: "Its colors have nothing this app can use.", "它的颜色没有本应用可用的内容。", "Sus colores no tienen nada que esta app pueda usar.",
        theme_value_unusable: "It has a color or image this app cannot use.", "它包含本应用无法使用的颜色或图片。", "Tiene un color o una imagen que esta app no puede usar.",
        colors_have_nothing_usable: "These colors have nothing this app can use.", "这些颜色没有本应用可用的内容。", "Estos colores no tienen nada que esta app pueda usar.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sentences these keys replaced, word for word, as they were literals
    /// in `utils::theme_share`, `state::saved_themes` and `utils::theme_library`.
    /// Moving a sentence into the catalog must not rewrite it: an English user
    /// has to read the same bytes as before.
    #[test]
    fn english_keeps_the_sentences_the_theme_flow_showed() {
        let expected = [
            ("share_theme_name_required", EN.share_theme_name_required, "Give the theme a name first."),
            (
                "share_theme_missing",
                EN.share_theme_missing,
                "The selected theme is missing. Pick a theme and try again.",
            ),
            (
                "share_image_not_an_image",
                EN.share_image_not_an_image,
                "'{}' is not a PNG, JPEG, GIF, WebP or BMP image.",
            ),
            ("share_image_unreadable", EN.share_image_unreadable, "Could not read the image '{}'."),
            (
                "share_image_unreadable_retry",
                EN.share_image_unreadable_retry,
                "Could not read the image '{}'. Pick it again and retry.",
            ),
            (
                "share_image_unreadable_cause",
                EN.share_image_unreadable_cause,
                "Could not read the image '{}': {}",
            ),
            (
                "share_image_too_large",
                EN.share_image_too_large,
                "The image '{}' is larger than 10 MB. Use a smaller one.",
            ),
            (
                "share_image_url_too_large",
                EN.share_image_url_too_large,
                "The image {} is larger than 10 MB. Use a smaller one.",
            ),
            (
                "share_image_download_failed",
                EN.share_image_download_failed,
                "Could not download the image {}. Check your connection, or pick the image from your computer.",
            ),
            (
                "share_file_exists",
                EN.share_file_exists,
                "There is already a file called \"{}.{}\". Share again and pick another name.",
            ),
            ("could_not_write_the_theme", EN.could_not_write_the_theme, "Could not write the theme: {}"),
            (
                "could_not_save_theme_images",
                EN.could_not_save_theme_images,
                "Could not save the theme's images. {}",
            ),
            ("theme_file_not_a_theme", EN.theme_file_not_a_theme, "This is not a Mechvibes theme file."),
            ("theme_file_too_large", EN.theme_file_too_large, "This theme file is too large."),
            (
                "theme_file_from_newer_app",
                EN.theme_file_from_newer_app,
                "This theme was made with a newer Mechvibes. Update the app to use it.",
            ),
            ("theme_file_damaged", EN.theme_file_damaged, "This theme file is damaged."),
            (
                "theme_file_image_damaged",
                EN.theme_file_image_damaged,
                "This theme file is damaged: an image is missing or unreadable.",
            ),
            (
                "theme_file_no_colors",
                EN.theme_file_no_colors,
                "This theme has no colors this app can use.",
            ),
            (
                "theme_file_color_unusable",
                EN.theme_file_color_unusable,
                "This theme file has a color this app cannot use.",
            ),
            (
                "theme_colors_unusable",
                EN.theme_colors_unusable,
                "Its colors have nothing this app can use.",
            ),
            (
                "theme_value_unusable",
                EN.theme_value_unusable,
                "It has a color or image this app cannot use.",
            ),
            (
                "colors_have_nothing_usable",
                EN.colors_have_nothing_usable,
                "These colors have nothing this app can use.",
            ),
        ];
        for (key, actual, shipped) in expected {
            assert_eq!(actual, shipped, "errors.{key} changed the words users already had");
        }
    }

    /// The one sentence this zone shares with a picker outside the theme flow:
    /// opening the picked file failed, and both callers say it the same way.
    #[test]
    fn opening_a_picked_file_has_one_sentence_for_every_picker() {
        assert_eq!(EN.mood_sound_open_failed, "Could not open the file: {}");
    }

    /// The sentences the import dialog used to show, word for word, as they were
    /// literals in `utils::soundpack_validator`. Moving a sentence into the
    /// catalog must not rewrite it: an English user imports the same broken ZIP
    /// and has to read the same bytes as before.
    #[test]
    fn english_keeps_the_sentences_the_import_dialog_showed() {
        let expected = [
            ("soundpack_zip_missing", EN.soundpack_zip_missing, "File does not exist"),
            ("soundpack_zip_not_archive", EN.soundpack_zip_not_archive, "File must be a ZIP archive"),
            ("soundpack_zip_open_failed", EN.soundpack_zip_open_failed, "Cannot open file: {}"),
            ("soundpack_zip_invalid", EN.soundpack_zip_invalid, "Invalid ZIP file: {}"),
            ("soundpack_zip_empty", EN.soundpack_zip_empty, "ZIP file is empty"),
            (
                "soundpack_entry_unreadable",
                EN.soundpack_entry_unreadable,
                "Failed to read archive entry: {}",
            ),
            (
                "soundpack_archive_entry_unreadable",
                EN.soundpack_archive_entry_unreadable,
                "Cannot read file in archive: {}",
            ),
            (
                "soundpack_entry_name_unusable",
                EN.soundpack_entry_name_unusable,
                "The ZIP contains a file with a name that cannot be used: '{}'",
            ),
            (
                "soundpack_entry_name_repeated",
                EN.soundpack_entry_name_repeated,
                "The ZIP has more than one file named '{}'. Every file in a sound pack needs a name of its own.",
            ),
            (
                "soundpack_config_not_found",
                EN.soundpack_config_not_found,
                "No config.json found in ZIP file",
            ),
            (
                "soundpack_config_read_failed",
                EN.soundpack_config_read_failed,
                "Failed to read config.json: {}",
            ),
            (
                "soundpack_config_too_large",
                EN.soundpack_config_too_large,
                "The sound pack's config.json is too large.",
            ),
            (
                "soundpack_archive_open_failed",
                EN.soundpack_archive_open_failed,
                "Cannot open ZIP file: {}",
            ),
            ("soundpack_archive_invalid", EN.soundpack_archive_invalid, "Invalid ZIP archive: {}"),
            (
                "soundpack_no_audio_files",
                EN.soundpack_no_audio_files,
                "No audio files found in soundpack",
            ),
            (
                "soundpack_temp_config_failed",
                EN.soundpack_temp_config_failed,
                "Cannot write temp config: {}",
            ),
            (
                "soundpack_structure_invalid",
                EN.soundpack_structure_invalid,
                "Invalid soundpack structure: {}",
            ),
            ("soundpack_fields_missing", EN.soundpack_fields_missing, "Missing required fields: {}"),
            (
                "soundpack_version_unsupported",
                EN.soundpack_version_unsupported,
                "Invalid or unsupported soundpack version",
            ),
            (
                "soundpack_requires_newer_app",
                EN.soundpack_requires_newer_app,
                "This soundpack requires a newer version of Mechvibes",
            ),
            (
                "soundpack_config_file_unreadable",
                EN.soundpack_config_file_unreadable,
                "Cannot read config file: {}",
            ),
            ("soundpack_config_json_invalid", EN.soundpack_config_json_invalid, "Invalid JSON: {}"),
            (
                "soundpack_issue_timing_expected",
                EN.soundpack_issue_timing_expected,
                "Invalid definitions entry for '{}': expected timing array",
            ),
            (
                "soundpack_issue_timing_pair",
                EN.soundpack_issue_timing_pair,
                "Invalid timing array for '{}[{}]': expected [start, end]",
            ),
            (
                "soundpack_issue_timing_entry",
                EN.soundpack_issue_timing_entry,
                "Invalid timing entry for '{}[{}]': expected array",
            ),
            (
                "soundpack_issue_definitions_not_object",
                EN.soundpack_issue_definitions_not_object,
                "definitions field should be an object",
            ),
            (
                "soundpack_issue_mouse_not_boolean",
                EN.soundpack_issue_mouse_not_boolean,
                "mouse field should be boolean",
            ),
        ];
        for (key, actual, shipped) in expected {
            assert_eq!(actual, shipped, "errors.{key} changed the words users already had");
        }
    }

    /// The wording a screen shows follows the catalog the caller hands in, and
    /// the `Display` a log prints stays English.
    ///
    /// The validator is compiled into the headless `pack-convert` too, so it has
    /// no language of its own: the sentence has to come from whoever holds the
    /// catalog (the import dialogs, here through `errors`).
    #[test]
    fn the_validators_failures_are_worded_by_the_caller_and_logged_in_english() {
        use crate::utils::soundpack_validator::{
            PackArchiveError,
            SoundpackStructureError,
            SoundpackZipError,
            StructureIssue,
            StructureProblem,
        };

        let fields = SoundpackStructureError::MissingFields(
            vec!["author".to_string(), "definitions".to_string()]
        );
        assert_eq!(fields.ui_message(&EN), "Missing required fields: author, definitions");
        assert_eq!(fields.to_string(), "Missing required fields: author, definitions");
        assert_eq!(fields.ui_message(&ES), "Faltan campos obligatorios: author, definitions");

        // A finding underneath the wrapper: the cause is worded first, in the
        // same language, and the wrapper keeps its placeholder in the middle of
        // the sentence where the translation puts it.
        let nested = SoundpackStructureError::Invalid(
            StructureProblem::Issues(
                vec![
                    StructureIssue::TimingPair("KeyA".to_string(), 1),
                    StructureIssue::MouseNotBoolean
                ]
            )
        );
        assert_eq!(
            nested.ui_message(&EN),
            "Invalid soundpack structure: Invalid timing array for 'KeyA[1]': expected [start, end]; mouse field should be boolean"
        );
        assert_eq!(
            nested.ui_message(&ZH),
            "音效包结构无效：“KeyA[1]”的 timing 数组无效：应为 [start, end]; mouse 字段应为布尔值"
        );

        // The cause the import dialog wraps in its own sentence, the pre-open
        // checks, and the log form of both.
        let no_config = SoundpackStructureError::Archive(PackArchiveError::NoConfig);
        assert_eq!(no_config.ui_message(&ZH), "ZIP 文件中未找到 config.json");
        assert_eq!(no_config.to_string(), "No config.json found in ZIP file");
        assert_eq!(SoundpackZipError::Empty.ui_message(&ES), "El archivo ZIP está vacío");
        assert_eq!(SoundpackZipError::Empty.to_string(), "ZIP file is empty");
    }

    /// Every identifier `text` reads as `marker<name>`.
    ///
    /// Used to read the validator's source instead of restating its key list here:
    /// a key it starts reading is covered without anyone remembering this test.
    fn identifiers_after<'a>(text: &'a str, marker: &str) -> Vec<&'a str> {
        text.match_indices(marker)
            .filter_map(|(at, _)| {
                let rest = &text[at + marker.len()..];
                let end = rest
                    .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                    .unwrap_or(rest.len());
                (end > 0).then(|| &rest[..end])
            })
            .collect()
    }

    /// The headless `pack-convert` binary cannot link this crate's catalog — it
    /// compiles `utils::soundpack_validator` with `#[path]` and supplies its own
    /// `i18n` stub, because the real one pulls in Dioxus and the config signal.
    /// That stub retypes the English column, so this is the guard that stops the
    /// two copies drifting: every key the validator reads from the catalog must
    /// appear in the stub with these exact words.
    ///
    /// If this fails, re-word `src/bin/pack_convert.rs`'s `i18n::catalog::errors`
    /// (or this column) until they agree again.
    #[test]
    fn the_headless_stub_carries_every_sentence_the_validator_reads_from_the_catalog() {
        const VALIDATOR: &str = include_str!("../../utils/soundpack_validator.rs");
        const STUB: &str = include_str!("../../bin/pack_convert.rs");

        let keys = identifiers_after(VALIDATOR, "errors.");
        assert!(
            !keys.is_empty(),
            "the scan found no `errors.` read in the validator; the marker changed and this \
             guard is now doing nothing"
        );

        for key in keys {
            let Some((_, english, ..)) = ENTRIES.iter().find(|(name, ..)| *name == key) else {
                panic!("`utils::soundpack_validator` reads `errors.{key}`, which is not a key here");
            };
            assert!(
                STUB.contains(*english),
                "`pack_convert`'s i18n stub is missing errors.{key} ({english:?}), so the headless \
                 tool would print a sentence the app no longer says"
            );
        }
    }
}
