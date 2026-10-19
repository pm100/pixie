//! Runs the real CLIP models. Needs the three model files: set PIXIE_MODEL_DIR, or leave it unset to download them
//! into target/clip-models (about 155 MB, once).
use image::{DynamicImage, Rgb, RgbImage};
use pixie_analysis::clip::ClipEmbedder;
use pixie_analysis::embedder::Embedder;
use pixie_analysis::models::{ensure_models, HttpDownloader, CLIP_FILES};
use pixie_analysis::vectors::dot;
use std::path::PathBuf;
use std::sync::OnceLock;

fn model_dir() -> PathBuf {
    if let Ok(d) = std::env::var("PIXIE_MODEL_DIR") {
        return d.into();
    }
    static DIR: OnceLock<PathBuf> = OnceLock::new(); // the tests run in parallel; download once
    DIR.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/clip-models");
        ensure_models(&dir, CLIP_FILES, &HttpDownloader, &mut |_, _| true).expect("download models");
        dir
    })
    .clone()
}

fn solid(r: u8, g: u8, b: u8) -> DynamicImage {
    DynamicImage::ImageRgb8(RgbImage::from_pixel(224, 224, Rgb([r, g, b])))
}

#[test]
#[ignore = "downloads or needs the real CLIP model files"]
fn text_matches_the_right_image_and_vectors_are_unit_length() {
    let e = ClipEmbedder::load(&model_dir()).unwrap();
    let red = e.embed_image(&solid(220, 20, 20)).unwrap();
    let blue = e.embed_image(&solid(20, 20, 220)).unwrap();
    assert_eq!(red.len(), 512);
    assert!((dot(&red, &red) - 1.0).abs() < 1e-3);
    let q = e.embed_text("a plain red image").unwrap();
    assert!((dot(&q, &q) - 1.0).abs() < 1e-3);
    assert!(dot(&q, &red) > dot(&q, &blue), "red {} vs blue {}", dot(&q, &red), dot(&q, &blue));
    let q2 = e.embed_text("a plain blue image").unwrap();
    assert!(dot(&q2, &blue) > dot(&q2, &red));
}

#[test]
#[ignore = "downloads or needs the real CLIP model files"]
fn very_long_and_empty_text_do_not_fail() {
    let e = ClipEmbedder::load(&model_dir()).unwrap();
    assert_eq!(e.embed_text(&"word ".repeat(500)).unwrap().len(), 512);
    assert_eq!(e.embed_text("").unwrap().len(), 512);
}

#[test]
#[ignore = "downloads or needs the real CLIP model files"]
fn tags_pick_the_obvious_label_for_a_plain_colour_picture_and_stay_quiet_for_nonsense() {
    use pixie_analysis::tags::{TagIndex, VOCAB};
    let e = ClipEmbedder::load(&model_dir()).unwrap();
    let idx = TagIndex::build(&e, VOCAB).unwrap();
    let snow = e.embed_image(&solid(245, 245, 250)).unwrap();
    let tags: Vec<String> = idx.tags_for(&snow).into_iter().map(|t| t.0).collect();
    println!("white picture -> {tags:?}");
    assert!(tags.len() <= 5);
    // A flat colour is not a scene: tagging it with five confident labels would mean the floor is too low.
    assert!(idx.tags_for(&e.embed_image(&solid(128, 128, 128)).unwrap()).len() <= 3);
}
