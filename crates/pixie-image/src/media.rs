use crate::{meta::read_meta, video_meta::read_video_meta};
use pixie_core::{MediaKind, PhotoMeta};
use std::path::Path;

/// Best-effort metadata for a photo or a video; never fails.
pub fn read_media_meta(path: &Path, kind: MediaKind) -> PhotoMeta {
    match kind {
        MediaKind::Photo => read_meta(path),
        MediaKind::Video => read_video_meta(path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatches_by_kind() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("x.mp4");
        std::fs::write(&p, []).unwrap();
        assert_eq!(read_media_meta(&p, MediaKind::Video).kind, MediaKind::Video);
        assert_eq!(read_media_meta(&p, MediaKind::Photo).kind, MediaKind::Photo);
    }
}
