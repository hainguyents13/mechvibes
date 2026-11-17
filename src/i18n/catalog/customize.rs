//! Customize: the page, its four tabs and the dialogs they open.
//!
//! **Theme names are content, the theme *mode* is copy.** `Light`, `Dark` and
//! `System` are the app's own words for the three looks it ships with, so they
//! are catalog keys here. Every other built-in name (`Cupcake`, `Dracula`, …) is
//! a DaisyUI identifier: it names a theme in files people share and in the CSS
//! the app ships, so it is written the same in every language and is never a
//! key. `theme_mode_*` is what a `format!("{:?}", …)` became.
//!
//! **A word's position is part of its key.** `text` is the logo's text colour,
//! a standalone label; `color_text` is the same English word inside the list of
//! theme colours a logo colour can follow. Spanish lower-cases a word inside a
//! sentence and capitalises a label, and Chinese does not capitalise at all, so
//! one key per position is what keeps both right without touching case at
//! runtime.

zone! {
    /// The Customize page, its tabs, and every dialog behind them.
    CustomizeStrings {
        // ---- The page ----
        page_title: "Customize", "自定义", "Personalizar",
        page_subtitle: "Vibe it your way!", "按你的风格来！", "¡Dale tu estilo!",

        // ---- The tabs ----
        tab_themes: "Themes", "主题", "Temas",
        tab_colors: "Colors", "颜色", "Colores",
        tab_logo: "Logo", "标志", "Logotipo",
        tab_background: "Background", "背景", "Fondo",

        // ---- The theme on screen ----
        share: "Share", "分享", "Compartir",
        import_theme: "Import", "导入", "Importar",
        find_more: "Find more", "发现更多", "Ver más",
        theme_mode_light: "Light", "浅色", "Claro",
        theme_mode_dark: "Dark", "深色", "Oscuro",
        theme_mode_system: "System", "跟随系统", "Sistema",

        // ---- The Themes tab ----
        themes_intro: "A theme is a whole look: colors, logo and background. Put one on, then change any part in the Colors, Logo and Background tabs.",
        "主题是一整套外观：颜色、标志和背景。先套用一个主题，再到颜色、标志和背景标签页里调整任意部分。",
        "Un tema es un aspecto completo: colores, logotipo y fondo. Pon uno y luego cambia lo que quieras en las pestañas Colores, Logotipo y Fondo.",
        themes_file_hint_double_click: "Got a theme file? Double-click it, or drop it on this window.",
        "有主题文件？双击它，或把它拖到此窗口。",
        "¿Tienes un archivo de tema? Haz doble clic en él o arrástralo a esta ventana.",
        themes_file_hint_drop: "Got a theme file? Drop it on this window, or use Import.",
        "有主题文件？把它拖到此窗口，或使用“导入”。",
        "¿Tienes un archivo de tema? Arrástralo a esta ventana o usa Importar.",
        group_ready_made: "Ready-made", "预设", "Prediseñados",
        group_my_themes: "My themes", "我的主题", "Mis temas",
        group_installed: "Installed", "已安装", "Instalados",
        default_look_is_on: "Default look is on.", "已启用默认外观。", "El aspecto predeterminado está activo.",
        themes_made_by_others: "Themes made by other people", "其他人制作的主题", "Temas creados por otras personas",
        delete_title: "Delete {}?", "删除 {}？", "¿Eliminar {}?",
        delete_message: "This cannot be undone.", "此操作无法撤销。", "Esto no se puede deshacer.",

        // ---- A card's menu ----
        duplicate: "Duplicate", "创建副本", "Duplicar",
        more: "More", "更多", "Más",
        more_for: "More for {}", "{} 的更多操作", "Más opciones para {}",

        // ---- The Colors tab ----
        colors_intro: "Pick the app colors, or make your own",
        "选择应用的颜色，或创建自己的颜色",
        "Elige los colores de la app o crea los tuyos",
        colors_ready_made: "Ready-made colors", "预设颜色", "Colores prediseñados",
        colors_mine: "My colors", "我的颜色", "Mis colores",
        colors_none_yet: "You have not made any colors yet",
        "你还没有创建任何颜色",
        "Todavía no has creado ningún color",
        edit: "Edit", "编辑", "Editar",
        new_colors: "New colors", "新建颜色", "Colores nuevos",
        colors_in_use: "These colors belong to the theme you are using. Pick other colors first, then delete them.",
        "这些颜色属于你正在使用的主题。请先选择其他颜色，再删除它们。",
        "Estos colores pertenecen al tema que estás usando. Elige otros colores primero y luego elimínalos.",

        // ---- The colors editor ----
        name_first: "Give it a name first.", "请先输入名称。", "Primero ponle un nombre.",
        colors_no_colors: "There are no colors here. Each line should look like --color-primary: #3b82f6;",
        "这里没有颜色。每一行应形如 --color-primary: #3b82f6;",
        "Aquí no hay colores. Cada línea debería verse así: --color-primary: #3b82f6;",
        colors_some_lines_left_out: "Some lines were left out because they are not colors.",
        "有些行被忽略，因为它们不是颜色。",
        "Algunas líneas se omitieron porque no son colores.",
        edit_colors: "Edit colors", "编辑颜色", "Editar colores",
        name: "Name", "名称", "Nombre",
        colors_name_placeholder: "My colors", "我的颜色", "Mis colores",
        colors_code_legend: "Color code (advanced)", "颜色代码（高级）", "Código de color (avanzado)",
        colors_code_help: "Change the values after each colon. Each line is one color.",
        "修改每个冒号后面的值。每一行代表一种颜色。",
        "Cambia los valores después de cada dos puntos. Cada línea es un color.",

        // ---- The rename dialog ----
        rename_theme_title: "Rename theme", "重命名主题", "Renombrar tema",

        // ---- The Background tab ----
        use_my_background: "Use my own background", "使用我自己的背景", "Usar mi propio fondo",
        background_field: "Background", "背景", "Fondo",

        // ---- The Logo tab ----
        use_my_logo: "Use my own logo", "使用我自己的标志", "Usar mi propio logotipo",
        sound_on: "Sound on", "有声", "Con sonido",
        sound_off: "Sound off", "静音", "Sin sonido",
        border: "Border", "边框", "Borde",
        text: "Text", "文字", "Texto",
        shadow: "Shadow", "阴影", "Sombra",
        logo_background: "Logo background", "标志背景", "Fondo del logotipo",
        logo_muted_background: "Logo background when sound is off",
        "静音时的标志背景",
        "Fondo del logotipo sin sonido",
        logo_fade_when_muted: "Fade the logo when sound is off", "静音时淡化标志", "Atenuar el logotipo sin sonido",

        // ---- The panel the Logo and Background tabs share ----
        preview: "Preview", "预览", "Vista previa",
        part_panel_note: "Changing anything below turns this on.",
        "更改下面的任何设置都会启用此项。",
        "Cambiar cualquier cosa de abajo lo activa.",
        start_over: "Start over", "重新开始", "Empezar de nuevo",
        changes_save_themselves: "Changes save on their own.", "更改会自动保存。", "Los cambios se guardan solos.",

        // ---- The colours a logo or background colour can follow ----
        color_main: "Main color", "主色", "Color principal",
        color_text_on_main: "Text on main color", "主色上的文字", "Texto sobre el color principal",
        color_second: "Second color", "次色", "Color secundario",
        color_text_on_second: "Text on second color", "次色上的文字", "Texto sobre el color secundario",
        color_text: "Text", "文字", "Texto",
        color_background: "Background", "背景", "Fondo",
        color_panel: "Panel", "面板", "Recuadro",
        color_panel_edge: "Panel edge", "面板边框", "Borde del recuadro",
        color_success: "Success", "成功", "Éxito",
        color_text_on_success: "Text on success", "成功色上的文字", "Texto sobre el color de éxito",
        color_warning: "Warning", "警告", "Advertencia",
        color_text_on_warning: "Text on warning", "警告色上的文字", "Texto sobre el color de advertencia",
        color_text_on_error: "Text on error", "错误色上的文字", "Texto sobre el color de error",
        color_type_placeholder: "Or type a color, like #ff0000",
        "或输入颜色，例如 #ff0000",
        "O escribe un color, como #ff0000",

        // ---- The theme import dialog ----
        import_title_use: "Use this theme?", "使用此主题？", "¿Usar este tema?",
        import_title_on: "Theme is on", "主题已启用", "El tema está activo",
        import_title_failed: "Could not use this theme", "无法使用此主题", "No se pudo usar este tema",
        import_wait_opening: "Opening the theme...", "正在打开主题…", "Abriendo el tema...",
        import_wait_looking_up: "Looking up the theme...", "正在查找主题…", "Buscando el tema...",
        import_wait_downloading: "Downloading the theme...", "正在下载主题…", "Descargando el tema...",
        import_wait_putting_on: "Putting the theme on...", "正在应用主题…", "Aplicando el tema...",
        import_choose: "Choose what to use", "选择要使用的内容", "Elige qué usar",
        import_untick_note: "Anything you untick stays as you have it now.",
        "取消勾选的内容会保持现在的样子。",
        "Lo que desmarques se queda como lo tienes ahora.",
        import_yours_stays: "Your current theme stays in the Themes tab.",
        "你当前的主题仍会保留在“主题”标签页。",
        "Tu tema actual permanece en la pestaña Temas.",
        import_find_it: "Find it any time in Customize > Themes. The look you had before is there too.",
        "随时可以在“自定义 > 主题”里找到它。你之前的外观也在那里。",
        "Lo encontrarás en Personalizar > Temas. El aspecto que tenías también está ahí.",
        import_use: "Use this theme", "使用此主题", "Usar este tema",
        // One full sentence per part: "Kept your {}" would need Spanish to agree
        // in number and gender with the word inside it, which a template cannot.
        import_kept_colors: "Kept your colors", "已保留你的颜色", "Conservaste tus colores",
        import_kept_logo: "Kept your logo", "已保留你的标志", "Conservaste tu logotipo",
        import_kept_background: "Kept your background", "已保留你的背景", "Conservaste tu fondo",
        import_colors_line: "Colors: {}", "颜色：{}", "Colores: {}",
        // What the theme brings for a part, as `theme_share::SharedTheme` reads
        // it from the file. The part's own name, when the theme brings nothing,
        // is the tab word above (`tab_logo`, `tab_background`) reused rather
        // than repeated here: same word, same position on screen.
        custom_colors: "Custom colors", "自定义颜色", "Colores personalizados",
        logo_with_images: "Logo with images", "带图片的标志", "Logotipo con imágenes",
        logo_style: "Logo style", "标志样式", "Estilo del logotipo",
        background_image: "Background image", "背景图片", "Imagen de fondo",
        background_color: "Background color", "背景颜色", "Color de fondo",
        // What ticking a part does to the user's own part, and what a part the
        // theme leaves out does (nothing at all).
        import_not_in_theme: "Not in this theme. Yours stays as it is.",
        "此主题不含此项。你的设置保持不变。",
        "Este tema no incluye esto. Lo tuyo se queda como está.",
        import_replaces_logo: "Replaces your logo", "将替换你的标志", "Reemplaza tu logotipo",
        import_replaces_background: "Replaces your background", "将替换你的背景", "Reemplaza tu fondo",
        by_author: "by {}", "作者：{}", "por {}",
        error_opening_stopped: "Opening stopped unexpectedly.",
        "打开意外中断。",
        "La apertura se interrumpió inesperadamente.",
        done: "Done", "完成", "Listo",

        // ---- Words from the look editor, shown as a toast ----
        could_not_put_on: "Could not put {} on. {}", "无法启用 {}。{}", "No se pudo activar {}. {}",
        could_not_copy: "Could not copy {}. {}", "无法复制 {}。{}", "No se pudo copiar {}. {}",
        no_colors_to_copy: "This theme has no colors to copy.",
        "此主题没有可复制的颜色。",
        "Este tema no tiene colores para copiar.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tab labels are the four words on the Customize tab bar and the four
    /// the page showed before this migration. A translator touching the English
    /// column fails here, next to the key.
    #[test]
    fn english_keeps_the_words_customize_already_showed() {
        let expected = [
            ("page_title", EN.page_title, "Customize"),
            ("page_subtitle", EN.page_subtitle, "Vibe it your way!"),
            ("tab_themes", EN.tab_themes, "Themes"),
            ("tab_colors", EN.tab_colors, "Colors"),
            ("tab_logo", EN.tab_logo, "Logo"),
            ("tab_background", EN.tab_background, "Background"),
            ("group_ready_made", EN.group_ready_made, "Ready-made"),
            ("group_my_themes", EN.group_my_themes, "My themes"),
            ("group_installed", EN.group_installed, "Installed"),
        ];
        for (key, actual, shipped) in expected {
            assert_eq!(actual, shipped, "customize.{key} changed the words users already had");
        }
    }

    /// The lines the theme import dialog showed while they were literals in
    /// `utils::theme_share`. The move into the catalog must not rewrite one
    /// word of them.
    #[test]
    fn english_keeps_the_words_the_import_checklist_showed() {
        let expected = [
            ("custom_colors", EN.custom_colors, "Custom colors"),
            ("logo_with_images", EN.logo_with_images, "Logo with images"),
            ("logo_style", EN.logo_style, "Logo style"),
            ("background_image", EN.background_image, "Background image"),
            ("background_color", EN.background_color, "Background color"),
            (
                "import_not_in_theme",
                EN.import_not_in_theme,
                "Not in this theme. Yours stays as it is.",
            ),
            ("import_replaces_logo", EN.import_replaces_logo, "Replaces your logo"),
            ("import_replaces_background", EN.import_replaces_background, "Replaces your background"),
        ];
        for (key, actual, shipped) in expected {
            assert_eq!(actual, shipped, "customize.{key} changed the words users already had");
        }
    }

    /// The theme mode is app copy; a DaisyUI name is an identifier. Only these
    /// three are keys, and they are keys because the screen used to print
    /// `format!("{:?}", …)`.
    #[test]
    fn the_theme_mode_is_the_only_theme_name_in_the_catalog() {
        let entries: Vec<&str> = ENTRIES
            .iter()
            .filter(|(key, ..)| key.starts_with("theme_mode_"))
            .map(|(key, ..)| *key)
            .collect();
        assert_eq!(entries, ["theme_mode_light", "theme_mode_dark", "theme_mode_system"]);
    }
}
