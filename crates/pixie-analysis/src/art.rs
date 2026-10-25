//! Is a face a person, or a face in a painting, drawing or statue? Photos of artwork are full of faces that are not people
//! to name, and the photo's own Auto tags miss many of them (a Picasso that fills the frame may be tagged "dancing"). So
//! each face is looked at on its own: the picture model is asked which description its crop fits best.
use crate::embedder::Embedder;
use crate::people::crop_square;
use crate::vectors::dot;
use image::DynamicImage;

/// Descriptions of a face that is not a real person's.
const ART: [&str; 12] = [
    "a painting of a person",
    "an oil painting portrait",
    "an old master portrait painting in a museum",
    "a renaissance painting of a face",
    "a drawing of a face",
    "a statue of a person",
    "a sculpture of a head",
    "a cartoon illustration of a person",
    "a toy figure or doll face",
    "a carved mask or figurine",
    "a mural of a face",
    "a mosaic of a person",
];
/// Descriptions of a face that is.
const REAL: [&str; 4] = [
    "a photo of a real person",
    "a selfie of a person",
    "a candid photograph of a person's face",
    "a close-up photo of a person smiling",
];
/// How sharply the descriptions compete (CLIP's own scale).
const TEMPERATURE: f32 = 100.0;
/// The share of the probability the art descriptions must hold for the face to count as artwork.
pub const ART_SHARE: f32 = 0.6;
/// The face is cut out with this much around it, and shown to the model at this size.
const MARGIN: f32 = 0.35;
const CROP: u32 = 224;

pub struct ArtIndex {
    art: Vec<Vec<f32>>,
    real: Vec<Vec<f32>>,
}

impl ArtIndex {
    pub fn build(e: &dyn Embedder) -> Result<ArtIndex, String> {
        let embed = |list: &[&str]| list.iter().map(|t| e.embed_text(t)).collect::<Result<Vec<_>, String>>();
        Ok(ArtIndex { art: embed(&ART)?, real: embed(&REAL)? })
    }

    /// The share (0 to 1) of the probability the art descriptions hold for this face crop's vector.
    pub fn art_share(&self, crop: &[f32]) -> f32 {
        let scores: Vec<f32> = self.art.iter().chain(&self.real).map(|v| dot(crop, v) * TEMPERATURE).collect();
        let top = scores.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let weights: Vec<f32> = scores.iter().map(|s| (s - top).exp()).collect();
        weights[..self.art.len()].iter().sum::<f32>() / weights.iter().sum::<f32>()
    }

    pub fn is_art(&self, crop: &[f32]) -> bool {
        self.art_share(crop) >= ART_SHARE
    }
}

/// The picture to show the model for a face (its box is fractions of `img`).
pub fn face_crop(img: &DynamicImage, face: [f32; 4]) -> DynamicImage {
    crop_square(img, face[0], face[1], face[2], face[3], MARGIN, CROP)
}

/// Whether the face in `img` at `face` is artwork.
pub fn face_is_art(e: &dyn Embedder, index: &ArtIndex, img: &DynamicImage, face: [f32; 4]) -> Result<bool, String> {
    Ok(index.is_art(&e.embed_image(&face_crop(img, face))?))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each description is its own axis, and a "face" is a vector over the axes.
    struct Axes;

    impl Embedder for Axes {
        fn embed_image(&self, _: &DynamicImage) -> Result<Vec<f32>, String> {
            Err("unused".into())
        }
        fn embed_text(&self, text: &str) -> Result<Vec<f32>, String> {
            let all: Vec<&str> = ART.iter().chain(REAL.iter()).copied().collect();
            let i = all.iter().position(|t| *t == text).ok_or("unknown")?;
            let mut v = vec![0.0; all.len()];
            v[i] = 1.0;
            Ok(v)
        }
        fn version(&self) -> &str {
            "axes"
        }
    }

    fn face(axis: usize) -> Vec<f32> {
        let mut v = vec![0.05; ART.len() + REAL.len()];
        v[axis] = 1.0;
        v
    }

    #[test]
    fn a_face_that_fits_an_art_description_is_artwork_and_one_that_fits_a_photo_is_not() {
        let index = ArtIndex::build(&Axes).unwrap();
        assert!(index.is_art(&face(0)), "painting");
        assert!(index.is_art(&face(2)), "statue");
        assert!(!index.is_art(&face(ART.len())), "photo of a real person");
        assert!(!index.is_art(&face(ART.len() + 1)), "selfie");
    }

    #[test]
    fn the_share_is_a_probability_and_a_tie_is_half() {
        let index = ArtIndex::build(&Axes).unwrap();
        let mut both = vec![0.0; ART.len() + REAL.len()];
        both[0] = 1.0;
        both[ART.len()] = 1.0;
        let s = index.art_share(&both);
        assert!((s - 0.5).abs() < 1e-5, "{s}");
        assert!((0.0..=1.0).contains(&index.art_share(&face(3))));
    }

    /// Looks at real faces: `PIXIE_LIVE_DB=copy.db PIXIE_MODELS=<clip model dir> PIXIE_ART_OUT=<dir>`, then view the sheets.
    /// `cargo test --release -p pixie-analysis live_art -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn live_art() {
        use image::{GenericImage, RgbImage};
        let db = pixie_index::Db::open(std::path::Path::new(&std::env::var("PIXIE_LIVE_DB").unwrap())).unwrap();
        let clip = crate::clip::ClipEmbedder::load_with(std::path::Path::new(&std::env::var("PIXIE_MODELS").unwrap()), 2).unwrap();
        let out = std::path::PathBuf::from(std::env::var("PIXIE_ART_OUT").unwrap());
        let index = ArtIndex::build(&clip).unwrap();
        let mut ids: Vec<i64> = db.unsorted_faces(0, 5000).unwrap().iter().map(|u| u.tile.id).collect();
        ids = ids.into_iter().step_by(6).collect();
        if let Ok(photo) = std::env::var("PIXIE_ART_PHOTO") {
            ids.extend(db.faces_of(photo.parse().unwrap()).unwrap().iter().map(|f| f.id));
        }
        let mut scored: Vec<(f32, DynamicImage, i64)> = Vec::new();
        for id in ids {
            let Some((photo, b)) = db.face_location(id).unwrap() else { continue };
            let Ok(bytes) = std::fs::read(&photo.path) else { continue };
            let Ok(img) = pixie_image::decode::decode_scaled_bytes(std::path::Path::new(&photo.path), &bytes, 1024) else { continue };
            let crop = face_crop(&img, b);
            let share = index.art_share(&clip.embed_image(&crop).unwrap());
            scored.push((share, crop, id));
        }
        scored.sort_by(|a, b| b.0.total_cmp(&a.0));
        for row in scored.chunks(12).take(6) {
            println!("  {}", row.iter().map(|s| format!("{:.2}", s.0)).collect::<Vec<_>>().join(" "));
        }
        println!("{} faces; art (>= {ART_SHARE}): {}", scored.len(), scored.iter().filter(|s| s.0 >= ART_SHARE).count());
        for (k, chunk) in scored.chunks(72).enumerate() {
            let mut sheet = RgbImage::new(12 * 96, 6 * 96);
            for (i, (_, crop, _)) in chunk.iter().enumerate() {
                let small = crop.resize_exact(96, 96, image::imageops::FilterType::Triangle).to_rgb8();
                sheet.copy_from(&small, (i as u32 % 12) * 96, (i as u32 / 12) * 96).unwrap();
            }
            sheet.save(out.join(format!("art{k}.png"))).unwrap();
            println!("sheet {k}: shares {:.2} .. {:.2}", chunk.first().unwrap().0, chunk.last().unwrap().0);
        }
    }
}
