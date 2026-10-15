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
}
