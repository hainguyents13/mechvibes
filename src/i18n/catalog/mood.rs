//! Mood: the ambient sound mixer and the built-in ambience texts.
//!
//! **The ten built-in ambiences are looked up by `id`, never by their English
//! name.** Their name and description currently live as data in
//! `state::ambiance` (`AmbianceSound` derives `Serialize`/`Deserialize`, so its
//! shape is not something a translation gets to change). The catalog is the
//! source of the visible text and [`ambience_text`] maps an ambience `id` to it;
//! the struct fields stay as the fallback for a sound the catalog does not know
//! — a custom one, or a built-in id that was added to the list before its text
//! landed.
//!
//! That lookup is the one gap this design can leave: nothing makes the compiler
//! notice that an ambience was added to the list without its two keys.
//! `every_built_in_ambience_has_its_text_in_every_language` walks every embedded
//! id in every language and fails until they are there.

zone! {
    /// Every string the Mood page shows, plus the ten built-in ambiences.
    MoodStrings {
        // ----- Page header -----
        page_title: "Mood", "氛围", "Ambiente",
        page_subtitle: "Ambient sounds to set the perfect atmosphere", "营造完美氛围的环境音效", "Sonidos ambientales para crear la atmósfera perfecta",

        // ----- The mixer -----
        mixer_title: "Ambiance sounds", "环境音效", "Sonidos ambientales",
        // The empty state and the counter say the same thing two ways, so they
        // are two keys rather than one with a zero in its slot.
        no_sounds_selected: "Select at least one sound to play", "至少选择一个音效来播放", "Elige al menos un sonido para reproducir",
        // Was `format!("{} active sound{}", n, if n == 1 { "" } else { "s" })`:
        // a real pair now, picked with `plural` at the call site.
        active_sounds_one: "{} active sound", "{} 个已启用音效", "{} sonido activo",
        active_sounds_other: "{} active sounds", "{} 个已启用音效", "{} sonidos activos",
        silenced_hint: "Mood is quiet while Mechvibes is muted or your microphone is in use.", "当 Mechvibes 静音或正在使用麦克风时，氛围音效会保持安静。", "El ambiente queda en silencio mientras Mechvibes está silenciado o el micrófono está en uso.",
        remove_sound_title: "Remove this sound", "移除此音效", "Quitar este sonido",

        // ----- Adding a sound of your own -----
        // The button and the picker's title are the same English sentence in two
        // positions: Spanish wants the infinitive in a title (`Añadir`) and the
        // imperative on a button (`Añade`), so they are two keys.
        add_sound: "Add your own sound", "添加你自己的音效", "Añade tu propio sonido",
        add_sound_title: "Add your own sound", "添加你自己的音效", "Añadir tu propio sonido",
        sounds_filter: "Sounds", "音频文件", "Sonidos",
        adding: "Adding...", "正在添加…", "Añadiendo…",
        add_failed: "Could not add the sound: {}", "无法添加音效：{}", "No se pudo añadir el sonido: {}",
        already_added: "This sound is already in your list.", "此音效已在你的列表中。", "Este sonido ya está en tu lista.",
        sound_formats_hint: "MP3, WAV, OGG, FLAC or M4A. It plays on repeat.", "支持 MP3、WAV、OGG、FLAC 或 M4A，循环播放。", "MP3, WAV, OGG, FLAC o M4A. Se reproduce en bucle.",
        // The description a sound the user added shows in the list. Its own
        // field keeps the English default; the page reads this key instead.
        custom_sound_description: "Your sound", "你的音效", "Tu sonido",

        // ----- The ten built-in ambiences, by id -----
        // `ambiance_<id>_name` / `ambiance_<id>_description`. The id is the
        // stable data key in `state::ambiance`; the English column is exactly the
        // text the list already carried.
        ambiance_rain_name: "Rain", "雨声", "Lluvia",
        ambiance_rain_description: "Gentle rainfall", "轻柔的雨声", "Lluvia suave",
        ambiance_forest_name: "Forest", "森林", "Bosque",
        ambiance_forest_description: "Birds chirping in the forest", "森林中的鸟鸣", "Pájaros cantando en el bosque",
        ambiance_ocean_name: "Ocean Waves", "海浪", "Olas del mar",
        ambiance_ocean_description: "Calming ocean waves", "平静的海浪", "Olas del mar relajantes",
        ambiance_thunderstorm_name: "Thunderstorm", "雷雨", "Tormenta",
        ambiance_thunderstorm_description: "Thunder and heavy rain", "雷声与大雨", "Truenos y lluvia intensa",
        ambiance_fire_name: "Campfire", "篝火", "Hoguera",
        ambiance_fire_description: "Crackling campfire", "噼啪作响的篝火", "Hoguera crepitante",
        ambiance_wind_name: "Wind", "风声", "Viento",
        ambiance_wind_description: "Gentle wind through trees", "穿过树林的微风", "Viento suave entre los árboles",
        ambiance_stream_name: "Stream", "溪流", "Arroyo",
        ambiance_stream_description: "Babbling brook", "潺潺的小溪", "Arroyo murmurante",
        ambiance_river_name: "River", "河流", "Río",
        ambiance_river_description: "Flowing river water", "流淌的河水", "Agua de río que fluye",
        ambiance_cricket_name: "Night Crickets", "夜晚蟋蟀", "Grillos nocturnos",
        ambiance_cricket_description: "Crickets and night sounds", "蟋蟀与夜晚的声音", "Grillos y sonidos de la noche",
        ambiance_chatter_name: "Coffee Shop", "咖啡馆", "Cafetería",
        ambiance_chatter_description: "Ambient coffee shop chatter", "咖啡馆的嘈杂人声", "Murmullo de cafetería",
    }
}

/// The name and description of a built-in ambience, in the language being drawn.
///
/// Looked up by the sound's `id` (`"rain"`, `"forest"`, …), not by its English
/// name: the name is what the fallback shows, and matching on it would make an
/// English word the key. Same reasoning as the colour labels, which are looked
/// up by the colour.
///
/// `None` for a sound the user added (`custom-<hash>`) and for a built-in id the
/// catalog does not know yet. The page then shows the `name` and `description`
/// the sound was listed with, which is why `AmbianceSound` keeps both fields and
/// its serialized shape.
pub fn ambience_text<'a>(strings: &'a MoodStrings, id: &str) -> Option<(&'a str, &'a str)> {
    Some(match id {
        "rain" => (strings.ambiance_rain_name, strings.ambiance_rain_description),
        "forest" => (strings.ambiance_forest_name, strings.ambiance_forest_description),
        "ocean" => (strings.ambiance_ocean_name, strings.ambiance_ocean_description),
        "thunderstorm" => (
            strings.ambiance_thunderstorm_name,
            strings.ambiance_thunderstorm_description,
        ),
        "fire" => (strings.ambiance_fire_name, strings.ambiance_fire_description),
        "wind" => (strings.ambiance_wind_name, strings.ambiance_wind_description),
        "stream" => (strings.ambiance_stream_name, strings.ambiance_stream_description),
        "river" => (strings.ambiance_river_name, strings.ambiance_river_description),
        "cricket" => (strings.ambiance_cricket_name, strings.ambiance_cricket_description),
        "chatter" => (strings.ambiance_chatter_name, strings.ambiance_chatter_description),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::{ ambience_text, EN, ES, ZH };
    use crate::i18n::{ fill, plural, Language };
    use crate::state::ambiance::AmbiancePlayerState;

    /// Every embedded ambience id resolves, in each of the three languages, to a
    /// non-blank name and description.
    ///
    /// This is the gap the by-id design leaves: the list is data, so nothing
    /// makes the compiler notice that a sound was added and its
    /// `ambiance_<id>_name`/`_description` were not. A missing arm returns
    /// `None` here and the failure names the id, instead of the page quietly
    /// falling back to English on a translated screen.
    #[test]
    fn every_built_in_ambience_has_its_text_in_every_language() {
        let builtin = AmbiancePlayerState::get_builtin_sounds();
        assert!(!builtin.is_empty(), "the built-in list must not be empty");

        for language in Language::ALL {
            let strings = language.strings();
            for sound in &builtin {
                let (name, description) = ambience_text(&strings.mood, &sound.id).unwrap_or_else(|| {
                    panic!(
                        "ambience '{}' has no text in {}: add `ambiance_{}_name` and \
                         `ambiance_{}_description` to MoodStrings",
                        sound.id,
                        language.tag(),
                        sound.id,
                        sound.id
                    )
                });
                assert!(
                    !name.trim().is_empty(),
                    "ambience '{}' has a blank name in {}",
                    sound.id,
                    language.tag()
                );
                assert!(
                    !description.trim().is_empty(),
                    "ambience '{}' has a blank description in {}",
                    sound.id,
                    language.tag()
                );
            }
        }
    }

    /// The English column is the text the app already showed, byte for byte:
    /// this migration moves the words, it does not re-word them. The list's own
    /// fields are the reference, since they are what the page drew until now.
    #[test]
    fn the_english_ambience_text_is_the_text_the_list_already_had() {
        for sound in AmbiancePlayerState::get_builtin_sounds() {
            let (name, description) = ambience_text(&EN, &sound.id)
                .unwrap_or_else(|| panic!("ambience '{}' has no English text", sound.id));
            assert_eq!(name, sound.name, "ambience '{}' changed its name", sound.id);
            assert_eq!(
                description,
                sound.description,
                "ambience '{}' changed its description",
                sound.id
            );
        }
    }

    /// The page shows the sound's own fields when the lookup misses, so a sound
    /// the user added — whose id is `custom-<hash>` — has to miss: its text is
    /// the file's name plus `custom_sound_description`, never a built-in's.
    #[test]
    fn an_id_that_is_not_a_built_in_ambience_has_no_catalog_text() {
        assert!(ambience_text(&EN, "custom-0f3a5c").is_none());
        assert!(ambience_text(&ES, "rain2").is_none());
    }

    /// The page-level English, pinned next to its key. A translator editing the
    /// English column by accident fails here, next to the key, instead of in a
    /// screenshot nobody compares.
    #[test]
    fn english_keeps_the_text_the_mixer_already_had() {
        let expected = [
            ("page_title", EN.page_title, "Mood"),
            (
                "page_subtitle",
                EN.page_subtitle,
                "Ambient sounds to set the perfect atmosphere",
            ),
            ("mixer_title", EN.mixer_title, "Ambiance sounds"),
            ("no_sounds_selected", EN.no_sounds_selected, "Select at least one sound to play"),
            (
                "silenced_hint",
                EN.silenced_hint,
                "Mood is quiet while Mechvibes is muted or your microphone is in use.",
            ),
            ("remove_sound_title", EN.remove_sound_title, "Remove this sound"),
            ("add_sound", EN.add_sound, "Add your own sound"),
            ("add_sound_title", EN.add_sound_title, "Add your own sound"),
            ("sounds_filter", EN.sounds_filter, "Sounds"),
            ("adding", EN.adding, "Adding..."),
            ("add_failed", EN.add_failed, "Could not add the sound: {}"),
            ("already_added", EN.already_added, "This sound is already in your list."),
            (
                "sound_formats_hint",
                EN.sound_formats_hint,
                "MP3, WAV, OGG, FLAC or M4A. It plays on repeat.",
            ),
            ("custom_sound_description", EN.custom_sound_description, "Your sound"),
        ];
        for (key, actual, shipped) in expected {
            assert_eq!(actual, shipped, "mood.{key} changed text users already had");
        }
    }

    /// The panel used to build `"{} active sound(s)"` with an `if` on the count
    /// at the call site. The pair is a real plural now, and the call site picks
    /// it with `plural`.
    #[test]
    fn the_active_sound_counter_uses_real_plurals() {
        let one = plural(EN.active_sounds_one, EN.active_sounds_other, 1);
        let other = plural(EN.active_sounds_one, EN.active_sounds_other, 3);
        assert_eq!(fill(one, &[&1]), "1 active sound");
        assert_eq!(fill(other, &[&3]), "3 active sounds");
        assert_eq!(fill(EN.active_sounds_other, &[&0]), "0 active sounds");
        assert!(!EN.active_sounds_other.contains("(s)"), "the literal `(s)` is not a plural");
        assert_eq!(
            ZH.active_sounds_one, ZH.active_sounds_other,
            "Chinese does not inflect for number"
        );
    }
}
