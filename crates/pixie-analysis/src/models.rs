use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

/// A model file we download once and keep: its name on disk, where it comes from, and what it must hash to.
pub struct ModelFile {
    pub name: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub bytes: u64,
}

/// OpenAI CLIP ViT-B/32 (MIT), int8 ONNX export: image tower, text tower and tokenizer.
pub const CLIP_FILES: &[ModelFile] = &[
    ModelFile {
        name: "vision.onnx",
        url: "https://huggingface.co/Xenova/clip-vit-base-patch32/resolve/d15189d7028b43f1d3e65039190477f6af591c2a/onnx/vision_model_quantized.onnx",
        sha256: "583fd1110a514667812fee7d684952aaf82a99b959760c8d7dca7e0ab9839299",
        bytes: 89_117_001,
    },
    ModelFile {
        name: "text.onnx",
        url: "https://huggingface.co/Xenova/clip-vit-base-patch32/resolve/d15189d7028b43f1d3e65039190477f6af591c2a/onnx/text_model_quantized.onnx",
        sha256: "73baab855d406190da9faa498cfedf65f15cf309f4cc7385b7b032e6d08e5c3a",
        bytes: 64_504_507,
    },
    ModelFile {
        name: "tokenizer.json",
        url: "https://huggingface.co/Xenova/clip-vit-base-patch32/resolve/d15189d7028b43f1d3e65039190477f6af591c2a/tokenizer.json",
        sha256: "f7f3b7af117d467b58374797691a6438d3e6b9e9cef800dfd5dced7f697a90cd",
        bytes: 2_224_119,
    },
];

/// OpenCV Zoo (commit 47534e2): YuNet face detector (MIT) and SFace face recogniser (Apache 2.0), both ONNX.
pub const FACE_FILES: &[ModelFile] = &[
    ModelFile {
        name: "yunet.onnx",
        url: "https://github.com/opencv/opencv_zoo/raw/47534e27c9851bb1128ccc0102f1145e27f23f98/models/face_detection_yunet/face_detection_yunet_2023mar.onnx",
        sha256: "8f2383e4dd3cfbb4553ea8718107fc0423210dc964f9f4280604804ed2552fa4",
        bytes: 232_589,
    },
    ModelFile {
        name: "sface.onnx",
        url: "https://github.com/opencv/opencv_zoo/raw/47534e27c9851bb1128ccc0102f1145e27f23f98/models/face_recognition_sface/face_recognition_sface_2021dec.onnx",
        sha256: "0ba9fbfa01b5270c96627c4ef784da859931e02f04419c829e83484087c34e79",
        bytes: 38_696_353,
    },
];

/// Streams a URL to `sink`; the sink returns false to stop early.
pub trait Downloader: Send + Sync {
    fn fetch(&self, url: &str, sink: &mut dyn FnMut(&[u8]) -> bool) -> Result<(), String>;
}

pub struct HttpDownloader;

impl Downloader for HttpDownloader {
    fn fetch(&self, url: &str, sink: &mut dyn FnMut(&[u8]) -> bool) -> Result<(), String> {
        // Timeouts, so a connection that goes quiet fails (and Retry works) instead of hanging forever.
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(20))
            .timeout_read(Duration::from_secs(30))
            .build();
        let resp = agent.get(url).call().map_err(|e| format!("download failed: {e}"))?;
        let mut r = resp.into_reader();
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = r.read(&mut buf).map_err(|e| format!("download failed: {e}"))?;
            if n == 0 || !sink(&buf[..n]) {
                return Ok(());
            }
        }
    }
}

fn hex(h: Sha256) -> String {
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

fn file_ok(path: &Path, f: &ModelFile) -> bool {
    let Ok(mut file) = std::fs::File::open(path) else { return false };
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => h.update(&buf[..n]),
            Err(_) => return false,
        }
    }
    hex(h) == f.sha256
}

/// Every file is on disk and hashes correctly.
pub fn models_ready(dir: &Path, files: &[ModelFile]) -> bool {
    files.iter().all(|f| file_ok(&dir.join(f.name), f))
}

/// Download whatever is missing or damaged into `dir`. Each file goes to `name.part` and is renamed only after its
/// hash checks out. `progress(done, total)` runs as bytes arrive; returning false cancels.
pub fn ensure_models(
    dir: &Path,
    files: &[ModelFile],
    dl: &dyn Downloader,
    progress: &mut dyn FnMut(u64, u64) -> bool,
) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let total: u64 = files.iter().map(|f| f.bytes).sum();
    let mut done: u64 = 0;
    for f in files {
        let dest = dir.join(f.name);
        if file_ok(&dest, f) {
            done += f.bytes;
            if !progress(done, total) {
                return Err("cancelled".into());
            }
            continue;
        }
        let part = dir.join(format!("{}.part", f.name));
        let result = (|| {
            let mut out = std::fs::File::create(&part).map_err(|e| format!("cannot write {}: {e}", part.display()))?;
            let mut h = Sha256::new();
            let (mut got, mut write_err, mut cancelled) = (0u64, None, false);
            dl.fetch(f.url, &mut |chunk| {
                if let Err(e) = out.write_all(chunk) {
                    write_err = Some(e.to_string());
                    return false;
                }
                h.update(chunk);
                got += chunk.len() as u64;
                if !progress(done + got, total) {
                    cancelled = true;
                    return false;
                }
                true
            })?;
            if let Some(e) = write_err {
                return Err(format!("cannot write {}: {e}", f.name));
            }
            if cancelled {
                return Err("cancelled".to_string());
            }
            if hex(h) != f.sha256 {
                return Err(format!("checksum mismatch for {}", f.name));
            }
            drop(out);
            std::fs::rename(&part, &dest).map_err(|e| format!("cannot finish {}: {e}", f.name))
        })();
        if let Err(e) = result {
            let _ = std::fs::remove_file(&part);
            return Err(e);
        }
        done += f.bytes;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    struct FakeDownloader {
        files: HashMap<String, Vec<u8>>,
        calls: Mutex<usize>,
    }

    impl Downloader for FakeDownloader {
        fn fetch(&self, url: &str, sink: &mut dyn FnMut(&[u8]) -> bool) -> Result<(), String> {
            *self.calls.lock().unwrap() += 1;
            let data = self.files.get(url).ok_or("404")?;
            for chunk in data.chunks(4) {
                if !sink(chunk) {
                    break;
                }
            }
            Ok(())
        }
    }

    fn hex(data: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect()
    }

    /// One 10-byte file "m.bin" served at "u://m" whose hash is `claimed` (or the true hash when None).
    fn setup(claimed: Option<&str>) -> (FakeDownloader, Vec<ModelFile>) {
        let data = b"0123456789".to_vec();
        let sha: &'static str = Box::leak(claimed.map(str::to_owned).unwrap_or_else(|| hex(&data)).into_boxed_str());
        let dl = FakeDownloader { files: HashMap::from([("u://m".to_string(), data)]), calls: Mutex::new(0) };
        (dl, vec![ModelFile { name: "m.bin", url: "u://m", sha256: sha, bytes: 10 }])
    }

    #[test]
    fn downloads_verifies_and_skips_when_present() {
        let dir = tempfile::tempdir().unwrap();
        let (dl, files) = setup(None);
        assert!(!models_ready(dir.path(), &files));
        ensure_models(dir.path(), &files, &dl, &mut |_, _| true).unwrap();
        assert!(models_ready(dir.path(), &files));
        assert_eq!(std::fs::read(dir.path().join("m.bin")).unwrap(), b"0123456789");
        ensure_models(dir.path(), &files, &dl, &mut |_, _| true).unwrap();
        assert_eq!(*dl.calls.lock().unwrap(), 1); // the second call found the file and fetched nothing
    }

    #[test]
    fn corrupt_file_on_disk_is_fetched_again() {
        let dir = tempfile::tempdir().unwrap();
        let (dl, files) = setup(None);
        std::fs::write(dir.path().join("m.bin"), b"garbage!!!").unwrap();
        assert!(!models_ready(dir.path(), &files));
        ensure_models(dir.path(), &files, &dl, &mut |_, _| true).unwrap();
        assert_eq!(std::fs::read(dir.path().join("m.bin")).unwrap(), b"0123456789");
    }

    #[test]
    fn checksum_mismatch_leaves_no_file_and_errors() {
        let dir = tempfile::tempdir().unwrap();
        let (dl, files) = setup(Some("00"));
        let err = ensure_models(dir.path(), &files, &dl, &mut |_, _| true).unwrap_err();
        assert!(err.contains("checksum mismatch"), "{err}");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0); // neither m.bin nor m.bin.part
    }

    #[test]
    fn cancel_stops_and_removes_the_part_file() {
        let dir = tempfile::tempdir().unwrap();
        let (dl, files) = setup(None);
        let err = ensure_models(dir.path(), &files, &dl, &mut |_, _| false).unwrap_err();
        assert!(err.contains("cancelled"), "{err}");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn progress_reaches_the_total() {
        let dir = tempfile::tempdir().unwrap();
        let (dl, files) = setup(None);
        let mut last = (0, 0);
        ensure_models(dir.path(), &files, &dl, &mut |d, t| {
            last = (d, t);
            true
        })
        .unwrap();
        assert_eq!(last, (10, 10));
    }

    #[test]
    fn a_failed_fetch_is_an_error_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        let (_, files) = setup(None);
        let empty = FakeDownloader { files: HashMap::new(), calls: Mutex::new(0) };
        assert!(ensure_models(dir.path(), &files, &empty, &mut |_, _| true).is_err());
    }

    #[test]
    fn face_files_are_pinned() {
        assert_eq!(FACE_FILES.len(), 2);
        for f in FACE_FILES {
            assert_eq!(f.sha256.len(), 64, "{}", f.name);
            assert!(f.sha256.chars().all(|c| c.is_ascii_hexdigit()));
            assert!(f.bytes > 0 && f.url.starts_with("https://"));
        }
    }
}
