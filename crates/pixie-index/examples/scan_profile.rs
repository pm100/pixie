//! Time read_meta for every supported photo under a folder; show the slowest.
//! Usage: cargo run --release -p pixie-index --example scan_profile --features pixie-image/heic -- <folder>
use pixie_core::is_supported_image;
use pixie_image::meta::read_meta;
use std::collections::BTreeMap;
use std::time::Instant;
use walkdir::WalkDir;

fn main() {
    let dir = std::env::args().nth(1).expect("folder");
    let t = Instant::now();
    let files: Vec<_> = WalkDir::new(&dir).into_iter().filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && is_supported_image(e.path())).map(|e| e.into_path()).collect();
    println!("walk: {} files in {:.1}s", files.len(), t.elapsed().as_secs_f64());

    let mut times: Vec<(f64, String)> = vec![];
    let mut by_ext: BTreeMap<String, (usize, f64)> = BTreeMap::new();
    let t_all = Instant::now();
    let limit: usize = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(usize::MAX);
    let mut block = Instant::now();
    for (n, p) in files.iter().take(limit).enumerate() {
        if n > 0 && n % 250 == 0 {
            println!("files {:>5}..{:<5} avg {:6.1} ms/file", n - 250, n, block.elapsed().as_secs_f64() * 1000.0 / 250.0);
            block = Instant::now();
        }
        let t = Instant::now();
        let _ = read_meta(p);
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        let e = by_ext.entry(ext).or_default();
        e.0 += 1;
        e.1 += ms;
        times.push((ms, p.display().to_string()));
    }
    println!("read_meta total: {:.1}s", t_all.elapsed().as_secs_f64());
    for (ext, (n, ms)) in &by_ext {
        println!("  .{ext}: {n} files, avg {:.1} ms, total {:.1}s", ms / *n as f64, ms / 1000.0);
    }
    times.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    println!("slowest 8:");
    for (ms, p) in times.iter().take(8) { println!("  {ms:8.1} ms  {p}"); }
}
