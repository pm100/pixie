use pixie_image::{service::ThumbService, thumbs::ThumbClass};
use pixie_index::{scan_folder, Db};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

fn main() {
    let n: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(50_000);
    let dir = tempfile::tempdir().unwrap();
    let sample = pixie_testutil::jpeg(1600, 1200);
    for i in 0..n {
        let sub = dir.path().join(format!("d{:03}", i % 200));
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join(format!("{i}.jpg")), &sample).unwrap();
    }

    let db = Db::open_in_memory().unwrap();
    let f = db.add_folder(dir.path().to_str().unwrap()).unwrap();

    let t = Instant::now();
    let s = scan_folder(&db, f, &AtomicBool::new(false), |_| {}).unwrap();
    println!("first scan:  {n} files in {:.1}s  ({s:?})", t.elapsed().as_secs_f64());

    let t = Instant::now();
    scan_folder(&db, f, &AtomicBool::new(false), |_| {}).unwrap();
    println!("rescan:      {:.1}s (nothing changed)", t.elapsed().as_secs_f64());

    let cache: PathBuf = dir.path().join("cache");
    let svc = ThumbService::new(cache, std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4));
    let photos = db.list_photos(0, 500).unwrap();
    let t = Instant::now();
    let rxs: Vec<_> = photos
        .iter()
        .map(|p| svc.request(ThumbClass::Grid, p.id, p.mtime, PathBuf::from(&p.path)))
        .collect();
    for rx in rxs {
        rx.recv().unwrap().unwrap();
    }
    let secs = t.elapsed().as_secs_f64();
    println!("thumbnails:  {} in {:.1}s ({:.0}/s)", photos.len(), secs, photos.len() as f64 / secs);
}
