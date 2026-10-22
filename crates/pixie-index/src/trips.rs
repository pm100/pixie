//! Trips: runs of days spent away from home.
//!
//! A day is *away* when any of its located items is further than the home radius from home, *home* when it has located
//! items and none are away, and *undecided* when none of its items has a position. A trip is a run of away days with no
//! home day between them; undecided days neither start nor break one. Days are UTC calendar days of the item's date,
//! the same as the "When" list uses.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

const DAY: i64 = 86_400;
/// Items dated before this (1990-01-01) are placeholders such as the 1980 dates some cameras write, not real dates.
pub const EARLIEST_REAL: i64 = 631_152_000;

/// One dated item: the UTC day number (days since 1970-01-01), where it was taken if known, and whether it is a video.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DayItem {
    pub day: i64,
    pub pos: Option<(f64, f64)>,
    pub video: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TripSettings {
    pub home_lat: f64,
    pub home_lon: f64,
    pub radius_km: f64,
    /// A trip needs at least this many nights away: the days from its first to its last away day. 0 allows a day trip.
    pub min_nights: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trip {
    /// First and last away day, "YYYY-MM-DD".
    pub start: String,
    pub end: String,
    /// The time range of the trip in unix seconds: `since` is inclusive, `until` exclusive (the start of the day after).
    pub since: i64,
    pub until: i64,
    /// Nights away: the days from the first to the last away day.
    pub nights: u32,
    /// Distinct away days (fewer than nights + 1 when some days have nothing taken on them).
    pub days: u32,
    pub photos: i64,
    pub videos: i64,
    /// The user's name for the trip, if they gave it one.
    pub name: Option<String>,
}

/// The UTC day number of a unix time.
pub fn day_of(t: i64) -> i64 {
    t.div_euclid(DAY)
}

/// "YYYY-MM-DD" of a day number.
pub fn day_string(day: i64) -> String {
    // Howard Hinnant's civil-from-days.
    let z = day + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{:02}-{:02}", if m <= 2 { y + 1 } else { y }, m, d)
}

/// Great-circle distance in kilometres.
pub fn distance_km(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (la1, lo1, la2, lo2) = (a.0.to_radians(), a.1.to_radians(), b.0.to_radians(), b.1.to_radians());
    let h = ((la2 - la1) / 2.0).sin().powi(2) + la1.cos() * la2.cos() * ((lo2 - lo1) / 2.0).sin().powi(2);
    2.0 * 6371.0 * h.sqrt().asin()
}

#[derive(Default)]
struct DayAgg {
    photos: i64,
    videos: i64,
    located: bool,
    away: bool,
}

/// The trips in `items`, newest first. Names are not set here.
pub fn compute_trips(items: &[DayItem], s: &TripSettings) -> Vec<Trip> {
    let mut days: BTreeMap<i64, DayAgg> = BTreeMap::new();
    for it in items {
        let d = days.entry(it.day).or_default();
        if it.video {
            d.videos += 1;
        } else {
            d.photos += 1;
        }
        if let Some(p) = it.pos {
            d.located = true;
            if distance_km((s.home_lat, s.home_lon), p) > s.radius_km {
                d.away = true;
            }
        }
    }
    let mut out = Vec::new();
    // The open trip: first away day, last away day, away days so far.
    let mut open: Option<(i64, i64, u32)> = None;
    let close = |open: &mut Option<(i64, i64, u32)>, out: &mut Vec<Trip>| {
        if let Some((first, last, n)) = open.take() {
            if (last - first) as u32 >= s.min_nights {
                let (mut photos, mut videos) = (0, 0);
                for (_, d) in days.range(first..=last) {
                    photos += d.photos;
                    videos += d.videos;
                }
                out.push(Trip { start: day_string(first), end: day_string(last), since: first * DAY, until: (last + 1) * DAY, nights: (last - first) as u32, days: n, photos, videos, name: None });
            }
        }
    };
    for (&day, d) in &days {
        if d.away {
            open = Some(match open {
                Some((first, _, n)) => (first, day, n + 1),
                None => (day, day, 1),
            });
        } else if d.located {
            close(&mut open, &mut out); // a day at home ends the trip
        }
    }
    close(&mut open, &mut out);
    out.reverse();
    out
}

/// A place the trip stopped at: the middle of the located items taken within the stop radius of each other.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stop {
    pub lat: f64,
    pub lon: f64,
    /// First and last day something was taken here, "YYYY-MM-DD".
    pub first: String,
    pub last: String,
    /// Distinct days with something taken here.
    pub days: u32,
    pub items: i64,
}

/// Where a trip went: its stops in the order first visited, and the order of visits (indexes into `stops`, a stop
/// appearing again when the trip came back to it).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Route {
    pub stops: Vec<Stop>,
    pub path: Vec<usize>,
}

struct StopAcc {
    lat: f64,
    lon: f64,
    n: i64,
    days: HashSet<i64>,
    first: i64,
    last: i64,
}

/// The stops of a run of located items, given as (time, latitude, longitude) in time order. An item joins the nearest
/// stop within `stop_km` of it, else starts a new one.
pub fn compute_route(items: &[(i64, f64, f64)], stop_km: f64) -> Route {
    let mut acc: Vec<StopAcc> = Vec::new();
    let mut path: Vec<usize> = Vec::new();
    for &(t, lat, lon) in items {
        let day = day_of(t);
        let near = acc
            .iter()
            .enumerate()
            .map(|(i, a)| (i, distance_km((a.lat / a.n as f64, a.lon / a.n as f64), (lat, lon))))
            .filter(|(_, d)| *d <= stop_km)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i);
        let i = match near {
            Some(i) => i,
            None => {
                acc.push(StopAcc { lat: 0.0, lon: 0.0, n: 0, days: HashSet::new(), first: day, last: day });
                acc.len() - 1
            }
        };
        let a = &mut acc[i];
        a.lat += lat;
        a.lon += lon;
        a.n += 1;
        a.days.insert(day);
        a.last = a.last.max(day);
        a.first = a.first.min(day);
        if path.last() != Some(&i) {
            path.push(i);
        }
    }
    let stops = acc
        .into_iter()
        .map(|a| Stop { lat: a.lat / a.n as f64, lon: a.lon / a.n as f64, first: day_string(a.first), last: day_string(a.last), days: a.days.len() as u32, items: a.n })
        .collect();
    Route { stops, path }
}

/// A guess at where home is: the middle of the 0.1° square (about 10 km) that has photos on the most different days.
pub fn infer_home(items: &[DayItem]) -> Option<(f64, f64)> {
    struct Cell {
        days: HashSet<i64>,
        n: usize,
        lat: f64,
        lon: f64,
    }
    let mut cells: HashMap<(i64, i64), Cell> = HashMap::new();
    for it in items {
        let Some((lat, lon)) = it.pos else { continue };
        let c = cells.entry(((lat * 10.0).floor() as i64, (lon * 10.0).floor() as i64)).or_insert(Cell { days: HashSet::new(), n: 0, lat: 0.0, lon: 0.0 });
        c.days.insert(it.day);
        c.n += 1;
        c.lat += lat;
        c.lon += lon;
    }
    // Ties go to the cell with more items, then to the lower cell, so the answer does not depend on hash order.
    let (_, best) = cells.into_iter().max_by(|a, b| (a.1.days.len(), a.1.n, (-a.0 .0, -a.0 .1)).cmp(&(b.1.days.len(), b.1.n, (-b.0 .0, -b.0 .1))))?;
    Some((best.lat / best.n as f64, best.lon / best.n as f64))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Home is Paris; Rome is about 1,100 km away.
    const PARIS: (f64, f64) = (48.8566, 2.3522);
    const NEAR_PARIS: (f64, f64) = (48.9, 2.4);
    const ROME: (f64, f64) = (41.9028, 12.4964);
    const LYON: (f64, f64) = (45.764, 4.8357); // about 390 km from Paris

    fn s() -> TripSettings {
        TripSettings { home_lat: PARIS.0, home_lon: PARIS.1, radius_km: 50.0, min_nights: 0 }
    }
    fn at(day: i64, pos: (f64, f64)) -> DayItem {
        DayItem { day, pos: Some(pos), video: false }
    }
    fn nogps(day: i64) -> DayItem {
        DayItem { day, pos: None, video: false }
    }

    #[test]
    fn day_strings_are_calendar_dates() {
        assert_eq!(day_string(0), "1970-01-01");
        assert_eq!(day_string(19_723), "2024-01-01");
        assert_eq!(day_string(19_782), "2024-02-29");
        assert_eq!(day_string(-1), "1969-12-31");
        assert_eq!(day_of(1_704_067_200), 19_723);
        assert_eq!(day_of(1_704_067_199), 19_722);
    }

    #[test]
    fn distance_is_great_circle_kilometres() {
        assert!((distance_km(PARIS, ROME) - 1_105.0).abs() < 15.0);
        assert!(distance_km(PARIS, PARIS) < 0.001);
    }

    #[test]
    fn a_run_of_away_days_is_one_trip_and_a_day_at_home_ends_it() {
        let items = [at(100, PARIS), at(101, ROME), at(102, ROME), at(103, LYON), at(104, NEAR_PARIS), at(105, ROME)];
        let t = compute_trips(&items, &s());
        assert_eq!(t.len(), 2);
        assert_eq!((t[0].start.as_str(), t[0].end.as_str()), (day_string(105).as_str(), day_string(105).as_str())); // newest first
        assert_eq!((t[1].start.as_str(), t[1].end.as_str(), t[1].days), (day_string(101).as_str(), day_string(103).as_str(), 3));
        assert_eq!((t[1].since, t[1].until), (101 * DAY, 104 * DAY));
    }

    #[test]
    fn a_day_with_home_and_away_items_counts_as_away() {
        let items = [at(1, ROME), at(2, PARIS), at(2, ROME), at(3, ROME)];
        let t = compute_trips(&items, &s());
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].days, 3);
    }

    #[test]
    fn items_without_a_position_never_start_or_break_a_trip() {
        let items = [nogps(1), nogps(2), at(3, ROME), nogps(4), at(5, ROME), nogps(6)];
        let t = compute_trips(&items, &s());
        assert_eq!(t.len(), 1);
        assert_eq!((t[0].start.as_str(), t[0].end.as_str()), (day_string(3).as_str(), day_string(5).as_str()));
        assert_eq!(t[0].days, 2, "only days with an away item are counted");
        assert_eq!(t[0].photos, 3, "the undated-position photo of day 4 is inside the trip; days 1, 2 and 6 are outside");
    }

    #[test]
    fn counts_split_photos_and_videos_and_include_every_item_of_the_range() {
        let items = [at(3, ROME), DayItem { day: 3, pos: Some(ROME), video: true }, DayItem { day: 4, pos: None, video: true }, at(5, ROME)];
        let t = compute_trips(&items, &s());
        assert_eq!((t[0].photos, t[0].videos), (2, 2));
    }

    #[test]
    fn the_minimum_counts_nights_from_the_first_to_the_last_away_day() {
        let items = [at(1, ROME), at(1, ROME), at(1, ROME), at(2, PARIS), at(3, ROME), at(4, ROME), at(5, ROME), at(6, PARIS)];
        let one = TripSettings { min_nights: 1, ..s() };
        let t = compute_trips(&items, &one);
        assert_eq!(t.len(), 1, "day 1 is a day trip, however many photos it has: no night away");
        assert_eq!((t[0].nights, t[0].days), (2, 3));
        let two = TripSettings { min_nights: 2, ..s() };
        assert_eq!(compute_trips(&items, &two).len(), 1);
        let three = TripSettings { min_nights: 3, ..s() };
        assert!(compute_trips(&items, &three).is_empty());
    }

    #[test]
    fn nights_include_days_with_nothing_taken_on_them() {
        let t = compute_trips(&[at(10, ROME), at(13, ROME)], &s());
        assert_eq!((t[0].nights, t[0].days), (3, 2));
    }

    #[test]
    fn a_minimum_of_zero_nights_allows_a_day_trip() {
        assert_eq!(compute_trips(&[at(1, ROME)], &s()).len(), 1);
        assert!(compute_trips(&[at(1, ROME)], &TripSettings { min_nights: 1, ..s() }).is_empty());
    }

    #[test]
    fn nothing_away_means_no_trips_and_so_does_no_data() {
        assert!(compute_trips(&[at(1, PARIS), at(2, NEAR_PARIS)], &s()).is_empty());
        assert!(compute_trips(&[], &s()).is_empty());
        assert!(compute_trips(&[nogps(1), nogps(2)], &s()).is_empty());
    }

    #[test]
    fn the_radius_decides_what_is_away() {
        let lyon_only = [at(1, LYON)];
        assert_eq!(compute_trips(&lyon_only, &s()).len(), 1);
        let wide = TripSettings { radius_km: 500.0, ..s() };
        assert!(compute_trips(&lyon_only, &wide).is_empty());
    }

    #[test]
    fn a_trip_still_open_at_the_end_of_the_data_is_kept() {
        let t = compute_trips(&[at(1, PARIS), at(2, ROME), at(3, ROME)], &s());
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].days, 2);
    }

    const FLORENCE: (f64, f64) = (43.7696, 11.2558); // about 230 km from Rome
    const NEAR_ROME: (f64, f64) = (41.95, 12.5); // about 6 km from Rome
    fn timed(day: i64, p: (f64, f64)) -> (i64, f64, f64) {
        (day * DAY + 3600, p.0, p.1)
    }

    #[test]
    fn nearby_items_are_one_stop_and_far_ones_start_another() {
        let r = compute_route(&[timed(1, ROME), timed(1, NEAR_ROME), timed(2, ROME), timed(3, FLORENCE)], 25.0);
        assert_eq!(r.stops.len(), 2);
        assert_eq!((r.stops[0].items, r.stops[0].days), (3, 2));
        assert_eq!((r.stops[0].first.as_str(), r.stops[0].last.as_str()), (day_string(1).as_str(), day_string(2).as_str()));
        assert_eq!((r.stops[1].items, r.stops[1].days), (1, 1));
        assert_eq!(r.path, vec![0, 1]);
    }

    #[test]
    fn a_stop_is_the_middle_of_its_items() {
        let r = compute_route(&[timed(1, ROME), timed(1, NEAR_ROME)], 25.0);
        assert!((r.stops[0].lat - (ROME.0 + NEAR_ROME.0) / 2.0).abs() < 1e-9);
    }

    #[test]
    fn the_stop_radius_decides_what_merges() {
        let items = [timed(1, ROME), timed(2, NEAR_ROME)];
        assert_eq!(compute_route(&items, 25.0).stops.len(), 1);
        assert_eq!(compute_route(&items, 2.0).stops.len(), 2);
    }

    #[test]
    fn coming_back_to_a_stop_repeats_it_in_the_path_but_not_in_the_stops() {
        let r = compute_route(&[timed(1, ROME), timed(1, ROME), timed(2, FLORENCE), timed(3, ROME), timed(4, ROME)], 25.0);
        assert_eq!(r.stops.len(), 2);
        assert_eq!(r.path, vec![0, 1, 0], "consecutive items at one stop make one visit");
        assert_eq!(r.stops[0].days, 3);
    }

    #[test]
    fn no_items_means_no_route() {
        let r = compute_route(&[], 25.0);
        assert!(r.stops.is_empty() && r.path.is_empty());
    }

    #[test]
    fn home_is_where_photos_were_taken_on_the_most_different_days() {
        // 40 photos of one rome day beat 3 paris days of 1 photo only if days were not what counts.
        let mut items: Vec<DayItem> = (0..40).map(|_| at(50, ROME)).collect();
        items.extend([at(1, PARIS), at(2, NEAR_PARIS), at(3, PARIS)]);
        let h = infer_home(&items).unwrap();
        assert!(distance_km(h, PARIS) < 15.0, "{h:?}");
    }

    #[test]
    fn home_cannot_be_guessed_without_positions() {
        assert!(infer_home(&[nogps(1)]).is_none());
        assert!(infer_home(&[]).is_none());
    }

    #[test]
    fn the_home_guess_does_not_depend_on_item_order() {
        let mut items = vec![at(1, PARIS), at(2, ROME)]; // a tie on days and items
        let a = infer_home(&items);
        items.reverse();
        assert_eq!(a, infer_home(&items));
    }
}
