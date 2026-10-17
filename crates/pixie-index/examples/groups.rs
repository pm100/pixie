//! Time the day-group and paging queries on a real folder (in-memory database).
//! Usage: cargo run --release -p pixie-index --example groups --features pixie-image/heic -- <folder>
use pixie_core::{Bounds, MediaKind, PhotoQuery, SortOrder};
use pixie_index::{scan_folder, Db};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

fn main() {
    let dir = std::env::args().nth(1).expect("folder to scan");
    let db = Db::open_in_memory().unwrap();
    let f = db.add_folder(&dir).unwrap();
    let t = Instant::now();
    let stats = scan_folder(&db, f, &AtomicBool::new(false), |_| {}).unwrap();
    println!("scan: {:.1} s, {stats:?}", t.elapsed().as_secs_f64());
    for kind in [None, Some(MediaKind::Photo), Some(MediaKind::Video)] {
        let q = PhotoQuery { kind, ..Default::default() };
        let t = Instant::now();
        let n = db.count_photos_in(&q).unwrap();
        let groups = db.day_groups(&q).unwrap();
        let page = db.list_photos_in(&q, 0, 5).unwrap();
        println!("{kind:?}: {n} items, {} day groups, first page {:.1} ms", groups.len(), t.elapsed().as_secs_f64() * 1000.0);
        if kind == Some(MediaKind::Video) {
            for p in page {
                println!("  {} {:?}x{:?} {:?} ms taken_at {:?} gps {:?},{:?}", p.path, p.meta.width, p.meta.height, p.meta.duration_ms, p.meta.taken_at, p.meta.gps_lat, p.meta.gps_lon);
            }
        }
    }
    let t = Instant::now();
    let points = db.list_geo_points(None, None).unwrap();
    println!("geo points: {} in {:.1} ms", points.len(), t.elapsed().as_secs_f64() * 1000.0);
    // A box around the Bay Area, as the map would send for a zoomed-in view.
    let bay = PhotoQuery { bounds: Some(Bounds { south: 36.0, west: -123.0, north: 38.5, east: -121.0 }), ..Default::default() };
    let t = Instant::now();
    let n = db.count_photos_in(&bay).unwrap();
    let groups = db.day_groups(&bay).unwrap().len();
    let page = db.list_photos_in(&bay, 0, 200).unwrap().len();
    println!("bay box: {n} items, {groups} day groups, page of {page}: {:.1} ms", t.elapsed().as_secs_f64() * 1000.0);
    // Paging with and without a snapshot id: the viewer asks for 7 rows around the current position.
    let snap = db.max_photo_id().unwrap();
    for (label, max_id) in [("no snapshot", None), ("snapshot", Some(snap))] {
        let q = PhotoQuery { max_id, ..Default::default() };
        let t = Instant::now();
        for off in [0i64, 1500, 3000, 5000, 5900] {
            let _ = db.list_photos_in(&q, off, 7).unwrap();
        }
        println!("{label}: 5 viewer-sized pages in {:.1} ms total", t.elapsed().as_secs_f64() * 1000.0);
    }
    for order in [SortOrder::Newest, SortOrder::Oldest] {
        let q = PhotoQuery { order, folder_id: None, kind: None, bounds: None, max_id: None };
        let t = Instant::now();
        let groups = db.day_groups(&q).unwrap();
        println!("{order:?}: day_groups {:.1} ms, {} groups, {} photos", t.elapsed().as_secs_f64() * 1000.0, groups.len(), groups.iter().map(|g| g.count).sum::<i64>());
        let t = Instant::now();
        let page = db.list_photos_in(&q, 3000, 200).unwrap();
        println!("{order:?}: page of {} at offset 3000: {:.1} ms", page.len(), t.elapsed().as_secs_f64() * 1000.0);
        println!("  undated: {:?}", groups.last().filter(|g| g.day.is_none()).map(|g| g.count));
    }
}
