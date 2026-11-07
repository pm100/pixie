//! Zip files as a source: read the entries of a zip (names, sizes, dates; nothing is decompressed), decide which are photos and videos to
//! extract, make their names safe, and extract them into a folder (see `extract_zip`).

use pixie_core::{media_kind, MediaKind, PixieError, Result};
use std::collections::HashSet;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use zip::ZipArchive;

/// The most entries a zip may have before it is refused.
pub const MAX_ENTRIES: usize = 500_000;
/// The most an entry may expand to, as a multiple of what it takes in the zip (above 1 MiB), before it is left out.
pub const MAX_RATIO: u64 = 200;
/// The most all the entries together may expand to: 2 TiB.
pub const MAX_TOTAL: u64 = 2 << 40;
/// Longest a file or folder name from a zip may be, in bytes.
const MAX_PART: usize = 120;

/// The limits a zip is read with (a test lowers them).
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub entries: usize,
    pub ratio: u64,
    pub total: u64,
}

const LIMITS: Limits = Limits { entries: MAX_ENTRIES, ratio: MAX_RATIO, total: MAX_TOTAL };

/// Why an entry is not extracted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Skip {
    Directory,
    /// Not a photo or a video.
    NotMedia,
    Encrypted,
    /// Its name would reach outside the folder it is extracted into, or cannot be made into a name (the reason is a sentence).
    Unsafe(String),
    /// It would expand far beyond what it takes in the zip (a zip bomb).
    TooBig,
    Link,
    /// A zip inside the zip (not opened).
    Archive,
}

/// One entry of a zip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipEntry {
    /// Its place in the zip.
    pub index: usize,
    /// Its safe relative path, with forward slashes (made unique among the entries to extract), or what the zip calls it when it has none.
    pub name: String,
    /// What the zip calls it.
    pub original: String,
    pub size: u64,
    pub packed: u64,
    pub crc: u32,
    /// The entry's own date (local time with no time zone, read as UTC), in unix milliseconds.
    pub mtime: Option<i64>,
    pub kind: Option<MediaKind>,
    /// Why it is not extracted; `None` for the photos and videos that will be.
    pub skip: Option<Skip>,
}

/// What a zip holds.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ZipListing {
    pub entries: Vec<ZipEntry>,
    pub photos: usize,
    pub videos: usize,
    /// Files that are not photos or videos.
    pub other: usize,
    /// Zips inside the zip.
    pub archives: usize,
    pub encrypted: usize,
    pub unsafe_names: usize,
    /// Entries left out for expanding far more than they take.
    pub too_big: usize,
    /// The bytes of the entries that will be extracted.
    pub bytes: u64,
}

const RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// One folder or file name from a zip, made fit for any file system: characters that cannot be in a name become `_`, trailing dots and
/// spaces go, a name Windows reserves gets a `_` in front, and one that is too long is shortened (keeping its extension).
fn clean_part(part: &str) -> String {
    let mut s: String = part.chars().map(|c| if c.is_control() || "<>:\"|?*".contains(c) { '_' } else { c }).collect();
    s = s.trim_end_matches(['.', ' ']).to_string();
    let stem = s.split('.').next().unwrap_or("").to_uppercase();
    if RESERVED.contains(&stem.as_str()) {
        s.insert(0, '_');
    }
    if s.len() > MAX_PART {
        let ext = s.rfind('.').map(|i| s[i..].to_string()).filter(|e| e.len() <= 10).unwrap_or_default();
        let mut cut = MAX_PART - ext.len();
        while !s.is_char_boundary(cut) {
            cut -= 1;
        }
        s = format!("{}{ext}", &s[..cut]);
    }
    s
}

/// A path from a zip as a safe relative path with forward slashes, or why it cannot be one: nothing that reaches outside the folder it is
/// extracted into (a leading slash or drive, or more `..` than folders), and every part fit for any file system.
pub fn safe_name(raw: &str) -> std::result::Result<String, String> {
    if raw.contains('\0') {
        return Err("its name has a hidden character in it".into());
    }
    let s = raw.replace('\\', "/");
    if s.starts_with('/') {
        return Err("it starts at the root of a drive or share".into());
    }
    let b = s.as_bytes();
    if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
        return Err("it names a drive".into());
    }
    let mut parts: Vec<String> = Vec::new();
    for part in s.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return Err("it climbs out of the folder it would go into".into());
                }
            }
            p => {
                let clean = clean_part(p);
                if !clean.is_empty() {
                    parts.push(clean);
                }
            }
        }
    }
    if parts.is_empty() {
        return Err("it has no name".into());
    }
    Ok(parts.join("/"))
}

/// Days since 1970-01-01 of a calendar date.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// `name` with a number before its extension (`a.jpg` and 1 give `a-1.jpg`).
fn numbered_name(name: &str, n: u32) -> String {
    let (dir, file) = name.rsplit_once('/').map_or(("", name), |(d, f)| (d, f));
    let (stem, ext) = file.rfind('.').filter(|i| *i > 0).map_or((file, ""), |i| (&file[..i], &file[i..]));
    let numbered = format!("{stem}-{n}{ext}");
    if dir.is_empty() { numbered } else { format!("{dir}/{numbered}") }
}

/// Reads the entries of a zip (from its directory: nothing is decompressed). See `ZipListing`.
pub fn list_zip(path: &Path) -> Result<ZipListing> {
    list_zip_with(path, &LIMITS)
}

/// `list_zip` with the limits given.
pub fn list_zip_with(path: &Path, limits: &Limits) -> Result<ZipListing> {
    let file = std::fs::File::open(path).map_err(|e| PixieError::Invalid(format!("That file cannot be opened ({e}).")))?;
    let mut archive = ZipArchive::new(file).map_err(|_| PixieError::Invalid("That is not a zip file, or it is damaged.".into()))?;
    if archive.len() > limits.entries {
        return Err(PixieError::Invalid(format!("This zip has too many entries ({}).", archive.len())));
    }
    let mut out = ZipListing::default();
    let mut taken: HashSet<String> = HashSet::new();
    let mut total = 0u64;
    for index in 0..archive.len() {
        let file = archive.by_index_raw(index).map_err(|_| PixieError::Invalid("That zip is damaged: one of its entries cannot be read.".into()))?;
        let original = file.name().map(|n| n.into_owned()).unwrap_or_else(|_| String::from_utf8_lossy(file.name_raw()).into_owned());
        let (size, packed, crc) = (file.size(), file.compressed_size(), file.crc32());
        let mtime = file.last_modified().map(|d| {
            let days = days_from_civil(d.year() as i64, d.month() as i64, d.day() as i64);
            (days * 86_400 + d.hour() as i64 * 3600 + d.minute() as i64 * 60 + d.second() as i64) * 1000
        });
        let is_link = file.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000);
        let (is_dir, encrypted) = (file.is_dir(), file.encrypted());
        drop(file);
        if !is_dir {
            total = total.saturating_add(size);
            if total > limits.total {
                return Err(PixieError::Invalid("This zip would expand to too much data to extract.".into()));
            }
        }
        let safe = if is_dir { Ok(original.trim_end_matches(['/', '\\']).to_string()) } else { safe_name(&original) };
        let mut entry = ZipEntry { index, name: safe.clone().unwrap_or_else(|_| original.clone()), original, size, packed, crc, mtime, kind: None, skip: None };
        if is_dir {
            entry.skip = Some(Skip::Directory);
        } else if let Err(why) = &safe {
            entry.skip = Some(Skip::Unsafe(why.clone()));
            out.unsafe_names += 1;
        } else if is_link {
            entry.skip = Some(Skip::Link);
        } else if encrypted {
            entry.skip = Some(Skip::Encrypted);
            out.encrypted += 1;
        } else if entry.name.to_lowercase().ends_with(".zip") {
            entry.skip = Some(Skip::Archive);
            out.archives += 1;
        } else if let Some(kind) = media_kind(Path::new(&entry.name)) {
            entry.kind = Some(kind);
            if size > packed.max(1).saturating_mul(limits.ratio) && size > (1 << 20) {
                entry.skip = Some(Skip::TooBig);
                out.too_big += 1;
            } else {
                // A name another entry has (ignoring case, as Windows and macOS do) gets a number.
                let mut name = entry.name.clone();
                let mut n = 0;
                while !taken.insert(name.to_lowercase()) {
                    n += 1;
                    name = numbered_name(&entry.name, n);
                }
                entry.name = name;
                out.bytes += size;
                if kind == MediaKind::Video { out.videos += 1 } else { out.photos += 1 }
            }
        } else {
            entry.skip = Some(Skip::NotMedia);
            out.other += 1;
        }
        out.entries.push(entry);
    }
    Ok(out)
}

/// How far an extraction has got.
#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
pub struct ExtractProgress {
    pub done: usize,
    pub total: usize,
    pub bytes_done: u64,
    pub bytes_total: u64,
    /// How many photos and how many videos are extracted so far.
    pub photos: usize,
    pub videos: usize,
}

/// What an extraction did.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ExtractReport {
    pub extracted: usize,
    pub bytes: u64,
    pub failed: usize,
    /// The first few problems, as sentences naming the entry.
    pub problems: Vec<String>,
    pub stopped: bool,
}

/// How many problems a report keeps to show.
const PROBLEMS_SHOWN: usize = 5;

/// The first free name for `name` in `dir`: the name itself, then `name-1`, `name-2`, ...
fn free_path(dir: &Path, name: &str) -> std::path::PathBuf {
    let mut n = 0;
    loop {
        let candidate = dir.join(if n == 0 { name.to_string() } else { numbered_name(name, n) });
        if !candidate.exists() {
            return candidate;
        }
        n += 1;
    }
}

/// Why one entry could not be extracted, or that the person stopped it.
enum Stopped {
    Problem(String),
    Stop,
}

/// Extracts one entry to a temporary name next to where it goes, checks it (the zip reader reports a wrong checksum when it reaches the
/// end), gives it the entry's date, and puts it in place under a free name. Nothing is left behind on any failure.
fn extract_one(archive: &mut ZipArchive<std::fs::File>, entry: &ZipEntry, dest: &Path, stop: &AtomicBool, on_bytes: &mut dyn FnMut(u64)) -> std::result::Result<u64, Stopped> {
    let problem = |s: String| Stopped::Problem(s);
    let relative = Path::new(&entry.name);
    // Names are safe already; this is the second line of defence.
    if relative.components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
        return Err(problem("its name is not a plain relative path".into()));
    }
    let target = dest.join(relative);
    let dir = target.parent().unwrap_or(dest).to_path_buf();
    std::fs::create_dir_all(&dir).map_err(|e| problem(format!("cannot make the folder ({e})")))?;
    let file_name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "unnamed".into());
    let temp = dir.join(format!(".pixie-extract-{}.part", std::process::id()));
    let result = (|| {
        let mut from = archive.by_index(entry.index).map_err(|e| problem(format!("cannot be read ({e})")))?;
        let mut to = std::fs::File::create(&temp).map_err(|e| problem(format!("cannot write ({e})")))?;
        let mut buf = vec![0u8; 1 << 20];
        let mut written = 0u64;
        loop {
            if stop.load(Ordering::Acquire) {
                return Err(Stopped::Stop);
            }
            let n = from.read(&mut buf).map_err(|e| problem(if e.kind() == std::io::ErrorKind::InvalidData { "is damaged (its checksum is wrong)".to_string() } else { format!("cannot be read ({e})") }))?;
            if n == 0 {
                break;
            }
            written += n as u64;
            if written > entry.size {
                return Err(problem("is larger than the zip says it is".into()));
            }
            to.write_all(&buf[..n]).map_err(|e| problem(if e.kind() == std::io::ErrorKind::StorageFull { "the disk is full".to_string() } else { format!("cannot write ({e})") }))?;
            on_bytes(written);
        }
        if written != entry.size {
            return Err(problem("is shorter than the zip says it is".into()));
        }
        if let Some(ms) = entry.mtime.filter(|m| *m >= 0) {
            let _ = to.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_millis(ms as u64));
        }
        Ok(written)
    })();
    match result {
        Ok(written) => {
            let place = free_path(&dir, &file_name);
            std::fs::rename(&temp, &place).map_err(|e| {
                let _ = std::fs::remove_file(&temp);
                problem(format!("cannot be put in place ({e})"))
            })?;
            Ok(written)
        }
        Err(e) => {
            let _ = std::fs::remove_file(&temp);
            Err(e)
        }
    }
}

/// Extracts the photos and videos of a zip (the entries of `listing` with no `skip`) into `dest`, below their safe names. Nothing is written
/// outside `dest`, nothing is overwritten (a taken name gives the new file a number), a damaged entry is removed and reported while the rest go
/// on, and stopping leaves no partial file. The zip is only read.
pub fn extract_zip(zip: &Path, listing: &ZipListing, dest: &Path, stop: &AtomicBool, mut progress: impl FnMut(&ExtractProgress)) -> Result<ExtractReport> {
    let file = std::fs::File::open(zip).map_err(|e| PixieError::Invalid(format!("That file cannot be opened ({e}).")))?;
    let mut archive = ZipArchive::new(file).map_err(|_| PixieError::Invalid("That is not a zip file, or it is damaged.".into()))?;
    std::fs::create_dir_all(dest).map_err(|e| PixieError::Invalid(format!("The Inbox folder cannot be made ({e}).")))?;
    let todo: Vec<&ZipEntry> = listing.entries.iter().filter(|e| e.skip.is_none()).collect();
    let bytes_total: u64 = todo.iter().map(|e| e.size).sum();
    let mut report = ExtractReport::default();
    let mut bytes_before = 0u64;
    let (mut photos, mut videos) = (0usize, 0usize);
    let mut last = std::time::Instant::now() - std::time::Duration::from_secs(1);
    for (i, entry) in todo.iter().enumerate() {
        if stop.load(Ordering::Acquire) {
            report.stopped = true;
            break;
        }
        progress(&ExtractProgress { done: i, total: todo.len(), bytes_done: bytes_before, bytes_total, photos, videos });
        let result = extract_one(&mut archive, entry, dest, stop, &mut |n| {
            if last.elapsed() >= std::time::Duration::from_millis(150) {
                last = std::time::Instant::now();
                progress(&ExtractProgress { done: i, total: todo.len(), bytes_done: (bytes_before + n).min(bytes_total), bytes_total, photos, videos });
            }
        });
        match result {
            Ok(written) => {
                report.extracted += 1;
                report.bytes += written;
                bytes_before += entry.size;
                if entry.kind == Some(MediaKind::Video) { videos += 1 } else { photos += 1 }
            }
            Err(Stopped::Stop) => {
                report.stopped = true;
                break;
            }
            Err(Stopped::Problem(why)) => {
                report.failed += 1;
                if report.problems.len() < PROBLEMS_SHOWN {
                    report.problems.push(format!("{} {why}", entry.original));
                }
                bytes_before += entry.size;
            }
        }
    }
    progress(&ExtractProgress { done: if report.stopped { report.extracted + report.failed } else { todo.len() }, total: todo.len(), bytes_done: bytes_before, bytes_total, photos, videos });
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, DateTime, ZipWriter};

    /// A zip in a temp dir with these entries (name, bytes), stored (not compressed) and dated 2024-03-05 10:11:12.
    fn make_zip(entries: &[(&str, &[u8])]) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.zip");
        let mut z = ZipWriter::new(std::fs::File::create(&path).unwrap());
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Stored).last_modified_time(DateTime::from_date_and_time(2024, 3, 5, 10, 11, 12).unwrap());
        for (name, bytes) in entries {
            if name.ends_with('/') {
                z.add_directory(*name, opts).unwrap();
            } else {
                z.start_file(*name, opts).unwrap();
                z.write_all(bytes).unwrap();
            }
        }
        z.finish().unwrap();
        (dir, path)
    }

    fn names(l: &ZipListing) -> Vec<&str> {
        l.entries.iter().filter(|e| e.skip.is_none()).map(|e| e.name.as_str()).collect()
    }

    #[test]
    fn safe_names_are_made_from_what_the_zip_calls_things() {
        assert_eq!(safe_name("DCIM/a.jpg").unwrap(), "DCIM/a.jpg");
        assert_eq!(safe_name("a\\b.jpg").unwrap(), "a/b.jpg", "backslashes are folders");
        assert_eq!(safe_name("./a.jpg").unwrap(), "a.jpg");
        assert_eq!(safe_name("a//b.jpg").unwrap(), "a/b.jpg");
        assert_eq!(safe_name("a/b/../c.jpg").unwrap(), "a/c.jpg", "a dot-dot that stays inside is fine");
        for bad in ["../../evil.jpg", "/etc/x.jpg", "C:/x.jpg", "C:\\x.jpg", "\\\\server\\share\\x.jpg", "", "a/../../x.jpg", "..", "a/..", "c:x.jpg"] {
            assert!(safe_name(bad).is_err(), "{bad:?} must not be accepted");
        }
    }

    #[test]
    fn names_a_file_system_cannot_hold_are_made_safe() {
        assert_eq!(safe_name("CON.jpg").unwrap(), "_CON.jpg");
        assert_eq!(safe_name("dir/NUL").unwrap(), "dir/_NUL");
        assert_eq!(safe_name("com1.txt").unwrap(), "_com1.txt");
        assert_eq!(safe_name("a. /x .jpg").unwrap(), "a/x .jpg", "trailing dots and spaces are trimmed from each part, so a part left empty is dropped");
        assert_eq!(safe_name("we<ird>:name?.jpg").unwrap(), "we_ird__name_.jpg");
        assert_eq!(safe_name("tab\there.jpg").unwrap(), "tab_here.jpg");
        let long = format!("{}.jpg", "x".repeat(300));
        let short = safe_name(&long).unwrap();
        assert!(short.len() <= 125 && short.ends_with(".jpg"), "{short}");
    }

    #[test]
    fn a_listing_counts_photos_videos_and_other_files_and_sums_the_bytes_to_extract() {
        let (_d, p) = make_zip(&[("a.jpg", b"12345"), ("B.JPG", b"123"), ("c.mp4", b"1234567"), ("notes.txt", b"hi"), ("dir/", b""), ("inner.zip", b"zz")]);
        let l = list_zip(&p).unwrap();
        assert_eq!((l.photos, l.videos, l.other, l.archives), (2, 1, 1, 1));
        assert_eq!(l.bytes, 5 + 3 + 7);
        assert_eq!(names(&l), vec!["a.jpg", "B.JPG", "c.mp4"]);
        let skip = |n: &str| l.entries.iter().find(|e| e.original == n).unwrap().skip.clone();
        assert_eq!(skip("dir/"), Some(Skip::Directory));
        assert_eq!(skip("notes.txt"), Some(Skip::NotMedia));
        assert_eq!(skip("inner.zip"), Some(Skip::Archive));
        let a = l.entries.iter().find(|e| e.original == "a.jpg").unwrap();
        assert_eq!((a.size, a.kind), (5, Some(pixie_core::MediaKind::Photo)));
    }

    #[test]
    fn unsafe_names_are_skipped_with_a_reason_and_counted() {
        let (_d, p) = make_zip(&[("../x.jpg", b"1"), ("good.jpg", b"2"), ("/abs.jpg", b"3")]);
        let l = list_zip(&p).unwrap();
        assert_eq!(l.unsafe_names, 2);
        assert_eq!(names(&l), vec!["good.jpg"]);
        assert!(matches!(l.entries[0].skip, Some(Skip::Unsafe(_))));
        assert_eq!(l.bytes, 1);
    }

    #[test]
    fn names_that_collide_are_kept_apart_ignoring_case() {
        let (_d, p) = make_zip(&[("A.jpg", b"1"), ("a.jpg", b"2"), ("sub/a.jpg", b"3"), ("SUB/A.JPG", b"4"), ("a.JPG", b"5")]);
        let l = list_zip(&p).unwrap();
        assert_eq!(names(&l), vec!["A.jpg", "a-1.jpg", "sub/a.jpg", "SUB/A-1.JPG", "a-2.JPG"]);
    }

    #[test]
    fn an_encrypted_entry_is_skipped_and_counted() {
        let (_d, p) = make_zip(&[("a.jpg", b"1"), ("secret.jpg", b"2")]);
        mark_encrypted(&p, "secret.jpg");
        let l = list_zip(&p).unwrap();
        assert_eq!(l.encrypted, 1);
        assert_eq!(names(&l), vec!["a.jpg"]);
        assert_eq!(l.entries[1].skip, Some(Skip::Encrypted));
    }

    /// Sets the "encrypted" flag (bit 0 of the general purpose flags) of one entry in the central directory, as a zip with a password has it.
    fn mark_encrypted(path: &std::path::Path, name: &str) {
        let mut bytes = std::fs::read(path).unwrap();
        let mut at = 0;
        while at + 46 < bytes.len() {
            if bytes[at..at + 4] == [0x50, 0x4b, 0x01, 0x02] {
                let len = u16::from_le_bytes([bytes[at + 28], bytes[at + 29]]) as usize;
                if &bytes[at + 46..at + 46 + len] == name.as_bytes() {
                    bytes[at + 8] |= 1;
                }
            }
            at += 1;
        }
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn a_zip_bomb_is_refused_by_ratio_and_by_total() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bomb.zip");
        let mut z = ZipWriter::new(std::fs::File::create(&path).unwrap());
        z.start_file("zeros.jpg", SimpleFileOptions::default().compression_method(CompressionMethod::Deflated)).unwrap();
        z.write_all(&vec![0u8; 20 << 20]).unwrap();
        z.start_file("fine.jpg", SimpleFileOptions::default().compression_method(CompressionMethod::Stored)).unwrap();
        z.write_all(b"12345").unwrap();
        z.finish().unwrap();
        let l = list_zip(&path).unwrap();
        assert_eq!(l.entries[0].skip, Some(Skip::TooBig), "20 MiB of zeros packs far beyond the ratio");
        assert_eq!(names(&l), vec!["fine.jpg"]);
        let e = list_zip_with(&path, &Limits { entries: 10, ratio: MAX_RATIO, total: 3 }).unwrap_err().user_message();
        assert!(e.contains("too much"), "{e}");
        let e = list_zip_with(&path, &Limits { entries: 1, ratio: MAX_RATIO, total: MAX_TOTAL }).unwrap_err().user_message();
        assert!(e.contains("too many"), "{e}");
    }

    #[test]
    fn a_file_that_is_not_a_zip_is_a_plain_error() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.zip");
        std::fs::write(&p, b"this is not a zip file at all").unwrap();
        assert!(list_zip(&p).unwrap_err().user_message().contains("not a zip"));
        assert!(list_zip(&dir.path().join("missing.zip")).is_err());
    }

    #[test]
    fn a_truncated_zip_cannot_be_listed() {
        let (_d, p) = make_zip(&[("a.jpg", b"1234567890")]);
        let bytes = std::fs::read(&p).unwrap();
        std::fs::write(&p, &bytes[..bytes.len() - 30]).unwrap();
        assert!(list_zip(&p).is_err());
    }

    #[test]
    fn an_entrys_date_is_its_own_local_time_read_as_utc() {
        let (_d, p) = make_zip(&[("a.jpg", b"1")]);
        let l = list_zip(&p).unwrap();
        // 2024-03-05 10:11:12 UTC
        assert_eq!(l.entries[0].mtime, Some(1_709_633_472_000));
    }

    // ---- extracting ----

    fn extract(zip: &std::path::Path, dest: &std::path::Path) -> (ZipListing, ExtractReport) {
        let l = list_zip(zip).unwrap();
        let r = extract_zip(zip, &l, dest, &std::sync::atomic::AtomicBool::new(false), |_| {}).unwrap();
        (l, r)
    }
    fn files_below(dir: &std::path::Path) -> Vec<String> {
        let mut out = Vec::new();
        fn walk(base: &std::path::Path, dir: &std::path::Path, out: &mut Vec<String>) {
            if let Ok(rd) = std::fs::read_dir(dir) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.is_dir() { walk(base, &p, out) } else { out.push(p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/")) }
                }
            }
        }
        walk(dir, dir, &mut out);
        out.sort();
        out
    }

    #[test]
    fn entries_land_below_the_destination_with_their_folders_and_dates() {
        let (_d, zip) = make_zip(&[("DCIM/a.jpg", b"alpha"), ("b.jpg", b"bravo!"), ("notes.txt", b"skipped")]);
        let out = tempfile::tempdir().unwrap();
        let (_, r) = extract(&zip, out.path());
        assert_eq!((r.extracted, r.failed, r.stopped, r.bytes), (2, 0, false, 11));
        assert_eq!(files_below(out.path()), vec!["DCIM/a.jpg", "b.jpg"], "no temporary file and no skipped file");
        assert_eq!(std::fs::read(out.path().join("DCIM/a.jpg")).unwrap(), b"alpha");
        let modified = std::fs::metadata(out.path().join("b.jpg")).unwrap().modified().unwrap();
        assert_eq!(modified.duration_since(std::time::UNIX_EPOCH).unwrap().as_millis(), 1_709_633_472_000);
    }

    #[test]
    fn nothing_is_written_outside_the_destination() {
        let (_d, zip) = make_zip(&[("../../evil.jpg", b"1"), ("/abs.jpg", b"2"), ("C:/x.jpg", b"3"), ("good.jpg", b"4")]);
        let root = tempfile::tempdir().unwrap();
        let dest = root.path().join("inbox");
        let (l, r) = extract(&zip, &dest);
        assert_eq!(r.extracted, 1);
        assert_eq!(l.unsafe_names, 3);
        assert_eq!(files_below(root.path()), vec!["inbox/good.jpg"]);
        assert!(!root.path().parent().unwrap().join("evil.jpg").exists());
    }

    #[test]
    fn an_existing_file_is_never_overwritten() {
        let (_d, zip) = make_zip(&[("a.jpg", b"from the zip")]);
        let out = tempfile::tempdir().unwrap();
        std::fs::write(out.path().join("a.jpg"), b"already here").unwrap();
        let (_, r) = extract(&zip, out.path());
        assert_eq!(r.extracted, 1);
        assert_eq!(std::fs::read(out.path().join("a.jpg")).unwrap(), b"already here");
        assert_eq!(std::fs::read(out.path().join("a-1.jpg")).unwrap(), b"from the zip");
    }

    #[test]
    fn a_wrong_checksum_removes_the_file_reports_it_and_the_rest_go_on() {
        let (_d, zip) = make_zip(&[("good.jpg", b"GOODGOODGOOD"), ("bad.jpg", b"BADBADBADBAD"), ("last.jpg", b"LASTLASTLAST")]);
        let mut bytes = std::fs::read(&zip).unwrap();
        let at = bytes.windows(12).position(|w| w == b"BADBADBADBAD").unwrap();
        bytes[at + 3] ^= 0xFF;
        std::fs::write(&zip, bytes).unwrap();
        let out = tempfile::tempdir().unwrap();
        let (_, r) = extract(&zip, out.path());
        assert_eq!((r.extracted, r.failed), (2, 1));
        assert_eq!(files_below(out.path()), vec!["good.jpg", "last.jpg"]);
        assert!(r.problems[0].contains("bad.jpg"), "{:?}", r.problems);
    }

    #[test]
    fn an_entry_larger_than_the_zip_said_is_stopped_and_removed() {
        let (_d, zip) = make_zip(&[("a.jpg", b"0123456789")]);
        let mut l = list_zip(&zip).unwrap();
        l.entries[0].size = 3;
        let out = tempfile::tempdir().unwrap();
        let r = extract_zip(&zip, &l, out.path(), &std::sync::atomic::AtomicBool::new(false), |_| {}).unwrap();
        assert_eq!((r.extracted, r.failed), (0, 1));
        assert!(files_below(out.path()).is_empty());
    }

    #[test]
    fn stopping_leaves_nothing_behind_and_says_so() {
        let (_d, zip) = make_zip(&[("big.jpg", &vec![7u8; 3 << 20]), ("next.jpg", b"x")]);
        let out = tempfile::tempdir().unwrap();
        let l = list_zip(&zip).unwrap();
        let stop = std::sync::atomic::AtomicBool::new(false);
        let r = extract_zip(&zip, &l, out.path(), &stop, |_| stop.store(true, std::sync::atomic::Ordering::Relaxed)).unwrap();
        assert!(r.stopped);
        assert_eq!(r.extracted, 0);
        assert!(files_below(out.path()).is_empty(), "{:?}", files_below(out.path()));
    }

    #[test]
    fn progress_counts_entries_and_bytes_and_ends_complete() {
        let (_d, zip) = make_zip(&[("a.jpg", b"12345"), ("b.jpg", b"1234567"), ("c.mp4", b"12")]);
        let out = tempfile::tempdir().unwrap();
        let l = list_zip(&zip).unwrap();
        let mut last = ExtractProgress::default();
        extract_zip(&zip, &l, out.path(), &std::sync::atomic::AtomicBool::new(false), |p| last = *p).unwrap();
        assert_eq!((last.done, last.total, last.bytes_done, last.bytes_total), (3, 3, 14, 14));
        assert_eq!((last.photos, last.videos), (2, 1), "the photos and videos extracted so far are counted as it goes");
    }
}
