use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

pub const RANGE_CHUNK: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangePlan {
    /// Inclusive byte range to send with a 206.
    Partial { start: u64, end: u64 },
    Unsatisfiable,
}

/// Plan the byte range for a `Range` request header. A missing or unparseable header
/// means the first chunk: browsers always send `Range` for `<video>`, and sending a
/// whole multi-gigabyte file in one response is never what we want.
pub fn plan_range(header: Option<&str>, size: u64) -> RangePlan {
    if size == 0 {
        return RangePlan::Unsatisfiable;
    }
    let last = size - 1;
    let first_chunk = RangePlan::Partial { start: 0, end: last.min(RANGE_CHUNK - 1) };
    let Some(spec) = header.and_then(|h| h.trim().strip_prefix("bytes=")) else { return first_chunk };
    if spec.contains(',') {
        return first_chunk;
    }
    let Some((a, b)) = spec.split_once('-') else { return first_chunk };
    let (a, b) = (a.trim(), b.trim());
    let cap = |start: u64, end: u64| RangePlan::Partial { start, end: end.min(start.saturating_add(RANGE_CHUNK - 1)).min(last) };
    match (a.is_empty(), b.is_empty()) {
        (true, true) => first_chunk,
        (true, false) => match b.parse::<u64>() {
            Ok(0) => RangePlan::Unsatisfiable,
            Ok(n) => cap(size.saturating_sub(n), last),
            Err(_) => first_chunk,
        },
        (false, true) => match a.parse::<u64>() {
            Ok(s) if s <= last => cap(s, last),
            Ok(_) => RangePlan::Unsatisfiable,
            Err(_) => first_chunk,
        },
        (false, false) => match (a.parse::<u64>(), b.parse::<u64>()) {
            (Ok(s), Ok(e)) if s > last || e < s => RangePlan::Unsatisfiable,
            (Ok(s), Ok(e)) => cap(s, e),
            _ => first_chunk,
        },
    }
}

/// Inclusive end of what was actually read, so `Content-Range` always matches the body
/// even when the file shrank after its size was read. `None` when nothing was read.
pub fn served_end(start: u64, bytes_read: usize) -> Option<u64> {
    (bytes_read > 0).then(|| start + bytes_read as u64 - 1)
}

/// The bytes `start..=end` of a file. Reads exactly that slice, never the whole file.
pub fn read_range(path: &Path, start: u64, end: u64) -> std::io::Result<Vec<u8>> {
    if end < start {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "end before start"));
    }
    let mut f = std::fs::File::open(path)?;
    f.seek(SeekFrom::Start(start))?;
    let mut buf = Vec::with_capacity((end - start + 1) as usize);
    f.take(end - start + 1).read_to_end(&mut buf)?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    const MB: u64 = 1024 * 1024;

    fn p(h: &str, size: u64) -> RangePlan {
        plan_range(Some(h), size)
    }

    #[test]
    fn explicit_range_is_honoured_and_clamped_to_the_file() {
        assert_eq!(p("bytes=0-99", 1000), RangePlan::Partial { start: 0, end: 99 });
        assert_eq!(p("bytes=900-5000", 1000), RangePlan::Partial { start: 900, end: 999 });
        assert_eq!(p("bytes=999-999", 1000), RangePlan::Partial { start: 999, end: 999 });
    }

    #[test]
    fn open_ended_and_oversized_ranges_are_capped_at_the_chunk_size() {
        let big = 100 * MB;
        assert_eq!(p("bytes=0-", big), RangePlan::Partial { start: 0, end: RANGE_CHUNK - 1 });
        assert_eq!(p("bytes=10-", big), RangePlan::Partial { start: 10, end: 10 + RANGE_CHUNK - 1 });
        assert_eq!(p("bytes=0-99999999", big), RangePlan::Partial { start: 0, end: RANGE_CHUNK - 1 });
        assert_eq!(p("bytes=5-", 100), RangePlan::Partial { start: 5, end: 99 });
    }

    #[test]
    fn suffix_ranges_count_from_the_end() {
        assert_eq!(p("bytes=-100", 1000), RangePlan::Partial { start: 900, end: 999 });
        assert_eq!(p("bytes=-5000", 1000), RangePlan::Partial { start: 0, end: 999 });
        assert_eq!(p("bytes=-0", 1000), RangePlan::Unsatisfiable);
        let big = 100 * MB;
        assert_eq!(p("bytes=-999999999", big), RangePlan::Partial { start: 0, end: RANGE_CHUNK - 1 }); // longer than the file
        assert_eq!(p("bytes=-99999999", big), RangePlan::Partial { start: big - 99_999_999, end: big - 99_999_999 + RANGE_CHUNK - 1 });
    }

    #[test]
    fn unsatisfiable_cases() {
        assert_eq!(p("bytes=1000-", 1000), RangePlan::Unsatisfiable);
        assert_eq!(p("bytes=2000-2100", 1000), RangePlan::Unsatisfiable);
        assert_eq!(p("bytes=0-", 0), RangePlan::Unsatisfiable);
        assert_eq!(plan_range(None, 0), RangePlan::Unsatisfiable);
        assert_eq!(p("bytes=50-10", 1000), RangePlan::Unsatisfiable);
    }

    #[test]
    fn missing_malformed_or_multi_range_headers_mean_the_first_chunk() {
        for h in [None, Some("bytes=abc"), Some("items=0-5"), Some(""), Some("bytes=0-1,5-6"), Some("bytes=-")] {
            assert_eq!(plan_range(h, 100), RangePlan::Partial { start: 0, end: 99 }, "{h:?}");
        }
        assert_eq!(plan_range(None, 100 * MB), RangePlan::Partial { start: 0, end: RANGE_CHUNK - 1 });
    }

    #[test]
    fn read_range_returns_exactly_the_inclusive_slice() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("f.bin");
        std::fs::write(&f, (0u8..=255).collect::<Vec<_>>()).unwrap();
        assert_eq!(read_range(&f, 10, 14).unwrap(), vec![10, 11, 12, 13, 14]);
        assert_eq!(read_range(&f, 250, 255).unwrap().len(), 6);
        assert_eq!(read_range(&f, 0, 0).unwrap(), vec![0]);
        assert!(read_range(&d.path().join("missing"), 0, 1).is_err());
    }

    #[test]
    fn read_range_rejects_an_inverted_range_and_returns_what_exists_for_a_short_file() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("f.bin");
        std::fs::write(&f, [1u8, 2, 3]).unwrap();
        assert!(read_range(&f, 5, 2).is_err());
        // The file shrank after the size was read: fewer bytes come back than were asked for.
        assert_eq!(read_range(&f, 1, 100).unwrap(), vec![2, 3]);
        assert_eq!(served_end(1, 2), Some(2));
        assert_eq!(served_end(1, 0), None);
    }
}
