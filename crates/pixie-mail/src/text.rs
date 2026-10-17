use pixie_core::{validate_header_text, PixieError, Result};
use std::collections::HashSet;

pub fn check_subject(s: &str) -> Result<String> {
    let s = validate_header_text(s, 200, "subject")?;
    if s.is_empty() {
        return Err(PixieError::Invalid("Please give the email a subject.".into()));
    }
    Ok(s)
}

pub fn check_body(s: &str) -> Result<String> {
    let s = s.replace("\r\n", "\n").replace('\r', "\n");
    if s.chars().count() > 5000 {
        return Err(PixieError::Invalid("The message is longer than 5,000 characters.".into()));
    }
    if s.chars().any(|c| c.is_control() && c != '\n' && c != '\t') {
        return Err(PixieError::Invalid("The message contains characters that cannot be sent.".into()));
    }
    Ok(s)
}

/// A file name that is safe to put in a header: no path, no separators or control characters, no
/// leading dot, bounded length, and not used before in `taken`. `ext` replaces the extension.
pub fn safe_filename(name: &str, ext: Option<&str>, taken: &mut HashSet<String>) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("");
    let (stem, own_ext) = match base.rfind('.') {
        Some(i) if i > 0 => (&base[..i], &base[i + 1..]),
        _ => (base, ""),
    };
    let clean = |s: &str| -> String {
        s.chars().map(|c| if c.is_control() || "\\/:*?\"<>|".contains(c) { '_' } else { c }).collect()
    };
    let mut stem = clean(stem).trim_start_matches('.').trim().to_string();
    if stem.is_empty() {
        stem = "attachment".into();
    }
    if stem.chars().count() > 100 {
        stem = stem.chars().take(100).collect();
    }
    let ext = clean(ext.unwrap_or(own_ext));
    let join = |s: &str| if ext.is_empty() { s.to_string() } else { format!("{s}.{ext}") };
    let mut candidate = join(&stem);
    let mut n = 2;
    while !taken.insert(candidate.to_lowercase()) {
        candidate = join(&format!("{stem} ({n})"));
        n += 1;
    }
    candidate
}

/// Size on the wire: base64 grows binary by 4/3; plus a little for headers.
pub fn encoded_size(attachment_bytes: &[u64]) -> u64 {
    attachment_bytes.iter().sum::<u64>() * 4 / 3 + 2048
}

pub fn check_limit(encoded: u64, limit_mb: u32) -> Result<()> {
    if encoded > limit_mb as u64 * 1024 * 1024 {
        return Err(PixieError::Invalid(format!(
            "This email would be about {:.1} MB, more than the {limit_mb} MB your mail provider allows. Choose a smaller size or fewer items.",
            encoded as f64 / (1024.0 * 1024.0)
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subject_and_body_checks() {
        assert_eq!(check_subject("  3 photos  ").unwrap(), "3 photos");
        for s in ["", "   ", "a\nb", "a\rb", &"x".repeat(201)] {
            assert!(check_subject(s).is_err(), "{s:?}");
        }
        assert_eq!(check_body("hi\r\nthere\r").unwrap(), "hi\nthere\n"); // line endings normalised
        assert_eq!(check_body("").unwrap(), "");
        assert!(check_body(&"x".repeat(5001)).is_err());
        assert!(check_body("bad\0char").is_err());
    }

    #[test]
    fn file_names_are_safe_and_unique() {
        let mut taken = HashSet::new();
        assert_eq!(safe_filename("IMG_1.JPG", None, &mut taken), "IMG_1.JPG");
        assert_eq!(safe_filename("IMG_1.JPG", None, &mut taken), "IMG_1 (2).JPG");
        assert_eq!(safe_filename("IMG_1.JPG", None, &mut taken), "IMG_1 (3).JPG");
        assert_eq!(safe_filename("photo.heic", Some("jpg"), &mut taken), "photo.jpg"); // resized copies are jpegs
        assert_eq!(safe_filename("..\\..\\evil.png", None, &mut taken), "evil.png");
        assert_eq!(safe_filename("a/b\\c:d*e?.jpg", None, &mut taken), "c_d_e_.jpg");
        assert_eq!(safe_filename(".hidden", None, &mut taken), "hidden");
        assert_eq!(safe_filename("line\nbreak\r.jpg", None, &mut taken), "line_break_.jpg");
        assert_eq!(safe_filename("", None, &mut taken), "attachment");
        assert!(safe_filename(&"n".repeat(300), None, &mut taken).len() <= 120);
    }

    #[test]
    fn encoded_size_is_base64_plus_overhead_and_limits_are_checked() {
        assert_eq!(encoded_size(&[3_000_000, 3_000_000]), 8_000_000 + 2048); // 6 MB x 4/3 + 2 KB
        assert!(check_limit(20 * 1024 * 1024, 20).is_ok());
        let e = check_limit(20 * 1024 * 1024 + 1, 20).unwrap_err();
        assert!(e.user_message().contains("20 MB"), "{}", e.user_message());
    }
}
