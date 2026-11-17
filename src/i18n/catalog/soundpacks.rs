//! The Sound packs page and everything under it: the table, the
//! manager, the import modal, the delete confirmation and the library install
//! modal that a `mechvibes://install/<id>` link opens.
//!
//! **The two pack categories need two keys each.** `Keyboard`/`Mouse` are
//! standalone labels — tab captions, the pack picker's heading, folder buttons —
//! and the same nouns also appear *inside sentences* ("Search keyboard sound
//! packs…"). Spanish spells those two positions differently (`Teclado` versus
//! `teclado`), and Chinese does not capitalise at all, so one key cannot serve
//! both. `keyboard_label`/`mouse_label` are the standalone form,
//! `keyboard_noun`/`mouse_noun` the one that goes mid-sentence. Never lower-case
//! or capitalise a key at runtime: that is what breaks the next language added.
//!
//! The files on disk are a third thing again: a pack's `soundpack_type` is the
//! literal `"Keyboard"`/`"Mouse"` and stays untranslated, because it names a
//! directory.

zone! {
    /// Every string the Sound packs page and its modals show.
    SoundpacksStrings {
        // ----- Page header and tabs -----
        page_title: "Sound Packs", "音效包", "Paquetes de sonido",
        page_subtitle: "Manage your sound packs", "管理你的音效包", "Administra tus paquetes de sonido",
        tab_manage: "Manage", "管理", "Administrar",

        // ----- The pack categories: standalone label vs. noun in a sentence -----
        keyboard_label: "Keyboard", "键盘", "Teclado",
        mouse_label: "Mouse", "鼠标", "Ratón",
        keyboard_noun: "keyboard", "键盘", "teclado",
        mouse_noun: "mouse", "鼠标", "ratón",

        // ----- The pack picker on Home -----
        select_placeholder: "Select a {} sound pack...", "选择一个{}音效包…", "Elige un paquete de sonido de {}…",
        search_hint: "Search {} sound packs...", "搜索{}音效包…", "Buscar paquetes de sonido de {}…",
        not_found: "No {} sound packs found", "未找到{}音效包", "No se encontraron paquetes de sonido de {}",
        no_soundpacks_available: "No sound packs available", "没有可用的音效包", "No hay paquetes de sonido disponibles",
        random_pack_title: "Pick a random sound pack", "随机选择一个音效包", "Elegir un paquete de sonido al azar",
        by_author: "by {}", "作者：{}", "por {}",
        by_unknown: "by N/A", "作者：未知", "por N/A",
        new_badge: "New", "新", "Nuevo",
        starred_hint: "Star more packs on the Sound packs page to list them here.", "在音效包页面收藏更多音效包，即可在此列出。", "Marca más paquetes en la página Paquetes de sonido para verlos aquí.",
        load_failed: "Failed to load {} soundpack: {}", "加载{}音效包失败：{}", "No se pudo cargar el paquete de sonido de {}: {}",

        // ----- The table -----
        // The sort hint names the current order in a sentence, so its wording is
        // finished here instead of being lower-cased from a label at runtime.
        sort_hint_recently_added: "Sorted by recently added - click to change", "按最近添加排序 - 点击切换", "Ordenado por más recientes - clic para cambiar",
        sort_hint_name: "Sorted by name - click to change", "按名称排序 - 点击切换", "Ordenado por nombre - clic para cambiar",
        refresh_title: "Refresh sound pack list", "刷新音效包列表", "Actualizar la lista de paquetes de sonido",
        // One sentence per category, not one with the noun in a slot: the English
        // this migration must not re-word capitalises the category mid-sentence
        // ("No Keyboard sound pack found"), while `keyboard_noun` is lower-case
        // for the sentence positions that already were. Two keys keep the shipped
        // English untouched without case-mangling it at render time.
        empty_keyboard: "No Keyboard sound pack found. You can add new sound packs by clicking the 'Add' button above.", "未找到键盘音效包。你可以点击上方的“添加”按钮添加新的音效包。", "No se encontró ningún paquete de sonido de teclado. Puedes añadir paquetes nuevos con el botón «Añadir» de arriba.",
        empty_mouse: "No Mouse sound pack found. You can add new sound packs by clicking the 'Add' button above.", "未找到鼠标音效包。你可以点击上方的“添加”按钮添加新的音效包。", "No se encontró ningún paquete de sonido de ratón. Puedes añadir paquetes nuevos con el botón «Añadir» de arriba.",
        no_search_results: "No result match your search!", "没有与搜索匹配的结果！", "¡Ningún resultado coincide con tu búsqueda!",
        star_add_title: "Show on the Home list", "在主页列表中显示", "Mostrar en la lista de Inicio",
        star_remove_title: "Remove from the Home list", "从主页列表中移除", "Quitar de la lista de Inicio",
        open_folder_title: "Open soundpack folder", "打开音效包文件夹", "Abrir la carpeta del paquete de sonido",
        row_delete_title: "Delete this soundpack", "删除此音效包", "Eliminar este paquete de sonido",

        // ----- The manager panel -----
        scan_hint: "Click refresh to scan for sound packs", "点击刷新以扫描音效包", "Pulsa actualizar para buscar paquetes de sonido",
        found_one: "Found {} sound pack", "找到 {} 个音效包", "Se encontró {} paquete de sonido",
        found_other: "Found {} sound packs", "找到 {} 个音效包", "Se encontraron {} paquetes de sonido",
        // The colon belongs to the label, where Chinese uses its own full-width
        // one; leaving it at the call site would hard-code the Latin punctuation.
        count_keyboard: "Keyboard: {}", "键盘：{}", "Teclado: {}",
        count_mouse: "Mouse: {}", "鼠标：{}", "Ratón: {}",
        manager_refresh_hint: "Refresh sound pack list to detect newly added or removed sound packs.", "刷新音效包列表以检测新增或移除的音效包。", "Actualiza la lista de paquetes de sonido para detectar los nuevos o los eliminados.",
        refreshing: "Refreshing...", "正在刷新…", "Actualizando…",
        last_scan: "Last scan: {}", "上次扫描：{}", "Último escaneo: {}",
        builtin_folder_title: "Built-in sound packs folder", "内置音效包文件夹", "Carpeta de paquetes de sonido integrados",
        builtin_folder_hint: "Default sound packs that ship with the app.", "随应用自带的默认音效包。", "Los paquetes de sonido predeterminados que incluye la app.",
        custom_folder_title: "Custom sound packs folder", "自定义音效包文件夹", "Carpeta de paquetes de sonido personalizados",
        custom_folder_hint: "Add your own custom sound packs here.", "在此添加你自己的音效包。", "Añade aquí tus propios paquetes de sonido.",
        need_more_title: "Need more sound packs?", "需要更多音效包？", "¿Necesitas más paquetes de sonido?",
        need_more_hint: "Check out the Mechvibes website to find more sound packs. You can also create your own sound packs using the Sound Pack Editor.", "访问 Mechvibes 网站可以找到更多音效包。你也可以用音效包编辑器制作自己的音效包。", "Visita el sitio web de Mechvibes para encontrar más paquetes de sonido. También puedes crear los tuyos con el Editor de paquetes de sonido.",
        browse_packs: "Browse sound packs", "浏览音效包", "Explorar paquetes de sonido",
        open_editor: "Open Editor", "打开编辑器", "Abrir el editor",

        // ----- The import modal -----
        import_title: "Import sound pack", "导入音效包", "Importar paquete de sonido",
        import_instructions: "To import a sound pack, select a ZIP file containing the sound pack structure as shown below:", "导入音效包时，请选择一个包含如下音效包结构的 ZIP 文件：", "Para importar un paquete de sonido, elige un archivo ZIP con la estructura que se muestra abajo:",
        import_notes: "Notes:", "说明：", "Notas:",
        import_note_single: "Use a single sound file to play for the entire sound pack.", "整个音效包只播放这一个音频文件。", "Usa un solo archivo de sonido para todo el paquete.",
        import_note_multi: "Use multiple sound files for different keys.", "为不同按键使用多个音频文件。", "Usa varios archivos de sonido para distintas teclas.",
        import_note_config: "The config.json file must be in the root of the ZIP file", "config.json 文件必须位于 ZIP 文件的根目录", "El archivo config.json debe estar en la raíz del ZIP",
        import_step_file: "Select file (zip)", "选择文件（zip）", "Elegir archivo (zip)",
        import_step_validating: "Validating", "校验中", "Validando",
        import_step_conflicts: "Checking conflicts", "检查冲突", "Comprobando conflictos",
        import_step_installing: "Installing sound pack", "正在安装音效包", "Instalando el paquete de sonido",
        import_step_finalizing: "Finalizing", "收尾中", "Finalizando",
        import_step_refreshing: "Refreshing sound pack list", "正在刷新音效包列表", "Actualizando la lista de paquetes de sonido",
        select_file: "Select file", "选择文件", "Elegir archivo",
        importing: "Importing...", "正在导入…", "Importando…",
        zip_dialog_title: "Select Sound Pack ZIP File", "选择音效包 ZIP 文件", "Elegir archivo ZIP del paquete de sonido",
        zip_filter: "ZIP Files", "ZIP 文件", "Archivos ZIP",
        invalid_file: "Invalid file: {}", "文件无效：{}", "Archivo no válido: {}",
        already_newer: "You already have version {} of this sound pack, which is this one or newer.", "你已有此音效包的 {} 版本，它就是当前这个或更新的版本。", "Ya tienes la versión {} de este paquete, que es esta o una más reciente.",
        already_exists: "A sound pack with ID '{}' already exists.\nPlease remove the existing sound pack and try again.", "已存在 ID 为“{}”的音效包。\n请移除现有的音效包后重试。", "Ya existe un paquete con el ID «{}».\nElimina el paquete existente e inténtalo de nuevo.",
        shadows_builtin: "'{}' is the ID of a built-in sound pack, so the built-in one would hide the pack you are importing.\nEdit the pack's config.json to use a different ID and try again.", "“{}”是内置音效包的 ID，内置包会遮住你要导入的包。\n请修改该包 config.json 中的 ID 后重试。", "«{}» es el ID de un paquete integrado, así que el integrado ocultaría el que importas.\nCambia el ID en el config.json del paquete e inténtalo de nuevo.",
        read_id_failed: "Failed to read soundpack ID: {}", "读取音效包 ID 失败：{}", "No se pudo leer el ID del paquete: {}",
        import_success: "Successfully installed: {}", "安装成功：{}", "Instalado correctamente: {}",

        // ----- The delete confirmation -----
        delete_title: "Delete soundpack", "删除音效包", "Eliminar paquete de sonido",
        delete_question: "Are you sure you want to delete this soundpack?", "确定要删除这个音效包吗？", "¿Seguro que quieres eliminar este paquete de sonido?",
        delete_body: "This action cannot be undone. The soundpack \"{}\" and all its files will be permanently removed.", "此操作无法撤销。音效包“{}”及其所有文件将被永久删除。", "Esta acción no se puede deshacer. El paquete «{}» y todos sus archivos se eliminarán para siempre.",

        // ----- The library install modal -----
        library_title_install: "Install sound pack", "安装音效包", "Instalar paquete de sonido",
        library_title_update: "Update sound pack", "更新音效包", "Actualizar paquete de sonido",
        library_title_updating: "Updating sound pack", "正在更新音效包", "Actualizando el paquete de sonido",
        library_title_updated: "Sound pack updated", "音效包已更新", "Paquete de sonido actualizado",
        library_title_installed: "Sound pack installed", "音效包已安装", "Paquete de sonido instalado",
        library_looking_up: "Looking up the sound pack...", "正在查找音效包…", "Buscando el paquete de sonido…",
        library_meta: "{} · Version {} · {}", "{} · 版本 {} · {}", "{} · Versión {} · {}",
        library_have_version: "You have version {}. Updating replaces it.", "你已有 {} 版本，更新会替换它。", "Tienes la versión {}. Actualizar la reemplaza.",
        library_have_pack: "You already have this pack. Updating replaces it.", "你已拥有这个音效包，更新会替换它。", "Ya tienes este paquete. Actualizar lo reemplaza.",
        library_current: "You already have version {} of this pack.", "你已拥有这个音效包的 {} 版本。", "Ya tienes la versión {} de este paquete.",
        library_trust: "Only install sound packs you trust.", "只安装你信任的音效包。", "Instala solo paquetes de sonido en los que confíes.",
        library_done_installed: "Installed {}. Pick it on the Home page to hear it.", "已安装 {}。在主页选择它即可试听。", "Se instaló {}. Elígelo en la página Inicio para escucharlo.",
        library_done_updated: "Updated {}. Pick it on the Home page to hear it.", "已更新 {}。在主页选择它即可试听。", "Se actualizó {}. Elígelo en la página Inicio para escucharlo.",
        library_update: "Update", "更新", "Actualizar",
        library_install: "Install", "安装", "Instalar",
        library_updating: "Updating...", "正在更新…", "Actualizando…",
        library_installing: "Installing...", "正在安装…", "Instalando…",
        library_install_stopped: "The install stopped unexpectedly.", "安装意外中断。", "La instalación se interrumpió inesperadamente.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{ fill, plural };

    /// English is the language the app already shipped in, so its text is a
    /// contract: this migration re-words nothing on screen except the counter
    /// below. A translator editing the English column by accident fails here,
    /// next to the key, instead of in a screenshot nobody compares.
    #[test]
    fn english_keeps_the_text_the_page_already_had() {
        let expected = [
            ("page_title", EN.page_title, "Sound Packs"),
            ("page_subtitle", EN.page_subtitle, "Manage your sound packs"),
            ("tab_manage", EN.tab_manage, "Manage"),
            ("keyboard_label", EN.keyboard_label, "Keyboard"),
            ("mouse_label", EN.mouse_label, "Mouse"),
            ("select_placeholder", EN.select_placeholder, "Select a {} sound pack..."),
            ("search_hint", EN.search_hint, "Search {} sound packs..."),
            ("not_found", EN.not_found, "No {} sound packs found"),
            ("random_pack_title", EN.random_pack_title, "Pick a random sound pack"),
            ("by_author", EN.by_author, "by {}"),
            ("new_badge", EN.new_badge, "New"),
            ("refresh_title", EN.refresh_title, "Refresh sound pack list"),
            ("import_title", EN.import_title, "Import sound pack"),
            ("import_notes", EN.import_notes, "Notes:"),
            ("delete_question", EN.delete_question, "Are you sure you want to delete this soundpack?"),
            ("library_trust", EN.library_trust, "Only install sound packs you trust."),
        ];
        for (key, actual, shipped) in expected {
            assert_eq!(actual, shipped, "soundpacks.{key} changed text users already had");
        }
    }

    /// The two-key split exists for the languages that spell the positions
    /// differently. A single key plus `to_lowercase()`/`to_uppercase()` at the
    /// call site is the mistake this pins against.
    #[test]
    fn the_category_noun_is_a_separate_key_from_its_label() {
        for (language, strings, keyboard_label, keyboard_noun) in [
            ("en", &EN, "Keyboard", "keyboard"),
            ("es", &ES, "Teclado", "teclado"),
        ] {
            assert_eq!(strings.keyboard_label, keyboard_label, "{language} keyboard label");
            assert_eq!(strings.keyboard_noun, keyboard_noun, "{language} keyboard noun");
        }
        // Chinese does not capitalise, so both positions are the same glyphs —
        // and both are still separate keys, so a later language can split them.
        assert_eq!(ZH.keyboard_label, ZH.keyboard_noun);
        assert_eq!(ZH.mouse_label, ZH.mouse_noun);
    }

    /// The English used to read `Found N sound pack(s)`. The pair is a real
    /// plural now, and the call site picks it with `plural`.
    #[test]
    fn the_pack_counter_uses_real_plurals() {
        let one = plural(EN.found_one, EN.found_other, 1);
        let other = plural(EN.found_one, EN.found_other, 8);
        assert_eq!(fill(one, &[&1]), "Found 1 sound pack");
        assert_eq!(fill(other, &[&8]), "Found 8 sound packs");
        assert!(!EN.found_other.contains("(s)"), "the literal `(s)` is not a plural");
        assert_eq!(ZH.found_one, ZH.found_other, "Chinese does not inflect for number");
    }

    /// The two category-dependent sentences are complete, so neither can be
    /// assembled out of a key that has been case-mangled first.
    #[test]
    fn the_table_says_which_category_is_empty() {
        assert!(EN.empty_keyboard.starts_with("No Keyboard sound pack found."));
        assert!(EN.empty_mouse.starts_with("No Mouse sound pack found."));
        assert!(!ES.empty_keyboard.contains("Teclado"), "mid-sentence Spanish is not capitalised");
        assert!(!ES.empty_mouse.contains("Ratón"), "mid-sentence Spanish is not capitalised");
    }
}
