//! Tags the user puts on photos (Auto tags, made by the analysis, are separate).
use serde::Serialize;

/// The longest a tag name may be, in characters.
pub const MAX_TAG_LEN: usize = 60;

/// A tag name as it is stored: spaces trimmed and collapsed, one leading `#` dropped. `None` when nothing is left or it is too long.
pub fn normalize_tag(raw: &str) -> Option<String> {
    let t = raw.trim();
    let t = t.strip_prefix('#').unwrap_or(t);
    let name = t.split_whitespace().collect::<Vec<_>>().join(" ");
    (!name.is_empty() && name.chars().count() <= MAX_TAG_LEN).then_some(name)
}

/// A tag and how many photos (that are listed in Browse) have it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UserTagCount {
    pub name: String,
    pub count: i64,
}

/// A tag and how many of some chosen photos have it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TagUse {
    pub name: String,
    pub on: i64,
}

/// What the browse tree can show only: the picked photos, or the photos with one tag.
#[derive(Debug, Clone, PartialEq)]
pub enum UserTagFilter {
    Picked,
    Tag(String),
}

impl UserTagFilter {
    /// `"picked"` or `"tag:<name>"`: how the filter travels between the app and the window.
    pub fn key(&self) -> String {
        match self {
            UserTagFilter::Picked => "picked".to_string(),
            UserTagFilter::Tag(t) => format!("tag:{t}"),
        }
    }

    pub fn parse(key: &str) -> Option<UserTagFilter> {
        if key == "picked" {
            return Some(UserTagFilter::Picked);
        }
        key.strip_prefix("tag:").filter(|t| !t.is_empty()).map(|t| UserTagFilter::Tag(t.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_names_ignore_case_spaces_and_hash() {
        assert_eq!(normalize_tag("  #Holiday  2024 ").as_deref(), Some("Holiday 2024"));
        assert_eq!(normalize_tag("a\t b"), Some("a b".to_string()));
        assert_eq!(normalize_tag("  # "), None);
        assert_eq!(normalize_tag(""), None);
        assert_eq!(normalize_tag(&"x".repeat(60)).as_deref(), Some("x".repeat(60).as_str()));
        assert_eq!(normalize_tag(&"x".repeat(61)), None);
    }

    #[test]
    fn the_filter_key_round_trips() {
        for f in [UserTagFilter::Picked, UserTagFilter::Tag("Holiday 2024".into())] {
            assert_eq!(UserTagFilter::parse(&f.key()), Some(f));
        }
        assert_eq!(UserTagFilter::Picked.key(), "picked");
        assert_eq!(UserTagFilter::Tag("x".into()).key(), "tag:x");
        assert_eq!(UserTagFilter::parse("nonsense"), None);
        assert_eq!(UserTagFilter::parse("tag:"), None);
    }
}
