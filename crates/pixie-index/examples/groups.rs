//! Time the day-group and paging queries on a real folder (in-memory database).
//! Usage: cargo run --release -p pixie-index --example groups --features pixie-image/heic -- <folder>
use pixie_core::{PhotoQuery, SortOrder};
use pixie_index::{scan_folder, Db};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

fn main() {
    let dir = std::env::args().nth(1).expect("folder to scan");
    let db = Db::open_in_memory().unwrap();
    let f = db.add_folder(&dir).unwrap();
    scan_folder(&db, f, &AtomicBool::new(false), |_| {}).unwrap();
    for order in [SortOrder::Newest, SortOrder::Oldest] {
        let q = PhotoQuery { order, folder_id: None };
        let t = Instant::now();
        let groups = db.day_groups(&q).unwrap();
        println!("{order:?}: day_groups {:.1} ms, {} groups, {} photos", t.elapsed().as_secs_f64() * 1000.0, groups.len(), groups.iter().map(|g| g.count).sum::<i64>());
        let t = Instant::now();
        let page = db.list_photos_in(&q, 3000, 200).unwrap();
        println!("{order:?}: page of {} at offset 3000: {:.1} ms", page.len(), t.elapsed().as_secs_f64() * 1000.0);
        println!("  undated: {:?}", groups.last().filter(|g| g.day.is_none()).map(|g| g.count));
    }
}
