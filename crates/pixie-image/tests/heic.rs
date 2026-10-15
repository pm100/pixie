#![cfg(feature = "heic")]
use image::GenericImageView;
use pixie_image::{
    decode::decode_oriented,
    meta::read_meta,
    thumbs::{ensure_thumbnail, ThumbClass},
};
use std::path::Path;

const SAMPLE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/sample.heic");

#[test]
fn heic_decodes_and_metadata_matches_decoded_dimensions() {
    let img = decode_oriented(Path::new(SAMPLE)).expect("decode heic");
    let meta = read_meta(Path::new(SAMPLE));
    // Guards against double-applying rotation (libheif already rotates).
    assert_eq!((meta.width, meta.height), (Some(img.width()), Some(img.height())));
}

#[test]
fn heic_thumbnail_is_generated() {
    let dir = tempfile::tempdir().unwrap();
    let t = ensure_thumbnail(dir.path(), ThumbClass::Grid, 1, 1, Path::new(SAMPLE)).unwrap();
    let (w, h) = image::open(&t).unwrap().dimensions();
    assert!(w.max(h) <= 256 && w.max(h) > 0);
}
