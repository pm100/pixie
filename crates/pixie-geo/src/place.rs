use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Place {
    pub landmark: Option<String>,
    pub locality: Option<String>,
}

/// A Wikipedia article this close to the photo counts as "the place" (metres).
pub const LANDMARK_RADIUS_M: f64 = 150.0;

const TOWN_KEYS: &[&str] = &["city", "town", "village", "municipality", "hamlet", "suburb"];

fn text<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key)?.as_str().map(str::trim).filter(|s| !s.is_empty())
}

/// "Town, region, country" from a Nominatim `format=jsonv2` reverse-lookup body.
/// `None` for an error body, garbage, or no usable address.
pub fn parse_locality(json: &str) -> Option<String> {
    let v: Value = serde_json::from_str(json).ok()?;
    if v.get("error").is_some() {
        return None;
    }
    let a = v.get("address")?;
    let town = TOWN_KEYS.iter().find_map(|k| text(a, k));
    let mut parts: Vec<&str> = [town, text(a, "state"), text(a, "country")].into_iter().flatten().collect();
    parts.dedup();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(", "))
    }
}

/// The nearest Wikipedia article within `LANDMARK_RADIUS_M` from a MediaWiki
/// `list=geosearch` (`formatversion=2`) body. A place with an encyclopedia
/// article is, in practice, a well-known one.
pub fn parse_landmark(json: &str) -> Option<String> {
    let v: Value = serde_json::from_str(json).ok()?;
    let hits = v.get("query")?.get("geosearch")?.as_array()?;
    hits.iter()
        .filter_map(|h| Some((h.get("dist")?.as_f64()?, text(h, "title")?)))
        .filter(|(d, _)| *d <= LANDMARK_RADIUS_M)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, t)| strip_disambiguator(t).to_string())
}

/// "Bellagio (resort)" -> "Bellagio". Leaves titles that are only a parenthetical alone.
fn strip_disambiguator(title: &str) -> &str {
    match title.strip_suffix(')').and_then(|t| t.rfind(" (").map(|i| &t[..i])) {
        Some(base) if !base.trim().is_empty() => base.trim(),
        _ => title,
    }
}

/// Cache key: coordinates rounded to 4 decimals (about 11 m).
pub fn place_key(lat: f64, lon: f64) -> String {
    let f = |v: f64| {
        let s = format!("{v:.4}");
        if s == "-0.0000" {
            "0.0000".to_string()
        } else {
            s
        }
    };
    format!("{},{}", f(lat), f(lon))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROME: &str = r#"{"category":"highway","type":"pedestrian","name":"",
        "address":{"quarter":"Celio","suburb":"Municipio Roma I","city":"Roma","state":"Lazio","country":"Italia"}}"#;
    const PARIS: &str = r#"{"category":"highway","type":"residential","name":"Rue de Rivoli",
        "address":{"road":"Rue de Rivoli","city":"Paris","state":"Île-de-France","country":"France"}}"#;
    const ERROR: &str = r#"{"error":"Unable to geocode"}"#;

    const WIKI: &str = r#"{"batchcomplete":true,"query":{"geosearch":[
        {"pageid":1,"ns":0,"title":"Colossus of Nero","lat":41.0,"lon":12.0,"dist":122.4,"primary":""},
        {"pageid":2,"ns":0,"title":"Colosseum","lat":41.0,"lon":12.0,"dist":10.2,"primary":""}]}}"#;

    #[test]
    fn locality_is_town_region_country() {
        assert_eq!(parse_locality(ROME).as_deref(), Some("Roma, Lazio, Italia"));
        assert_eq!(parse_locality(PARIS).as_deref(), Some("Paris, Île-de-France, France"));
    }

    #[test]
    fn locality_falls_back_through_town_keys() {
        let j = r#"{"address":{"village":"Bray","country":"Ireland"}}"#;
        assert_eq!(parse_locality(j).as_deref(), Some("Bray, Ireland"));
    }

    #[test]
    fn repeated_locality_parts_are_collapsed() {
        let j = r#"{"address":{"city":"Singapore","country":"Singapore"}}"#;
        assert_eq!(parse_locality(j).as_deref(), Some("Singapore"));
    }

    #[test]
    fn locality_error_garbage_and_empty_give_none() {
        assert_eq!(parse_locality(ERROR), None);
        assert_eq!(parse_locality("not json"), None);
        assert_eq!(parse_locality("{}"), None);
        assert_eq!(parse_locality(r#"{"address":{"road":"X"}}"#), None);
    }

    #[test]
    fn landmark_is_the_nearest_article_whatever_the_order() {
        assert_eq!(parse_landmark(WIKI).as_deref(), Some("Colosseum"));
    }

    #[test]
    fn landmark_drops_the_wikipedia_disambiguator() {
        let j = r#"{"query":{"geosearch":[{"title":"Bellagio (resort)","dist":53.0}]}}"#;
        assert_eq!(parse_landmark(j).as_deref(), Some("Bellagio"));
        let j = r#"{"query":{"geosearch":[{"title":"O (Cirque du Soleil)","dist":53.0}]}}"#;
        assert_eq!(parse_landmark(j).as_deref(), Some("O"));
        let j = r#"{"query":{"geosearch":[{"title":"(Untitled)","dist":5.0}]}}"#;
        assert_eq!(parse_landmark(j).as_deref(), Some("(Untitled)"));
    }

    #[test]
    fn landmark_ignores_articles_too_far_away() {
        let j = r#"{"query":{"geosearch":[{"title":"Far Away Castle","dist":151.0}]}}"#;
        assert_eq!(parse_landmark(j), None);
        let j = r#"{"query":{"geosearch":[{"title":"Near Castle","dist":149.9}]}}"#;
        assert_eq!(parse_landmark(j).as_deref(), Some("Near Castle"));
    }

    #[test]
    fn landmark_none_for_empty_garbage_and_malformed_hits() {
        assert_eq!(parse_landmark(r#"{"query":{"geosearch":[]}}"#), None);
        assert_eq!(parse_landmark("not json"), None);
        assert_eq!(parse_landmark("{}"), None);
        assert_eq!(parse_landmark(r#"{"query":{"geosearch":[{"title":"","dist":1.0},{"dist":2.0},{"title":"No dist"}]}}"#), None);
    }

    #[test]
    fn key_rounds_to_four_decimals_and_has_no_negative_zero() {
        assert_eq!(place_key(41.890212, 12.492231), "41.8902,12.4922");
        assert_eq!(place_key(41.89021, 12.49223), place_key(41.890199, 12.492249));
        assert_eq!(place_key(-0.00001, 0.0), "0.0000,0.0000");
        assert_eq!(place_key(-33.8568, 151.2153), "-33.8568,151.2153");
    }
}
