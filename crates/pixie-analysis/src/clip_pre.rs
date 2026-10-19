use image::{imageops::FilterType, DynamicImage};

const EDGE: u32 = 224;
const MEAN: [f32; 3] = [0.48145466, 0.4578275, 0.40821073];
const STD: [f32; 3] = [0.26862954, 0.26130258, 0.27577711];

/// CLIP's input: shortest side scaled to 224, centre-cropped square, then per-channel normalised, laid out CHW.
pub fn to_tensor(img: &DynamicImage) -> Vec<f32> {
    let rgb = img.resize_to_fill(EDGE, EDGE, FilterType::CatmullRom).to_rgb8();
    let plane = (EDGE * EDGE) as usize;
    let mut out = vec![0.0f32; 3 * plane];
    for (i, p) in rgb.pixels().enumerate() {
        for c in 0..3 {
            out[c * plane + i] = (p[c] as f32 / 255.0 - MEAN[c]) / STD[c];
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tensor_has_chw_shape_and_normalised_values() {
        let img = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(640, 480, image::Rgb([255, 255, 255])));
        let t = to_tensor(&img);
        assert_eq!(t.len(), 3 * 224 * 224);
        assert!((t[0] - (1.0 - 0.48145466) / 0.26862954).abs() < 1e-4); // R plane first
        assert!((t[224 * 224] - (1.0 - 0.4578275) / 0.26130258).abs() < 1e-4); // then G
    }

    #[test]
    fn tall_image_is_centre_cropped_not_squashed() {
        // Top half black, bottom half white, 100x400: the 100x100 centre crop straddles the seam.
        let mut im = image::RgbImage::new(100, 400);
        for (_, y, p) in im.enumerate_pixels_mut() {
            *p = if y < 200 { image::Rgb([0, 0, 0]) } else { image::Rgb([255, 255, 255]) };
        }
        let t = to_tensor(&image::DynamicImage::ImageRgb8(im));
        assert!(t[10 * 224] < t[200 * 224]); // top of the crop is darker than its bottom
    }
}
