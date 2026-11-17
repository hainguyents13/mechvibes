//! Text that reaches the screen from outside the component tree: the notice
//! templates, the messages the audio and device layers surface, and the failures
//! adding a sound to Mood can raise.
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

        // ----- Mood's own sounds (`utils::mood_sounds`) -----
        // The failures that carry a `{}` take the picked file's name or the OS
        // cause; the others are whole sentences.
        mood_sound_pick_supported_file: "Pick an MP3, WAV, OGG, FLAC or M4A file.", "请选择 MP3、WAV、OGG、FLAC 或 M4A 文件。", "Elige un archivo MP3, WAV, OGG, FLAC o M4A.",
        mood_sound_open_failed: "Could not open the file: {}", "无法打开该文件：{}", "No se pudo abrir el archivo: {}",
        mood_sound_read_failed: "Could not read the file: {}", "无法读取该文件：{}", "No se pudo leer el archivo: {}",
        mood_sound_save_failed: "Could not save the sound: {}", "无法保存该音效：{}", "No se pudo guardar el sonido: {}",
        mood_sound_open_sound_failed: "Could not open the sound: {}", "无法打开该音效：{}", "No se pudo abrir el sonido: {}",
        mood_sound_not_playable: "This file is not a sound Mechvibes can play.", "该文件不是 Mechvibes 能播放的音效。", "Este archivo no es un sonido que Mechvibes pueda reproducir.",
        mood_sound_empty: "This sound file is empty.", "该音效文件是空的。", "Este archivo de sonido está vacío.",
        // A list name, not a failure: what the list shows when the picked file
        // has nothing printable in its name. Stored in config as it is, so the
        // module reads the English column at add time (see `mood_sounds`).
        mood_sound_fallback_name: "My sound", "我的音效", "Mi sonido",
    }
}
