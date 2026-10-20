use crate::cluster::{absorb, assign_excluding, Centroid};
use image::DynamicImage;
use pixie_index::Db;

/// Group every face that has no person yet: each joins the most similar person (at least `threshold`), else starts a
/// new one. Returns how many faces were assigned.
pub fn assign_new_faces(db: &Db, threshold: f32) -> Result<usize, String> {
    assign_with(db, load_people(db)?, threshold)
}

fn load_people(db: &Db) -> Result<Vec<Centroid>, String> {
    Ok(db.people_centroids().map_err(|e| e.to_string())?.into_iter().map(|(id, vector, n)| Centroid { id, vector, n }).collect())
}

/// `assign_new_faces` starting from a snapshot of the people. The snapshot can go stale while this runs (the user
/// merges or hides someone): a face whose person has vanished is not lost, the people are re-read and it is tried
/// again, so one failure never strands the rest.
fn assign_with(db: &Db, mut people: Vec<Centroid>, threshold: f32) -> Result<usize, String> {
    let mut assigned = 0;
    for _ in 0..3 {
        let mut failed = None;
        let rejections = db.face_rejections().map_err(|e| e.to_string())?;
        for (face_id, embedding) in db.unassigned_faces().map_err(|e| e.to_string())? {
            let skip = rejections.get(&face_id).map(Vec::as_slice).unwrap_or(&[]); // people it was rejected from
            let outcome = match assign_excluding(&embedding, &people, threshold, skip) {
                Some(i) => {
                    let mut moved = people[i].clone();
                    absorb(&mut moved, &embedding);
                    db.assign_face(face_id, moved.id, &moved.vector, moved.n).map(|_| people[i] = moved)
                }
                None => db.create_person(&embedding).and_then(|id| {
                    db.assign_face(face_id, id, &embedding, 1)?;
                    people.push(Centroid { id, vector: embedding.clone(), n: 1 });
                    Ok(())
                }),
            };
            match outcome {
                Ok(()) => assigned += 1,
                Err(e) => failed = Some(e.to_string()),
            }
        }
        match failed {
            None => return Ok(assigned),
            Some(e) => {
                eprintln!("[people] grouping again after: {e}");
                people = load_people(db)?;
            }
        }
    }
    Ok(assigned)
}

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
    use pixie_core::{NewFace, PhotoMeta};

    fn face(v: [f32; 2]) -> NewFace {
        NewFace { x: 0.1, y: 0.1, w: 0.2, h: 0.2, score: 0.9, embedding: v.to_vec() }
    }

    fn library(n: usize) -> (Db, Vec<i64>) {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder("/p").unwrap();
        let ids = (0..n).map(|i| db.upsert_photo(f, &format!("/p/{i}.jpg"), 1, 1, &PhotoMeta::default()).unwrap()).collect();
        (db, ids)
    }

    #[test]
    fn similar_faces_share_a_person_and_a_different_one_starts_another() {
        let (db, ids) = library(3);
        db.replace_faces(ids[0], "v", &[face([1.0, 0.0])]).unwrap();
        db.replace_faces(ids[1], "v", &[face([0.99, 0.141])]).unwrap();
        db.replace_faces(ids[2], "v", &[face([0.0, 1.0])]).unwrap();
        assert_eq!(assign_new_faces(&db, 0.42).unwrap(), 3);
        let a = db.faces_of(ids[0]).unwrap()[0].person_id;
        let b = db.faces_of(ids[1]).unwrap()[0].person_id;
        let c = db.faces_of(ids[2]).unwrap()[0].person_id;
        assert!(a.is_some() && a == b && a != c && c.is_some());
        assert_eq!(db.people_centroids().unwrap().len(), 2);
        assert_eq!(assign_new_faces(&db, 0.42).unwrap(), 0); // nothing left to do
    }

    #[test]
    fn a_new_face_joins_an_existing_person_and_updates_the_count() {
        let (db, ids) = library(2);
        db.replace_faces(ids[0], "v", &[face([1.0, 0.0])]).unwrap();
        assign_new_faces(&db, 0.42).unwrap();
        db.replace_faces(ids[1], "v", &[face([0.98, 0.2])]).unwrap();
        assign_new_faces(&db, 0.42).unwrap();
        let cs = db.people_centroids().unwrap();
        assert_eq!(cs.len(), 1);
        assert_eq!(cs[0].2, 2);
    }

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
    fn a_stale_snapshot_does_not_strand_faces() {
        // The snapshot says person 999 (who no longer exists, e.g. merged away meanwhile) is the match for both faces.
        let (db, ids) = library(2);
        db.replace_faces(ids[0], "v", &[face([1.0, 0.0])]).unwrap();
        db.replace_faces(ids[1], "v", &[face([0.0, 1.0])]).unwrap();
        let stale = vec![Centroid { id: 999, vector: vec![0.7, 0.7], n: 3 }];
        assert_eq!(assign_with(&db, stale, 0.42).unwrap(), 2);
        assert!(db.unassigned_faces().unwrap().is_empty());
        assert_eq!(db.faces_of(ids[0]).unwrap()[0].person_id.is_some(), true);
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

    #[test]
    fn a_rejected_face_is_guessed_again_and_never_returns_to_the_person_it_was_rejected_from() {
        let (db, ids) = library(4);
        db.replace_faces(ids[0], "v", &[face([1.0, 0.0])]).unwrap();
        db.replace_faces(ids[1], "v", &[face([0.99, 0.141])]).unwrap();
        db.replace_faces(ids[2], "v", &[face([0.6, 0.8])]).unwrap(); // someone a bit like them
        assign_new_faces(&db, 0.42).unwrap();
        let first = db.faces_of(ids[0]).unwrap()[0].person_id.unwrap();
        let other = db.faces_of(ids[2]).unwrap()[0].person_id.unwrap();
        // the first two were grouped; the third joined them too (it is similar enough) or is its own person
        let face = db.faces_of(ids[0]).unwrap()[0].id;
        db.reject_face(face).unwrap();
        assign_new_faces(&db, 0.42).unwrap();
        let after = db.faces_of(ids[0]).unwrap()[0].person_id;
        assert!(after.is_some(), "it was given to someone");
        assert_ne!(after, Some(first), "but not to the person it was rejected from");
        let _ = other;
        assert_eq!(assign_new_faces(&db, 0.42).unwrap(), 0);
    }

    #[test]
    fn a_rejected_face_nobody_else_matches_becomes_a_person_of_its_own() {
        let (db, ids) = library(2);
        db.replace_faces(ids[0], "v", &[face([1.0, 0.0])]).unwrap();
        db.replace_faces(ids[1], "v", &[face([1.0, 0.0])]).unwrap();
        assign_new_faces(&db, 0.42).unwrap();
        let only = db.faces_of(ids[0]).unwrap()[0].person_id.unwrap();
        db.reject_face(db.faces_of(ids[0]).unwrap()[0].id).unwrap();
        assign_new_faces(&db, 0.42).unwrap();
        let now = db.faces_of(ids[0]).unwrap()[0].person_id;
        assert!(now.is_some() && now != Some(only));
        // rejecting it again never sends it back to the first person (the one it was rejected from still exists)
        db.reject_face(db.faces_of(ids[0]).unwrap()[0].id).unwrap();
        assign_new_faces(&db, 0.42).unwrap();
        let third = db.faces_of(ids[0]).unwrap()[0].person_id;
        assert!(third.is_some() && third != Some(only));
    }
}
