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

/// JPEG width/height by reading only the header. (The `image` crate's JPEG
/// decoder reads the whole file into memory to answer the same question.)
fn jpeg_dimensions_from<R: std::io::Read>(reader: R) -> Option<(u32, u32)> {
    let mut dec = jpeg_decoder::Decoder::new(std::io::BufReader::new(reader));
    dec.read_info().ok()?;
    let info = dec.info()?;
    Some((info.width as u32, info.height as u32))
}

fn is_jpeg(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(),
        Some("jpg") | Some("jpeg")
    )
}

fn raw_dimensions(path: &Path) -> Option<(u32, u32)> {
    if is_jpeg(path) {
        if let Some(d) = std::fs::File::open(path).ok().and_then(jpeg_dimensions_from) {
            return Some(d);
        }
    }
    image::image_dimensions(path).ok()
}

/// Both coordinates present, finite and in range, and not the (0, 0) "no fix" value
/// many cameras write; otherwise neither is kept.
fn sanitize_gps(lat: Option<f64>, lon: Option<f64>) -> (Option<f64>, Option<f64>) {
    match (lat, lon) {
        (Some(a), Some(o))
            if a.is_finite() && o.is_finite() && a.abs() <= 90.0 && o.abs() <= 180.0 && !(a == 0.0 && o == 0.0) =>
        {
            (Some(a), Some(o))
        }
        _ => (None, None),
    }
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
    } else if let Some((w, h)) = raw_dimensions(path) {
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
        (meta.gps_lat, meta.gps_lon) = sanitize_gps(
            gps_coord(e, exif::Tag::GPSLatitude, exif::Tag::GPSLatitudeRef),
            gps_coord(e, exif::Tag::GPSLongitude, exif::Tag::GPSLongitudeRef),
        );
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

    /// Counts bytes handed out, to prove only the header is read.
    struct Counting<R>(R, std::rc::Rc<std::cell::Cell<usize>>);
    impl<R: std::io::Read> std::io::Read for Counting<R> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let n = self.0.read(buf)?;
            self.1.set(self.1.get() + n);
            Ok(n)
        }
    }

    #[test]
    fn jpeg_dimensions_read_only_the_header_not_the_whole_file() {
        let mut data = jpeg(3000, 2000);
        let image_len = data.len();
        data.extend(std::iter::repeat(0u8).take(5_000_000)); // a "large file": junk after the image
        let read = std::rc::Rc::new(std::cell::Cell::new(0));
        let dims = jpeg_dimensions_from(Counting(std::io::Cursor::new(data), read.clone()));
        assert_eq!(dims, Some((3000, 2000)));
        assert!(read.get() < 64 * 1024, "read {} bytes (image is {image_len}); must only read the header", read.get());
    }

    #[test]
    fn jpeg_dimensions_is_none_for_junk() {
        assert_eq!(jpeg_dimensions_from(std::io::Cursor::new(b"not a jpeg".to_vec())), None);
    }

    #[test]
    fn gps_must_be_a_complete_in_range_non_zero_fix() {
        assert_eq!(sanitize_gps(Some(51.5), Some(-0.12)), (Some(51.5), Some(-0.12)));
        assert_eq!(sanitize_gps(Some(-90.0), Some(180.0)), (Some(-90.0), Some(180.0)));
        assert_eq!(sanitize_gps(Some(0.0), Some(0.0)), (None, None)); // camera "no fix"
        assert_eq!(sanitize_gps(Some(0.0), Some(10.0)), (Some(0.0), Some(10.0))); // the equator is real
        assert_eq!(sanitize_gps(Some(90.5), Some(10.0)), (None, None));
        assert_eq!(sanitize_gps(Some(10.0), Some(-180.5)), (None, None));
        assert_eq!(sanitize_gps(Some(10.0), None), (None, None));
        assert_eq!(sanitize_gps(None, Some(10.0)), (None, None));
        assert_eq!(sanitize_gps(Some(f64::NAN), Some(10.0)), (None, None));
    }

}
