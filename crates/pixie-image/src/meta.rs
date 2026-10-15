use pixie_core::PhotoMeta;
use std::path::Path;

fn is_heif(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(),
        Some("heic") | Some("heif")
    )
}

fn read_exif(path: &Path) -> Option<exif::Exif> {
    let f = std::fs::File::open(path).ok()?;
    exif::Reader::new()
        .read_from_container(&mut std::io::BufReader::new(f))
        .ok()
}

fn ascii(e: &exif::Exif, tag: exif::Tag) -> Option<String> {
    match &e.get_field(tag, exif::In::PRIMARY)?.value {
        exif::Value::Ascii(v) => v
            .first()
            .map(|b| String::from_utf8_lossy(b).trim_matches('\0').trim().to_string())
            .filter(|s| !s.is_empty()),
        _ => None,
    }
}

/// EXIF orientation 1..=8; 1 when absent or unreadable.
pub fn read_orientation(path: &Path) -> u16 {
    read_exif(path)
        .and_then(|e| {
            e.get_field(exif::Tag::Orientation, exif::In::PRIMARY)
                .and_then(|f| f.value.get_uint(0))
        })
        .map(|v| v as u16)
        .filter(|v| (1..=8).contains(v))
        .unwrap_or(1)
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// "YYYY:MM:DD HH:MM:SS" -> Unix seconds (treated as UTC). None if invalid.
pub fn parse_exif_datetime(s: &str) -> Option<i64> {
    let b = s.trim();
    if b.len() < 19 {
        return None;
    }
    let n = |r: std::ops::Range<usize>| b.get(r)?.parse::<i64>().ok();
    let (y, mo, d) = (n(0..4)?, n(5..7)?, n(8..10)?);
    let (h, mi, se) = (n(11..13)?, n(14..16)?, n(17..19)?);
    if y < 1 || !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || se > 60 {
        return None;
    }
    Some(days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + se)
}

fn gps_coord(e: &exif::Exif, tag: exif::Tag, ref_tag: exif::Tag) -> Option<f64> {
    let f = e.get_field(tag, exif::In::PRIMARY)?;
    let exif::Value::Rational(r) = &f.value else { return None };
    if r.len() < 3 {
        return None;
    }
    let v = r[0].to_f64() + r[1].to_f64() / 60.0 + r[2].to_f64() / 3600.0;
    let sign = match ascii(e, ref_tag).as_deref() {
        Some("S") | Some("W") => -1.0,
        _ => 1.0,
    };
    Some(v * sign).filter(|v| v.is_finite())
}

/// Best-effort metadata; never fails. Dimensions are as displayed (after orientation).
pub fn read_meta(path: &Path) -> PhotoMeta {
    let mut meta = PhotoMeta::default();
    let exif = read_exif(path);

    if is_heif(path) {
        // libheif applies rotation itself; ignore the EXIF orientation tag here.
        #[cfg(feature = "heic")]
        if let Some((w, h)) = crate::heic::dimensions(path) {
            meta.width = Some(w);
            meta.height = Some(h);
        }
    } else if let Ok((w, h)) = image::image_dimensions(path) {
        let o = read_orientation(path);
        let (w, h) = if o >= 5 { (h, w) } else { (w, h) };
        meta.width = Some(w);
        meta.height = Some(h);
    }

    if let Some(e) = &exif {
        meta.taken_at = ascii(e, exif::Tag::DateTimeOriginal)
            .or_else(|| ascii(e, exif::Tag::DateTime))
            .and_then(|s| parse_exif_datetime(&s));
        meta.camera = ascii(e, exif::Tag::Model);
        meta.gps_lat = gps_coord(e, exif::Tag::GPSLatitude, exif::Tag::GPSLatitudeRef);
        meta.gps_lon = gps_coord(e, exif::Tag::GPSLongitude, exif::Tag::GPSLongitudeRef);
    }
    meta
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixie_testutil::{jpeg, jpeg_with_exif};

    #[test]
    fn parses_exif_datetime_as_utc_seconds() {
        assert_eq!(parse_exif_datetime("1970:01:01 00:00:00"), Some(0));
        assert_eq!(parse_exif_datetime("2024:06:01 12:00:00"), Some(1_717_243_200));
        assert_eq!(parse_exif_datetime("0000:00:00 00:00:00"), None);
        assert_eq!(parse_exif_datetime("garbage"), None);
    }

    #[test]
    fn reads_orientation_and_defaults_to_1() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.jpg");
        std::fs::write(&a, jpeg_with_exif(40, 20, 6, None)).unwrap();
        assert_eq!(read_orientation(&a), 6);
        let b = dir.path().join("b.jpg");
        std::fs::write(&b, jpeg(40, 20)).unwrap();
        assert_eq!(read_orientation(&b), 1);
        assert_eq!(read_orientation(&dir.path().join("missing.jpg")), 1);
    }

    #[test]
    fn meta_reports_displayed_dimensions_and_date() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("r.jpg");
        std::fs::write(&p, jpeg_with_exif(40, 20, 6, Some("2024:06:01 12:00:00"))).unwrap();
        let m = read_meta(&p);
        assert_eq!((m.width, m.height), (Some(20), Some(40))); // swapped for orientation 6
        assert_eq!(m.taken_at, Some(1_717_243_200));
    }

    #[test]
    fn meta_without_exif_has_no_date_and_never_panics() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("plain.jpg");
        std::fs::write(&p, jpeg(40, 20)).unwrap();
        let m = read_meta(&p);
        assert_eq!((m.width, m.height, m.taken_at), (Some(40), Some(20), None));

        let bad = dir.path().join("bad.jpg");
        std::fs::write(&bad, b"not a jpeg").unwrap();
        assert_eq!(read_meta(&bad), PhotoMeta::default());
        let empty = dir.path().join("empty.jpg");
        std::fs::write(&empty, b"").unwrap();
        assert_eq!(read_meta(&empty), PhotoMeta::default());
    }
}
