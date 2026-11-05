//! Saving a frame of a video as a photo: the file's name, a small EXIF block that gives it the video's date and place (so it sorts
//! next to the video and shows on the map), and a file that never replaces another.
use std::io::Write;
use std::path::{Path, PathBuf};

/// The most bytes a saved frame may have (a full-size JPEG of a 4K frame is a few MB; this only guards against a runaway page).
pub const MAX_FRAME_BYTES: usize = 40 * 1024 * 1024;

/// What a frame takes from its video.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FrameSource {
    /// The video's date (Unix seconds, as Pixie stores it), if it has one.
    pub taken_at: Option<i64>,
    pub gps: Option<(f64, f64)>,
    /// The camera the video names, if any.
    pub camera: Option<String>,
}

/// The name (without extension) of a frame `ms` milliseconds into the video called `video_stem`: `Beach-0m32.480s`, and
/// `Beach-1h02m32s` from an hour in (milliseconds are left off when there are none).
pub fn frame_stem(video_stem: &str, ms: u64) -> String {
    let (total, milli) = (ms / 1000, ms % 1000);
    let (h, m, s) = (total / 3600, (total / 60) % 60, total % 60);
    let frac = if milli == 0 { String::new() } else { format!(".{milli:03}") };
    if h > 0 {
        format!("{video_stem}-{h}h{m:02}m{s:02}{frac}s")
    } else {
        format!("{video_stem}-{m}m{s}{frac}s")
    }
}

/// Check the bytes from the page are a JPEG of a sensible size before they are written anywhere.
pub fn validate_frame(bytes: &[u8]) -> Result<(), &'static str> {
    if bytes.is_empty() || bytes.len() > MAX_FRAME_BYTES {
        return Err("the frame has an unreasonable size");
    }
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return Err("the frame is not a JPEG");
    }
    let (w, h) = image::ImageReader::with_format(std::io::Cursor::new(bytes), image::ImageFormat::Jpeg)
        .into_dimensions()
        .map_err(|_| "the frame is not a readable JPEG")?;
    if w == 0 || h == 0 || w > 16_384 || h > 16_384 {
        return Err("the frame's dimensions are out of range");
    }
    Ok(())
}

// ---- EXIF

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// "YYYY:MM:DD HH:MM:SS" for Unix seconds, read as UTC (the way Pixie reads an EXIF date back).
pub fn exif_datetime(secs: i64) -> String {
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}:{m:02}:{d:02} {:02}:{:02}:{:02}", rem / 3600, (rem / 60) % 60, rem % 60)
}

const ASCII: u16 = 2;
const LONG: u16 = 4;
const RATIONAL: u16 = 5;
const BYTE: u16 = 1;

struct Entry {
    tag: u16,
    kind: u16,
    count: u32,
    /// Little-endian bytes of the value; one of 4 bytes or fewer is kept in the entry itself.
    data: Vec<u8>,
}

fn ascii(tag: u16, s: &str) -> Entry {
    let mut data = s.as_bytes().to_vec();
    data.push(0);
    Entry { tag, kind: ASCII, count: data.len() as u32, data }
}

fn long(tag: u16, v: u32) -> Entry {
    Entry { tag, kind: LONG, count: 1, data: v.to_le_bytes().to_vec() }
}

/// Degrees, minutes and seconds of a non-negative coordinate, as three rationals.
fn dms(v: f64) -> Entry {
    let v = v.abs();
    let deg = v.floor();
    let min = ((v - deg) * 60.0).floor();
    let sec = ((v - deg) * 60.0 - min) * 60.0;
    let mut data = Vec::new();
    for (n, d) in [(deg as u32, 1u32), (min as u32, 1), ((sec * 10_000.0).round() as u32, 10_000)] {
        data.extend_from_slice(&n.to_le_bytes());
        data.extend_from_slice(&d.to_le_bytes());
    }
    Entry { tag: 0, kind: RATIONAL, count: 3, data }
}

/// The bytes of one IFD placed at `start` in the TIFF block: its entries (sorted by tag), then the values that do not fit in an entry.
fn ifd_bytes(mut entries: Vec<Entry>, start: u32) -> Vec<u8> {
    entries.sort_by_key(|e| e.tag);
    let table = 2 + 12 * entries.len() as u32 + 4;
    let mut out = Vec::new();
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    let mut extra: Vec<u8> = Vec::new();
    for e in &entries {
        out.extend_from_slice(&e.tag.to_le_bytes());
        out.extend_from_slice(&e.kind.to_le_bytes());
        out.extend_from_slice(&e.count.to_le_bytes());
        if e.data.len() <= 4 {
            let mut v = e.data.clone();
            v.resize(4, 0);
            out.extend_from_slice(&v);
        } else {
            out.extend_from_slice(&(start + table + extra.len() as u32).to_le_bytes());
            extra.extend_from_slice(&e.data);
            if extra.len() % 2 == 1 {
                extra.push(0);
            }
        }
    }
    out.extend_from_slice(&0u32.to_le_bytes()); // no next IFD
    out.extend_from_slice(&extra);
    out
}

fn ifd_len(entries: &[Entry]) -> u32 {
    2 + 12 * entries.len() as u32 + 4 + entries.iter().filter(|e| e.data.len() > 4).map(|e| (e.data.len() as u32 + 1) & !1).sum::<u32>()
}

/// The TIFF block of an EXIF segment holding the frame's date (with the milliseconds), place and camera. None when there is nothing to say.
pub fn exif_tiff(source: &FrameSource, ms_in: u64) -> Option<Vec<u8>> {
    let mut exif_ifd: Vec<Entry> = Vec::new();
    if let Some(t) = source.taken_at {
        let at = t + (ms_in / 1000) as i64;
        exif_ifd.push(ascii(0x9003, &exif_datetime(at))); // DateTimeOriginal
        exif_ifd.push(ascii(0x9004, &exif_datetime(at))); // DateTimeDigitized
        exif_ifd.push(ascii(0x9291, &format!("{:03}", ms_in % 1000))); // SubSecTimeOriginal
    }
    let mut gps_ifd: Vec<Entry> = Vec::new();
    if let Some((lat, lon)) = source.gps {
        gps_ifd.push(Entry { tag: 0x0000, kind: BYTE, count: 4, data: vec![2, 2, 0, 0] });
        gps_ifd.push(ascii(0x0001, if lat < 0.0 { "S" } else { "N" }));
        gps_ifd.push(Entry { tag: 0x0002, ..dms(lat) });
        gps_ifd.push(ascii(0x0003, if lon < 0.0 { "W" } else { "E" }));
        gps_ifd.push(Entry { tag: 0x0004, ..dms(lon) });
    }
    let camera = source.camera.as_deref().filter(|c| !c.is_empty());
    if exif_ifd.is_empty() && gps_ifd.is_empty() && camera.is_none() {
        return None;
    }

    // Where each IFD lands: IFD0 right after the 8-byte header, then the Exif IFD, then the GPS IFD.
    let mut ifd0: Vec<Entry> = Vec::new();
    if let Some(c) = camera {
        ifd0.push(ascii(0x0110, c)); // Model
    }
    let n0 = ifd0.len() + usize::from(!exif_ifd.is_empty()) + usize::from(!gps_ifd.is_empty());
    let placeholder = |tag| long(tag, 0);
    let mut sizing: Vec<Entry> = ifd0.iter().map(|e| Entry { tag: e.tag, kind: e.kind, count: e.count, data: e.data.clone() }).collect();
    while sizing.len() < n0 {
        sizing.push(placeholder(0));
    }
    let exif_at = 8 + ifd_len(&sizing);
    let gps_at = exif_at + if exif_ifd.is_empty() { 0 } else { ifd_len(&exif_ifd) };
    if !exif_ifd.is_empty() {
        ifd0.push(long(0x8769, exif_at));
    }
    if !gps_ifd.is_empty() {
        ifd0.push(long(0x8825, gps_at));
    }

    let mut tiff = vec![b'I', b'I', 42, 0, 8, 0, 0, 0];
    tiff.extend_from_slice(&ifd_bytes(ifd0, 8));
    if !exif_ifd.is_empty() {
        tiff.extend_from_slice(&ifd_bytes(exif_ifd, exif_at));
    }
    if !gps_ifd.is_empty() {
        tiff.extend_from_slice(&ifd_bytes(gps_ifd, gps_at));
    }
    Some(tiff)
}

/// `jpeg` with an EXIF segment for the frame put in (after the JFIF header if there is one). A JPEG with nothing to add is returned as it is.
pub fn with_exif(jpeg: &[u8], source: &FrameSource, ms_in: u64) -> Vec<u8> {
    let Some(tiff) = exif_tiff(source, ms_in) else { return jpeg.to_vec() };
    let mut at = 2;
    if jpeg.len() >= 6 && jpeg[2] == 0xFF && jpeg[3] == 0xE0 {
        at = 4 + u16::from_be_bytes([jpeg[4], jpeg[5]]) as usize;
    }
    let at = at.min(jpeg.len());
    let mut out = Vec::with_capacity(jpeg.len() + tiff.len() + 10);
    out.extend_from_slice(&jpeg[..at]);
    out.extend_from_slice(&[0xFF, 0xE1]);
    out.extend_from_slice(&((2 + 6 + tiff.len()) as u16).to_be_bytes());
    out.extend_from_slice(b"Exif\0\0");
    out.extend_from_slice(&tiff);
    out.extend_from_slice(&jpeg[at..]);
    out
}

// ---- writing

/// Write `bytes` as `dir/stem.ext`, or `dir/stem (2).ext`, `stem (3).ext`... when that name is taken: no file is ever replaced.
pub fn save_unique(dir: &Path, stem: &str, ext: &str, bytes: &[u8]) -> std::io::Result<PathBuf> {
    for n in 1..1000 {
        let name = if n == 1 { format!("{stem}.{ext}") } else { format!("{stem} ({n}).{ext}") };
        let path = dir.join(name);
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut f) => {
                if let Err(e) = f.write_all(bytes).and_then(|_| f.flush()) {
                    drop(f);
                    let _ = std::fs::remove_file(&path);
                    return Err(e);
                }
                return Ok(path);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "too many frames with this name"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::read_meta_bytes;

    #[test]
    fn a_frame_is_named_after_the_video_and_the_time_in_it() {
        assert_eq!(frame_stem("Beach", 32_000), "Beach-0m32s");
        assert_eq!(frame_stem("Beach", 32_480), "Beach-0m32.480s");
        assert_eq!(frame_stem("Beach", 5), "Beach-0m0.005s");
        assert_eq!(frame_stem("Beach", 62_000), "Beach-1m2s");
        assert_eq!(frame_stem("Beach", 3_752_000), "Beach-1h02m32s");
        assert_eq!(frame_stem("Beach", 3_752_480), "Beach-1h02m32.480s");
    }

    #[test]
    fn the_date_is_written_the_way_exif_dates_are() {
        assert_eq!(exif_datetime(0), "1970:01:01 00:00:00");
        assert_eq!(exif_datetime(1_717_243_200), "2024:06:01 12:00:00");
        assert_eq!(exif_datetime(1_717_243_200 + 43_199), "2024:06:01 23:59:59");
    }

    #[test]
    fn pixie_reads_back_the_date_place_and_camera_the_frame_was_given() {
        let source = FrameSource { taken_at: Some(1_717_243_200), gps: Some((-33.8688, 151.2093)), camera: Some("Pixel 8".into()) };
        let jpeg = with_exif(&pixie_testutil::jpeg(64, 48), &source, 32_480);
        validate_frame(&jpeg).unwrap();
        let m = read_meta_bytes(Path::new("frame.jpg"), &jpeg);
        assert_eq!(m.taken_at, Some(1_717_243_200 + 32), "the video's date plus the time into it");
        let (lat, lon) = (m.gps_lat.unwrap(), m.gps_lon.unwrap());
        assert!((lat + 33.8688).abs() < 1e-4 && (lon - 151.2093).abs() < 1e-4, "{lat} {lon}");
        assert_eq!(m.camera.as_deref(), Some("Pixel 8"));
        assert_eq!((m.width, m.height), (Some(64), Some(48)));
    }

    #[test]
    fn only_what_the_video_has_is_written() {
        let date_only = FrameSource { taken_at: Some(1_717_243_200), ..Default::default() };
        let m = read_meta_bytes(Path::new("f.jpg"), &with_exif(&pixie_testutil::jpeg(8, 8), &date_only, 0));
        assert_eq!((m.taken_at, m.gps_lat, m.camera), (Some(1_717_243_200), None, None));
        let none = pixie_testutil::jpeg(8, 8);
        assert_eq!(with_exif(&none, &FrameSource::default(), 1_000), none, "nothing to say: the JPEG is left as it is");
    }

    #[test]
    fn a_frame_is_put_after_the_jfif_header_when_there_is_one() {
        let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
        jpeg.extend_from_slice(b"JFIF\0\x01\x01\0\0\x01\0\x01\0\0");
        jpeg.extend_from_slice(&[0xFF, 0xD9]);
        let out = with_exif(&jpeg, &FrameSource { camera: Some("X".into()), ..Default::default() }, 0);
        assert_eq!(&out[..20], &jpeg[..20]);
        assert_eq!(&out[20..22], &[0xFF, 0xE1]);
        assert_eq!(&out[out.len() - 2..], &[0xFF, 0xD9]);
    }

    #[test]
    fn bytes_that_are_not_a_sensible_jpeg_are_refused() {
        assert!(validate_frame(&pixie_testutil::jpeg(1920, 1080)).is_ok());
        assert!(validate_frame(&[]).is_err());
        assert!(validate_frame(b"not a jpeg").is_err());
        assert!(validate_frame(&vec![0xFF; MAX_FRAME_BYTES + 1]).is_err());
    }

    #[test]
    fn a_name_that_is_taken_gets_a_number_and_nothing_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let first = save_unique(dir.path(), "Beach-0m32s", "jpg", b"one").unwrap();
        let second = save_unique(dir.path(), "Beach-0m32s", "jpg", b"two").unwrap();
        let third = save_unique(dir.path(), "Beach-0m32s", "jpg", b"three").unwrap();
        assert_eq!(first.file_name().unwrap(), "Beach-0m32s.jpg");
        assert_eq!(second.file_name().unwrap(), "Beach-0m32s (2).jpg");
        assert_eq!(third.file_name().unwrap(), "Beach-0m32s (3).jpg");
        assert_eq!(std::fs::read(first).unwrap(), b"one");
        assert_eq!(std::fs::read(second).unwrap(), b"two");
    }
}
