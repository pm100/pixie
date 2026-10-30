use serde::{Deserialize, Serialize};
use std::path::Path;

pub mod camera;
pub mod dates;
pub mod mail;
pub mod usertags;
pub use camera::normalize_camera;
pub use dates::{DateRules, DateSummary};
pub use mail::*;
pub use usertags::*;

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

pub const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mov", "m4v"];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaKind {
    #[default]
    Photo,
    Video,
}

fn has_extension_in(path: &Path, list: &[&str]) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if name.starts_with("._") {
        return false;
    }
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| list.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// True for supported video files. Ignores macOS `._name` resource-fork files.
pub fn is_supported_video(path: &Path) -> bool {
    has_extension_in(path, VIDEO_EXTENSIONS)
}

/// Photo or video, by extension; `None` for anything Pixie does not index.
pub fn media_kind(path: &Path) -> Option<MediaKind> {
    if is_supported_image(path) {
        Some(MediaKind::Photo)
    } else if is_supported_video(path) {
        Some(MediaKind::Video)
    } else {
        None
    }
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
    #[serde(default)]
    pub kind: MediaKind,
    /// Videos only.
    #[serde(default)]
    pub duration_ms: Option<i64>,
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

/// A watched folder with the number of live (not missing) items in it, for the Settings page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FolderInfo {
    pub id: FolderId,
    pub path: String,
    pub count: i64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    #[default]
    Newest,
    Oldest,
}

/// A date window whose `since` is this value selects the items that have no date at all (an undated item never falls
/// inside a real window). It is the lowest whole number a browser can hold exactly, so the window passes through the
/// UI unchanged.
pub const UNDATED: i64 = -9_007_199_254_740_991;

/// What the library view is showing: a sort order and an optional folder filter.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PhotoQuery {
    pub order: SortOrder,
    pub folder_id: Option<FolderId>,
    pub kind: Option<MediaKind>,
    pub bounds: Option<Bounds>,
    /// Snapshot: only items with `id <= max_id`. New items get higher ids, so the listing (and every
    /// position in it) stays put while a scan adds photos.
    pub max_id: Option<i64>,
    /// Only items taken at or after this time (unix seconds). Undated items never match, unless this is `UNDATED`, which
    /// matches only them.
    pub since: Option<i64>,
    /// Only items taken before this time (unix seconds, exclusive). Undated items never match.
    pub until: Option<i64>,
    /// Only items in the current search result (see `Db::set_search_hits`).
    pub search: bool,
}

/// All photos taken on one day (`day` is "YYYY-MM-DD"; `None` = no date).
/// `first_index` is the position of the group's first photo in the listing order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DayGroup {
    pub day: Option<String>,
    pub count: i64,
    pub first_index: i64,
}

/// A latitude/longitude rectangle. `west > east` means it crosses the antimeridian.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Bounds {
    pub south: f64,
    pub west: f64,
    pub north: f64,
    pub east: f64,
}

impl Bounds {
    pub fn wraps(&self) -> bool {
        self.west > self.east
    }

    /// Clamp to valid coordinates. `None` for non-finite values or `south > north`.
    /// A rectangle 360 degrees wide or more becomes the full longitude range.
    pub fn normalized(&self) -> Option<Bounds> {
        let all = [self.south, self.west, self.north, self.east];
        if all.iter().any(|v| !v.is_finite()) || self.south > self.north {
            return None;
        }
        let lat = |v: f64| v.clamp(-90.0, 90.0);
        let lon = |v: f64| v.clamp(-180.0, 180.0);
        let (south, north) = (lat(self.south), lat(self.north));
        let width = if self.west <= self.east { self.east - self.west } else { self.east + 360.0 - self.west };
        if width >= 360.0 {
            return Some(Bounds { south, west: -180.0, north, east: 180.0 });
        }
        Some(Bounds { south, west: lon(self.west), north, east: lon(self.east) })
    }
}

/// Where one photo or video was taken, for the map.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeoPoint {
    pub id: PhotoId,
    pub lat: f64,
    pub lon: f64,
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
    /// Input the user can fix; the text is written for them.
    #[error("{0}")]
    Invalid(String),
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
            PixieError::Invalid(m) => m.clone(),
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
    fn media_kind_detects_photos_videos_and_ignores_the_rest() {
        assert_eq!(media_kind(Path::new("a.JPG")), Some(MediaKind::Photo));
        assert_eq!(media_kind(Path::new("a.heic")), Some(MediaKind::Photo));
        for n in ["a.mp4", "a.MOV", "dir/b.M4V", "Café ☕.Mp4"] {
            assert_eq!(media_kind(Path::new(n)), Some(MediaKind::Video), "{n}");
        }
        for n in ["a.avi", "a.mkv", "a.txt", "noext", "._IMG_1.mp4", "dir/._x.mov"] {
            assert_eq!(media_kind(Path::new(n)), None, "{n}");
        }
        assert!(is_supported_video(Path::new("x.mp4")));
        assert!(!is_supported_video(Path::new("x.jpg")));
    }

    #[test]
    fn media_kind_serializes_lowercase_and_photos_are_the_default() {
        assert_eq!(serde_json::to_string(&MediaKind::Video).unwrap(), "\"video\"");
        assert_eq!(serde_json::from_str::<MediaKind>("\"photo\"").unwrap(), MediaKind::Photo);
        assert_eq!(PhotoMeta::default().kind, MediaKind::Photo);
        assert_eq!(PhotoMeta::default().duration_ms, None);
        assert_eq!(PhotoQuery::default().kind, None);
    }

    #[test]
    fn bounds_normalize_clamps_wraps_and_rejects_nonsense() {
        let b = |s, w, n, e| Bounds { south: s, west: w, north: n, east: e };
        assert_eq!(b(10.0, 20.0, 30.0, 40.0).normalized(), Some(b(10.0, 20.0, 30.0, 40.0)));
        // latitudes clamp to the poles, longitudes to +-180
        assert_eq!(b(-95.0, -200.0, 95.0, 200.0).normalized(), Some(b(-90.0, -180.0, 90.0, 180.0)));
        // a view 360 degrees or wider means every longitude
        assert_eq!(b(0.0, -180.0, 10.0, 180.0).normalized(), Some(b(0.0, -180.0, 10.0, 180.0)));
        // crossing the antimeridian keeps west > east
        let w = b(0.0, 170.0, 10.0, -170.0).normalized().unwrap();
        assert!(w.wraps());
        assert!(!b(0.0, -10.0, 10.0, 10.0).wraps());
        // nonsense is rejected
        assert_eq!(b(f64::NAN, 0.0, 1.0, 1.0).normalized(), None);
        assert_eq!(b(0.0, 0.0, f64::INFINITY, 1.0).normalized(), None);
        assert_eq!(b(20.0, 0.0, 10.0, 1.0).normalized(), None); // south > north
    }

    #[test]
    fn geo_point_and_query_defaults() {
        assert_eq!(PhotoQuery::default().bounds, None);
        let p = GeoPoint { id: 3, lat: 1.5, lon: -2.5 };
        assert_eq!(serde_json::to_string(&p).unwrap(), r#"{"id":3,"lat":1.5,"lon":-2.5}"#);
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

/// A person found in the photos, for the nav: `count` is the number of photos they are in, `face_id` the face to
/// show as their picture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersonInfo {
    pub id: i64,
    pub name: Option<String>,
    pub count: i64,
    pub face_id: Option<i64>,
}

/// A face in one photo. The box is a fraction (0..1) of the upright image.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FaceInfo {
    pub id: i64,
    pub person_id: Option<i64>,
    pub person_name: Option<String>,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// A face as the detector reports it, ready to store.
#[derive(Debug, Clone, PartialEq)]
pub struct NewFace {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub score: f32,
    pub embedding: Vec<f32>,
}

/// A person for the Faces tab: everyone, including unnamed people seen once and (on request) hidden ones.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FacePerson {
    pub id: i64,
    pub name: Option<String>,
    pub hidden: bool,
    /// The user has named or accepted it: its faces are fixed points for grouping the faces that are not confirmed.
    pub confirmed: bool,
    pub photos: i64,
    pub faces: i64,
    /// Faces that joined after it was confirmed, which the user has not looked at yet.
    pub unchecked: i64,
    /// The share of its face pairs that look like different people, or None when not measured.
    pub mixed: Option<f32>,
    /// It looks like more than one person (see `MIXED_ABOVE`): the card says so and offers to sort it out.
    pub looks_mixed: bool,
    /// Faces that show how it looks (the most typical first, then ones unlike those), not just its best faces.
    pub samples: Vec<i64>,
    /// For a group that is not confirmed yet: the confirmed person it looks most like, if any.
    pub suggestion: Option<Suggestion>,
}

/// Which people the People tab lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FaceView {
    /// Nobody has named them yet.
    Groups,
    /// Named people.
    People,
    /// Hidden from the nav and from the other lists.
    Hidden,
}

/// How a person's faces are ordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FaceOrder {
    /// The clearest first.
    Best,
    /// The ones least like the rest first, which is where the wrongly grouped faces are.
    Doubtful,
    /// Only faces that joined after the person was confirmed and have not been checked, the most doubtful first.
    Unchecked,
}

/// "This looks like <person>", with how sure (the mean similarity of its closest faces, 0 to 1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Suggestion {
    pub person_id: i64,
    pub name: Option<String>,
    pub score: f32,
}

/// A face that belongs to no group, with who it looks like.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnsortedFace {
    pub tile: FaceTile,
    pub suggestion: Option<Suggestion>,
}

/// What the People tab's sections hold.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct FaceOverview {
    /// Groups waiting to be named.
    pub groups: i64,
    /// Confirmed people.
    pub people: i64,
    /// Faces in no group.
    pub unsorted: i64,
    /// Groups the user hid.
    pub hidden: i64,
    /// Faces that joined confirmed people since the user last looked.
    pub unchecked: i64,
    /// Changes whenever who-is-who changes (see `PeopleStatus::rev`).
    pub rev: i64,
}

/// What a regrouping pass did.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TidyReport {
    /// Faces that were open to regrouping.
    pub faces: usize,
    /// Groups now (not confirmed).
    pub groups: usize,
    /// Faces left in no group.
    pub unsorted: usize,
    /// Faces that clearly looked like a confirmed person and joined them, to be checked.
    pub attached: usize,
}

/// Copies of the same file: every live photo whose contents hash to `hash`, the one to keep first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DupGroup {
    pub hash: String,
    pub photos: Vec<Photo>,
    /// The copy Browse shows: the one the user chose, else the oldest.
    pub keeper: PhotoId,
}

/// How far the background content hashing has got: `done` of `total` live items are hashed or known to have no
/// possible copy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DupStatus {
    pub done: i64,
    pub total: i64,
    pub groups: i64,
    /// Files that are extra copies (all but one of each group), and the space they take.
    pub extra_files: i64,
    pub extra_bytes: i64,
    /// Live items Browse hides because another copy is shown instead.
    pub hidden: i64,
    /// The user paused the background work (the same Pause as for Auto tags and faces).
    pub paused: bool,
}

/// How far the analysis has got for some folders, counted directly from their photos (so it does not depend on what
/// else is in the library, or on hidden duplicates shrinking the library's totals). Photos only: videos are not here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadProgress {
    /// Every live photo in the folders, hidden duplicates included.
    pub photos: i64,
    /// Those Browse shows (not hidden extra copies): what Auto tags and faces work on.
    pub shown: i64,
    /// Photos whose file has been read and hashed (or found unreadable).
    pub hashed: i64,
    /// Shown photos with their Auto tags vector.
    pub tagged: i64,
    /// Shown photos scanned for faces.
    pub scanned: i64,
    /// Faces found in the shown photos.
    pub faces: i64,
    /// Extra copies in the sets of identical files that include one of these photos.
    pub duplicates: i64,
    /// Photos a walk found in the folders that the analysis has not stored yet (they are not in `photos` until it has).
    #[serde(default)]
    pub pending: i64,
    /// The same for videos.
    #[serde(default)]
    pub pending_videos: i64,
}

/// What the left-hand tree shows beside its entries under the filters in force. Each section is counted with every
/// filter except its own, so choosing a folder narrows People, Auto tags and Trips but not the Folders list.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct FacetCounts {
    pub photos: i64,
    pub videos: i64,
    pub folders: Vec<(FolderId, i64)>,
    pub tags: Vec<(String, i64)>,
    /// Different photos in each group of tags asked for (a category), in the order asked.
    pub tag_groups: Vec<i64>,
    /// The name of each of those groups (the category), in the same order.
    pub tag_group_names: Vec<String>,
    pub people: Vec<(i64, i64)>,
    /// Items in each trip window asked for, in the order asked.
    pub trips: Vec<i64>,
    /// Items taken since the `recent_since` asked for (the Last 30 days row); 0 when none was asked for.
    pub recent: i64,
    /// Each camera (by its tidied name, see `normalize_camera`) with its item count, most first.
    pub cameras: Vec<(String, i64)>,
    /// Items with no position, under every filter except the map area and the "no location" choice itself.
    pub no_location: i64,
}

/// One face to look at: its id, the photo it is in (with that photo's version, to show it) and the face's box,
/// as fractions (0..1) of the upright photo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FaceTile {
    pub id: i64,
    pub photo_id: i64,
    pub mtime: i64,
    pub size: i64,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}
