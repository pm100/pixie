//! Time a first scan and a rescan of a real folder (in-memory database).
//! Usage: cargo run --release -p pixie-index --example scan_dir --features pixie-image/heic -- <folder>
use pixie_index::{scan_folder, Db};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

fn main() {
    let dir = std::env::args().nth(1).expect("folder to scan");
    let db = Db::open_in_memory().unwrap();
    let f = db.add_folder(&dir).unwrap();
    for pass in ["first scan", "rescan   "] {
        let t = Instant::now();
        let s = scan_folder(&db, f, &AtomicBool::new(false), |_| {}).unwrap();
        println!("{pass}: {:.1}s  {s:?}", t.elapsed().as_secs_f64());
    }
}
