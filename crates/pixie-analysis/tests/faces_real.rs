//! Runs the real YuNet and SFace models on a crowd photo. Models and photo are downloaded into target/face-models
//! (about 39 MB plus 1 MB, once); the photo is test data only and is never committed.
use image::DynamicImage;
use pixie_analysis::faces::{FaceEngine, FaceModels};
use pixie_analysis::models::{ensure_models, HttpDownloader, ModelFile, FACE_FILES};
use pixie_analysis::vectors::dot;
use std::path::PathBuf;
use std::sync::OnceLock;

const SELFIE: ModelFile = ModelFile {
    name: "largest_selfie.jpg",
    url: "https://github.com/opencv/opencv_zoo/raw/47534e27c9851bb1128ccc0102f1145e27f23f98/models/face_detection_yunet/example_outputs/largest_selfie.jpg",
    sha256: "ab8413ad9bb4f53068f4fb63c6747e5989991dd02241c923d5595b614ecf2bf6",
    bytes: 1_147_146,
};
static PHOTO: [ModelFile; 1] = [SELFIE];

fn dir() -> PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new(); // the tests run in parallel; download once
    DIR.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/face-models");
        ensure_models(&dir, FACE_FILES, &HttpDownloader, &mut |_, _| true).expect("download face models");
        ensure_models(&dir, &PHOTO, &HttpDownloader, &mut |_, _| true).expect("download test photo");
        dir
    })
    .clone()
}

fn selfie() -> DynamicImage {
    image::open(dir().join("largest_selfie.jpg")).expect("open test photo")
}

#[test]
#[ignore = "downloads the real face models"]
fn finds_the_faces_in_a_crowd_photo() {
    let e = FaceModels::load(&dir()).unwrap();
    let faces = e.detect(&selfie()).unwrap();
    assert!(faces.len() >= 3, "found {}", faces.len());
    let mut by_size: Vec<_> = faces.iter().collect();
    by_size.sort_by(|a, b| (b.w * b.h).total_cmp(&(a.w * a.h)));
    for f in &by_size[..3] {
        assert!(f.score > 0.9, "score {}", f.score);
    }
    for f in &faces {
        assert!(f.x >= 0.0 && f.y >= 0.0 && f.x + f.w <= 1.0001 && f.y + f.h <= 1.0001);
    }
}

#[test]
#[ignore = "downloads the real face models"]
fn embeddings_are_unit_stable_and_tell_people_apart() {
    let e = FaceModels::load(&dir()).unwrap();
    let img = selfie();
    let mut faces = e.detect(&img).unwrap();
    faces.sort_by(|a, b| (b.w * b.h).total_cmp(&(a.w * a.h)));
    let a = e.embed(&img, &faces[0]).unwrap();
    let a2 = e.embed(&img, &faces[0]).unwrap();
    let b = e.embed(&img, &faces[1]).unwrap();
    assert_eq!(a.len(), 128);
    assert!((dot(&a, &a) - 1.0).abs() < 1e-3);
    assert!(dot(&a, &a2) > 0.999);
    assert!(dot(&a, &b) < 0.3, "two different people: {}", dot(&a, &b));
}

#[test]
#[ignore = "downloads the real face models"]
fn the_same_face_in_a_mirrored_photo_is_recognised() {
    let e = FaceModels::load(&dir()).unwrap();
    let img = selfie();
    let flipped = img.fliph();
    let largest = |im: &DynamicImage| {
        let mut f = e.detect(im).unwrap();
        f.sort_by(|a, b| (b.w * b.h).total_cmp(&(a.w * a.h)));
        e.embed(im, &f[0]).unwrap()
    };
    let s = dot(&largest(&img), &largest(&flipped));
    assert!(s > 0.6, "same face, mirrored: {s}");
}
