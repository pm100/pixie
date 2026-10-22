use crate::cache::PlaceCache;
use crate::fetch::{Fetcher, GeoError};
use crate::place::{parse_landmark, parse_locality, place_key, Lang, Place};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct PlaceService {
    cache: Mutex<PlaceCache>,
    fetcher: Arc<dyn Fetcher>,
    last_request: Mutex<Option<Instant>>,
    gap: Duration,
    /// Counts lookups started. A lookup that finds a newer one behind it while it
    /// waits for its turn gives up: the UI only ever wants the latest answer.
    started: AtomicU64,
}

impl PlaceService {
    pub fn new(cache: PlaceCache, fetcher: Arc<dyn Fetcher>, gap: Duration) -> Self {
        PlaceService { cache: Mutex::new(cache), fetcher, last_request: Mutex::new(None), gap, started: AtomicU64::new(0) }
    }

    /// The place for something the user is looking at. Only the newest such request matters: one still waiting its
    /// turn when a newer one arrives gives up with `Superseded`, so paging quickly through photos does not queue up
    /// lookups for ones that are gone.
    pub fn place_for(&self, lat: f64, lon: f64, lang: &str) -> Result<Option<Place>, GeoError> {
        self.lookup(lat, lon, lang, true)
    }

    /// The place for work nobody is watching (naming trips). It waits its turn like any other lookup, but it never
    /// supersedes a request for what the user is looking at, and such a request never supersedes it.
    pub fn place_for_background(&self, lat: f64, lon: f64, lang: &str) -> Result<Option<Place>, GeoError> {
        self.lookup(lat, lon, lang, false)
    }

    fn lookup(&self, lat: f64, lon: f64, lang: &str, newest_wins: bool) -> Result<Option<Place>, GeoError> {
        if !(lat.is_finite() && lon.is_finite() && lat.abs() <= 90.0 && lon.abs() <= 180.0) {
            return Ok(None);
        }
        let lang = Lang::parse(lang);
        let key = format!("{}|{}", place_key(lat, lon), lang.accept);
        let ticket = if newest_wins { self.started.fetch_add(1, Ordering::SeqCst) + 1 } else { 0 };
        if let Some(hit) = self.cache.lock().unwrap().get(&key)? {
            return Ok(hit);
        }
        // Holding this lock through the Nominatim request serialises lookups, which is
        // what keeps us under its one-request-per-second policy.
        let mut last = self.last_request.lock().unwrap();
        // A caller ahead of us may have just filled this key.
        if let Some(hit) = self.cache.lock().unwrap().get(&key)? {
            return Ok(hit);
        }
        if newest_wins && self.started.load(Ordering::SeqCst) != ticket {
            return Err(GeoError::Superseded);
        }
        if let Some(t) = *last {
            let since = t.elapsed();
            if since < self.gap {
                std::thread::sleep(self.gap - since);
            }
        }
        // The address line comes from Nominatim; the landmark from the nearest
        // Wikipedia article (reverse geocoding alone returns the nearest object of any
        // kind: a clock, a clinic, a footpath).
        let address = self.fetcher.get(&format!(
            "https://nominatim.openstreetmap.org/reverse?format=jsonv2&lat={lat:.5}&lon={lon:.5}&zoom=14&addressdetails=1&accept-language={}",
            lang.accept
        ));
        *last = Some(Instant::now());
        let address = address?;
        let wiki = |edition: &str| {
            self.fetcher.get(&format!(
                "https://{edition}.wikipedia.org/w/api.php?action=query&list=geosearch&gscoord={lat:.5}%7C{lon:.5}&gsradius=200&gslimit=10&format=json&formatversion=2"
            ))
        };
        let mut landmark = parse_landmark(&String::from_utf8_lossy(&wiki(&lang.wiki)?));
        if landmark.is_none() && lang.wiki != "en" {
            // Their edition may simply lack an article for this place.
            landmark = parse_landmark(&String::from_utf8_lossy(&wiki("en")?));
        }
        let locality = parse_locality(&String::from_utf8_lossy(&address));
        let place = (locality.is_some() || landmark.is_some()).then_some(Place { landmark, locality });
        self.cache.lock().unwrap().put(&key, place.as_ref())?;
        Ok(place)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fetch::testing::FakeFetcher;

    const ROME: &str = r#"{"category":"highway","address":{"city":"Roma","state":"Lazio","country":"Italia"}}"#;
    const WIKI_COLOSSEUM: &str = r#"{"query":{"geosearch":[{"title":"Colosseum","dist":10.2}]}}"#;
    const WIKI_NONE: &str = r#"{"query":{"geosearch":[]}}"#;
    const NOTHING: &str = r#"{"error":"Unable to geocode"}"#;

    fn fake(nominatim: &str, wiki: &str) -> Arc<FakeFetcher> {
        let f = Arc::new(FakeFetcher::new(Ok(nominatim.as_bytes().to_vec())));
        f.set_wiki(Ok(wiki.as_bytes().to_vec()));
        f
    }

    fn service(f: &Arc<FakeFetcher>, gap_ms: u64) -> PlaceService {
        PlaceService::new(PlaceCache::open_in_memory().unwrap(), f.clone(), Duration::from_millis(gap_ms))
    }

    #[test]
    fn combines_locality_and_landmark() {
        let f = fake(ROME, WIKI_COLOSSEUM);
        let p = service(&f, 0).place_for(41.8902, 12.4922, "en").unwrap().unwrap();
        assert_eq!(p.landmark.as_deref(), Some("Colosseum"));
        assert_eq!(p.locality.as_deref(), Some("Roma, Lazio, Italia"));
    }

    #[test]
    fn locality_alone_when_no_article_is_nearby() {
        let f = fake(ROME, WIKI_NONE);
        let p = service(&f, 0).place_for(41.0, 12.0, "en").unwrap().unwrap();
        assert_eq!(p.landmark, None);
        assert_eq!(p.locality.as_deref(), Some("Roma, Lazio, Italia"));
    }

    #[test]
    fn landmark_alone_when_nominatim_has_no_address() {
        let f = fake(NOTHING, WIKI_COLOSSEUM);
        let p = service(&f, 0).place_for(41.0, 12.0, "en").unwrap().unwrap();
        assert_eq!(p.landmark.as_deref(), Some("Colosseum"));
        assert_eq!(p.locality, None);
    }

    #[test]
    fn looks_up_once_then_serves_from_cache() {
        let f = fake(ROME, WIKI_COLOSSEUM);
        let s = service(&f, 0);
        let a = s.place_for(41.8902, 12.4922, "en").unwrap().unwrap();
        assert_eq!(s.place_for(41.8902, 12.4922, "en").unwrap(), Some(a));
        assert_eq!(f.count_matching("nominatim"), 1);
        assert_eq!(f.count_matching("wikipedia"), 1);
    }

    #[test]
    fn points_within_rounding_share_one_lookup() {
        let f = fake(ROME, WIKI_COLOSSEUM);
        let s = service(&f, 0);
        s.place_for(41.89021, 12.49223, "en").unwrap();
        s.place_for(41.89019, 12.49224, "en").unwrap();
        assert_eq!(f.count(), 2);
    }

    #[test]
    fn a_miss_is_cached() {
        let f = fake(NOTHING, WIKI_NONE);
        let s = service(&f, 0);
        assert_eq!(s.place_for(10.0, 10.0, "en").unwrap(), None);
        assert_eq!(s.place_for(10.0, 10.0, "en").unwrap(), None);
        assert_eq!(f.count(), 2);
    }

    #[test]
    fn network_failure_is_reported_and_not_cached() {
        let f = Arc::new(FakeFetcher::new(Err(GeoError::Network("offline".into()))));
        let s = service(&f, 0);
        assert!(s.place_for(10.0, 10.0, "en").is_err());
        f.set(Ok(ROME.as_bytes().to_vec()));
        assert!(s.place_for(10.0, 10.0, "en").unwrap().is_some());
    }

    #[test]
    fn wikipedia_failure_is_not_cached_either() {
        let f = fake(ROME, WIKI_NONE);
        f.set_wiki(Err(GeoError::Network("down".into())));
        let s = service(&f, 0);
        assert!(s.place_for(10.0, 10.0, "en").is_err());
        f.set_wiki(Ok(WIKI_COLOSSEUM.as_bytes().to_vec()));
        let p = s.place_for(10.0, 10.0, "en").unwrap().unwrap();
        assert_eq!(p.landmark.as_deref(), Some("Colosseum"));
    }

    #[test]
    fn nominatim_requests_are_spaced_by_the_gap() {
        let f = fake(ROME, WIKI_NONE);
        let s = service(&f, 80);
        s.place_for(1.0, 1.0, "en").unwrap();
        s.place_for(2.0, 2.0, "en").unwrap();
        let calls = f.calls.lock().unwrap();
        let times: Vec<_> = calls.iter().filter(|(u, _)| u.contains("nominatim")).map(|(_, t)| *t).collect();
        assert_eq!(times.len(), 2);
        assert!(times[1].duration_since(times[0]) >= Duration::from_millis(80));
    }

    #[test]
    fn asks_in_the_users_language_and_prefers_their_wikipedia() {
        let f = fake(ROME, WIKI_COLOSSEUM);
        service(&f, 0).place_for(41.0, 12.0, "de-DE,de").unwrap();
        let calls = f.calls.lock().unwrap();
        assert!(calls[0].0.contains("accept-language=de-DE,de"), "{}", calls[0].0);
        assert!(calls[1].0.starts_with("https://de.wikipedia.org/"), "{}", calls[1].0);
        assert_eq!(calls.len(), 2); // found in de: no English fallback
    }

    #[test]
    fn falls_back_to_english_wikipedia_when_their_edition_has_nothing() {
        let f = fake(ROME, WIKI_NONE);
        let s = service(&f, 0);
        s.place_for(41.0, 12.0, "de").unwrap();
        let calls = f.calls.lock().unwrap();
        assert!(calls[1].0.starts_with("https://de.wikipedia.org/"));
        assert!(calls[2].0.starts_with("https://en.wikipedia.org/"));
        assert_eq!(calls.len(), 3);
    }

    #[test]
    fn the_cache_is_per_language() {
        let f = fake(ROME, WIKI_COLOSSEUM);
        let s = service(&f, 0);
        s.place_for(41.0, 12.0, "en").unwrap();
        s.place_for(41.0, 12.0, "de").unwrap();
        s.place_for(41.0, 12.0, "de").unwrap();
        assert_eq!(f.count_matching("nominatim"), 2);
    }

    /// First Nominatim request blocks until released; counts Nominatim requests.
    struct Gated {
        first: std::sync::atomic::AtomicBool,
        release: Mutex<std::sync::mpsc::Receiver<()>>,
        nominatim: std::sync::atomic::AtomicUsize,
    }

    impl Fetcher for Gated {
        fn get(&self, url: &str) -> Result<Vec<u8>, GeoError> {
            use std::sync::atomic::Ordering::SeqCst;
            if url.contains("nominatim") {
                self.nominatim.fetch_add(1, SeqCst);
                if self.first.swap(false, SeqCst) {
                    self.release.lock().unwrap().recv().unwrap();
                }
                Ok(ROME.as_bytes().to_vec())
            } else {
                Ok(WIKI_NONE.as_bytes().to_vec())
            }
        }
    }

    #[test]
    fn a_queued_lookup_is_superseded_by_a_newer_one_without_touching_the_network() {
        use std::sync::atomic::Ordering::SeqCst;
        let (tx, rx) = std::sync::mpsc::channel();
        let g = Arc::new(Gated {
            first: true.into(),
            release: Mutex::new(rx),
            nominatim: 0.into(),
        });
        let s = Arc::new(PlaceService::new(PlaceCache::open_in_memory().unwrap(), g.clone(), Duration::ZERO));
        let spawn = |lat: f64| {
            let s = s.clone();
            std::thread::spawn(move || s.place_for(lat, lat, "en"))
        };
        let a = spawn(1.0); // blocks inside the first request
        std::thread::sleep(Duration::from_millis(150));
        let b = spawn(2.0); // queued behind A
        std::thread::sleep(Duration::from_millis(150));
        let c = spawn(3.0); // newer than B
        std::thread::sleep(Duration::from_millis(150));
        tx.send(()).unwrap();
        assert!(a.join().unwrap().unwrap().is_some());
        assert_eq!(b.join().unwrap(), Err(GeoError::Superseded));
        assert!(c.join().unwrap().unwrap().is_some());
        assert_eq!(g.nominatim.load(SeqCst), 2); // A and C only
    }

    #[test]
    fn background_lookups_neither_supersede_nor_are_superseded() {
        use std::sync::atomic::Ordering::SeqCst;
        let (tx, rx) = std::sync::mpsc::channel();
        let g = Arc::new(Gated { first: true.into(), release: Mutex::new(rx), nominatim: 0.into() });
        let s = Arc::new(PlaceService::new(PlaceCache::open_in_memory().unwrap(), g.clone(), Duration::ZERO));
        let spawn = |lat: f64, fg: bool| {
            let s = s.clone();
            std::thread::spawn(move || if fg { s.place_for(lat, lat, "en") } else { s.place_for_background(lat, lat, "en") })
        };
        let a = spawn(1.0, false); // blocks inside the first request
        std::thread::sleep(Duration::from_millis(150));
        let b = spawn(2.0, true); // the user is looking at this one, queued behind A
        std::thread::sleep(Duration::from_millis(150));
        let c = spawn(3.0, false); // more background work arrives after B
        std::thread::sleep(Duration::from_millis(150));
        let d = spawn(4.0, false);
        std::thread::sleep(Duration::from_millis(150));
        tx.send(()).unwrap();
        assert!(a.join().unwrap().unwrap().is_some());
        assert!(b.join().unwrap().unwrap().is_some(), "background work must not cancel what the user is looking at");
        assert!(c.join().unwrap().unwrap().is_some());
        assert!(d.join().unwrap().unwrap().is_some());
        assert_eq!(g.nominatim.load(SeqCst), 4);
    }

    #[test]
    fn a_background_lookup_waiting_is_not_cancelled_by_a_newer_foreground_one() {
        use std::sync::atomic::Ordering::SeqCst;
        let (tx, rx) = std::sync::mpsc::channel();
        let g = Arc::new(Gated { first: true.into(), release: Mutex::new(rx), nominatim: 0.into() });
        let s = Arc::new(PlaceService::new(PlaceCache::open_in_memory().unwrap(), g.clone(), Duration::ZERO));
        let s1 = s.clone();
        let a = std::thread::spawn(move || s1.place_for(1.0, 1.0, "en")); // blocks inside the first request
        std::thread::sleep(Duration::from_millis(150));
        let s2 = s.clone();
        let b = std::thread::spawn(move || s2.place_for_background(2.0, 2.0, "en")); // queued behind A
        std::thread::sleep(Duration::from_millis(150));
        let s3 = s.clone();
        let c = std::thread::spawn(move || s3.place_for(3.0, 3.0, "en")); // the user moves on
        std::thread::sleep(Duration::from_millis(150));
        tx.send(()).unwrap();
        assert!(a.join().unwrap().unwrap().is_some());
        assert!(b.join().unwrap().unwrap().is_some());
        assert!(c.join().unwrap().unwrap().is_some());
        assert_eq!(g.nominatim.load(SeqCst), 3);
    }

    /// Hits the real services. Run by hand: `cargo test -p pixie-geo live -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn live_landmarks() {
        let s = PlaceService::new(
            PlaceCache::open_in_memory().unwrap(),
            Arc::new(crate::fetch::HttpFetcher::new()),
            Duration::from_millis(1100),
        );
        for (name, lat, lon) in [
            ("Colosseum", 41.89021, 12.49223),
            ("Bellagio", 36.11265, -115.17669),
            ("Louvre", 48.86065, 2.33764),
            ("Eiffel Tower", 48.8584, 2.2945),
            ("Mid-ocean", 0.0, -30.0),
        ] {
            println!("{name}: {:?}", s.place_for(lat, lon, "en"));
        }
    }

    #[test]
    fn out_of_range_coordinates_never_hit_the_network() {
        let f = fake(ROME, WIKI_COLOSSEUM);
        let s = service(&f, 0);
        for (la, lo) in [(91.0, 0.0), (0.0, 181.0), (f64::NAN, 0.0), (0.0, f64::INFINITY)] {
            assert_eq!(s.place_for(la, lo, "en").unwrap(), None);
        }
        assert_eq!(f.count(), 0);
    }
}
