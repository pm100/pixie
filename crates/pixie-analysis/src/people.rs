use image::DynamicImage;

/// A square picture of a face: centred on its box (fractions of `img`), `margin` wider than the box's longer side
/// (0.35 = 35 % more), kept inside the photo, scaled to `out` x `out`.
pub fn crop_square(img: &DynamicImage, x: f32, y: f32, w: f32, h: f32, margin: f32, out: u32) -> DynamicImage {
    let (iw, ih) = (img.width() as f32, img.height() as f32);
    let side = ((w * iw).max(h * ih) * (1.0 + margin)).clamp(1.0, iw.min(ih));
    let left = ((x + w / 2.0) * iw - side / 2.0).clamp(0.0, iw - side);
    let top = ((y + h / 2.0) * ih - side / 2.0).clamp(0.0, ih - side);
    let side = side.floor().max(1.0) as u32;
    img.crop_imm(left as u32, top as u32, side, side).resize_exact(out, out, image::imageops::FilterType::Triangle)
}

/// The picture sizes `pixie://face/{id}?s=` offers; anything else gets the smallest.
pub const FACE_SIZES: [u32; 3] = [96, 160, 320];

/// The size asked for in a URL query such as `s=160`, limited to `FACE_SIZES`.
pub fn face_size(query: Option<&str>) -> u32 {
    query
        .and_then(|q| q.split('&').find_map(|kv| kv.strip_prefix("s=")))
        .and_then(|v| v.parse::<u32>().ok())
        .filter(|s| FACE_SIZES.contains(s))
        .unwrap_or(FACE_SIZES[0])
}

/// The JPEG for a face's picture: `thumb` is the upright viewer thumbnail's bytes, `face` the `[x, y, w, h]` box,
/// `size` the side in pixels.
pub fn face_jpeg(thumb: &[u8], face: [f32; 4], size: u32) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(thumb).map_err(|e| e.to_string())?;
    let crop = crop_square(&img, face[0], face[1], face[2], face[3], 0.35, size).to_rgb8();
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 85).encode_image(&crop).map_err(|e| e.to_string())?;
    Ok(out)
}

/// A face picture kept on disk under `key` (which must say everything the picture depends on: photo, its
/// version, the box and the size), so it is made once. Written to a temporary file first, so a crash never leaves a
/// half picture to be served; a failure to make or to save is not remembered.
pub fn cached_face(dir: &std::path::Path, key: &str, make: impl FnOnce() -> Result<Vec<u8>, String>) -> Result<Vec<u8>, String> {
    let file = dir.join(format!("{key}.jpg"));
    if let Ok(bytes) = std::fs::read(&file) {
        if !bytes.is_empty() {
            return Ok(bytes);
        }
    }
    let bytes = make()?;
    if std::fs::create_dir_all(dir).is_ok() {
        let tmp = dir.join(format!("{key}.part"));
        if std::fs::write(&tmp, &bytes).is_ok() && std::fs::rename(&tmp, &file).is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;

    #[test]
    fn crop_square_is_centred_on_the_face_and_has_the_asked_size() {
        // 400x200 blue photo with a red 40x40 face at (180..220, 80..120)
        let mut im = image::RgbImage::from_pixel(400, 200, image::Rgb([0, 0, 255]));
        for yy in 80..120 {
            for xx in 180..220 {
                im.put_pixel(xx, yy, image::Rgb([255, 0, 0]));
            }
        }
        let img = DynamicImage::ImageRgb8(im);
        let c = crop_square(&img, 0.45, 0.4, 0.1, 0.2, 0.35, 48).to_rgb8();
        assert_eq!(c.dimensions(), (48, 48));
        assert_eq!(c.get_pixel(24, 24).0, [255, 0, 0]);
        assert_eq!(c.get_pixel(1, 1).0, [0, 0, 255]); // the margin shows what is around the face
        let tight = crop_square(&img, 0.45, 0.4, 0.1, 0.2, 0.0, 48).to_rgb8();
        assert_eq!(tight.get_pixel(1, 1).0, [255, 0, 0]);
    }

    #[test]
    fn crop_square_near_an_edge_stays_inside_the_photo() {
        let img = DynamicImage::ImageRgb8(image::RgbImage::from_pixel(400, 200, image::Rgb([9, 9, 9])));
        assert_eq!(crop_square(&img, 0.0, 0.0, 0.05, 0.1, 0.35, 32).dimensions(), (32, 32));
        assert_eq!(crop_square(&img, 0.95, 0.9, 0.05, 0.1, 0.35, 32).dimensions(), (32, 32));
    }

    #[test]
    fn crop_square_of_a_box_bigger_than_the_photo_still_works() {
        let img = DynamicImage::ImageRgb8(image::RgbImage::from_pixel(100, 60, image::Rgb([9, 9, 9])));
        assert_eq!(crop_square(&img, 0.0, 0.0, 1.0, 1.0, 0.35, 96).dimensions(), (96, 96));
    }

    #[test]
    fn face_jpeg_makes_a_96_pixel_jpeg_and_rejects_garbage() {
        let img = DynamicImage::ImageRgb8(image::RgbImage::from_pixel(300, 200, image::Rgb([200, 100, 50])));
        let mut bytes = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Jpeg).unwrap();
        let out = face_jpeg(&bytes, [0.4, 0.3, 0.2, 0.3], 96).unwrap();
        let back = image::load_from_memory(&out).unwrap();
        assert_eq!((back.width(), back.height()), (96, 96));
        assert!(face_jpeg(b"not an image", [0.0, 0.0, 0.1, 0.1], 96).is_err());
        let big = image::load_from_memory(&face_jpeg(&bytes, [0.4, 0.3, 0.2, 0.3], 320).unwrap()).unwrap();
        assert_eq!((big.width(), big.height()), (320, 320));
    }

    #[test]
    fn the_asked_picture_size_is_one_of_the_offered_ones() {
        assert_eq!(face_size(None), 96);
        assert_eq!(face_size(Some("s=160")), 160);
        assert_eq!(face_size(Some("v=1&s=320")), 320);
        assert_eq!(face_size(Some("s=5000")), 96); // not offered: the default, so nobody can ask for a huge crop
        assert_eq!(face_size(Some("s=abc")), 96);
    }

    #[test]
    fn a_made_picture_is_kept_and_not_made_again() {
        let dir = tempfile::tempdir().unwrap();
        let made = std::cell::Cell::new(0);
        let make = || {
            made.set(made.get() + 1);
            Ok(vec![1u8, 2, 3])
        };
        assert_eq!(cached_face(dir.path(), "7-1-2-96", make).unwrap(), vec![1, 2, 3]);
        assert_eq!(cached_face(dir.path(), "7-1-2-96", make).unwrap(), vec![1, 2, 3]);
        assert_eq!(made.get(), 1);
        assert_eq!(cached_face(dir.path(), "8-1-2-96", make).unwrap(), vec![1, 2, 3]); // another face is its own entry
        assert_eq!(made.get(), 2);
    }

    #[test]
    fn a_failure_is_reported_and_not_kept() {
        let dir = tempfile::tempdir().unwrap();
        assert!(cached_face(dir.path(), "9-1-2-96", || Err("no thumbnail".to_string())).is_err());
        assert_eq!(cached_face(dir.path(), "9-1-2-96", || Ok(vec![5])).unwrap(), vec![5]);
        // nothing half-written is left behind to be served
        let names: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(names, vec!["9-1-2-96.jpg".to_string()]);
    }

}
