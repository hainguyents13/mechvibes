//! The picture the import dialog shows for a theme, picked the way the theme
//! library picks the picture on its card: the look's own image when it shows
//! one, otherwise its colours.

use super::{ jpeg_orientation, small_enough_to_decode, upright, SharedColors, SharedTheme, MAX_SHRINK_PIXELS };
use base64::Engine;
use std::io::Cursor;

/// The app window's shape at its default size (470 x 820 pt), large enough
/// for the dialog's picture on a 2x screen. The theme library makes its card
/// pictures at the same size.
const THUMBNAIL_BOX: (u32, u32) = (360, 628);
const THUMBNAIL_JPEG_QUALITY: u8 = 80;

/// The stripes a picture made of colours shows, top to bottom, with the
/// theme variable each one paints.
const STRIPES: [(&str, &str); 5] = [
    ("--color-base-100", "bg-base-100"),
    ("--color-primary", "bg-primary"),
    ("--color-secondary", "bg-secondary"),
    ("--color-accent", "bg-accent"),
    ("--color-neutral", "bg-neutral"),
];

impl SharedTheme {
    /// The image that stands for the look: the wallpaper when the background
    /// shows one, otherwise the logo's image when the logo shows one.
    pub fn preview_image(&self) -> Option<&[u8]> {
        let shown = |on: bool, name: Option<&String>| {
            name.filter(|_| on).and_then(|name| self.images.get(name))
        };
        let background = self.background.as_ref().and_then(|b| shown(b.use_image, b.background_image.as_ref()));
        let logo = self.logo.as_ref().and_then(|l| shown(l.use_background_image, l.background_image.as_ref()));
        background.or(logo).map(Vec::as_slice)
    }

    /// `preview_image` cut to the window's shape and shrunk, as a `data:` URI
    /// the dialog can show without the page reaching any file. `None` when the
    /// look shows no image, or its image is too large or damaged to decode;
    /// the dialog then shows the colours. Slow for a large wallpaper, so call
    /// it off the UI thread.
    pub fn preview_data_uri(&self) -> Option<String> {
        let (bytes, media_type) = thumbnail(self.preview_image()?, MAX_SHRINK_PIXELS)?;
        Some(format!("data:{};base64,{}", media_type, base64::engine::general_purpose::STANDARD.encode(bytes)))
    }
}

/// The daisyUI classes of the stripes that picture `colors`. A built-in theme
/// shows all of them. Custom colours show only the ones their CSS sets, since
/// any other would take its colour from the theme the app has on.
pub fn preview_stripes(colors: &SharedColors) -> Vec<&'static str> {
    match colors {
        SharedColors::BuiltIn(_) => STRIPES.iter().map(|(_, class)| *class).collect(),
        SharedColors::Custom { css } => {
            let set: Vec<&str> = css
                .lines()
                .filter_map(|line| line.split_once(':').map(|(property, _)| property.trim()))
                .collect();
            STRIPES.iter()
                .filter(|(variable, _)| set.contains(variable))
                .map(|(_, class)| *class)
                .collect()
        }
    }
}

/// `bytes` cut around the centre to the window's shape and shrunk to fit
/// `THUMBNAIL_BOX`, never enlarged, with its media type. An animation keeps
/// its first frame. PNG when it has transparency, JPEG otherwise. `None` for
/// an image over `max_pixels`, which is not decoded at all, or one that
/// cannot be decoded.
fn thumbnail(bytes: &[u8], max_pixels: u64) -> Option<(Vec<u8>, &'static str)> {
    use image::{ codecs::jpeg::JpegEncoder, imageops::FilterType, ImageFormat };

    if !small_enough_to_decode(bytes, max_pixels) {
        return None;
    }
    let mut image = upright(image::load_from_memory(bytes).ok()?, jpeg_orientation(bytes));
    let (width, height) = (image.width(), image.height());
    let (box_width, box_height) = THUMBNAIL_BOX;
    let shape = (box_width as f64) / (box_height as f64);
    let crop_width = width.min(((height as f64) * shape).round() as u32).max(1);
    let crop_height = height.min(((width as f64) / shape).round() as u32).max(1);
    image = image.crop_imm((width - crop_width) / 2, (height - crop_height) / 2, crop_width, crop_height);
    if image.width() > box_width || image.height() > box_height {
        image = image.resize(box_width, box_height, FilterType::Triangle);
    }

    let mut out = Vec::new();
    if image.color().has_alpha() && image.to_rgba8().pixels().any(|pixel| pixel[3] < 255) {
        image.write_to(&mut Cursor::new(&mut out), ImageFormat::Png).ok()?;
        Some((out, "image/png"))
    } else {
        JpegEncoder::new_with_quality(&mut out, THUMBNAIL_JPEG_QUALITY).encode_image(&image.to_rgb8()).ok()?;
        Some((out, "image/jpeg"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::libs::theme::BuiltInTheme;
    use crate::state::config::{ BackgroundCustomization, LogoCustomization };
    use std::collections::HashMap;

    fn png(width: u32, height: u32, pixel: image::Rgba<u8>) -> Vec<u8> {
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(width, height, pixel))
            .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        bytes
    }

    const RED: image::Rgba<u8> = image::Rgba([255, 0, 0, 255]);
    const BLUE: image::Rgba<u8> = image::Rgba([0, 0, 255, 255]);

    /// A look whose wallpaper is red and whose logo image is blue, both shown.
    fn look() -> SharedTheme {
        let logo = LogoCustomization {
            background_image: Some("logo.png".to_string()),
            use_background_image: true,
            ..LogoCustomization::default()
        };
        let background = BackgroundCustomization {
            background_color: "#101010".to_string(),
            background_image: Some("background.png".to_string()),
            use_image: true,
        };
        SharedTheme {
            name: "Night drive".to_string(),
            colors: SharedColors::BuiltIn(BuiltInTheme::Synthwave),
            logo: Some(logo),
            background: Some(background),
            images: HashMap::from([
                ("logo.png".to_string(), png(4, 4, BLUE)),
                ("background.png".to_string(), png(4, 4, RED)),
            ]),
        }
    }

    #[test]
    fn the_wallpaper_stands_for_the_look_then_the_logo_image() {
        let theme = look();
        assert_eq!(theme.preview_image(), Some(theme.images["background.png"].as_slice()));

        let mut no_wallpaper = look();
        no_wallpaper.background.as_mut().unwrap().use_image = false;
        assert_eq!(no_wallpaper.preview_image(), Some(theme.images["logo.png"].as_slice()));

        let mut no_background = look();
        no_background.background = None;
        assert_eq!(no_background.preview_image(), Some(theme.images["logo.png"].as_slice()));

        let mut no_images_shown = look();
        no_images_shown.background.as_mut().unwrap().use_image = false;
        no_images_shown.logo.as_mut().unwrap().use_background_image = false;
        assert_eq!(no_images_shown.preview_image(), None, "an image that is switched off is not shown");

        let mut no_parts = look();
        no_parts.background = None;
        no_parts.logo = None;
        assert_eq!(no_parts.preview_image(), None);
        assert_eq!(no_parts.preview_data_uri(), None);
    }

    #[test]
    fn a_wallpaper_the_file_lacks_falls_back_to_the_logo_image() {
        let mut theme = look();
        theme.images.remove("background.png");
        assert_eq!(theme.preview_image(), Some(theme.images["logo.png"].as_slice()));
    }

    fn decoded(bytes: &[u8]) -> image::DynamicImage {
        image::load_from_memory(bytes).expect("the thumbnail decodes")
    }

    #[test]
    fn a_large_image_is_cut_to_the_window_shape_and_shrunk() {
        let (bytes, media_type) = thumbnail(&png(2000, 1000, RED), MAX_SHRINK_PIXELS).unwrap();
        assert_eq!(media_type, "image/jpeg");
        let image = decoded(&bytes);
        let (width, height) = (image.width(), image.height());
        assert!(width <= 360 && height <= 628 && width >= 358 && height >= 626, "{}x{}", width, height);
        assert!((bytes.len() as u32) < 360 * 628 * 3, "well under the raw pixels");

        let (bytes, _) = thumbnail(&png(3000, 9000, RED), MAX_SHRINK_PIXELS).unwrap();
        let image = decoded(&bytes);
        assert!(image.width() <= 360 && image.height() <= 628);
    }

    #[test]
    fn a_small_image_is_cut_but_never_enlarged() {
        let (bytes, _) = thumbnail(&png(100, 50, RED), MAX_SHRINK_PIXELS).unwrap();
        let image = decoded(&bytes);
        assert_eq!((image.width(), image.height()), (29, 50));
    }

    #[test]
    fn a_see_through_image_stays_see_through() {
        let (bytes, media_type) = thumbnail(&png(40, 70, image::Rgba([255, 0, 0, 100])), MAX_SHRINK_PIXELS).unwrap();
        assert_eq!(media_type, "image/png");
        assert!(decoded(&bytes).to_rgba8().pixels().all(|pixel| pixel[3] == 100));
    }

    #[test]
    fn an_image_over_the_limit_or_damaged_has_no_thumbnail() {
        assert!(thumbnail(&png(100, 100, RED), 10_000).is_some());
        assert_eq!(thumbnail(&png(100, 100, RED), 9_999), None);
        assert_eq!(thumbnail(b"\x89PNG\r\n\x1a\nnot-really", MAX_SHRINK_PIXELS), None);
        assert_eq!(thumbnail(b"", MAX_SHRINK_PIXELS), None);
    }

    #[test]
    fn a_webp_wallpaper_has_a_thumbnail() {
        use image::codecs::webp::WebPEncoder;

        let pixels = image::RgbaImage::from_pixel(400, 700, RED);
        let mut webp = Vec::new();
        WebPEncoder::new_lossless(&mut webp).encode(&pixels, 400, 700, image::ColorType::Rgba8).unwrap();
        let (bytes, _) = thumbnail(&webp, MAX_SHRINK_PIXELS).unwrap();
        assert_eq!((decoded(&bytes).width(), decoded(&bytes).height()), THUMBNAIL_BOX);
    }

    #[test]
    fn a_theme_from_the_library_has_its_picture() {
        let theme = super::super::from_bytes(include_bytes!("fixtures/v1/library-rewritten.mvtheme")).unwrap();
        assert!(theme.preview_data_uri().is_some(), "its WebP picture decodes");
    }

    /// A few bytes of lossless WebP that claim 16384 x 16384 pixels, which
    /// the image crate would decode in full just to report the size.
    #[test]
    fn a_webp_that_claims_a_huge_picture_has_no_thumbnail() {
        let mut bomb = b"RIFF\x12\0\0\0WEBPVP8L\x05\0\0\0\x2f\xff\xff\xff\x0f\0".to_vec();
        assert_eq!(thumbnail(&bomb, MAX_SHRINK_PIXELS), None);
        bomb[0x15..0x19].copy_from_slice(&[0, 0, 0, 0]);
        assert!(super::super::small_enough_to_decode(&bomb, MAX_SHRINK_PIXELS), "the same file at one pixel");
    }

    #[test]
    fn an_animation_shows_its_first_frame() {
        use image::codecs::gif::GifEncoder;

        let frame = |pixel| image::Frame::new(image::RgbaImage::from_pixel(36, 63, pixel));
        let mut gif = Vec::new();
        GifEncoder::new(&mut gif).encode_frames([frame(RED), frame(BLUE)]).unwrap();
        let (bytes, _) = thumbnail(&gif, MAX_SHRINK_PIXELS).unwrap();
        let pixel = decoded(&bytes).to_rgb8().get_pixel(18, 30).0;
        assert!(pixel[0] > 200 && pixel[2] < 60, "red, the first frame: {:?}", pixel);
    }

    #[test]
    fn the_preview_is_a_data_uri_of_the_thumbnail() {
        let mut theme = look();
        theme.images.insert("background.png".to_string(), png(720, 1256, RED));
        let uri = theme.preview_data_uri().unwrap();
        let encoded = uri.strip_prefix("data:image/jpeg;base64,").expect("a JPEG data URI");
        let bytes = base64::engine::general_purpose::STANDARD.decode(encoded).unwrap();
        let image = decoded(&bytes);
        assert_eq!((image.width(), image.height()), THUMBNAIL_BOX);

        theme.images.insert("background.png".to_string(), b"\x89PNG\r\n\x1a\ndamaged".to_vec());
        assert_eq!(theme.preview_data_uri(), None, "the dialog shows the colours instead");
    }

    #[test]
    fn built_in_colors_show_every_stripe_and_custom_colors_only_their_own() {
        let all = ["bg-base-100", "bg-primary", "bg-secondary", "bg-accent", "bg-neutral"];
        assert_eq!(preview_stripes(&SharedColors::BuiltIn(BuiltInTheme::Synthwave)), all);

        let css = super::super::safe_theme_css(
            "--color-accent: red; --color-base-100: #101010; --radius-box: 4px; --color-primary-content: blue;"
        );
        assert_eq!(preview_stripes(&SharedColors::Custom { css }), ["bg-base-100", "bg-accent"]);
        assert!(preview_stripes(&SharedColors::Custom { css: "--radius-box: 4px;".to_string() }).is_empty());
    }
}
