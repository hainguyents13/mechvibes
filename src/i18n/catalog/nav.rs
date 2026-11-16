//! Navigation labels: the dock and, later, the tab bars that reuse them.
//!
//! These are the shortest strings in the app and the ones with the least room.
//! Each dock button is one column of a five-column bar with a fixed slice of the
//! window width, so a label that wraps does not just look wrong — it pushes the
//! icon row up and misaligns the whole bar. A translation that does not fit is
//! shortened here; the dock is not restructured around it.

zone! {
    /// The five dock destinations, in the order the dock shows them.
    NavStrings {
        home: "Home", "首页", "Inicio",
        soundpacks: "Sound packs", "音效包", "Paquetes",
        customize: "Customize", "自定义", "Personalizar",
        mood: "Mood", "氛围", "Ambiente",
        settings: "Settings", "设置", "Ajustes",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// English is the language the app already shipped in, so its labels are a
    /// contract: this migration re-words nothing on screen. A translator editing
    /// the English column by accident fails here, next to the key, instead of in
    /// a screenshot nobody compares.
    #[test]
    fn english_keeps_the_labels_the_dock_already_had() {
        let expected = [
            ("home", EN.home, "Home"),
            ("soundpacks", EN.soundpacks, "Sound packs"),
            ("customize", EN.customize, "Customize"),
            ("mood", EN.mood, "Mood"),
            ("settings", EN.settings, "Settings"),
        ];
        for (key, actual, shipped) in expected {
            assert_eq!(actual, shipped, "nav.{key} changed the dock label users already had");
        }
    }
}
