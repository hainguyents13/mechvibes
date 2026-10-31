//! The picture the import dialog shows for a theme: the look itself, drawn
//! the way a card in the Themes tab draws it, with its images shrunk into
//! `data:` URIs so the page reaches no file.

use super::{ jpeg_orientation, small_enough_to_decode, upright, Look, LookColors, SharedTheme, MAX_SHRINK_PIXELS };
use base64::Engine;
use std::io::Cursor;

/// The app window's shape at its default size (470 x 820 pt), large enough
/// for the dialog's picture on a 2x screen. The theme library makes its card
/// pictures at the same size.
const THUMBNAIL_BOX: (u32, u32) = (360, 628);
/// Room for the logo's image in the picture's small logo on a 2x screen.
const LOGO_BOX: (u32, u32) = (320, 320);
const THUMBNAIL_JPEG_QUALITY: u8 = 80;

impl SharedTheme {
    /// The look as the dialog's picture shows it: the wallpaper cut to the
    /// window's shape and the logo's image shrunk, each as a `data:` URI.
    /// An image that is switched off, missing, too large or damaged is left
    /// out, so the picture shows the colour behind it. The muted logo never
    /// shows in the picture and is left out too. Slow for a large wallpaper,
    /// so call it off the UI thread.
    pub fn preview_look(&self) -> Look {
        let shown = |on: bool, name: &Option<String>, cut: Option<(u32, u32)>, fit: (u32, u32)| {
            let bytes = name.as_ref().filter(|_| on).and_then(|name| self.images.get(name))?;
            let (bytes, media_type) = thumbnail(bytes, MAX_SHRINK_PIXELS, cut, fit)?;
            Some(format!("data:{};base64,{}", media_type, base64::engine::general_purpose::STANDARD.encode(bytes)))
        };
        let mut logo = self.logo.clone();
        if let Some(logo) = &mut logo {
            logo.background_image = shown(logo.use_background_image, &logo.background_image, None, LOGO_BOX);
            logo.muted_background_image = None;
        }
        let mut background = self.background.clone();
        if let Some(background) = &mut background {
            background.background_image = shown(
                background.use_image,
                &background.background_image,
                Some(THUMBNAIL_BOX),
                THUMBNAIL_BOX
            );
        }
        Look { colors: LookColors::Known(self.colors.clone()), logo, background }
    }
}

/// `bytes` cut around the centre to the shape of `cut`, if given, and shrunk
/// to fit `fit`, never enlarged, with its media type. An animation keeps its
/// first frame. PNG when it has transparency, JPEG otherwise. `None` for an
/// image over `max_pixels`, which is not decoded at all, or one that cannot
/// be decoded.
fn thumbnail(
    bytes: &[u8],
    max_pixels: u64,
    cut: Option<(u32, u32)>,
    fit: (u32, u32)
) -> Option<(Vec<u8>, &'static str)> {
    use image::{ codecs::jpeg::JpegEncoder, imageops::FilterType, ImageFormat };

    if !small_enough_to_decode(bytes, max_pixels) {
        return None;
    }
    let mut image = upright(image::load_from_memory(bytes).ok()?, jpeg_orientation(bytes));
    if let Some((cut_width, cut_height)) = cut {
        let (width, height) = (image.width(), image.height());
        let shape = (cut_width as f64) / (cut_height as f64);
        let crop_width = width.min(((height as f64) * shape).round() as u32).max(1);
        let crop_height = height.min(((width as f64) / shape).round() as u32).max(1);
        image = image.crop_imm((width - crop_width) / 2, (height - crop_height) / 2, crop_width, crop_height);
    }
    let (box_width, box_height) = fit;
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
    use super::super::SharedColors;
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

    fn wallpaper(look: &Look) -> Option<&str> {
        look.background.as_ref().and_then(|background| background.background_image.as_deref())
    }

    fn logo_image(look: &Look) -> Option<&str> {
        look.logo.as_ref().and_then(|logo| logo.background_image.as_deref())
    }

    #[test]
    fn the_picture_shows_the_look_with_its_images_in_the_page() {
        let theme = look();
        let shown = theme.preview_look();
        assert_eq!(shown.colors, LookColors::Known(theme.colors.clone()));
        assert!(wallpaper(&shown).unwrap().starts_with("data:image/jpeg;base64,"));
        assert!(logo_image(&shown).unwrap().starts_with("data:image/jpeg;base64,"));
        assert_eq!(shown.background.as_ref().unwrap().background_color, "#101010");
        assert_eq!(shown.logo.as_ref().unwrap().muted_background_image, None, "the picture never shows it muted");

        let mut no_parts = look();
        no_parts.background = None;
        no_parts.logo = None;
        let shown = no_parts.preview_look();
        assert_eq!((shown.logo, shown.background), (None, None));
    }

    #[test]
    fn an_image_switched_off_or_missing_is_left_out() {
        let mut theme = look();
        theme.background.as_mut().unwrap().use_image = false;
        theme.images.remove("logo.png");
        let shown = theme.preview_look();
        assert_eq!(wallpaper(&shown), None, "the background colour shows instead");
        assert_eq!(logo_image(&shown), None, "never a file name the page would try to load");
        assert!(shown.logo.is_some() && shown.background.is_some());
    }

    #[test]
    fn a_logo_image_is_shrunk_but_never_cut() {
        let (bytes, _) = thumbnail(&png(1000, 200, BLUE), MAX_SHRINK_PIXELS, None, LOGO_BOX).unwrap();
        let image = decoded(&bytes);
        assert_eq!((image.width(), image.height()), (320, 64));
    }

    fn decoded(bytes: &[u8]) -> image::DynamicImage {
        image::load_from_memory(bytes).expect("the thumbnail decodes")
    }

    /// A wallpaper's thumbnail.
    fn window(bytes: &[u8], max_pixels: u64) -> Option<(Vec<u8>, &'static str)> {
        thumbnail(bytes, max_pixels, Some(THUMBNAIL_BOX), THUMBNAIL_BOX)
    }

    #[test]
    fn a_large_image_is_cut_to_the_window_shape_and_shrunk() {
        let (bytes, media_type) = window(&png(2000, 1000, RED), MAX_SHRINK_PIXELS).unwrap();
        assert_eq!(media_type, "image/jpeg");
        let image = decoded(&bytes);
        let (width, height) = (image.width(), image.height());
        assert!(width <= 360 && height <= 628 && width >= 358 && height >= 626, "{}x{}", width, height);
        assert!((bytes.len() as u32) < 360 * 628 * 3, "well under the raw pixels");

        let (bytes, _) = window(&png(3000, 9000, RED), MAX_SHRINK_PIXELS).unwrap();
        let image = decoded(&bytes);
        assert!(image.width() <= 360 && image.height() <= 628);
    }

    #[test]
    fn a_small_image_is_cut_but_never_enlarged() {
        let (bytes, _) = window(&png(100, 50, RED), MAX_SHRINK_PIXELS).unwrap();
        let image = decoded(&bytes);
        assert_eq!((image.width(), image.height()), (29, 50));
    }

    #[test]
    fn a_see_through_image_stays_see_through() {
        let (bytes, media_type) = window(&png(40, 70, image::Rgba([255, 0, 0, 100])), MAX_SHRINK_PIXELS).unwrap();
        assert_eq!(media_type, "image/png");
        assert!(decoded(&bytes).to_rgba8().pixels().all(|pixel| pixel[3] == 100));
    }

    #[test]
    fn an_image_over_the_limit_or_damaged_has_no_thumbnail() {
        assert!(window(&png(100, 100, RED), 10_000).is_some());
        assert_eq!(window(&png(100, 100, RED), 9_999), None);
        assert_eq!(window(b"\x89PNG\r\n\x1a\nnot-really", MAX_SHRINK_PIXELS), None);
        assert_eq!(window(b"", MAX_SHRINK_PIXELS), None);
    }

    #[test]
    fn a_webp_wallpaper_has_a_thumbnail() {
        use image::codecs::webp::WebPEncoder;

        let pixels = image::RgbaImage::from_pixel(400, 700, RED);
        let mut webp = Vec::new();
        WebPEncoder::new_lossless(&mut webp).encode(&pixels, 400, 700, image::ColorType::Rgba8).unwrap();
        let (bytes, _) = window(&webp, MAX_SHRINK_PIXELS).unwrap();
        assert_eq!((decoded(&bytes).width(), decoded(&bytes).height()), THUMBNAIL_BOX);
    }

    #[test]
    fn a_theme_from_the_library_has_its_picture() {
        let theme = super::super::from_bytes(include_bytes!("fixtures/v1/library-rewritten.mvtheme")).unwrap();
        let shown = theme.preview_look();
        assert!(wallpaper(&shown).is_some() || logo_image(&shown).is_some(), "its WebP pictures decode");
    }

    /// A few bytes of lossless WebP that claim 16384 x 16384 pixels, which
    /// the image crate would decode in full just to report the size.
    #[test]
    fn a_webp_that_claims_a_huge_picture_has_no_thumbnail() {
        let mut bomb = b"RIFF\x12\0\0\0WEBPVP8L\x05\0\0\0\x2f\xff\xff\xff\x0f\0".to_vec();
        assert_eq!(window(&bomb, MAX_SHRINK_PIXELS), None);
        bomb[0x15..0x19].copy_from_slice(&[0, 0, 0, 0]);
        assert!(super::super::small_enough_to_decode(&bomb, MAX_SHRINK_PIXELS), "the same file at one pixel");
    }

    #[test]
    fn an_animation_shows_its_first_frame() {
        use image::codecs::gif::GifEncoder;

        let frame = |pixel| image::Frame::new(image::RgbaImage::from_pixel(36, 63, pixel));
        let mut gif = Vec::new();
        GifEncoder::new(&mut gif).encode_frames([frame(RED), frame(BLUE)]).unwrap();
        let (bytes, _) = window(&gif, MAX_SHRINK_PIXELS).unwrap();
        let pixel = decoded(&bytes).to_rgb8().get_pixel(18, 30).0;
        assert!(pixel[0] > 200 && pixel[2] < 60, "red, the first frame: {:?}", pixel);
    }

    #[test]
    fn the_wallpaper_is_a_data_uri_of_its_thumbnail() {
        let mut theme = look();
        theme.images.insert("background.png".to_string(), png(720, 1256, RED));
        let shown = theme.preview_look();
        let encoded = wallpaper(&shown).unwrap().strip_prefix("data:image/jpeg;base64,").expect("a JPEG data URI");
        let bytes = base64::engine::general_purpose::STANDARD.decode(encoded).unwrap();
        let image = decoded(&bytes);
        assert_eq!((image.width(), image.height()), THUMBNAIL_BOX);

        theme.images.insert("background.png".to_string(), b"\x89PNG\r\n\x1a\ndamaged".to_vec());
        assert_eq!(wallpaper(&theme.preview_look()), None, "the background colour shows instead");
    }
}
