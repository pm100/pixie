use serde::{Deserialize, Serialize};
use std::path::Path;

pub type PhotoId = i64;
pub type FolderId = i64;

pub const SUPPORTED_EXTENSIONS: &[&str] =
    &["jpg", "jpeg", "png", "webp", "tif", "tiff", "heic", "heif"];

/// True for supported image files. Ignores macOS `._name` resource-fork files.
pub fn is_supported_image(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if name.starts_with("._") {
        return false;
    }
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| SUPPORTED_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PhotoMeta {
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// Unix seconds, EXIF local time treated as UTC.
    pub taken_at: Option<i64>,
    pub camera: Option<String>,
    pub gps_lat: Option<f64>,
    pub gps_lon: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Photo {
    pub id: PhotoId,
    pub folder_id: FolderId,
    pub path: String,
    pub size: i64,
    /// Unix milliseconds.
    pub mtime: i64,
    #[serde(flatten)]
    pub meta: PhotoMeta,
    pub missing: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Folder {
    pub id: FolderId,
    pub path: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    #[default]
    Newest,
    Oldest,
}

/// What the library view is showing: a sort order and an optional folder filter.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PhotoQuery {
    pub order: SortOrder,
    pub folder_id: Option<FolderId>,
}

/// All photos taken on one day (`day` is "YYYY-MM-DD"; `None` = no date).
/// `first_index` is the position of the group's first photo in the listing order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DayGroup {
    pub day: Option<String>,
    pub count: i64,
    pub first_index: i64,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum PixieError {
    #[error("io: {0}")]
    Io(String),
    #[error("database: {0}")]
    Db(String),
    #[error("image: {0}")]
    Image(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("busy: {0}")]
    Busy(String),
}

impl PixieError {
    /// Plain-language text safe to show to a non-technical user.
    pub fn user_message(&self) -> String {
        match self {
            PixieError::Io(_) => "Pixie couldn't read or write a file. Check that the folder or drive is connected.".into(),
            PixieError::Db(_) => "Pixie had a problem with its photo library data.".into(),
            PixieError::Image(_) => "This photo couldn't be opened. It may be damaged.".into(),
            PixieError::Unsupported(_) => "This kind of file isn't supported yet.".into(),
            PixieError::NotFound(_) => "Pixie couldn't find that folder or photo. It may have been moved or the drive disconnected.".into(),
            PixieError::Busy(_) => "A photo scan is already running. Please wait for it to finish.".into(),
        }
    }
}

impl From<std::io::Error> for PixieError {
    fn from(e: std::io::Error) -> Self {
        PixieError::Io(e.to_string())
    }
}

impl Serialize for PixieError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.user_message())
    }
}

pub type Result<T> = std::result::Result<T, PixieError>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn supports_known_extensions_case_insensitively() {
        for name in ["a.jpg", "a.JPEG", "a.png", "a.webp", "a.tif", "a.TIFF", "a.heic", "a.HEIF"] {
            assert!(is_supported_image(Path::new(name)), "{name}");
        }
    }

    #[test]
    fn rejects_other_files_and_macos_resource_forks() {
        for name in ["a.txt", "a.mp4", "noext", "._IMG_1.jpg", "dir/._x.heic"] {
            assert!(!is_supported_image(Path::new(name)), "{name}");
        }
    }

    #[test]
    fn handles_unicode_and_spaces() {
        assert!(is_supported_image(Path::new("Ferien 2024/Café ☕.JPG")));
    }

    #[test]
    fn sort_order_serializes_lowercase_and_defaults_to_newest() {
        assert_eq!(serde_json::to_string(&SortOrder::Oldest).unwrap(), "\"oldest\"");
        assert_eq!(serde_json::from_str::<SortOrder>("\"newest\"").unwrap(), SortOrder::Newest);
        assert_eq!(PhotoQuery::default().order, SortOrder::Newest);
        assert_eq!(PhotoQuery::default().folder_id, None);
    }

    #[test]
    fn busy_error_has_friendly_message() {
        let e = PixieError::Busy("scan".into());
        assert!(e.user_message().contains("already running"));
    }

    #[test]
    fn user_message_hides_internals() {
        let e = PixieError::Db("UNIQUE constraint failed: photos.path".into());
        assert!(!e.user_message().contains("UNIQUE"));
        let json = serde_json::to_string(&e).unwrap();
        assert!(!json.contains("UNIQUE"));
    }
}
