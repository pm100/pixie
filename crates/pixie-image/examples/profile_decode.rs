//! Profile thumbnail decode on real photos.
//! Usage: cargo run --release -p pixie-image --example profile_decode -- <folder> [count]
use image::GenericImageView;
use pixie_image::decode::{decode_oriented, decode_scaled};
use pixie_image::meta::{read_meta, read_orientation};
use std::path::PathBuf;
use std::time::Instant;

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().expect("folder"));
    let count: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(20);
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| matches!(p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(), Some("jpg") | Some("jpeg")))
        .collect();
    files.sort();
    files.truncate(count);
    println!("{:>9} {:>9} {:>11} {:>13}  file", "read ms", "scaled ms", "full ms", "scaled dims");
    let (mut tr, mut ts, mut tf) = (0.0, 0.0, 0.0);
    for p in &files {
        let t = Instant::now();
        let bytes = std::fs::read(p).unwrap();
        let read = t.elapsed().as_secs_f64() * 1000.0;
        drop(bytes);

        let t = Instant::now();
        let s = decode_scaled(p, 256);
        let scaled = t.elapsed().as_secs_f64() * 1000.0;
        let dims = s.as_ref().map(|i| i.dimensions()).unwrap_or((0, 0));

        let t = Instant::now();
        let f = decode_oriented(p);
        let full = t.elapsed().as_secs_f64() * 1000.0;
        let fdims = f.as_ref().map(|i| i.dimensions()).unwrap_or((0, 0));

        println!("{read:9.1} {scaled:9.1} {full:11.1} {:>6}x{:<6} (full {}x{})  {}", dims.0, dims.1, fdims.0, fdims.1, p.file_name().unwrap().to_string_lossy());
        tr += read;
        ts += scaled;
        tf += full;
    }
    let n = files.len() as f64;
    println!("-- metadata cost (what a scan pays per photo) --");
    for p in files.iter().take(8) {
        let t = Instant::now();
        let _ = read_orientation(p);
        let o = t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        let _ = image::image_dimensions(p);
        let d = t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        let _ = read_meta(p);
        let m = t.elapsed().as_secs_f64() * 1000.0;
        println!("orientation {o:7.1} ms  image_dimensions {d:7.1} ms  read_meta total {m:7.1} ms  {}", p.file_name().unwrap().to_string_lossy());
    }
    println!("avg: read {:.1} ms, scaled {:.1} ms, full {:.1} ms over {} files", tr / n, ts / n, tf / n, files.len());
}
