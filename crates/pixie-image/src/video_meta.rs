use nom_exif::{read_track, TrackInfoTag};
use pixie_core::{MediaKind, PhotoMeta};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const MAX_MOOV: u64 = 64 * 1024 * 1024;

/// Picture size of a track and whether it is rotated a quarter turn (so the displayed
/// size swaps).
struct Tkhd {
    width: u32,
    height: u32,
    quarter_turn: bool,
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// Child boxes of `data`: (type, payload).
fn children(data: &[u8]) -> Vec<([u8; 4], &[u8])> {
    let mut out = Vec::new();
    let mut at = 0usize;
    while at + 8 <= data.len() {
        let size = be32(data, at).unwrap_or(0) as usize;
        let kind: [u8; 4] = data[at + 4..at + 8].try_into().unwrap();
        let (header, end) = match size {
            0 => (8, data.len()),
            1 => (
                16,
                data.get(at + 8..at + 16)
                    .map(|b| u64::from_be_bytes(b.try_into().unwrap()) as usize)
                    .map_or(data.len(), |s| at.saturating_add(s)),
            ),
            n => (8, at.saturating_add(n)),
        };
        if end > data.len() || end < at + header {
            break;
        }
        out.push((kind, &data[at + header..end]));
        at = end;
    }
    out
}

fn parse_tkhd(p: &[u8]) -> Option<Tkhd> {
    let v1 = *p.first()? == 1;
    let matrix_at = if v1 { 52 } else { 40 };
    let (a, b) = (be32(p, matrix_at)? as i32, be32(p, matrix_at + 4)? as i32);
    let (c, d) = (be32(p, matrix_at + 12)? as i32, be32(p, matrix_at + 16)? as i32);
    let size_at = matrix_at + 36;
    let width = be32(p, size_at)? >> 16;
    let height = be32(p, size_at + 4)? >> 16;
    Some(Tkhd { width, height, quarter_turn: a == 0 && d == 0 && b != 0 && c != 0 })
}

/// The contents of the `moov` box, wherever it is in the file. Box sizes come from the file,
/// so every offset is computed with checked arithmetic.
fn read_moov(path: &Path) -> Option<Vec<u8>> {
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    let mut pos = 0u64;
    while pos.checked_add(8)? <= len {
        f.seek(SeekFrom::Start(pos)).ok()?;
        let mut h = [0u8; 16];
        let got = f.read(&mut h).ok()?;
        if got < 8 {
            return None;
        }
        let size32 = u32::from_be_bytes(h[0..4].try_into().unwrap()) as u64;
        let (header, size) = match size32 {
            0 => (8, len - pos),
            1 if got >= 16 => (16, u64::from_be_bytes(h[8..16].try_into().unwrap())),
            n => (8, n),
        };
        let end = pos.checked_add(size)?;
        if size < header || end > len {
            return None;
        }
        if &h[4..8] == b"moov" {
            if size - header > MAX_MOOV {
                return None;
            }
            f.seek(SeekFrom::Start(pos + header)).ok()?;
            let mut buf = vec![0u8; (size - header) as usize];
            f.read_exact(&mut buf).ok()?;
            return Some(buf);
        }
        pos = end;
    }
    None
}

/// Seconds between 1904-01-01 (the MP4 epoch) and 1970-01-01.
const MP4_EPOCH_OFFSET: u64 = 2_082_844_800;

struct Mvhd {
    /// Unix seconds (UTC); `None` when unset.
    created: Option<i64>,
    duration_ms: Option<i64>,
}

/// Movie header, version 0 (32-bit times) or 1 (64-bit times).
fn parse_mvhd(p: &[u8]) -> Option<Mvhd> {
    let v1 = *p.first()? == 1;
    let (created, timescale, duration) = if v1 {
        (
            u64::from_be_bytes(p.get(4..12)?.try_into().ok()?),
            be32(p, 20)?,
            u64::from_be_bytes(p.get(24..32)?.try_into().ok()?),
        )
    } else {
        (be32(p, 4)? as u64, be32(p, 12)?, be32(p, 16)? as u64)
    };
    Some(Mvhd {
        created: created.checked_sub(MP4_EPOCH_OFFSET).filter(|t| *t > 0).and_then(|t| i64::try_from(t).ok()),
        duration_ms: (timescale > 0)
            .then(|| duration.saturating_mul(1000) / timescale as u64)
            .and_then(|v| i64::try_from(v).ok()),
    })
}

/// UTC seconds shifted to approximate local time using longitude (15 degrees per hour).
/// Photo dates are stored as local-as-UTC, so videos need the same convention to land
/// on the same day.
pub fn approx_local(utc_secs: i64, lon: Option<f64>) -> i64 {
    match lon {
        Some(l) if l.is_finite() && l.abs() <= 180.0 => utc_secs + (l / 15.0).round() as i64 * 3600,
        _ => utc_secs,
    }
}

fn sanitize(lat: Option<f64>, lon: Option<f64>) -> (Option<f64>, Option<f64>) {
    match (lat, lon) {
        (Some(a), Some(o))
            if a.is_finite() && o.is_finite() && a.abs() <= 90.0 && o.abs() <= 180.0 && !(a == 0.0 && o == 0.0) =>
        {
            (Some(a), Some(o))
        }
        _ => (None, None),
    }
}

/// Best-effort metadata for a video; never fails. Dimensions are as displayed.
pub fn read_video_meta(path: &Path) -> PhotoMeta {
    let mut meta = PhotoMeta { kind: MediaKind::Video, ..Default::default() };
    let moov = read_moov(path);
    let kids = moov.as_deref().map(children).unwrap_or_default();
    let tkhd = kids
        .iter()
        .filter(|(k, _)| k == b"trak")
        .filter_map(|(_, trak)| children(trak).into_iter().find(|(k, _)| k == b"tkhd").and_then(|(_, p)| parse_tkhd(p)))
        .find(|t| t.width > 0 && t.height > 0);
    let mvhd = kids.iter().find(|(k, _)| k == b"mvhd").and_then(|(_, p)| parse_mvhd(p));
    if let Some(t) = &tkhd {
        let (w, h) = if t.quarter_turn { (t.height, t.width) } else { (t.width, t.height) };
        meta.width = Some(w);
        meta.height = Some(h);
    }
    meta.duration_ms = mvhd.as_ref().and_then(|m| m.duration_ms);
    let info = read_track(path).ok();
    if let Some(info) = &info {
        // nom-exif reads only version 0 headers; its duration is a fallback and must be sane.
        if meta.duration_ms.is_none() {
            meta.duration_ms = info
                .get(TrackInfoTag::DurationMs)
                .and_then(|v| v.as_u64())
                .and_then(|v| i64::try_from(v).ok());
        }
        meta.camera = info.get(TrackInfoTag::Model).and_then(|v| v.as_str()).map(str::to_owned).filter(|s| !s.is_empty());
        if tkhd.is_none() {
            meta.width = info.get(TrackInfoTag::Width).and_then(|v| v.as_u32());
            meta.height = info.get(TrackInfoTag::Height).and_then(|v| v.as_u32());
        }
        if let Some(g) = info.gps_info() {
            (meta.gps_lat, meta.gps_lon) = sanitize(g.latitude_decimal(), g.longitude_decimal());
        }
    }
    let nom_date = info.as_ref().and_then(|i| i.get(TrackInfoTag::CreateDate)).and_then(|v| v.as_datetime()).map(|d| match d.aware() {
        Some(dt) => dt.timestamp(),
        None => d.into_naive().and_utc().timestamp(),
    });
    meta.taken_at = nom_date
        .filter(|t| *t > 0)
        .or_else(|| mvhd.as_ref().and_then(|m| m.created))
        .map(|t| approx_local(t, meta.gps_lon));
    meta
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixie_testutil::{mp4, Mp4Spec, MP4_EPOCH_OFFSET};

    fn write(dir: &tempfile::TempDir, name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let p = dir.path().join(name);
        std::fs::write(&p, bytes).unwrap();
        p
    }

    // 2017-02-03T19:41:15Z
    const T: i64 = 1_486_150_875;

    fn spec() -> Mp4Spec {
        Mp4Spec { width: 1920, height: 1080, duration_ms: 11_861, created_1904: T as u64 + MP4_EPOCH_OFFSET, ..Default::default() }
    }

    #[test]
    fn reads_duration_dimensions_and_utc_date() {
        let d = tempfile::tempdir().unwrap();
        let m = read_video_meta(&write(&d, "a.mp4", &mp4(&spec())));
        assert_eq!(m.kind, MediaKind::Video);
        assert_eq!(m.duration_ms, Some(11_861));
        assert_eq!((m.width, m.height), (Some(1920), Some(1080)));
        assert_eq!(m.taken_at, Some(T)); // no GPS: UTC as is
        assert_eq!((m.gps_lat, m.gps_lon), (None, None));
    }

    #[test]
    fn portrait_videos_report_swapped_dimensions() {
        let d = tempfile::tempdir().unwrap();
        for rot in [90, 270] {
            let m = read_video_meta(&write(&d, "r.mp4", &mp4(&Mp4Spec { rotation: rot, ..spec() })));
            assert_eq!((m.width, m.height), (Some(1080), Some(1920)), "rotation {rot}");
        }
        for rot in [0, 180] {
            let m = read_video_meta(&write(&d, "r.mp4", &mp4(&Mp4Spec { rotation: rot, ..spec() })));
            assert_eq!((m.width, m.height), (Some(1920), Some(1080)), "rotation {rot}");
        }
    }

    #[test]
    fn gps_is_read_and_shifts_the_date_to_approximate_local_time() {
        let d = tempfile::tempdir().unwrap();
        let s = Mp4Spec { iso6709: Some("+20.6858-156.4440/".into()), ..spec() };
        let m = read_video_meta(&write(&d, "g.mp4", &mp4(&s)));
        assert!((m.gps_lat.unwrap() - 20.6858).abs() < 1e-4);
        assert!((m.gps_lon.unwrap() + 156.4440).abs() < 1e-4);
        assert_eq!(m.taken_at, Some(T - 10 * 3600)); // Hawaii: 09:41 local, not 19:41 UTC
    }

    #[test]
    fn a_zero_zero_fix_is_dropped() {
        let d = tempfile::tempdir().unwrap();
        let s = Mp4Spec { iso6709: Some("+00.0000+000.0000/".into()), ..spec() };
        let m = read_video_meta(&write(&d, "z.mp4", &mp4(&s)));
        assert_eq!((m.gps_lat, m.gps_lon), (None, None));
    }

    #[test]
    fn moov_after_mdat_is_found() {
        let d = tempfile::tempdir().unwrap();
        let m = read_video_meta(&write(&d, "last.mp4", &mp4(&Mp4Spec { moov_last: true, rotation: 90, ..spec() })));
        assert_eq!((m.width, m.height, m.duration_ms), (Some(1080), Some(1920), Some(11_861)));
    }

    #[test]
    fn corrupt_truncated_empty_and_missing_files_give_a_bare_video_meta() {
        let d = tempfile::tempdir().unwrap();
        let good = mp4(&spec());
        let cases: Vec<(&str, Vec<u8>)> = vec![
            ("empty.mp4", vec![]),
            ("junk.mp4", vec![0xAB; 500]),
            ("trunc.mp4", good[..good.len() / 2].to_vec()),
            ("zerosize.mp4", [0u8, 0, 0, 0, b'm', b'o', b'o', b'v', 1, 2, 3].to_vec()),
        ];
        for (name, bytes) in cases {
            let m = read_video_meta(&write(&d, name, &bytes));
            assert_eq!(m.kind, MediaKind::Video, "{name}");
            assert_eq!(m.taken_at, None, "{name}"); // the scanner falls back to mtime
        }
        let m = read_video_meta(&d.path().join("missing.mp4"));
        assert_eq!(m.kind, MediaKind::Video);
        assert_eq!((m.width, m.duration_ms), (None, None));
    }

    #[test]
    fn version_1_mvhd_gives_the_right_duration_and_date() {
        let d = tempfile::tempdir().unwrap();
        let m = read_video_meta(&write(&d, "v1.mp4", &mp4(&Mp4Spec { mvhd_v1: true, ..spec() })));
        assert_eq!(m.duration_ms, Some(11_861));
        assert_eq!(m.taken_at, Some(T));
        assert_eq!((m.width, m.height), (Some(1920), Some(1080)));
    }

    #[test]
    fn a_zero_timescale_means_unknown_duration_not_garbage() {
        let d = tempfile::tempdir().unwrap();
        for v1 in [false, true] {
            let m = read_video_meta(&write(&d, "zt.mp4", &mp4(&Mp4Spec { zero_timescale: true, mvhd_v1: v1, ..spec() })));
            assert_eq!(m.duration_ms, None, "v1={v1}");
        }
    }

    #[test]
    fn a_box_whose_64_bit_size_overflows_does_not_hang_or_panic() {
        let d = tempfile::tempdir().unwrap();
        let mut bytes = vec![0, 0, 0, 16, b'f', b'r', b'e', b'e', 0, 0, 0, 0, 0, 0, 0, 0]; // a 16-byte free box
        bytes.extend_from_slice(&[0, 0, 0, 1, b'x', b'x', b'x', b'x']); // size 1: a 64-bit size follows
        bytes.extend_from_slice(&(u64::MAX - 15).to_be_bytes()); // pos (16) + this wraps to 0
        bytes.extend_from_slice(&[0u8; 32]);
        let m = read_video_meta(&write(&d, "wrap.mp4", &bytes));
        assert_eq!(m.kind, MediaKind::Video);
        assert_eq!(m.width, None);
    }

    #[test]
    fn approx_local_rounds_longitude_to_whole_hours() {
        assert_eq!(approx_local(1000, None), 1000);
        assert_eq!(approx_local(1000, Some(-122.2)), 1000 - 8 * 3600);
        assert_eq!(approx_local(1000, Some(151.2)), 1000 + 10 * 3600);
        assert_eq!(approx_local(1000, Some(7.4)), 1000);
    }

    /// Real files only, by hand: `PIXIE_VIDEO=path cargo test -p pixie-image live_video -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn live_video() {
        let p = std::env::var("PIXIE_VIDEO").expect("set PIXIE_VIDEO");
        println!("{:?}", read_video_meta(Path::new(&p)));
    }
}
