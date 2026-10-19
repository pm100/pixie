use image::DynamicImage;

/// Turns photos and search text into vectors in one shared space. Both return unit vectors.
pub trait Embedder: Send + Sync {
    fn embed_image(&self, img: &DynamicImage) -> Result<Vec<f32>, String>;
    fn embed_text(&self, text: &str) -> Result<Vec<f32>, String>;
    /// Identifies the model; stored with every vector so a new model re-analyses the library.
    fn version(&self) -> &str;
}

/// A two-dimensional stand-in for tests: "cat" and red-dominant images are one direction, everything else the other.
pub struct FakeEmbedder;

impl Embedder for FakeEmbedder {
    fn embed_image(&self, img: &DynamicImage) -> Result<Vec<f32>, String> {
        let rgb = img.to_rgb8();
        let n = (rgb.width() * rgb.height()).max(1) as u64;
        let red: u64 = rgb.pixels().map(|p| p[0] as u64).sum::<u64>() / n;
        Ok(if red > 128 { vec![1.0, 0.0] } else { vec![0.0, 1.0] })
    }
    fn embed_text(&self, text: &str) -> Result<Vec<f32>, String> {
        Ok(if text.trim().eq_ignore_ascii_case("cat") { vec![1.0, 0.0] } else { vec![0.0, 1.0] })
    }
    fn version(&self) -> &str {
        "fake-1"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_embedder_maps_cat_and_red_together() {
        let e = FakeEmbedder;
        let red = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(8, 8, image::Rgb([250, 0, 0])));
        let blue = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(8, 8, image::Rgb([0, 0, 250])));
        assert_eq!(e.embed_text("cat").unwrap(), vec![1.0, 0.0]);
        assert_eq!(e.embed_text("dog").unwrap(), vec![0.0, 1.0]);
        assert_eq!(e.embed_image(&red).unwrap(), vec![1.0, 0.0]);
        assert_eq!(e.embed_image(&blue).unwrap(), vec![0.0, 1.0]);
    }
}
