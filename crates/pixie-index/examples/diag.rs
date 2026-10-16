//! Diagnostic: generate grid thumbnails for every photo in a library database
//! (into a throwaway cache), time them, and group failures by cause.
//! Usage: cargo run --release -p pixie-index --example diag -- <pixie.db> <cache-dir> [limit]
use pixie_core::PixieError;
use pixie_image::thumbs::{ensure_thumbnail, ThumbClass};
use pixie_index::Db;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn main() {
    let mut args = std::env::args().skip(1);
    let db = Db::open(Path::new(&args.next().expect("db path"))).unwrap();
    let cache = PathBuf::from(args.next().expect("cache dir"));
    let limit: i64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(i64::MAX);

    let total = db.count_photos().unwrap();
    let photos = db.list_photos(0, limit.min(total)).unwrap();
    println!("{} photos in library, testing {}", total, photos.len());

    let mut failures: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut times: Vec<(f64, String, i64)> = vec![];
    let t_all = Instant::now();
    for p in &photos {
        let t = Instant::now();
        let r = ensure_thumbnail(&cache, ThumbClass::Grid, p.id, p.mtime, p.size, Path::new(&p.path));
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        match r {
            Ok(_) => times.push((ms, p.path.clone(), p.size)),
            Err(e) => {
                let key = match &e {
                    PixieError::Image(m) | PixieError::Unsupported(m) | PixieError::Io(m) => m.clone(),
                    other => other.to_string(),
                };
                failures.entry(key).or_default().push(p.path.clone());
            }
        }
    }
    let secs = t_all.elapsed().as_secs_f64();
    println!("single thread: {} ok in {:.1}s ({:.0}/s)", times.len(), secs, photos.len() as f64 / secs);

    times.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    if !times.is_empty() {
        let pct = |q: f64| times[((times.len() - 1) as f64 * q) as usize].0;
        println!("per-photo ms: p50 {:.0}  p90 {:.0}  p99 {:.0}  max {:.0}", pct(0.5), pct(0.9), pct(0.99), times.last().unwrap().0);
        let (ms, path, size) = times.last().unwrap();
        println!("slowest: {ms:.0}ms {size} bytes {path}");
    }

    let nfail: usize = failures.values().map(|v| v.len()).sum();
    println!("FAILED: {nfail}");
    for (reason, paths) in &failures {
        println!("  {:>5} x {}", paths.len(), reason);
        for p in paths.iter().take(3) {
            println!("          e.g. {p}");
        }
    }
    let mut by_ext: BTreeMap<String, usize> = BTreeMap::new();
    for p in failures.values().flatten() {
        let ext = Path::new(p).extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        *by_ext.entry(ext).or_default() += 1;
    }
    println!("failures by extension: {by_ext:?}");
}
