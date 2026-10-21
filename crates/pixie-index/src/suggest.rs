//! Where a new user's photos probably are: well-known places first, then a bounded look around. Pure file-system
//! logic with no UI; the commands that call it live in the app.

use pixie_core::media_kind;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SuggestionKind {
    /// An operating-system or cloud-service photo place.
    Known,
    /// Found by looking around: always offered unticked.
    Further,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Suggestion {
    pub path: String,
    pub name: String,
    pub kind: SuggestionKind,
    pub ticked: bool,
    /// Why it looked likely ("412 photos and videos"); none for the well-known places.
    pub note: Option<String>,
    /// The name of the suggestion that already contains this folder: it is shown, but not added separately.
    pub included_in: Option<String>,
}

/// The operating system's folders, as the app learns them (any may be missing).
#[derive(Debug, Default, Clone)]
pub struct Places {
    pub pictures: Option<PathBuf>,
    pub videos: Option<PathBuf>,
    pub documents: Option<PathBuf>,
    pub downloads: Option<PathBuf>,
    pub desktop: Option<PathBuf>,
    pub home: Option<PathBuf>,
    /// OneDrive roots (personal and work); each may hold a `Pictures` folder.
    pub onedrive: Vec<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MediaCount {
    pub n: usize,
    /// Stopped at the cap or out of time: `n` is a lower bound.
    pub capped: bool,
}

/// Folders whose contents are never photo libraries (compared case-insensitively), besides anything hidden.
const SKIP: &[&str] = &[
    "appdata", "library", "applications", "node_modules", "windows", "program files", "program files (x86)",
    "programdata", "$recycle.bin", "system volume information", "target", "venv", "site-packages", "users", "recovery",
    "perflogs", "msocache", "windows.old", "$windows.~bt", "$winreagent", "boot",
];

/// Folders that never hold anyone's photos, wherever they are (compared case-insensitively), besides anything hidden.
/// Narrower than `SKIP`: a backup drive keeps its photos under `Users`, so a whole-drive check must still look there.
const JUNK: &[&str] = &["node_modules", "target", "venv", "site-packages", "__pycache__", "$recycle.bin", "system volume information"];

fn junk(name: &str) -> bool {
    name.starts_with('.') || JUNK.contains(&name.to_lowercase().as_str())
}

fn skipped(name: &str) -> bool {
    name.starts_with('.') || SKIP.contains(&name.to_lowercase().as_str())
}

fn text(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn known(path: &Path, name: &str, ticked: bool) -> Option<Suggestion> {
    path.is_dir().then(|| Suggestion { path: text(path), name: name.into(), kind: SuggestionKind::Known, ticked, note: None, included_in: None })
}

/// The well-known places that exist. Pictures, Videos and the OneDrive/Dropbox photo folders are ticked; Downloads and
/// Desktop are not. A folder inside an earlier suggestion carries `included_in` instead of being offered on its own.
pub fn known_places(p: &Places) -> Vec<Suggestion> {
    let mut out: Vec<Suggestion> = Vec::new();
    // The same folder can be reached two ways (Pictures redirected into OneDrive): the first name given is kept.
    let mut add = |s: Option<Suggestion>| {
        if let Some(s) = s.filter(|s| !out.iter().any(|o| Path::new(&o.path) == Path::new(&s.path))) {
            out.push(s);
        }
    };
    if let Some(d) = &p.pictures {
        add(known(d, "Pictures", true));
    }
    if let Some(d) = &p.videos {
        add(known(d, "Videos", true));
    }
    for root in &p.onedrive {
        add(known(&root.join("Pictures"), "OneDrive Pictures", true));
    }
    if let Some(h) = &p.home {
        add(known(&h.join("Dropbox").join("Camera Uploads"), "Dropbox Camera Uploads", true));
        add(known(&h.join("Dropbox").join("Photos"), "Dropbox Photos", true));
    }
    if let Some(d) = &p.pictures {
        for (sub, name) in [("iCloud Photos", "iCloud Photos"), ("Camera Roll", "Camera Roll"), ("Screenshots", "Screenshots")] {
            add(known(&d.join(sub), name, true));
        }
    }
    if let Some(d) = &p.documents {
        add(known(d, "Documents", false));
    }
    for root in &p.onedrive {
        add(known(&root.join("Documents"), "OneDrive Documents", false));
    }
    if let Some(d) = &p.downloads {
        add(known(d, "Downloads", false));
    }
    if let Some(d) = &p.desktop {
        add(known(d, "Desktop", false));
    }
    mark_nested(&mut out);
    out
}

/// Set `included_in` on every suggestion that lies inside another one.
fn mark_nested(v: &mut [Suggestion]) {
    let snapshot: Vec<(String, String)> = v.iter().map(|s| (s.path.clone(), s.name.clone())).collect();
    for s in v.iter_mut() {
        let me = Path::new(&s.path);
        s.included_in = snapshot.iter().find(|(p, _)| Path::new(p) != me && me.starts_with(p)).map(|(_, name)| name.clone());
    }
}

/// How many photos and videos are in `dir` (any depth), counting for at most `budget` and stopping at `cap`.
pub fn count_media(dir: &Path, budget: Duration, cap: usize) -> MediaCount {
    let start = Instant::now();
    let mut n = 0;
    for entry in WalkDir::new(dir).follow_links(false).into_iter().filter_map(Result::ok) {
        if start.elapsed() >= budget {
            return MediaCount { n, capped: true };
        }
        if entry.file_type().is_file() && media_kind(entry.path()).is_some() {
            n += 1;
            if n >= cap {
                return MediaCount { n, capped: true };
            }
        }
    }
    MediaCount { n, capped: false }
}

/// `count_media`, but not entering hidden or dependency folders (the `JUNK` list): a quick "are there pictures here
/// worth offering" for a whole drive, which must not burn its time inside `node_modules`.
pub fn count_media_skipping(dir: &Path, budget: Duration, cap: usize) -> MediaCount {
    let start = Instant::now();
    let mut n = 0;
    let walk = WalkDir::new(dir)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| e.depth() == 0 || !(e.file_type().is_dir() && junk(&e.file_name().to_string_lossy())));
    for entry in walk.filter_map(Result::ok) {
        if start.elapsed() >= budget {
            return MediaCount { n, capped: true };
        }
        if entry.file_type().is_file() && media_kind(entry.path()).is_some() {
            n += 1;
            if n >= cap {
                return MediaCount { n, capped: true };
            }
        }
    }
    MediaCount { n, capped: false }
}

/// Offer a whole drive (or share) when it holds at least `min_files` photos and videos, counted at any depth for at most
/// `budget`. Unticked; `kind_note` (such as "network drive") is added to the note.
pub fn probe_root(root: &Path, name: &str, kind_note: &str, budget: Duration, min_files: usize) -> Option<Suggestion> {
    let c = count_media_skipping(root, budget, 10_000);
    if c.n < min_files {
        return None;
    }
    let count = if c.capped { format!("{}+", with_commas(c.n)) } else { format!("about {}", with_commas(c.n)) };
    let note = if kind_note.is_empty() { format!("{count} photos and videos") } else { format!("{count} photos and videos · {kind_note}") };
    Some(Suggestion { path: text(root), name: name.into(), kind: SuggestionKind::Further, ticked: false, note: Some(note), included_in: None })
}

/// 10000 as "10,000".
fn with_commas(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// One drive to sniff: what to call it, and the work (run on its own thread, so a hung network share holds up nobody).
pub struct Probe {
    pub name: String,
    pub run: Box<dyn FnOnce() -> Option<Suggestion> + Send>,
}

#[derive(Debug)]
pub enum ProbeOutcome {
    Found(Suggestion),
    /// It answered and holds no pictures worth offering.
    Nothing,
    /// It did not answer in time.
    TimedOut,
}

#[derive(Debug)]
pub struct ProbeResult {
    pub name: String,
    pub outcome: ProbeOutcome,
}

/// Run every probe at once and hand each result to `on_result` as it arrives; whatever has not answered when `deadline`
/// passes is reported as timed out. The thread of a hung probe is left to finish on its own.
pub fn probe_all_with(probes: Vec<Probe>, deadline: Duration, on_result: &mut dyn FnMut(ProbeResult)) {
    let (tx, rx) = std::sync::mpsc::channel::<(usize, Option<Suggestion>)>();
    let names: Vec<String> = probes.iter().map(|p| p.name.clone()).collect();
    for (i, probe) in probes.into_iter().enumerate() {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send((i, (probe.run)()));
        });
    }
    drop(tx);
    let end = Instant::now() + deadline;
    let mut answered = vec![false; names.len()];
    while answered.iter().any(|a| !a) {
        match rx.recv_timeout(end.saturating_duration_since(Instant::now())) {
            Ok((i, found)) => {
                answered[i] = true;
                on_result(ProbeResult { name: names[i].clone(), outcome: found.map_or(ProbeOutcome::Nothing, ProbeOutcome::Found) });
            }
            Err(_) => break, // out of time, or the remaining probes died without answering
        }
    }
    for (i, name) in names.iter().enumerate().filter(|(i, _)| !answered[*i]) {
        let _ = i;
        on_result(ProbeResult { name: name.clone(), outcome: ProbeOutcome::TimedOut });
    }
}

/// `probe_all_with`, returning the results in the order the probes were given.
pub fn probe_all(probes: Vec<Probe>, deadline: Duration) -> Vec<ProbeResult> {
    let order: Vec<String> = probes.iter().map(|p| p.name.clone()).collect();
    let mut out: Vec<ProbeResult> = Vec::new();
    probe_all_with(probes, deadline, &mut |r| out.push(r));
    out.sort_by_key(|r| order.iter().position(|n| *n == r.name));
    out
}

/// Folders worth offering beyond the well-known places: directories one to three levels under each root that hold at
/// least `min_files` photos and videos (counted at any depth), the topmost such folder standing for what is below it.
/// Hidden, system, application and dependency folders are not entered, nor are folders inside `known` suggestions; a
/// folder that contains a known suggestion is never returned. Always unticked, and bounded by `budget` overall.
pub fn look_further(roots: &[PathBuf], known: &[Suggestion], budget: Duration, min_files: usize) -> Vec<Suggestion> {
    let mut out = Vec::new();
    look_further_with(roots, known, budget, min_files, &mut |s| out.push(s));
    out
}

/// `look_further`, handing each candidate to `on_found` the moment it is found, so a screen can show them as they come.
pub fn look_further_with(roots: &[PathBuf], known: &[Suggestion], budget: Duration, min_files: usize, on_found: &mut dyn FnMut(Suggestion)) {
    let start = Instant::now();
    let known_paths: Vec<&Path> = known.iter().map(|k| Path::new(&k.path)).collect();
    for root in roots {
        let mut it = WalkDir::new(root).min_depth(1).max_depth(3).follow_links(false).into_iter();
        while let Some(entry) = it.next() {
            if start.elapsed() >= budget {
                return;
            }
            let Ok(entry) = entry else { continue };
            if !entry.file_type().is_dir() {
                continue;
            }
            let path = entry.path();
            if skipped(&entry.file_name().to_string_lossy()) || known_paths.iter().any(|k| path.starts_with(k)) {
                it.skip_current_dir();
                continue;
            }
            if known_paths.iter().any(|k| k.starts_with(path)) {
                continue; // contains a known suggestion: not offered, but look inside
            }
            let left = budget.saturating_sub(start.elapsed());
            if count_media(path, left.min(Duration::from_millis(200)), min_files).n < min_files {
                continue;
            }
            let total = count_media(path, left.min(Duration::from_millis(500)), 10_000);
            let n = if total.capped && total.n >= 10_000 { "10,000+".to_string() } else { total.n.to_string() };
            on_found(Suggestion {
                path: text(path),
                name: entry.file_name().to_string_lossy().into_owned(),
                kind: SuggestionKind::Further,
                ticked: false,
                note: Some(format!("{n} photos and videos")),
                included_in: None,
            });
            it.skip_current_dir(); // what is below stands under this one
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::Duration;

    fn touch(dir: &Path, name: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join(name), b"x").unwrap();
    }

    fn jpgs(dir: &Path, n: usize) {
        for i in 0..n {
            touch(dir, &format!("p{i:03}.jpg"));
        }
    }

    fn by_name<'a>(v: &'a [Suggestion], name: &str) -> &'a Suggestion {
        v.iter().find(|s| s.name == name).unwrap_or_else(|| panic!("no suggestion named {name}: {v:?}"))
    }

    #[test]
    fn known_places_lists_only_existing_dirs_and_ticks_the_confident_ones() {
        let t = tempfile::tempdir().unwrap();
        let home = t.path();
        for d in ["Pictures", "Downloads", "Desktop", "OneDrive/Pictures"] {
            fs::create_dir_all(home.join(d)).unwrap();
        }
        let p = Places {
            pictures: Some(home.join("Pictures")),
            videos: Some(home.join("Videos")), // does not exist
            documents: None,
            downloads: Some(home.join("Downloads")),
            desktop: Some(home.join("Desktop")),
            home: Some(home.to_path_buf()),
            onedrive: vec![home.join("OneDrive")],
        };
        let v = known_places(&p);
        assert!(v.iter().all(|s| s.kind == SuggestionKind::Known));
        assert!(by_name(&v, "Pictures").ticked);
        assert!(by_name(&v, "OneDrive Pictures").ticked);
        assert!(!by_name(&v, "Downloads").ticked);
        assert!(!by_name(&v, "Desktop").ticked);
        assert!(v.iter().all(|s| s.name != "Videos"), "a missing folder is not suggested");
    }

    #[test]
    fn a_folder_inside_another_suggestion_is_included_not_offered() {
        let t = tempfile::tempdir().unwrap();
        let home = t.path();
        fs::create_dir_all(home.join("Pictures/Camera Roll")).unwrap();
        let p = Places { pictures: Some(home.join("Pictures")), home: Some(home.to_path_buf()), ..Default::default() };
        let v = known_places(&p);
        assert_eq!(by_name(&v, "Camera Roll").included_in.as_deref(), Some("Pictures"));
        assert_eq!(by_name(&v, "Pictures").included_in, None);
    }

    #[test]
    fn documents_and_onedrive_documents_are_offered_unticked() {
        let t = tempfile::tempdir().unwrap();
        let home = t.path();
        fs::create_dir_all(home.join("Documents")).unwrap();
        fs::create_dir_all(home.join("OneDrive/Documents")).unwrap();
        let p = Places { documents: Some(home.join("Documents")), home: Some(home.to_path_buf()), onedrive: vec![home.join("OneDrive")], ..Default::default() };
        let v = known_places(&p);
        assert!(!by_name(&v, "Documents").ticked);
        assert!(!by_name(&v, "OneDrive Documents").ticked);
        assert!(v.iter().all(|s| s.kind == SuggestionKind::Known));
    }

    #[test]
    fn look_further_with_reports_each_candidate_as_it_is_found() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        jpgs(&root.join("Alpha"), 60);
        jpgs(&root.join("Beta"), 60);
        let mut seen = Vec::new();
        look_further_with(&[root.to_path_buf()], &[], Duration::from_secs(10), 50, &mut |s| seen.push(s.name));
        seen.sort();
        assert_eq!(seen, vec!["Alpha", "Beta"]);
        let all = look_further(&[root.to_path_buf()], &[], Duration::from_secs(10), 50);
        assert_eq!(all.len(), 2, "look_further still returns the same list");
    }

    #[test]
    fn look_further_does_not_enter_system_folders_at_a_drive_root() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        for skipped in ["Users", "Recovery", "PerfLogs", "Windows.old", "$Recycle.Bin", "Program Files"] {
            jpgs(&root.join(skipped), 60);
        }
        jpgs(&root.join("Family Photos"), 60);
        let v = look_further(&[root.to_path_buf()], &[], Duration::from_secs(10), 50);
        assert_eq!(v.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), vec!["Family Photos"]);
    }

    #[test]
    fn probe_root_offers_a_drive_that_holds_pictures_and_skips_one_that_does_not() {
        let t = tempfile::tempdir().unwrap();
        jpgs(&t.path().join("2019/Trip"), 30);
        jpgs(&t.path().join("2020"), 30);
        let found = probe_root(t.path(), "Drive E:", "", Duration::from_secs(10), 50).expect("60 photos is enough");
        assert_eq!((found.name.as_str(), found.kind, found.ticked), ("Drive E:", SuggestionKind::Further, false));
        assert_eq!(found.note.as_deref(), Some("about 60 photos and videos"));
        let empty = tempfile::tempdir().unwrap();
        touch(empty.path(), "readme.txt");
        assert!(probe_root(empty.path(), "Drive F:", "", Duration::from_secs(10), 50).is_none());
    }

    #[test]
    fn count_media_skipping_leaves_out_junk_but_not_system_looking_folders_that_may_hold_a_backup() {
        let t = tempfile::tempdir().unwrap();
        jpgs(&t.path().join("Photos"), 3);
        jpgs(&t.path().join("Users/old-pc/Pictures"), 4); // a backup drive keeps its photos under Users
        for junk in ["node_modules/pkg", ".git/objects", "target/debug", "$RECYCLE.BIN/s-1"] {
            jpgs(&t.path().join(junk), 60);
        }
        assert_eq!(count_media_skipping(t.path(), Duration::from_secs(5), 1000).n, 7);
        assert_eq!(count_media(t.path(), Duration::from_secs(5), 1000).n, 247, "the plain count still sees everything");
    }

    #[test]
    fn a_drive_full_of_dependency_folders_has_no_pictures_worth_offering() {
        let t = tempfile::tempdir().unwrap();
        jpgs(&t.path().join("node_modules/pkg"), 100);
        assert!(probe_root(t.path(), "Drive F:", "", Duration::from_secs(5), 50).is_none());
    }

    #[test]
    fn probe_root_writes_big_counts_with_thousands_separators() {
        let t = tempfile::tempdir().unwrap();
        jpgs(t.path(), 1234);
        let found = probe_root(t.path(), "Drive E:", "", Duration::from_secs(20), 50).unwrap();
        assert_eq!(found.note.as_deref(), Some("about 1,234 photos and videos"));
        assert_eq!(with_commas(10_000), "10,000");
        assert_eq!(with_commas(7), "7");
        assert_eq!(with_commas(1_234_567), "1,234,567");
    }

    #[test]
    fn probe_root_adds_the_kind_of_drive_to_the_note() {
        let t = tempfile::tempdir().unwrap();
        jpgs(t.path(), 80);
        let found = probe_root(t.path(), "Network drive P:", "network drive", Duration::from_secs(10), 50).unwrap();
        assert_eq!(found.note.as_deref(), Some("about 80 photos and videos · network drive"));
    }

    #[test]
    fn probe_all_collects_in_order_and_gives_up_on_a_probe_that_hangs() {
        let quick = |name: &'static str| -> Probe { Probe { name: name.into(), run: Box::new(move || Some(Suggestion {
            path: name.into(), name: name.into(), kind: SuggestionKind::Further, ticked: false, note: None, included_in: None,
        })) } };
        let hangs: Probe = Probe { name: "P:".into(), run: Box::new(|| {
            std::thread::sleep(Duration::from_secs(5));
            None
        }) };
        let started = std::time::Instant::now();
        let out = probe_all(vec![quick("E:"), hangs, quick("F:")], Duration::from_millis(300));
        assert!(started.elapsed() < Duration::from_secs(3), "a hung drive must not hold the others up");
        assert_eq!(out.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), vec!["E:", "P:", "F:"]);
        assert!(matches!(out[0].outcome, ProbeOutcome::Found(_)));
        assert!(matches!(out[1].outcome, ProbeOutcome::TimedOut));
        assert!(matches!(out[2].outcome, ProbeOutcome::Found(_)));
    }

    #[test]
    fn probe_all_reports_a_probe_that_found_nothing() {
        let out = probe_all(vec![Probe { name: "E:".into(), run: Box::new(|| None) }], Duration::from_secs(1));
        assert!(matches!(out[0].outcome, ProbeOutcome::Nothing));
    }

    #[test]
    fn the_same_folder_reached_two_ways_is_listed_once() {
        // Windows can redirect Pictures into OneDrive, so the OS Pictures folder and OneDrive's are one folder.
        let t = tempfile::tempdir().unwrap();
        let home = t.path();
        fs::create_dir_all(home.join("OneDrive/Pictures")).unwrap();
        let p = Places {
            pictures: Some(home.join("OneDrive/Pictures")),
            home: Some(home.to_path_buf()),
            onedrive: vec![home.join("OneDrive")],
            ..Default::default()
        };
        let v = known_places(&p);
        let one = home.join("OneDrive").join("Pictures");
        assert_eq!(v.iter().filter(|s| Path::new(&s.path) == one).count(), 1, "{v:?}"); // compared by components, not text
        assert_eq!(v[0].name, "Pictures", "the first name given wins");
    }

    #[test]
    fn count_media_counts_media_files_only_and_caps() {
        let t = tempfile::tempdir().unwrap();
        jpgs(t.path(), 5);
        touch(t.path(), "notes.txt");
        touch(&t.path().join("sub"), "clip.mp4");
        let all = count_media(t.path(), Duration::from_secs(5), 1000);
        assert_eq!((all.n, all.capped), (6, false)); // 5 jpg + 1 mp4 in a subfolder; the txt is not media
        let few = count_media(t.path(), Duration::from_secs(5), 3);
        assert_eq!((few.n, few.capped), (3, true));
    }

    #[test]
    fn count_media_stops_at_the_time_budget() {
        let t = tempfile::tempdir().unwrap();
        jpgs(t.path(), 50);
        let c = count_media(t.path(), Duration::ZERO, 1000);
        assert!(c.capped, "out of time reads as capped, not as an exact count");
        assert!(c.n < 50);
    }

    #[test]
    fn look_further_finds_dense_media_folders_and_skips_the_rest() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        jpgs(&root.join("Holiday"), 60);
        jpgs(&root.join("Few"), 5);
        for skipped in ["node_modules/x", ".hidden", "AppData/Local", "Library/Caches"] {
            jpgs(&root.join(skipped), 60);
        }
        jpgs(&root.join("Pictures"), 60); // already a known suggestion
        let known = vec![Suggestion {
            path: root.join("Pictures").to_string_lossy().into_owned(),
            name: "Pictures".into(),
            kind: SuggestionKind::Known,
            ticked: true,
            note: None,
            included_in: None,
        }];
        let v = look_further(&[root.to_path_buf()], &known, Duration::from_secs(10), 50);
        let names: Vec<&str> = v.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["Holiday"]);
        assert!(v[0].kind == SuggestionKind::Further && !v[0].ticked);
        assert_eq!(v[0].note.as_deref(), Some("60 photos and videos"));
    }

    #[test]
    fn look_further_offers_the_topmost_dense_folder_within_three_levels() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        jpgs(&root.join("a/b/c/d"), 60); // the media is four levels down: its depth-1 ancestor stands for it
        let v = look_further(&[root.to_path_buf()], &[], Duration::from_secs(10), 50);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].path, root.join("a").to_string_lossy());
    }

    #[test]
    fn look_further_never_returns_a_folder_containing_a_known_suggestion() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        jpgs(&root.join("outer/Pictures"), 60);
        let known = vec![Suggestion {
            path: root.join("outer/Pictures").to_string_lossy().into_owned(),
            name: "Pictures".into(),
            kind: SuggestionKind::Known,
            ticked: true,
            note: None,
            included_in: None,
        }];
        let v = look_further(&[root.to_path_buf()], &known, Duration::from_secs(10), 50);
        assert!(v.is_empty(), "outer holds the known Pictures folder, so offering it would double-add: {v:?}");
    }
}
