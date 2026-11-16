//! Words that more than one screen uses: the buttons and labels that would
//! otherwise be translated three times with three different spellings.

zone! {
    /// Buttons, dialog actions and one-word labels shared across screens.
    ///
    /// A word that only one screen uses belongs in that screen's zone, not here.
    CommonStrings {
        cancel: "Cancel", "取消", "Cancelar",
        save: "Save", "保存", "Guardar",
        delete: "Delete", "删除", "Eliminar",
        remove: "Remove", "移除", "Quitar",
        add: "Add", "添加", "Añadir",
        close: "Close", "关闭", "Cerrar",
        rename: "Rename", "重命名", "Renombrar",
        refresh: "Refresh", "刷新", "Actualizar",
        retry: "Retry", "重试", "Reintentar",
        loading: "Loading…", "加载中…", "Cargando…",
        unknown: "Unknown", "未知", "Desconocido",
        error: "Error", "错误", "Error",
        none: "None", "无", "Ninguno",
        default: "Default", "默认", "Predeterminado",
        search: "Search", "搜索", "Buscar",
        browse: "Browse…", "浏览…", "Examinar…",
        not_now: "Not now", "以后再说", "Ahora no",
    }
}
