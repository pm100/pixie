use crate::meta::read_orientation;
use image::DynamicImage;
use pixie_core::{PixieError, Result};
use std::path::Path;

fn is_heif(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(),
        Some("heic") | Some("heif")
    )
}

pub fn apply_orientation(img: DynamicImage, o: u16) -> DynamicImage {
    match o {
        2 => img.fliph(),
        3 => img.rotate180(),
        4 => img.flipv(),
        5 => img.rotate90().fliph(),
        6 => img.rotate90(),
        7 => img.rotate270().fliph(),
        8 => img.rotate270(),
        _ => img,
    }
}

/// Decode and return the image upright. HEIC is already upright from libheif.
pub fn decode_oriented(path: &Path) -> Result<DynamicImage> {
    if is_heif(path) {
        #[cfg(feature = "heic")]
        return crate::heic::decode(path);
        #[cfg(not(feature = "heic"))]
        return Err(PixieError::Unsupported("HEIC support not compiled in".into()));
    }
    let img = image::ImageReader::open(path)?
        .with_guessed_format()?
        .decode()
        .map_err(|e| PixieError::Image(e.to_string()))?;
    Ok(apply_orientation(img, read_orientation(path)))
}

fn is_jpeg(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(),
        Some("jpg") | Some("jpeg")
    )
}

/// JPEG decoded at the smallest DCT scale (1/1, 1/2, 1/4, 1/8) that still has the
/// requested size on at least one axis. Far cheaper than a full decode.
/// None for anything it cannot handle (CMYK, 16-bit, corrupt...); callers fall back.
fn decode_jpeg_scaled(path: &Path, edge: u32) -> Option<DynamicImage> {
    let file = std::fs::File::open(path).ok()?;
    let mut dec = jpeg_decoder::Decoder::new(std::io::BufReader::new(file));
    dec.read_info().ok()?;
    let e = edge.min(u16::MAX as u32) as u16;
    dec.scale(e, e).ok()?;
    let pixels = dec.decode().ok()?;
    let info = dec.info()?;
    let (w, h) = (info.width as u32, info.height as u32);
    match info.pixel_format {
        jpeg_decoder::PixelFormat::L8 => image::GrayImage::from_raw(w, h, pixels).map(DynamicImage::ImageLuma8),
        jpeg_decoder::PixelFormat::RGB24 => image::RgbImage::from_raw(w, h, pixels).map(DynamicImage::ImageRgb8),
        _ => None,
    }
}

/// Decode for making a thumbnail with long edge `edge`: upright, and possibly
/// much smaller than the source (never smaller than `edge` on its long side).
pub fn decode_scaled(path: &Path, edge: u32) -> Result<DynamicImage> {
    if is_jpeg(path) {
        if let Some(img) = decode_jpeg_scaled(path, edge) {
            return Ok(apply_orientation(img, read_orientation(path)));
        }
    }
    decode_oriented(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, GenericImageView, Rgb, RgbImage};
    use pixie_testutil::{jpeg, jpeg_with_exif};

    fn two_tone() -> DynamicImage {
        // 4x2: left half red, right half blue
        let mut i = RgbImage::new(4, 2);
        for y in 0..2 {
            for x in 0..4 {
                i.put_pixel(x, y, if x < 2 { Rgb([255, 0, 0]) } else { Rgb([0, 0, 255]) });
            }
        }
        DynamicImage::ImageRgb8(i)
    }

    #[test]
    fn orientation_swaps_dimensions_for_5_to_8() {
        for o in 1..=8u16 {
            let r = apply_orientation(two_tone(), o);
            let swapped = o >= 5;
            assert_eq!(r.dimensions(), if swapped { (2, 4) } else { (4, 2) }, "o={o}");
        }
    }

    #[test]
    fn orientation_6_puts_left_side_on_top() {
        let r = apply_orientation(two_tone(), 6).to_rgb8();
        assert_eq!(r.get_pixel(0, 0).0, [255, 0, 0]); // top = old left = red
        assert_eq!(r.get_pixel(0, 3).0, [0, 0, 255]);
    }

    #[test]
    fn orientation_8_puts_left_side_on_bottom() {
        let r = apply_orientation(two_tone(), 8).to_rgb8();
        assert_eq!(r.get_pixel(0, 3).0, [255, 0, 0]);
        assert_eq!(r.get_pixel(0, 0).0, [0, 0, 255]);
    }

    #[test]
    fn decode_oriented_applies_exif() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.jpg");
        std::fs::write(&p, jpeg_with_exif(40, 20, 6, None)).unwrap();
        let img = decode_oriented(&p).unwrap();
        assert_eq!(img.dimensions(), (20, 40));
    }

    #[test]
    fn corrupt_and_empty_files_error_instead_of_panicking() {
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("bad.jpg");
        std::fs::write(&bad, b"not a jpeg").unwrap();
        assert!(matches!(decode_oriented(&bad), Err(PixieError::Image(_))));
        let empty = dir.path().join("empty.jpg");
        std::fs::write(&empty, b"").unwrap();
        assert!(decode_oriented(&empty).is_err());
        let plain = dir.path().join("ok.jpg");
        std::fs::write(&plain, jpeg(8, 8)).unwrap();
        assert!(decode_oriented(&plain).is_ok());
    }

    #[test]
    fn scaled_decode_is_at_least_target_size_and_upright() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("big.jpg");
        std::fs::write(&p, jpeg_with_exif(2000, 1000, 6, None)).unwrap();
        let img = decode_scaled(&p, 256).unwrap();
        let (w, h) = img.dimensions();
        assert!(h > w, "orientation 6 must come out portrait, got {w}x{h}");
        assert!(w.max(h) >= 256, "long edge must still cover the target, got {w}x{h}");
        assert!(w.max(h) < 2000, "must be downscaled during decode, got {w}x{h}");
        let rgb = img.to_rgb8();
        assert!(rgb.get_pixel(w / 2, 3)[0] > 200, "top is red");
        assert!(rgb.get_pixel(w / 2, h - 3)[2] > 200, "bottom is blue");
    }

    #[test]
    fn scaled_decode_falls_back_for_non_jpeg_and_errors_on_junk() {
        let dir = tempfile::tempdir().unwrap();
        let png = dir.path().join("a.png");
        image::RgbImage::new(64, 32).save(&png).unwrap();
        assert_eq!(decode_scaled(&png, 256).unwrap().dimensions(), (64, 32));
        let bad = dir.path().join("bad.jpg");
        std::fs::write(&bad, b"junk").unwrap();
        assert!(decode_scaled(&bad, 256).is_err());
    }
}
