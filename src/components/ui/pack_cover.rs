use dioxus::prelude::*;

/// The two letters a generated cover shows: the first letter of the first two words of the pack's name,
/// or the first two letters when the name is one word.
pub fn pack_initials(name: &str) -> (char, char) {
    let mut words = name.split(|c: char| !c.is_alphanumeric()).filter(|word| !word.is_empty());
    let first = words.next().unwrap_or("?");
    let mut first_chars = first.chars();
    let a = first_chars.next().unwrap_or('?');
    let b = words
        .next()
        .and_then(|word| word.chars().next())
        .or_else(|| first_chars.next())
        .unwrap_or(a);
    (upper(a), upper(b))
}

fn upper(c: char) -> char {
    c.to_uppercase().next().unwrap_or(c)
}

/// The hue a pack's cover is drawn in, worked out from its name so a pack keeps its colour everywhere.
pub fn pack_hue(name: &str) -> u32 {
    name.chars().fold(0u32, |hue, c| (hue * 31 + c as u32) % 360)
}

#[derive(Props, PartialEq, Clone)]
pub struct PackCoverProps {
    /// The pack's name, which gives the cover its letters and colour.
    pub name: String,
    /// Width and height in pixels.
    pub size: u32,
}

/// The cover of a pack that has no icon: a square in a colour picked from the pack's name, holding a 2x2
/// grid with the pack's initials top left and bottom right, and two squares a shade darker than the
/// background in the other corners.
#[component]
pub fn PackCover(props: PackCoverProps) -> Element {
    let (first, second) = pack_initials(&props.name);
    let hue = pack_hue(&props.name);
    let size = props.size as f32;
    let pad = (size * 0.1).round().max(2.0);
    let gap = (size * 0.06).round().max(2.0);
    let radius = (size * 0.22).round();
    let inner = (size * 0.1).round().max(2.0);
    let font = (size * 0.29).round();

    let box_style = format!(
        "display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); grid-template-rows: repeat(2, minmax(0, 1fr)); flex-shrink: 0; box-sizing: border-box; width: {size}px; height: {size}px; border-radius: {radius}px; padding: {pad}px; gap: {gap}px; background: hsl({hue} 70% 86%);"
    );
    let letter_style = format!(
        "display: flex; align-items: center; justify-content: center; font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; font-weight: 800; line-height: 1; border-radius: {inner}px; background: hsl({hue} 70% 91%); color: hsl({hue} 45% 20%); font-size: {font}px;"
    );
    let square_style = format!("border-radius: {inner}px; background: hsl({hue} 66% 81%);");

    rsx! {
      div {
        style: "{box_style}",
        "aria-hidden": "true",
        span {
          style: "{letter_style}",
          "{first}"
        }
        span { style: "{square_style}" }
        span { style: "{square_style}" }
        span {
          style: "{letter_style}",
          "{second}"
        }
      }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initials_come_from_the_first_two_words() {
        assert_eq!(pack_initials("amazin keychron q3 bluex"), ('A', 'K'));
        assert_eq!(pack_initials("ASMR Programming Soundpack"), ('A', 'P'));
        assert_eq!(pack_initials("(THAT IS THE CREAMIEST) WOB Rainy 75"), ('T', 'I'));
        assert_eq!(pack_initials("BFDI: TPOT Recovery Center"), ('B', 'T'));
    }

    #[test]
    fn a_one_word_name_uses_its_first_two_letters() {
        assert_eq!(pack_initials("Varmillo"), ('V', 'A'));
        assert_eq!(pack_initials("x"), ('X', 'X'));
        assert_eq!(pack_initials("  "), ('?', '?'));
    }

    #[test]
    fn a_name_always_gets_the_same_hue() {
        assert_eq!(pack_hue("Orange deluxe"), pack_hue("Orange deluxe"));
        assert!(pack_hue("Orange deluxe") < 360);
        assert_ne!(pack_hue("Orange deluxe"), pack_hue("Gateron Ink Red"));
    }
}
