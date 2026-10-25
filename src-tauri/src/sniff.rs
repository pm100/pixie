//! The first-run look-around: counts the known places, looks for dense photo folders under the home folder, and checks
//! every other drive for pictures. The three stages run at the same time and report through one callback.

use crate::drives::DriveRoot;
use pixie_index::suggest::{count_media, look_further_with, probe_all_with, probe_root, Probe, ProbeOutcome, Suggestion, SuggestionKind};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

#[derive(Debug)]
pub enum SniffEvent {
    /// What is being sniffed now ("your home folder"), for the spinner.
    Looking(String),
    Count { path: String, n: usize, capped: bool },
    Found(Suggestion),
    /// Always the last event, once every stage has finished.
    Done,
}

/// How long each part may take.
pub struct Budgets {
    /// Counting one known place.
    pub count: Duration,
    /// The whole look under the home folder.
    pub home: Duration,
    /// Counting the pictures on one drive.
    pub drive: Duration,
    /// All the drives together: one that has not answered by then is given up on.
    pub drive_deadline: Duration,
    /// Extra tries to open a network drive (Windows reconnects a mapped drive on first use, and the first try can fail
    /// at once), and the pause between tries.
    pub retries: usize,
    pub retry_pause: Duration,
}

impl Budgets {
    pub fn app() -> Budgets {
        Budgets {
            count: Duration::from_secs(2),
            home: Duration::from_secs(10),
            drive: Duration::from_secs(10),
            drive_deadline: Duration::from_secs(25),
            retries: 3,
            retry_pause: Duration::from_millis(1500),
        }
    }
}

const MIN_FILES: usize = 50;
const CAP: usize = 10_000;

fn unreachable(root: &Path, name: &str) -> Suggestion {
    Suggestion {
        path: root.to_string_lossy().into_owned(),
        name: name.into(),
        kind: SuggestionKind::Further,
        ticked: false,
        note: Some("not reachable right now".into()),
        included_in: None,
    }
}

/// Can the drive's top folder be listed? Tried again after a pause when it cannot, because a mapped network drive that
/// was asleep or disconnected often answers on the second try.
fn reachable(root: &Path, retries: usize, pause: Duration) -> bool {
    for attempt in 0..=retries {
        if std::fs::read_dir(root).is_ok() {
            return true;
        }
        if attempt < retries {
            std::thread::sleep(pause);
        }
    }
    false
}

fn sniff_drives(drives: Vec<DriveRoot>, label: fn(&Path) -> Option<String>, b: &Budgets, tx: &mpsc::Sender<SniffEvent>) {
    let _ = tx.send(SniffEvent::Looking("your other drives".into()));
    // For each probe: its name, whether it is a network drive, and its root, to name one that does not answer.
    let mut meta: Vec<(String, bool, PathBuf)> = Vec::new();
    let (drive_budget, retries, pause) = (b.drive, b.retries, b.retry_pause);
    let probes: Vec<Probe> = drives
        .into_iter()
        .map(|d| {
            let letter = d.root.to_string_lossy().chars().next().unwrap_or('?');
            let fallback = if d.network { format!("Network drive {letter}:") } else { format!("Drive {letter}:") };
            meta.push((fallback.clone(), d.network, d.root.clone()));
            Probe {
                name: fallback.clone(),
                run: Box::new(move || {
                    if d.network && !reachable(&d.root, retries, pause) {
                        return Some(unreachable(&d.root, &fallback));
                    }
                    let name = match (d.network, label(&d.root)) {
                        (false, Some(volume)) => format!("{volume} ({letter}:)"),
                        _ => fallback,
                    };
                    probe_root(&d.root, &name, if d.network { "network drive" } else { "" }, drive_budget, MIN_FILES)
                }),
            }
        })
        .collect();
    probe_all_with(probes, b.drive_deadline, &mut |r| match r.outcome {
        ProbeOutcome::Found(s) => {
            let _ = tx.send(SniffEvent::Found(s));
        }
        ProbeOutcome::TimedOut => {
            // It never answered at all: for a network drive that is worth saying; a local drive is simply left out.
            if let Some((name, _, root)) = meta.iter().find(|(n, net, _)| *net && *n == r.name) {
                let _ = tx.send(SniffEvent::Found(unreachable(root, name)));
            }
        }
        ProbeOutcome::Nothing => {}
    });
}

/// Run the whole look-around, handing events to `emit` as they happen. `label` reads a drive's volume name (it may
/// block, so it is called on the drive's own thread).
pub fn run_sniff(
    known: Vec<Suggestion>,
    home: Option<PathBuf>,
    drives: Vec<DriveRoot>,
    label: fn(&Path) -> Option<String>,
    b: &Budgets,
    emit: &mut dyn FnMut(SniffEvent),
) {
    run_sniff_with(true, known, home, drives, label, b, emit)
}

/// `run_sniff`, with the choice of whether to count what is in the known places (the look-around still needs them, to
/// leave them out of what it finds). Counting is skipped when they were counted already.
pub fn run_sniff_with(
    count_known: bool,
    known: Vec<Suggestion>,
    home: Option<PathBuf>,
    drives: Vec<DriveRoot>,
    label: fn(&Path) -> Option<String>,
    b: &Budgets,
    emit: &mut dyn FnMut(SniffEvent),
) {
    let (tx, rx) = mpsc::channel::<SniffEvent>();
    std::thread::scope(|scope| {
        if count_known {
            let (tx, known) = (tx.clone(), &known);
            scope.spawn(move || {
                for s in known {
                    let c = count_media(Path::new(&s.path), b.count, CAP);
                    let _ = tx.send(SniffEvent::Count { path: s.path.clone(), n: c.n, capped: c.capped });
                }
            });
        }
        if let Some(home) = home {
            let (tx, known) = (tx.clone(), &known);
            scope.spawn(move || {
                let _ = tx.send(SniffEvent::Looking("your home folder".into()));
                look_further_with(&[home], known, b.home, MIN_FILES, &mut |s| {
                    let _ = tx.send(SniffEvent::Found(s));
                });
            });
        }
        if !drives.is_empty() {
            let tx = tx.clone();
            scope.spawn(move || sniff_drives(drives, label, b, &tx));
        }
        drop(tx);
        for event in rx {
            emit(event);
        }
    });
    emit(SniffEvent::Done);
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixie_index::suggest::{Suggestion, SuggestionKind};
    use std::fs;
    use std::time::Duration;

    fn jpgs(dir: &std::path::Path, n: usize) {
        fs::create_dir_all(dir).unwrap();
        for i in 0..n {
            fs::write(dir.join(format!("p{i:03}.jpg")), b"x").unwrap();
        }
    }

    fn quick() -> Budgets {
        Budgets {
            count: Duration::from_secs(5),
            home: Duration::from_secs(5),
            drive: Duration::from_secs(5),
            drive_deadline: Duration::from_secs(10),
            retries: 2,
            retry_pause: Duration::from_millis(20),
        }
    }

    fn known(path: &std::path::Path, name: &str) -> Suggestion {
        Suggestion { path: path.to_string_lossy().into_owned(), name: name.into(), kind: SuggestionKind::Known, ticked: true, note: None, included_in: None }
    }

    #[test]
    fn the_known_places_can_be_left_uncounted_and_the_search_still_ends() {
        let dir = tempfile::tempdir().unwrap();
        jpgs(&dir.path().join("Pictures"), 3);
        let k = known(&dir.path().join("Pictures"), "Pictures");
        let mut events = Vec::new();
        run_sniff_with(false, vec![k.clone()], None, Vec::new(), |_| None, &quick(), &mut |e| events.push(e));
        assert!(events.iter().all(|e| !matches!(e, SniffEvent::Count { .. })), "{events:?}");
        assert!(matches!(events.last(), Some(SniffEvent::Done)));
        let mut counted = Vec::new();
        run_sniff_with(true, vec![k], None, Vec::new(), |_| None, &quick(), &mut |e| counted.push(e));
        assert!(counted.iter().any(|e| matches!(e, SniffEvent::Count { n: 3, .. })));
    }

    fn run(known_places: Vec<Suggestion>, home: Option<std::path::PathBuf>, drives: Vec<DriveRoot>) -> Vec<SniffEvent> {
        let mut events = Vec::new();
        run_sniff(known_places, home, drives, |_| None, &quick(), &mut |e| events.push(e));
        events
    }

    fn found(events: &[SniffEvent]) -> Vec<&Suggestion> {
        events.iter().filter_map(|e| if let SniffEvent::Found(s) = e { Some(s) } else { None }).collect()
    }

    /// By hand, read-only: the whole first-run sniff on this machine, with the app's own budgets, printed with timings.
    /// `cargo test -p pixie-app --lib live_sniff -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn live_sniff_of_this_machine() {
        use pixie_index::suggest::{known_places, Places};
        let home = std::env::var_os("USERPROFILE").map(std::path::PathBuf::from);
        let at = |name: &str| home.as_ref().map(|h| h.join(name));
        let onedrive: Vec<std::path::PathBuf> = ["OneDrive", "OneDriveConsumer", "OneDriveCommercial"].iter().filter_map(|k| std::env::var_os(k)).map(Into::into).collect();
        let places = Places { pictures: at("Pictures"), videos: at("Videos"), documents: at("Documents"), downloads: at("Downloads"), desktop: at("Desktop"), home: home.clone(), onedrive };
        let list = known_places(&places);
        let system = std::env::var("SystemDrive").unwrap_or_default().to_uppercase();
        let others: Vec<DriveRoot> = crate::drives::drive_roots().into_iter().filter(|d| !d.root.to_string_lossy().to_uppercase().starts_with(&system)).collect();
        eprintln!("known: {:?}", list.iter().map(|s| s.name.as_str()).collect::<Vec<_>>());
        eprintln!("drives: {others:?}");
        let start = std::time::Instant::now();
        run_sniff(list, home, others, crate::drives::volume_label, &Budgets::app(), &mut |e| eprintln!("{:>5.1}s  {:?}", start.elapsed().as_secs_f32(), e));
    }

    #[test]
    fn it_counts_the_known_places_and_always_finishes_with_done() {
        let t = tempfile::tempdir().unwrap();
        jpgs(&t.path().join("Pictures"), 7);
        let events = run(vec![known(&t.path().join("Pictures"), "Pictures")], None, vec![]);
        assert!(events.iter().any(|e| matches!(e, SniffEvent::Count { n: 7, capped: false, .. })));
        assert!(matches!(events.last(), Some(SniffEvent::Done)));
        assert_eq!(events.iter().filter(|e| matches!(e, SniffEvent::Done)).count(), 1);
    }

    #[test]
    fn a_dense_folder_under_home_is_found() {
        let t = tempfile::tempdir().unwrap();
        jpgs(&t.path().join("Trips"), 60);
        let events = run(vec![], Some(t.path().to_path_buf()), vec![]);
        assert_eq!(found(&events).iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), vec!["Trips"]);
    }

    #[test]
    fn a_drive_with_pictures_is_offered_and_one_without_is_not() {
        let with = tempfile::tempdir().unwrap();
        let without = tempfile::tempdir().unwrap();
        jpgs(&with.path().join("2020"), 60);
        fs::write(without.path().join("notes.txt"), b"x").unwrap();
        let drives = vec![DriveRoot { root: with.path().to_path_buf(), network: false }, DriveRoot { root: without.path().to_path_buf(), network: false }];
        let events = run(vec![], None, drives);
        let f = found(&events);
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(f[0].path, with.path().to_string_lossy());
        assert!(!f[0].ticked);
    }

    #[test]
    fn a_network_drive_with_pictures_says_so() {
        let net = tempfile::tempdir().unwrap();
        jpgs(net.path(), 80);
        let events = run(vec![], None, vec![DriveRoot { root: net.path().to_path_buf(), network: true }]);
        let f = found(&events);
        assert_eq!(f.len(), 1);
        assert!(f[0].note.as_deref().unwrap().contains("network drive"), "{:?}", f[0].note);
    }

    #[test]
    fn a_network_drive_that_cannot_be_opened_is_listed_as_not_reachable_not_silently_dropped() {
        // Windows reconnects a mapped drive on first use and the first try can fail at once: that must not make the
        // drive vanish from the list.
        let gone = std::env::temp_dir().join("pixie-sniff-no-such-network-drive");
        let _ = fs::remove_dir_all(&gone);
        let events = run(vec![], None, vec![DriveRoot { root: gone.clone(), network: true }]);
        let f = found(&events);
        assert_eq!(f.len(), 1, "{events:?}");
        assert_eq!(f[0].note.as_deref(), Some("not reachable right now"));
        assert_eq!(f[0].path, gone.to_string_lossy());
    }

    #[test]
    fn a_network_drive_that_comes_up_on_the_second_try_is_offered_normally() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().join("share");
        let (r2, r3) = (root.clone(), root.clone());
        let staged = t.path().join("staged");
        jpgs(&staged, 60);
        // the share appears 30 ms in, after the first attempt has failed (retry pause is 20 ms); renamed into place so it is whole at once
        let creator = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            fs::rename(&staged, &r2).unwrap();
        });
        // (plenty of retries, so a busy machine that runs the first attempt late cannot make the test fail)
        let mut events = Vec::new();
        run_sniff(Vec::new(), None, vec![DriveRoot { root: r3, network: true }], |_| None, &Budgets { retries: 12, ..quick() }, &mut |e| events.push(e));
        creator.join().unwrap();
        let f = found(&events);
        assert_eq!(f.len(), 1, "{events:?}");
        assert!(f[0].note.as_deref().unwrap().starts_with("about"), "{:?}", f[0].note);
    }

    #[test]
    fn a_local_drive_that_cannot_be_opened_is_just_left_out() {
        let gone = std::env::temp_dir().join("pixie-sniff-no-such-local-drive");
        let _ = fs::remove_dir_all(&gone);
        assert!(found(&run(vec![], None, vec![DriveRoot { root: gone, network: false }])).is_empty());
    }

    #[test]
    fn the_stages_run_at_the_same_time_so_a_slow_one_does_not_delay_the_others() {
        // many known places to count, plus a drive: the drive's answer must not wait for the counts one after another
        let t = tempfile::tempdir().unwrap();
        let net = tempfile::tempdir().unwrap();
        jpgs(net.path(), 60);
        let mut places = Vec::new();
        for i in 0..5 {
            jpgs(&t.path().join(format!("P{i}")), 5);
            places.push(known(&t.path().join(format!("P{i}")), &format!("P{i}")));
        }
        let mut order = Vec::new();
        run_sniff(places, None, vec![DriveRoot { root: net.path().to_path_buf(), network: true }], |_| None, &quick(), &mut |e| {
            order.push(match e {
                SniffEvent::Count { .. } => 'c',
                SniffEvent::Found(_) => 'f',
                _ => '.',
            })
        });
        assert!(order.contains(&'f') && order.iter().filter(|c| **c == 'c').count() == 5);
    }
}
