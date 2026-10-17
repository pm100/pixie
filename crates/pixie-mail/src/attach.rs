use crate::text::safe_filename;
use image::codecs::jpeg::JpegEncoder;
use pixie_core::{MediaKind, Photo, PixieError, Result};
use serde::Deserialize;
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SizeChoice {
    Small,
    Medium,
    Original,
}

impl SizeChoice {
    pub fn edge(self) -> Option<u32> {
        match self {
            SizeChoice::Small => Some(1600),
            SizeChoice::Medium => Some(2400),
            SizeChoice::Original => None,
        }
    }
}

#[derive(Debug)]
pub struct Attachment {
    pub filename: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

fn unreadable(name: &str) -> PixieError {
    PixieError::Invalid(format!("Pixie could not read \"{name}\" to attach it. It may be damaged or moved."))
}

fn mime_of(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref() {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("tif") | Some("tiff") => "image/tiff",
        Some("heic") => "image/heic",
        Some("heif") => "image/heif",
        Some("mov") => "video/quicktime",
        Some("m4v") => "video/x-m4v",
        Some("mp4") => "video/mp4",
        _ => "application/octet-stream",
    }
}

/// One attachment, prepared from the file on disk. Photos are shrunk (upright, JPEG, no metadata)
/// unless `Original`; videos are always sent as they are. The original file is never changed.
pub fn prepare_attachment(photo: &Photo, choice: SizeChoice, taken: &mut HashSet<String>) -> Result<Attachment> {
    let path = Path::new(&photo.path);
    let shown = path.file_name().and_then(|n| n.to_str()).unwrap_or("file").to_string();
    let edge = match (photo.meta.kind, choice.edge()) {
        (MediaKind::Photo, Some(edge)) => edge,
        _ => {
            let bytes = std::fs::read(path).map_err(|_| unreadable(&shown))?;
            return Ok(Attachment { filename: safe_filename(&shown, None, taken), content_type: mime_of(path).into(), bytes });
        }
    };
    let img = pixie_image::decode::decode_scaled(path, edge).map_err(|_| unreadable(&shown))?;
    let img = if img.width().max(img.height()) > edge {
        img.resize(edge, edge, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };
    let mut bytes = Vec::new();
    JpegEncoder::new_with_quality(&mut bytes, 85)
        .encode_image(&img.to_rgb8())
        .map_err(|_| unreadable(&shown))?;
    Ok(Attachment { filename: safe_filename(&shown, Some("jpg"), taken), content_type: "image/jpeg".into(), bytes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixie_core::{MediaKind, PhotoMeta};

    fn photo(path: &std::path::Path, kind: MediaKind, size: i64) -> Photo {
        Photo {
            id: 1, folder_id: 1, path: path.to_string_lossy().into(), size, mtime: 0,
            meta: PhotoMeta { kind, ..Default::default() }, missing: false,
        }
    }

    #[test]
    fn small_and_medium_shrink_the_longer_side_and_never_enlarge() {
        let d = tempfile::tempdir().unwrap();
        let big = d.path().join("big.jpg");
        std::fs::write(&big, pixie_testutil::jpeg(3200, 2000)).unwrap();
        let mut taken = HashSet::new();
        let a = prepare_attachment(&photo(&big, MediaKind::Photo, 1), SizeChoice::Small, &mut taken).unwrap();
        assert_eq!((a.filename.as_str(), a.content_type.as_str()), ("big.jpg", "image/jpeg"));
        let img = image::load_from_memory(&a.bytes).unwrap();
        assert_eq!((img.width(), img.height()), (1600, 1000));
        let m = prepare_attachment(&photo(&big, MediaKind::Photo, 1), SizeChoice::Medium, &mut HashSet::new()).unwrap();
        assert_eq!(image::load_from_memory(&m.bytes).unwrap().width(), 2400);
        let small = d.path().join("small.jpg");
        std::fs::write(&small, pixie_testutil::jpeg(400, 300)).unwrap();
        let s = prepare_attachment(&photo(&small, MediaKind::Photo, 1), SizeChoice::Small, &mut HashSet::new()).unwrap();
        assert_eq!(image::load_from_memory(&s.bytes).unwrap().width(), 400); // not enlarged
    }

    #[test]
    fn resized_copies_carry_no_exif_and_respect_orientation() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("rot.jpg");
        std::fs::write(&p, pixie_testutil::jpeg_with_exif(300, 200, 6, Some("2020:01:02 03:04:05"))).unwrap(); // rotated 90
        let a = prepare_attachment(&photo(&p, MediaKind::Photo, 1), SizeChoice::Small, &mut HashSet::new()).unwrap();
        let img = image::load_from_memory(&a.bytes).unwrap();
        assert_eq!((img.width(), img.height()), (200, 300)); // upright
        assert!(!a.bytes.windows(6).any(|w| w == b"Exif\0\0"), "no EXIF in the copy");
    }

    #[test]
    fn original_sends_the_file_unchanged_and_videos_are_always_original() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("a.jpg");
        let bytes = pixie_testutil::jpeg(100, 100);
        std::fs::write(&p, &bytes).unwrap();
        let a = prepare_attachment(&photo(&p, MediaKind::Photo, 1), SizeChoice::Original, &mut HashSet::new()).unwrap();
        assert_eq!(a.bytes, bytes);
        let v = d.path().join("clip.MOV");
        std::fs::write(&v, [1, 2, 3, 4]).unwrap();
        let a = prepare_attachment(&photo(&v, MediaKind::Video, 4), SizeChoice::Small, &mut HashSet::new()).unwrap();
        assert_eq!((a.bytes.as_slice(), a.content_type.as_str(), a.filename.as_str()), (&[1u8, 2, 3, 4][..], "video/quicktime", "clip.MOV"));
    }

    #[test]
    fn unreadable_files_are_reported_by_name() {
        let d = tempfile::tempdir().unwrap();
        let missing = d.path().join("gone.jpg");
        let e = prepare_attachment(&photo(&missing, MediaKind::Photo, 1), SizeChoice::Small, &mut HashSet::new()).unwrap_err();
        assert!(e.user_message().contains("gone.jpg"), "{}", e.user_message());
        let junk = d.path().join("junk.jpg");
        std::fs::write(&junk, [0xAB; 50]).unwrap();
        assert!(prepare_attachment(&photo(&junk, MediaKind::Photo, 1), SizeChoice::Small, &mut HashSet::new())
            .unwrap_err()
            .user_message()
            .contains("junk.jpg"));
    }

    #[test]
    fn two_files_with_the_same_name_get_different_attachment_names() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir(d.path().join("x")).unwrap();
        std::fs::create_dir(d.path().join("y")).unwrap();
        let (a, b) = (d.path().join("x/IMG.jpg"), d.path().join("y/IMG.jpg"));
        std::fs::write(&a, pixie_testutil::jpeg(10, 10)).unwrap();
        std::fs::write(&b, pixie_testutil::jpeg(10, 10)).unwrap();
        let mut taken = HashSet::new();
        let n1 = prepare_attachment(&photo(&a, MediaKind::Photo, 1), SizeChoice::Small, &mut taken).unwrap().filename;
        let n2 = prepare_attachment(&photo(&b, MediaKind::Photo, 1), SizeChoice::Small, &mut taken).unwrap().filename;
        assert_ne!(n1, n2);
    }
}
