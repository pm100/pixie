//! What each background source says about itself, as updates for the activity registry. Pure: the sources are read where they
//! already report (see `lib.rs`), and nothing here does I/O.

use pixie_analysis::people_service::PeopleStatus;
use pixie_analysis::service::AnalysisStatus;
use pixie_core::activity::{Activity, CurrentItem, RunState, Snapshot, TaskKind, TaskUpdate};
use pixie_index::cloud::CloudCounts;
use pixie_index::copier::{Phase as CopyPhase, Progress as CopyProgress};
use pixie_index::scanner::ScanProgress;
use pixie_index::working::{Work, WorkMeter};

/// An update with only counts to say.
fn counted(kind: TaskKind, state: RunState, done: u64, total: Option<u64>) -> TaskUpdate {
    TaskUpdate { kind, state, done, total, bytes_done: 0, bytes_total: None, note: None }
}

fn count(n: impl TryInto<u64>) -> u64 {
    n.try_into().unwrap_or(0)
}

/// Finished when nothing is left (a pause does not un-finish it), else paused by the person, else running.
fn state_of(done: u64, total: u64, paused: bool) -> RunState {
    if done >= total {
        RunState::Done
    } else if paused {
        RunState::Paused
    } else {
        RunState::Running
    }
}

/// A model that has to be fetched first: the row for the download, or for the failure, or its end once it is not being fetched or
/// failing any more (so it never outlives the download).
fn models_updates(kind: TaskKind, models: &str, model_done: u64, model_total: u64, error: &Option<String>) -> Vec<TaskUpdate> {
    match models {
        "downloading" => vec![TaskUpdate { bytes_done: model_done, bytes_total: Some(model_total), ..counted(kind, RunState::Running, 0, None) }],
        "error" => vec![TaskUpdate { note: error.clone(), ..counted(kind, RunState::Error, 0, None) }],
        _ => vec![TaskUpdate { bytes_done: model_done, bytes_total: (model_total > 0).then_some(model_total), ..counted(kind, RunState::Done, 0, None) }],
    }
}

/// The picture-analysis model's row (its download, its failure, its end).
pub fn from_analysis(s: &AnalysisStatus) -> Vec<TaskUpdate> {
    models_updates(TaskKind::Models, &s.models, s.model_done, s.model_total, &s.error)
}

/// The face models' row.
pub fn from_people(s: &PeopleStatus) -> Vec<TaskUpdate> {
    models_updates(TaskKind::FaceModels, &s.models, s.model_done, s.model_total, &s.error)
}

/// How far a pass over the photos has got, (done, total, paused), once its models are ready and there are photos.
fn pass_counts(models: &str, done: i64, total: i64, paused: bool) -> Option<(u64, u64, bool)> {
    (models == "ready" && total > 0).then(|| (count(done), count(total), paused))
}

/// The row for the pass over the photos. One pass does both the Auto tags and the faces, so when both are working they are one row
/// (counting the smaller of the two, so it is never done early); with only one working, that one keeps its own name.
pub fn analysis_rows(tags: Option<&AnalysisStatus>, faces: Option<&PeopleStatus>) -> Vec<TaskUpdate> {
    let t = tags.and_then(|s| pass_counts(&s.models, s.done, s.total, s.paused));
    let f = faces.and_then(|s| pass_counts(&s.models, s.done, s.total, s.paused));
    let row = |kind, done: u64, total: u64, paused: bool, note: Option<&str>| TaskUpdate {
        note: note.map(String::from),
        ..counted(kind, state_of(done, total, paused), done, Some(total))
    };
    match (t, f) {
        (Some(t), Some(f)) => vec![row(TaskKind::Analysing, t.0.min(f.0), t.1.max(f.1), t.2 || f.2, Some("Auto tags and faces"))],
        (Some(t), None) => vec![row(TaskKind::AutoTags, t.0, t.1, t.2, None)],
        (None, Some(f)) => vec![row(TaskKind::Faces, f.0, f.1, f.2, None)],
        (None, None) => Vec::new(),
    }
}

/// Give the analysis row to the registry, and end the others (the row may change name as the other analysis gets going or stops).
pub fn apply_analysis(activity: &Activity, tags: Option<&AnalysisStatus>, faces: Option<&PeopleStatus>) {
    let rows = analysis_rows(tags, faces);
    for kind in [TaskKind::Analysing, TaskKind::AutoTags, TaskKind::Faces] {
        if !rows.iter().any(|u| u.kind == kind) {
            activity.finish(kind);
        }
    }
    apply(activity, rows);
}

/// How many of the files being waited for are passed on (the screen shows the first few).
const WAITING_FILES_KEPT: usize = 8;

/// Tell the registry which files the cloud drive is being waited on for right now, the biggest first.
pub fn feed_waiting(activity: &Activity, c: &CloudCounts) {
    let items = c.waiting_list.iter().take(WAITING_FILES_KEPT).map(|w| CurrentItem { name: w.name.clone(), bytes: w.bytes, secs: w.millis / 1000, source: w.source.to_string() }).collect();
    activity.set_items(TaskKind::CloudWait, items);
}

/// Tell the registry which watched folder the loading and analysis work is in (empty: not known), shown on those rows.
pub fn feed_place(activity: &Activity, folder: &str) {
    let place = (!folder.is_empty()).then(|| folder.to_string());
    for kind in [TaskKind::Read, TaskKind::Analysing, TaskKind::AutoTags, TaskKind::Faces] {
        activity.set_place(kind, place.clone());
    }
}

/// A file taking at least this long is named on its row, so a row that is working on one big file does not look stuck.
pub const SLOW_FILE_MS: u64 = 3000;

/// Tell the registry which file each kind of work is on, if it has been on it for at least `min_ms`.
pub fn feed_current(activity: &Activity, meter: &WorkMeter, min_ms: u64) {
    // (what the pipeline is reading is shown on whichever of its rows exist: the analysis row is the one that looks stuck)
    let rows: [(Work, &[TaskKind]); 2] = [
        (Work::Reading, &[TaskKind::Read, TaskKind::Analysing, TaskKind::AutoTags, TaskKind::Faces]),
        (Work::Copying, &[TaskKind::CopyToMaster]),
    ];
    for (work, kinds) in rows {
        let item = meter.slowest(work).filter(|d| d.millis >= min_ms).map(|d| CurrentItem { name: d.name, bytes: d.bytes, secs: d.millis / 1000, source: String::new() });
        for kind in kinds {
            activity.set_current(*kind, item.clone());
        }
    }
}

/// A folder walk: finding the files (no total until the walk ends), then reading them (`bytes_read` is what the loading work has
/// read from disk so far).
pub fn from_scan(p: &ScanProgress, bytes_read: u64) -> Vec<TaskUpdate> {
    let seen = p.seen as u64;
    if p.to_index == 0 {
        let note = (!p.place.is_empty()).then(|| format!("Looking in {}", p.place));
        return vec![TaskUpdate { note, ..counted(TaskKind::Scan, RunState::Running, seen, None) }];
    }
    let (indexed, to_index) = (p.indexed as u64, p.to_index as u64);
    vec![
        counted(TaskKind::Scan, RunState::Done, seen, Some(seen)),
        TaskUpdate { bytes_done: bytes_read, ..counted(TaskKind::Read, state_of(indexed, to_index, false), indexed, Some(to_index)) },
    ]
}

/// A file waited for longer than this makes the row "waiting": a few seconds is just a download starting.
const CLOUD_WAIT_SHOWN_MS: u64 = 5000;

/// Online-only cloud files met so far: how many have arrived, and whether one is being waited for now.
pub fn from_cloud(c: &CloudCounts) -> Option<TaskUpdate> {
    if c.files_total == 0 {
        return None;
    }
    let state = if c.waiting_files > 0 && c.waiting_ms > CLOUD_WAIT_SHOWN_MS {
        RunState::Waiting
    } else if c.files_done < c.files_total {
        RunState::Running
    } else {
        RunState::Done
    };
    Some(TaskUpdate { bytes_done: c.bytes_done, bytes_total: Some(c.bytes_total), ..counted(TaskKind::CloudWait, state, c.files_done, Some(c.files_total)) })
}

/// The sync copy. `running` is whether a copy is going; an idle one is over.
pub fn from_copy(running: bool, p: &CopyProgress) -> TaskUpdate {
    let state = if running { RunState::Running } else { RunState::Done };
    let note = (running && p.phase == CopyPhase::Checking).then(|| "Checking what was synced".to_string());
    TaskUpdate {
        bytes_done: p.bytes_done,
        bytes_total: (p.bytes_total > 0).then_some(p.bytes_total),
        note,
        ..counted(TaskKind::CopyToMaster, state, p.done as u64, Some(p.total as u64))
    }
}

pub fn from_mail(done: usize, total: usize) -> TaskUpdate {
    counted(TaskKind::Email, state_of(done as u64, total as u64, false), done as u64, Some(total as u64))
}

/// Decides when the snapshot goes to the window: every tick while something runs or waits (the speed, the time left and the wait
/// all move), and once more after anything changes, so a task that just ended shows as ended.
pub struct Emitter {
    last: String,
}

impl Emitter {
    pub fn new() -> Emitter {
        Emitter { last: "[[],[]]".into() }
    }

    pub fn should_emit(&mut self, s: &Snapshot) -> bool {
        let busy = s.tasks.iter().any(|t| matches!(t.state, RunState::Running | RunState::Waiting));
        let now = serde_json::to_string(&(&s.tasks, &s.finished)).unwrap_or_default();
        let changed = now != self.last;
        self.last = now;
        busy || changed
    }
}

/// Hand updates to the registry: a task still going is reported, one that is over is finished (and a source that was never
/// running makes no row).
pub fn apply(activity: &Activity, updates: Vec<TaskUpdate>) {
    for u in updates {
        if u.state == RunState::Done {
            activity.finish_with(u);
        } else {
            activity.report(u);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixie_analysis::people_service::PeopleStatus;
    use pixie_analysis::service::AnalysisStatus;
    use pixie_core::activity::SystemClock;
    use pixie_index::cloud::{CloudCounts, WaitingFile};
    use pixie_index::scanner::ScanProgress;
    use pixie_index::working::busy_with;

    /// A clock the test moves by hand.
    struct Hand(std::sync::atomic::AtomicI64);
    impl pixie_core::activity::Clock for Hand {
        fn now_ms(&self) -> i64 {
            self.0.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    fn by_hand() -> (std::sync::Arc<Hand>, Activity) {
        let clock = std::sync::Arc::new(Hand(std::sync::atomic::AtomicI64::new(1000)));
        (clock.clone(), Activity::new(clock))
    }

    /// Time passes: longer than a source has to stay idle before its task counts as done.
    fn past_the_grace(clock: &Hand) {
        clock.0.fetch_add(16_000, std::sync::atomic::Ordering::SeqCst);
    }

    fn analysis(models: &str, done: i64, total: i64) -> AnalysisStatus {
        AnalysisStatus { models: models.into(), model_done: 0, model_total: 0, done, total, paused: false, error: None }
    }

    fn scan(seen: usize, indexed: usize, to_index: usize) -> ScanProgress {
        ScanProgress { folder_id: 1, seen, indexed, to_index, latest: None, place: String::new() }
    }

    #[test]
    fn a_model_download_is_a_models_update_with_bytes() {
        let mut s = analysis("downloading", 0, 100);
        s.model_done = 30_000_000;
        s.model_total = 90_000_000;
        let u = from_analysis(&s);
        assert_eq!(u.len(), 1);
        assert_eq!((u[0].kind, u[0].state, u[0].bytes_done, u[0].bytes_total), (TaskKind::Models, RunState::Running, 30_000_000, Some(90_000_000)));
    }

    #[test]
    fn a_models_error_is_an_error_with_the_note() {
        let mut s = analysis("error", 0, 0);
        s.error = Some("no network".into());
        let u = from_analysis(&s);
        assert_eq!((u[0].kind, u[0].state, u[0].note.as_deref()), (TaskKind::Models, RunState::Error, Some("no network")));
    }

    #[test]
    fn missing_models_make_no_row() {
        let a = Activity::new(std::sync::Arc::new(SystemClock));
        apply(&a, from_analysis(&analysis("missing", 0, 100)));
        assert!(a.snapshot().tasks.is_empty());
    }

    fn downloading() -> AnalysisStatus {
        AnalysisStatus { model_done: 30, model_total: 90, ..analysis("downloading", 0, 100) }
    }

    fn state_of_kind(a: &Activity, kind: TaskKind) -> Option<RunState> {
        a.snapshot().tasks.iter().find(|t| t.kind == kind).map(|t| t.state)
    }

    #[test]
    fn the_models_row_is_finished_once_the_models_are_ready() {
        let (clock, a) = by_hand();
        apply(&a, from_analysis(&downloading()));
        assert_eq!(state_of_kind(&a, TaskKind::Models), Some(RunState::Running));
        apply(&a, from_analysis(&analysis("ready", 0, 100)));
        past_the_grace(&clock);
        assert_eq!(state_of_kind(&a, TaskKind::Models), Some(RunState::Done), "it must not stay on Getting models for ever");
    }

    #[test]
    fn a_models_error_is_cleared_when_a_retry_gets_them() {
        let a = Activity::new(std::sync::Arc::new(SystemClock));
        let mut e = analysis("error", 0, 0);
        e.error = Some("offline".into());
        apply(&a, from_analysis(&e));
        assert_eq!(state_of_kind(&a, TaskKind::Models), Some(RunState::Error));
        apply(&a, from_analysis(&analysis("ready", 0, 100)));
        assert_eq!(state_of_kind(&a, TaskKind::Models), Some(RunState::Done));
    }

    #[test]
    fn the_face_models_have_their_own_row_so_the_two_downloads_do_not_fight() {
        let (clock, a) = by_hand();
        let people = |models: &str| PeopleStatus { models: models.into(), model_done: 5, model_total: 50, done: 0, total: 10, paused: false, rev: 0, error: None };
        apply(&a, from_analysis(&downloading()));
        apply(&a, from_people(&people("downloading")));
        assert_eq!(state_of_kind(&a, TaskKind::Models), Some(RunState::Running));
        assert_eq!(state_of_kind(&a, TaskKind::FaceModels), Some(RunState::Running));
        apply(&a, from_people(&people("ready")));
        past_the_grace(&clock);
        assert_eq!(state_of_kind(&a, TaskKind::FaceModels), Some(RunState::Done));
        assert_eq!(state_of_kind(&a, TaskKind::Models), Some(RunState::Running), "the other download is still going");
    }

    #[test]
    fn the_file_being_read_is_named_on_the_analysis_row_too() {
        let a = Activity::new(std::sync::Arc::new(SystemClock));
        apply(&a, analysis_rows(Some(&analysis("ready", 5, 20)), Some(&faces("ready", 5, 20))));
        let meter = WorkMeter::default();
        let _on = busy_with(&meter, Work::Reading, std::path::Path::new("C:/OneDrive/Camera Roll/huge.mp4"), 3_000_000_000);
        feed_current(&a, &meter, 0);
        let c = a.snapshot().tasks[0].current.clone().expect("the analysis row says which file it is stuck on");
        assert_eq!((c.name.as_str(), c.bytes), ("huge.mp4", 3_000_000_000));
    }

    #[test]
    fn the_file_a_slow_job_is_on_is_passed_to_the_registry_only_once_it_is_slow() {
        let a = Activity::new(std::sync::Arc::new(SystemClock));
        a.report(TaskUpdate { kind: TaskKind::CopyToMaster, state: RunState::Running, done: 1, total: Some(9), bytes_done: 0, bytes_total: None, note: None });
        let meter = WorkMeter::default();
        let _on = busy_with(&meter, Work::Copying, std::path::Path::new("D:/v/big.mp4"), 5_800_000_000);
        feed_current(&a, &meter, 60_000);
        assert_eq!(a.snapshot().tasks[0].current, None, "it has only just started on it");
        feed_current(&a, &meter, 0);
        let c = a.snapshot().tasks[0].current.clone().unwrap();
        assert_eq!((c.name.as_str(), c.bytes), ("big.mp4", 5_800_000_000));
        drop(_on);
        feed_current(&a, &meter, 0);
        assert_eq!(a.snapshot().tasks[0].current, None, "and it goes when the file is done");
    }

    #[test]
    fn the_scan_row_says_which_folder_it_is_in_while_it_walks() {
        let walking = ScanProgress { place: "2019/Summer trip".into(), ..scan(120, 0, 0) };
        assert_eq!(from_scan(&walking, 0)[0].note.as_deref(), Some("Looking in 2019/Summer trip"));
        assert_eq!(from_scan(&scan(120, 0, 0), 0)[0].note, None, "no file found yet");
    }

    #[test]
    fn the_watched_folder_is_shown_on_the_analysis_row() {
        let a = Activity::new(std::sync::Arc::new(SystemClock));
        apply(&a, analysis_rows(Some(&analysis("ready", 5, 20)), Some(&faces("ready", 5, 20))));
        feed_place(&a, "P:/Pictures");
        assert_eq!(a.snapshot().tasks[0].place.as_deref(), Some("P:/Pictures"));
        feed_place(&a, "");
        assert_eq!(a.snapshot().tasks[0].place, None);
    }

    #[test]
    fn mail_is_a_row_until_every_photo_is_sent() {
        let (clock, a) = by_hand();
        apply(&a, vec![from_mail(1, 3)]);
        assert_eq!(state_of_kind(&a, TaskKind::Email), Some(RunState::Running));
        apply(&a, vec![from_mail(3, 3)]);
        past_the_grace(&clock);
        assert_eq!(state_of_kind(&a, TaskKind::Email), Some(RunState::Done));
        let b = Activity::new(std::sync::Arc::new(SystemClock));
        apply(&b, vec![from_mail(3, 3)]);
        assert!(b.snapshot().tasks.is_empty(), "a source that was never running makes no row");
    }

    fn faces(models: &str, done: i64, total: i64) -> PeopleStatus {
        PeopleStatus { models: models.into(), model_done: 0, model_total: 0, done, total, paused: false, rev: 0, error: None }
    }

    #[test]
    fn auto_tags_and_faces_are_one_row_since_one_pass_does_both() {
        let r = analysis_rows(Some(&analysis("ready", 40, 100)), Some(&faces("ready", 40, 100)));
        assert_eq!(r.len(), 1);
        assert_eq!((r[0].kind, r[0].state, r[0].done, r[0].total, r[0].note.as_deref()), (TaskKind::Analysing, RunState::Running, 40, Some(100), Some("Auto tags and faces")));
    }

    #[test]
    fn the_one_row_counts_the_smaller_so_it_never_claims_to_be_done_early() {
        let r = analysis_rows(Some(&analysis("ready", 90, 100)), Some(&faces("ready", 60, 100)));
        assert_eq!((r[0].done, r[0].state), (60, RunState::Running));
        let both_done = analysis_rows(Some(&analysis("ready", 100, 100)), Some(&faces("ready", 100, 100)));
        assert_eq!(both_done[0].state, RunState::Done);
    }

    #[test]
    fn with_only_one_of_them_working_that_one_has_the_row_under_its_own_name() {
        let tags_only = analysis_rows(Some(&analysis("ready", 5, 20)), Some(&faces("off", 0, 0)));
        assert_eq!((tags_only.len(), tags_only[0].kind, tags_only[0].done), (1, TaskKind::AutoTags, 5));
        let faces_only = analysis_rows(Some(&analysis("missing", 0, 0)), Some(&faces("ready", 7, 20)));
        assert_eq!((faces_only.len(), faces_only[0].kind, faces_only[0].done), (1, TaskKind::Faces, 7));
        assert!(analysis_rows(Some(&analysis("ready", 0, 0)), Some(&faces("ready", 0, 0))).is_empty(), "no photos: no row");
        assert!(analysis_rows(None, None).is_empty(), "nothing heard yet: no row");
    }

    #[test]
    fn a_pause_with_work_left_pauses_the_row_and_nothing_left_is_done_however_paused() {
        let paused = AnalysisStatus { paused: true, ..analysis("ready", 40, 100) };
        assert_eq!(analysis_rows(Some(&paused), Some(&faces("ready", 40, 100)))[0].state, RunState::Paused);
        let paused_done = AnalysisStatus { paused: true, ..analysis("ready", 100, 100) };
        assert_eq!(analysis_rows(Some(&paused_done), Some(&faces("ready", 100, 100)))[0].state, RunState::Done);
        assert_eq!(analysis_rows(Some(&paused), None)[0].state, RunState::Paused);
    }

    #[test]
    fn the_row_changes_name_when_the_other_analysis_gets_going_and_the_old_row_ends() {
        let (_clock, a) = by_hand();
        apply_analysis(&a, Some(&analysis("ready", 5, 20)), Some(&faces("missing", 0, 0)));
        assert_eq!(state_of_kind(&a, TaskKind::AutoTags), Some(RunState::Running));
        apply_analysis(&a, Some(&analysis("ready", 6, 20)), Some(&faces("ready", 6, 20)));
        assert_eq!(state_of_kind(&a, TaskKind::Analysing), Some(RunState::Running));
        assert_eq!(state_of_kind(&a, TaskKind::AutoTags), Some(RunState::Done), "the single-name row is over, not left running beside it");
        apply_analysis(&a, None, None);
        assert_eq!(state_of_kind(&a, TaskKind::Analysing), Some(RunState::Done), "and with neither working the row ends");
    }

    #[test]
    fn a_scan_still_walking_has_no_total() {
        let u = from_scan(&scan(120, 0, 0), 0);
        assert_eq!(u.len(), 1);
        assert_eq!((u[0].kind, u[0].state, u[0].done, u[0].total), (TaskKind::Scan, RunState::Running, 120, None));
    }

    #[test]
    fn a_scan_that_has_found_its_files_is_done_and_the_reading_has_a_total() {
        let u = from_scan(&scan(300, 40, 250), 0);
        assert_eq!((u[0].kind, u[0].state, u[0].done, u[0].total), (TaskKind::Scan, RunState::Done, 300, Some(300)));
        assert_eq!((u[1].kind, u[1].state, u[1].done, u[1].total), (TaskKind::Read, RunState::Running, 40, Some(250)));
        assert_eq!(from_scan(&scan(300, 250, 250), 0)[1].state, RunState::Done);
    }

    fn cloud(total: u64, done: u64, waiting_files: u64, waiting_ms: u64) -> CloudCounts {
        CloudCounts { files_total: total, files_done: done, bytes_total: total * 1000, bytes_done: done * 1000, longest_wait_ms: 0, waiting_files, waiting_ms, bytes_read: 0, waiting_list: Vec::new() }
    }

    #[test]
    fn the_files_being_waited_for_go_to_the_cloud_row_in_the_order_given() {
        let a = Activity::new(std::sync::Arc::new(SystemClock));
        let mut c = cloud(10, 4, 3, 9000);
        c.waiting_list = vec![
            WaitingFile { name: "huge.mp4".into(), source: "onedrive", bytes: 3_000_000_000, millis: 130_000 },
            WaitingFile { name: "mid.mov".into(), source: "icloud", bytes: 50_000_000, millis: 4_000 },
        ];
        apply(&a, from_cloud(&c).into_iter().collect());
        feed_waiting(&a, &c);
        let items = a.snapshot().tasks[0].items.clone();
        assert_eq!(
            items.iter().map(|i| (i.name.as_str(), i.bytes, i.secs, i.source.as_str())).collect::<Vec<_>>(),
            [("huge.mp4", 3_000_000_000, 130, "onedrive"), ("mid.mov", 50_000_000, 4, "icloud")]
        );
    }

    #[test]
    fn no_cloud_files_met_makes_no_row() {
        assert!(from_cloud(&CloudCounts::default()).is_none());
    }

    #[test]
    fn cloud_files_are_running_while_they_arrive_and_done_when_all_have() {
        let u = from_cloud(&cloud(10, 4, 1, 1200)).unwrap();
        assert_eq!((u.kind, u.state, u.done, u.total, u.bytes_done, u.bytes_total), (TaskKind::CloudWait, RunState::Running, 4, Some(10), 4000, Some(10_000)));
        assert_eq!(from_cloud(&cloud(10, 10, 0, 0)).unwrap().state, RunState::Done);
    }

    #[test]
    fn a_file_waited_for_more_than_five_seconds_makes_the_row_waiting() {
        assert_eq!(from_cloud(&cloud(10, 4, 1, 5000)).unwrap().state, RunState::Running, "five seconds is not yet worth a warning");
        assert_eq!(from_cloud(&cloud(10, 4, 1, 5001)).unwrap().state, RunState::Waiting);
        assert_eq!(from_cloud(&cloud(10, 4, 0, 0)).unwrap().state, RunState::Running, "nothing is waiting this moment");
    }

    #[test]
    fn the_reading_row_carries_the_bytes_read_so_far() {
        let u = from_scan(&scan(300, 40, 250), 9_000_000);
        assert_eq!((u[1].kind, u[1].bytes_done), (TaskKind::Read, 9_000_000));
    }

    #[test]
    fn a_running_copy_follows_its_progress_and_an_idle_one_is_done() {
        let p = CopyProgress { done: 2, total: 5, bytes_done: 0, bytes_total: 0, phase: CopyPhase::Copying };
        let u = from_copy(true, &p);
        assert_eq!((u.kind, u.state, u.done, u.total), (TaskKind::CopyToMaster, RunState::Running, 2, Some(5)));
        assert_eq!(u.bytes_total, None, "no bytes known: the items alone are counted");
        assert_eq!(from_copy(false, &p).state, RunState::Done);
    }

    #[test]
    fn one_big_file_moves_the_bytes_before_the_item_count_and_says_when_it_is_being_checked() {
        let writing = CopyProgress { done: 0, total: 5, bytes_done: 4_000_000_000, bytes_total: 6_000_000_000, phase: CopyPhase::Copying };
        let u = from_copy(true, &writing);
        assert_eq!((u.done, u.total, u.bytes_done, u.bytes_total, u.note), (0, Some(5), 4_000_000_000, Some(6_000_000_000), None));
        let checking = CopyProgress { phase: CopyPhase::Checking, ..writing };
        assert_eq!(from_copy(true, &checking).note.as_deref(), Some("Checking what was synced"));
        assert_eq!(from_copy(false, &checking).note, None, "an idle sync checks nothing");
    }

    #[test]
    fn mail_is_running_until_every_photo_is_sent() {
        assert_eq!((from_mail(1, 3).kind, from_mail(1, 3).state), (TaskKind::Email, RunState::Running));
        assert_eq!(from_mail(3, 3).state, RunState::Done);
    }

    #[test]
    fn the_snapshot_is_sent_while_something_runs_and_once_after_it_changes() {
        let (clock, a) = by_hand();
        let mut e = Emitter::new();
        assert!(!e.should_emit(&a.snapshot()), "nothing has happened: nothing to send");
        apply(&a, vec![from_mail(1, 3)]);
        assert!(e.should_emit(&a.snapshot()));
        assert!(e.should_emit(&a.snapshot()), "still running: sent again for the speed and time left");
        apply(&a, vec![from_mail(3, 3)]);
        assert!(e.should_emit(&a.snapshot()), "idle for a moment, still counted as running");
        past_the_grace(&clock);
        assert!(e.should_emit(&a.snapshot()), "it finished: sent once");
        assert!(!e.should_emit(&a.snapshot()), "and then not again until something changes");
    }

    #[test]
    fn a_waiting_task_keeps_the_snapshot_going_so_the_wait_counts_up() {
        let a = Activity::new(std::sync::Arc::new(SystemClock));
        let mut e = Emitter::new();
        a.report(TaskUpdate { kind: TaskKind::CloudWait, state: RunState::Waiting, done: 0, total: None, bytes_done: 0, bytes_total: None, note: None });
        assert!(e.should_emit(&a.snapshot()) && e.should_emit(&a.snapshot()));
    }

}
