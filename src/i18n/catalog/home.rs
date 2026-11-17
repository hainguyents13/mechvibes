//! The Home page: the pack pickers, the volume sliders and the
//! update banner.
//!
//! The pickers themselves (`soundpack_selector.rs`) and the sliders
//! (`volume_slider.rs`) read their own zones — `soundpacks` and `chrome` — and
//! this file holds only the copy that lives in `pages/home.rs` and in
//! `components/update_button.rs`: the "Enable all sounds" row, the editor link
//! and every sentence the update banner can show.
//!
//! **The relative-time pairs** (`time_*_one` / `time_*_other`) are read by
//! [`crate::utils::time::format_relative_time_in`]. Each member carries its own
//! number, so the singular is a finished sentence (`"1 minute ago"`) and the
//! plural is a template (`"{} minutes ago"`): the formatter picks a member with
//! [`crate::i18n::plural`] and always runs it through [`crate::i18n::fill`],
//! which leaves a template without `{}` untouched. It never assembles the
//! sentence itself, so a language can word it in any order. Chinese does not
//! inflect for number, so both members of a Chinese pair read the same apart
//! from the number they carry.
//!
//! **The update failure reasons** are the one place where copy arrives from
//! outside this component. `UpdateStage::Failed` carries a `String` built in
//! `auto_updater`/`update_installer`, where translating is not allowed (their
//! `Display` is a stable English description that also reaches the log). The
//! keys below give the frame of each of those sentences a translation; the
//! detail inside the frame — a hash, an OS error, a URL — stays exactly as the
//! failing layer wrote it.

zone! {
    /// See the module comment.
    HomeStrings {
        // ----- The Home page body -----
        enable_all_sounds: "Enable all sounds", "启用所有声音", "Activar todos los sonidos",
        sound_pack_editor: "Sound pack editor", "音效包编辑器", "Editor de paquetes de sonido",
        editor_open: "Open", "打开", "Abrir",

        // ----- The update banner -----
        update_now: "Update now", "立即更新", "Actualizar ahora",
        update_downloading: "Downloading new version...", "正在下载新版本…", "Descargando la nueva versión…",
        update_installing: "Installing...", "正在安装…", "Instalando…",
        // The version arrives already carrying its `v` from the call site: that
        // prefix is part of the version, not of the sentence.
        update_ready: "{} downloaded and verified.", "{} 已下载并校验。", "{} descargada y verificada.",
        update_get_version: "Get {}", "获取 {}", "Obtener {}",
        update_open_page: "Open download page", "打开下载页面", "Abrir la página de descarga",

        // ----- Why the update failed -----
        // The three sentences `auto_updater` writes itself.
        update_failed_no_asset: "No installer asset in this release", "此版本没有安装程序文件", "Esta versión no incluye instalador",
        update_failed_no_checksums: "This release has no published checksums; use the download page.", "此版本没有发布校验和，请使用下载页面。", "Esta versión no publica sumas de verificación; usa la página de descarga.",
        update_failed_checksum_discarded: "The downloaded installer failed verification and was discarded.", "下载的安装程序未通过校验，已被丢弃。", "El instalador descargado no pasó la verificación y se descartó.",
        // `StageError`'s `Display` frames, re-stated here so the frame can be
        // read in the user's language while the detail inside it is not touched.
        update_failed_download: "Download failed: {}", "下载失败：{}", "Error de descarga: {}",
        update_failed_checksum_unavailable: "Checksum unavailable: {}", "校验和不可用：{}", "Suma de verificación no disponible: {}",
        update_failed_checksum_mismatch: "Checksum mismatch: expected {}, got {}", "校验和不匹配：期望 {}，实际 {}", "Suma de verificación distinta: se esperaba {}, se obtuvo {}",
        update_failed_io: "I/O error: {}", "I/O 错误：{}", "Error de E/S: {}",
        update_failed_unsupported: "Unsupported: {}", "不支持：{}", "No compatible: {}",

        // ----- Relative time, read by utils/time.rs -----
        // The thresholds are Rust's, in `format_relative_time_in`; only the
        // wording lives here. One pair per unit, the singular a finished
        // sentence and the plural a template.
        time_just_now: "Just now", "刚刚", "Ahora mismo",
        time_minute_one: "1 minute ago", "1 分钟前", "Hace 1 minuto",
        time_minute_other: "{} minutes ago", "{} 分钟前", "Hace {} minutos",
        time_hour_one: "1 hour ago", "1 小时前", "Hace 1 hora",
        time_hour_other: "{} hours ago", "{} 小时前", "Hace {} horas",
        time_day_one: "1 day ago", "1 天前", "Hace 1 día",
        time_day_other: "{} days ago", "{} 天前", "Hace {} días",
        time_week_one: "1 week ago", "1 周前", "Hace 1 semana",
        time_week_other: "{} weeks ago", "{} 周前", "Hace {} semanas",
        time_month_one: "1 month ago", "1 个月前", "Hace 1 mes",
        time_month_other: "{} months ago", "{} 个月前", "Hace {} meses",
        time_year_one: "1 year ago", "1 年前", "Hace 1 año",
        time_year_other: "{} years ago", "{} 年前", "Hace {} años",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{ fill, plural };

    /// English is the language the app already shipped in, so its text is a
    /// contract: this migration re-words nothing on screen. A translator editing
    /// the English column by accident fails here, next to the key, instead of in
    /// a screenshot nobody compares.
    #[test]
    fn english_keeps_the_text_the_page_already_had() {
        let expected = [
            ("enable_all_sounds", EN.enable_all_sounds, "Enable all sounds"),
            ("sound_pack_editor", EN.sound_pack_editor, "Sound pack editor"),
            ("editor_open", EN.editor_open, "Open"),
            ("update_now", EN.update_now, "Update now"),
            ("update_downloading", EN.update_downloading, "Downloading new version..."),
            ("update_installing", EN.update_installing, "Installing..."),
            ("update_ready", EN.update_ready, "{} downloaded and verified."),
            ("update_get_version", EN.update_get_version, "Get {}"),
            ("update_open_page", EN.update_open_page, "Open download page"),
            ("update_failed_no_asset", EN.update_failed_no_asset, "No installer asset in this release"),
            (
                "update_failed_no_checksums",
                EN.update_failed_no_checksums,
                "This release has no published checksums; use the download page.",
            ),
            (
                "update_failed_checksum_discarded",
                EN.update_failed_checksum_discarded,
                "The downloaded installer failed verification and was discarded.",
            ),
            ("update_failed_download", EN.update_failed_download, "Download failed: {}"),
            (
                "update_failed_checksum_unavailable",
                EN.update_failed_checksum_unavailable,
                "Checksum unavailable: {}",
            ),
            (
                "update_failed_checksum_mismatch",
                EN.update_failed_checksum_mismatch,
                "Checksum mismatch: expected {}, got {}",
            ),
            ("update_failed_io", EN.update_failed_io, "I/O error: {}"),
            ("update_failed_unsupported", EN.update_failed_unsupported, "Unsupported: {}"),
            ("time_just_now", EN.time_just_now, "Just now"),
            ("time_minute_one", EN.time_minute_one, "1 minute ago"),
            ("time_minute_other", EN.time_minute_other, "{} minutes ago"),
            ("time_hour_one", EN.time_hour_one, "1 hour ago"),
            ("time_hour_other", EN.time_hour_other, "{} hours ago"),
            ("time_day_one", EN.time_day_one, "1 day ago"),
            ("time_day_other", EN.time_day_other, "{} days ago"),
            ("time_week_one", EN.time_week_one, "1 week ago"),
            ("time_week_other", EN.time_week_other, "{} weeks ago"),
            ("time_month_one", EN.time_month_one, "1 month ago"),
            ("time_month_other", EN.time_month_other, "{} months ago"),
            ("time_year_one", EN.time_year_one, "1 year ago"),
            ("time_year_other", EN.time_year_other, "{} years ago"),
        ];
        for (key, actual, shipped) in expected {
            assert_eq!(actual, shipped, "home.{key} changed text users already had");
        }
    }

    /// The singular member is a finished sentence and the plural one is a
    /// template, so `fill` must produce the same sentence from either when the
    /// count is 1. A language that inflects for number has to agree with itself
    /// here: this is what makes the formatter's "pick a member, then fill it"
    /// shape work without ever branching on the count to build the sentence.
    #[test]
    fn the_singular_member_is_already_a_whole_sentence() {
        for strings in [&EN, &ZH, &ES] {
            for (one, other) in [
                (strings.time_minute_one, strings.time_minute_other),
                (strings.time_hour_one, strings.time_hour_other),
                (strings.time_day_one, strings.time_day_other),
                (strings.time_week_one, strings.time_week_other),
                (strings.time_month_one, strings.time_month_other),
                (strings.time_year_one, strings.time_year_other),
            ] {
                assert_eq!(
                    fill(plural(one, other, 1), &[&1]),
                    one,
                    "the singular `{one}` is not what the pair says for 1"
                );
            }
        }
    }

    /// Chinese has no plural inflection, so its two members differ only in the
    /// number they carry — the plural template filled with 1 must read exactly
    /// like the singular. A translator who writes a different wording for the
    /// two halves of a Chinese pair fails here.
    #[test]
    fn chinese_does_not_inflect_for_number() {
        for (one, other) in [
            (ZH.time_minute_one, ZH.time_minute_other),
            (ZH.time_hour_one, ZH.time_hour_other),
            (ZH.time_day_one, ZH.time_day_other),
            (ZH.time_week_one, ZH.time_week_other),
            (ZH.time_month_one, ZH.time_month_other),
            (ZH.time_year_one, ZH.time_year_other),
        ] {
            assert_eq!(fill(other, &[&1]), one, "zh `{one}` / `{other}` inflect differently");
        }
    }

    /// The version arrives with its `v` from the call site, so both sentences
    /// have to read exactly as the banner did once it is filled in.
    #[test]
    fn the_version_sentences_still_read_as_shipped() {
        assert_eq!(fill(EN.update_get_version, &[&"v1.2.3"]), "Get v1.2.3");
        assert_eq!(fill(EN.update_ready, &[&"v1.2.3"]), "v1.2.3 downloaded and verified.");
    }
}
