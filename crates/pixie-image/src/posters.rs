use std::path::{Path, PathBuf};

pub const MAX_POSTER_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_POSTER_SIDE: u32 = 512;

pub fn poster_path(dir: &Path, id: i64, mtime: i64, size: i64) -> PathBuf {
    dir.join(format!("{id}-{mtime}-{size}.jpg"))
}

/// A poster must be a JPEG of at most 2 MiB and 512 px on a side. The bytes come from
/// the webview, so nothing else is accepted.
pub fn validate_poster(bytes: &[u8]) -> Result<(), &'static str> {
    if bytes.is_empty() || bytes.len() > MAX_POSTER_BYTES {
        return Err("poster has an unreasonable size");
    }
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return Err("poster is not a JPEG");
    }
    let (w, h) = image::ImageReader::with_format(std::io::Cursor::new(bytes), image::ImageFormat::Jpeg)
        .into_dimensions()
        .map_err(|_| "poster is not a readable JPEG")?;
    if w == 0 || h == 0 || w > MAX_POSTER_SIDE || h > MAX_POSTER_SIDE {
        return Err("poster dimensions are out of range");
    }
    Ok(())
}

pub fn save_poster(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_is_keyed_by_id_mtime_and_size() {
        let p = poster_path(Path::new("/x"), 7, 123, 456);
        assert_eq!(p, Path::new("/x").join("7-123-456.jpg"));
    }

    #[test]
    fn accepts_a_small_jpeg_and_rejects_everything_else() {
        assert_eq!(validate_poster(&pixie_testutil::jpeg(200, 120)), Ok(()));
        assert_eq!(validate_poster(&pixie_testutil::jpeg(512, 512)), Ok(()));
        assert!(validate_poster(&pixie_testutil::jpeg(513, 100)).is_err());
        assert!(validate_poster(&pixie_testutil::jpeg(100, 513)).is_err());
        assert!(validate_poster(b"not a jpeg").is_err());
        assert!(validate_poster(&[]).is_err());
        assert!(validate_poster(&vec![0xFF; MAX_POSTER_BYTES + 1]).is_err());
        let mut png = vec![0x89, b'P', b'N', b'G'];
        png.extend_from_slice(&[0; 64]);
        assert!(validate_poster(&png).is_err());
    }

    #[test]
    fn save_writes_atomically_and_creates_the_folder() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("nested").join("1-2-3.jpg");
        save_poster(&p, &[1, 2, 3]).unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), vec![1, 2, 3]);
        assert!(!p.with_extension("tmp").exists());
        save_poster(&p, &[9]).unwrap(); // overwrite
        assert_eq!(std::fs::read(&p).unwrap(), vec![9]);
    }
}
