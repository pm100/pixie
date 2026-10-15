use crate::decode::decode_oriented;
use image::{GenericImageView, ImageFormat};
use pixie_core::{PixieError, Result};
use std::path::{Path, PathBuf};

use std::sync::atomic::{AtomicU64, Ordering};

pub const GRID_EDGE: u32 = 256;
pub const VIEWER_EDGE: u32 = 1600;

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThumbClass {
    Grid,
    Viewer,
}

impl ThumbClass {
    pub fn edge(self) -> u32 {
        match self {
            ThumbClass::Grid => GRID_EDGE,
            ThumbClass::Viewer => VIEWER_EDGE,
        }
    }
    fn dir(self) -> &'static str {
        match self {
            ThumbClass::Grid => "grid",
            ThumbClass::Viewer => "view",
        }
    }
}

pub fn thumb_path(cache_dir: &Path, class: ThumbClass, photo_id: i64, mtime: i64, size: i64) -> PathBuf {
    cache_dir.join(class.dir()).join(format!("{photo_id}-{mtime}-{size}.jpg"))
}

/// Returns the cached thumbnail, generating it atomically if missing.
pub fn ensure_thumbnail(
    cache_dir: &Path,
    class: ThumbClass,
    photo_id: i64,
    mtime: i64,
    size: i64,
    src: &Path,
) -> Result<PathBuf> {
    let dst = thumb_path(cache_dir, class, photo_id, mtime, size);
    if dst.exists() {
        return Ok(dst);
    }
    let img = decode_oriented(src)?;
    let edge = class.edge();
    let (w, h) = img.dimensions();
    let thumb = if w <= edge && h <= edge { img } else { img.thumbnail(edge, edge) };

    std::fs::create_dir_all(dst.parent().expect("thumb path has parent"))?;
    // Unique temp name so concurrent callers never clobber each other.
    let tmp = dst.with_extension(format!("{}.tmp", TMP_COUNTER.fetch_add(1, Ordering::Relaxed)));
    thumb
        .to_rgb8()
        .save_with_format(&tmp, ImageFormat::Jpeg)
        .map_err(|e| PixieError::Image(e.to_string()))?;
    std::fs::rename(&tmp, &dst)?;
    Ok(dst)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixie_testutil::{jpeg, jpeg_with_exif};

    #[test]
    fn thumbnail_is_downscaled_cached_and_keyed_by_mtime() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.jpg");
        std::fs::write(&src, jpeg(1000, 500)).unwrap();
        let cache = dir.path().join("cache");

        let t1 = ensure_thumbnail(&cache, ThumbClass::Grid, 7, 111, 0, &src).unwrap();
        assert!(t1.starts_with(&cache) && t1.exists());
        let (w, h) = image::open(&t1).unwrap().dimensions();
        assert_eq!((w, h), (256, 128));

        // Same key -> same file, not regenerated even if the source vanishes.
        std::fs::remove_file(&src).unwrap();
        assert_eq!(ensure_thumbnail(&cache, ThumbClass::Grid, 7, 111, 0, &src).unwrap(), t1);

        // New mtime -> different path (replaced file gets a fresh thumbnail).
        assert_ne!(thumb_path(&cache, ThumbClass::Grid, 7, 222, 0), t1);
    }

    #[test]
    fn small_images_are_not_upscaled() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("s.jpg");
        std::fs::write(&src, jpeg(100, 50)).unwrap();
        let t = ensure_thumbnail(&dir.path().join("c"), ThumbClass::Grid, 1, 1, 0, &src).unwrap();
        assert_eq!(image::open(&t).unwrap().dimensions(), (100, 50));
    }

    #[test]
    fn exif_rotated_photo_gets_upright_thumbnail() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("r.jpg");
        std::fs::write(&src, jpeg_with_exif(400, 200, 6, None)).unwrap();
        let t = ensure_thumbnail(&dir.path().join("c"), ThumbClass::Grid, 2, 1, 0, &src).unwrap();
        let img = image::open(&t).unwrap();
        let (w, h) = img.dimensions();
        assert!(h > w, "expected portrait after rotation, got {w}x{h}");
        // top of the portrait is the old left side (red), bottom is blue
        let rgb = img.to_rgb8();
        assert!(rgb.get_pixel(w / 2, 5)[0] > 200);
        assert!(rgb.get_pixel(w / 2, h - 5)[2] > 200);
    }

    #[test]
    fn corrupt_source_returns_error_and_leaves_no_file() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("bad.jpg");
        std::fs::write(&src, b"junk").unwrap();
        let cache = dir.path().join("c");
        assert!(ensure_thumbnail(&cache, ThumbClass::Grid, 3, 1, 0, &src).is_err());
        assert!(!thumb_path(&cache, ThumbClass::Grid, 3, 1, 0).exists());
    }

    #[test]
    fn cache_key_changes_when_size_changes_with_same_mtime() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.jpg");
        std::fs::write(&src, jpeg(100, 50)).unwrap();
        let cache = dir.path().join("c");
        let a = ensure_thumbnail(&cache, ThumbClass::Grid, 1, 5, 1000, &src).unwrap();
        let b = ensure_thumbnail(&cache, ThumbClass::Grid, 1, 5, 2000, &src).unwrap();
        assert_ne!(a, b);
    }
}
