//! One registry of everything Pixie is doing in the background (reading folders, hashing, thumbnails, Auto tags, faces,
//! duplicates, syncing, model downloads, waiting for a cloud drive), with a speed and a time left worked out for each. The workers
//! report absolute counts; nothing here does I/O or knows what the work is.

use serde::Serialize;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Speed is judged over the last this many seconds. Long enough that a slow patch (a few big files, a stalled read) does not send the
/// time left from minutes to hours and back.
pub const WINDOW_SECS: u64 = 120;
/// No speed or time left until the samples span at least this long: two samples a moment apart say nothing.
pub const MIN_SAMPLE_SECS: u64 = 10;
/// Reports further apart than this (the computer slept) start the speed over, so the sleep is not counted as slow work.
pub const GAP_RESET_SECS: u64 = 60;
/// A source that goes idle is given this long to start again before its task counts as done, so work that stops and starts does not
/// flicker between running and finished.
pub const FINISH_GRACE_SECS: u64 = 15;
/// A row that is running (or waiting) is not shown until it has been going this long, so work that comes and goes does not flash by.
pub const APPEAR_AFTER_SECS: u64 = 3;
/// A run shorter than this is not listed once it is over.
pub const MIN_LISTED_SECS: u64 = 3;
/// How many finished runs are kept.
pub const HISTORY_MAX: usize = 10;

#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    Scan,
    Read,
    Thumbnails,
    AutoTags,
    Faces,
    /// Auto tags and faces together: one pass over each photo does both, so they are one row when both are working.
    Analysing,
    FaceGroups,
    Duplicates,
    CopyToMaster,
    Models,
    /// The face models: fetched on their own, so a second download does not share a row with the first.
    FaceModels,
    CloudWait,
    Email,
}

#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum RunState {
    Running,
    /// Blocked on something outside Pixie, such as a cloud drive downloading a file.
    Waiting,
    Paused,
    Error,
    Done,
}

/// What a worker says about its task: absolute counts, as of now.
#[derive(Clone, Debug)]
pub struct TaskUpdate {
    pub kind: TaskKind,
    pub state: RunState,
    pub done: u64,
    pub total: Option<u64>,
    pub bytes_done: u64,
    pub bytes_total: Option<u64>,
    pub note: Option<String>,
}

/// The file a task is on right now, when one file is taking a while: its name (not its folder), its size, and how long it has been on it.
#[derive(Serialize, Clone, PartialEq, Eq, Debug)]
pub struct CurrentItem {
    pub name: String,
    pub bytes: u64,
    pub secs: u64,
    /// For a cloud file, which cloud it comes from ("onedrive", "icloud", "google_drive", "other"); empty otherwise.
    pub source: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct TaskSnapshot {
    pub kind: TaskKind,
    pub state: RunState,
    pub done: u64,
    pub total: Option<u64>,
    pub bytes_done: u64,
    pub bytes_total: Option<u64>,
    pub per_sec: Option<f64>,
    pub bytes_per_sec: Option<f64>,
    pub eta_secs: Option<u64>,
    pub started_ms: i64,
    pub finished_ms: Option<i64>,
    pub waiting_secs: Option<u64>,
    pub note: Option<String>,
    pub current: Option<CurrentItem>,
    /// Files the task is waiting on or working through, such as the biggest cloud files being fetched (a list the source keeps up to date).
    pub items: Vec<CurrentItem>,
    /// The watched folder the work is in right now, as listed in Settings, Photo folders (a source keeps it up to date).
    pub place: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct Snapshot {
    pub tasks: Vec<TaskSnapshot>,
    /// Earlier runs that ended, newest first (a kind that runs again shows its new run in `tasks`).
    pub finished: Vec<TaskSnapshot>,
    /// How long until everything running is done: the longest of the running tasks' estimates, none if any of them has none.
    pub eta_secs: Option<u64>,
    pub now_ms: i64,
}

pub trait Clock: Send + Sync {
    fn now_ms(&self) -> i64;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> i64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
    }
}

struct Task {
    update: TaskUpdate,
    started_ms: i64,
    finished_ms: Option<i64>,
    waiting_since: Option<i64>,
    /// The source said it was idle at this time; the task is done once it stays so for `FINISH_GRACE_SECS`.
    ending_since: Option<i64>,
    current: Option<CurrentItem>,
    items: Vec<CurrentItem>,
    place: Option<String>,
    /// (time, done, bytes_done) as reported, newest last.
    samples: VecDeque<(i64, u64, u64)>,
}

struct Inner {
    tasks: Vec<Task>,
    /// Earlier runs that really ended, newest first.
    history: Vec<TaskSnapshot>,
    /// Milliseconds so far during which a cloud drive was being waited for: the speed is judged on the time left after taking this
    /// out, because the work stands still while a big file is fetched and that says nothing about how fast it goes.
    cloud_ms: i64,
    /// When `cloud_ms` was last brought up to date.
    last_ms: Option<i64>,
}

pub struct Activity {
    clock: Arc<dyn Clock>,
    /// How old a running row must be before the snapshot shows it (0: at once).
    appear_ms: i64,
    inner: Mutex<Inner>,
}

/// How fast `newest` moved from `oldest` per second, or none while the samples span too little time.
fn per_second(oldest: (i64, u64), newest: (i64, u64)) -> Option<f64> {
    let secs = (newest.0 - oldest.0) as f64 / 1000.0;
    (secs >= MIN_SAMPLE_SECS as f64).then(|| newest.1.saturating_sub(oldest.1) as f64 / secs)
}

fn eta(remaining: u64, rate: Option<f64>) -> Option<u64> {
    rate.filter(|r| *r > 0.0).map(|r| (remaining as f64 / r).ceil() as u64)
}

impl Task {
    fn fresh(update: TaskUpdate, now: i64) -> Task {
        Task { update, started_ms: now, finished_ms: None, waiting_since: None, ending_since: None, current: None, items: Vec::new(), place: None, samples: VecDeque::new() }
    }

    /// The task is over, as of `at`.
    fn end(&mut self, at: i64) {
        self.update.state = RunState::Done;
        self.finished_ms = Some(at);
        self.waiting_since = None;
        self.ending_since = None;
        self.current = None;
        self.items.clear();
        self.place = None;
        self.samples.clear();
    }

    fn snapshot(&self, now: i64) -> TaskSnapshot {
        let u = &self.update;
        let running = u.state == RunState::Running;
        let (mut per_sec, mut bytes_per_sec, mut eta_secs) = (None, None, None);
        if let (true, Some(old), Some(new)) = (running, self.samples.front(), self.samples.back()) {
            per_sec = per_second((old.0, old.1), (new.0, new.1));
            bytes_per_sec = per_second((old.0, old.2), (new.0, new.2)).filter(|r| *r > 0.0);
            // bytes judge the time left when they are known: a few big files take longer than many small ones
            eta_secs = match (u.bytes_total, bytes_per_sec) {
                (Some(total), Some(_)) => eta(total.saturating_sub(u.bytes_done), bytes_per_sec),
                _ => u.total.and_then(|t| eta(t.saturating_sub(u.done), per_sec)),
            };
        }
        TaskSnapshot {
            kind: u.kind,
            state: u.state,
            done: u.done,
            total: u.total,
            bytes_done: u.bytes_done,
            bytes_total: u.bytes_total,
            per_sec,
            bytes_per_sec,
            eta_secs,
            started_ms: self.started_ms,
            finished_ms: self.finished_ms,
            waiting_secs: self.waiting_since.map(|s| ((now - s).max(0) / 1000) as u64),
            note: u.note.clone(),
            current: self.current.clone(),
            items: self.items.clone(),
            place: self.place.clone(),
        }
    }
}

impl Inner {
    /// Brings `cloud_ms` up to `now`: the time since the last call counts if a cloud drive was being waited for.
    fn advance(&mut self, now: i64) {
        let waiting = self.tasks.iter().any(|t| t.update.kind == TaskKind::CloudWait && t.finished_ms.is_none() && t.update.state == RunState::Waiting);
        if let (true, Some(last)) = (waiting, self.last_ms) {
            self.cloud_ms += (now - last).max(0);
        }
        self.last_ms = Some(now);
    }

    /// Tasks whose source has been idle for the whole grace are done, as of when it went idle.
    fn settle(&mut self, now: i64) {
        for t in &mut self.tasks {
            if let (None, Some(since)) = (t.finished_ms, t.ending_since) {
                if now - since >= FINISH_GRACE_SECS as i64 * 1000 {
                    t.end(since);
                }
            }
        }
    }

    /// Keep a run that ended, unless it was too short to be worth a line.
    fn archive(&mut self, old: &Task, now: i64) {
        let long_enough = old.finished_ms.is_some_and(|f| f - old.started_ms >= MIN_LISTED_SECS as i64 * 1000);
        if long_enough {
            self.history.insert(0, old.snapshot(now));
            self.history.truncate(HISTORY_MAX);
        }
    }
}

impl Activity {
    pub fn new(clock: Arc<dyn Clock>) -> Activity {
        Activity::with_appear_delay(clock, 0)
    }

    /// As `new`, but a running or waiting row is shown only once it has gone on for `secs`, and a run that ended in under
    /// `MIN_LISTED_SECS` is not shown at all. A problem (error, pause) shows at once.
    pub fn with_appear_delay(clock: Arc<dyn Clock>, secs: u64) -> Activity {
        Activity { clock, appear_ms: secs as i64 * 1000, inner: Mutex::new(Inner { tasks: Vec::new(), history: Vec::new(), cloud_ms: 0, last_ms: None }) }
    }

    /// A worker's latest counts. A report for a kind that had finished starts a new run of it (the earlier one stays in the finished
    /// list); one that comes while the source was only just idle carries on the same run.
    pub fn report(&self, u: TaskUpdate) {
        let now = self.clock.now_ms();
        let mut inner = self.inner.lock().unwrap();
        inner.settle(now);
        inner.advance(now);
        let work = now - inner.cloud_ms; // the clock the speed is judged by: the waits for a cloud drive are cut out of it
        let i = match inner.tasks.iter().position(|t| t.update.kind == u.kind) {
            Some(i) if inner.tasks[i].finished_ms.is_some() => {
                let old = std::mem::replace(&mut inner.tasks[i], Task::fresh(u.clone(), now));
                inner.archive(&old, now);
                i
            }
            Some(i) => i,
            None => {
                inner.tasks.push(Task::fresh(u.clone(), now));
                inner.tasks.len() - 1
            }
        };
        let t = &mut inner.tasks[i];
        t.ending_since = None;
        t.waiting_since = if u.state == RunState::Waiting { t.waiting_since.or(Some(now)) } else { None };
        if u.state == RunState::Running {
            if t.samples.back().is_some_and(|s| work - s.0 > GAP_RESET_SECS as i64 * 1000) {
                t.samples.clear();
            }
            if t.samples.back().is_some_and(|s| s.0 == work) {
                t.samples.pop_back();
            }
            t.samples.push_back((work, u.done, u.bytes_done));
            while t.samples.front().is_some_and(|s| work - s.0 > WINDOW_SECS as i64 * 1000) {
                t.samples.pop_front();
            }
        } else {
            t.samples.clear(); // paused, waiting or failed: the speed starts again when it runs
        }
        t.update = u;
    }

    /// The task is definitely over (a scan, a copy or a send came to its end): it is done now, with its totals and how long it took.
    pub fn finish(&self, kind: TaskKind) {
        let now = self.clock.now_ms();
        let mut inner = self.inner.lock().unwrap();
        inner.settle(now);
        if let Some(t) = inner.tasks.iter_mut().find(|t| t.update.kind == kind && t.finished_ms.is_none()) {
            t.end(now);
        }
    }

    /// A source that says it is idle: if its task is still active, store the final counts and, once it has stayed idle for a short
    /// grace, finish it (work that starts again inside the grace is the same run). Otherwise nothing, so an idle source never makes
    /// a row. A task that was failed, paused or waiting is finished at once.
    pub fn finish_with(&self, u: TaskUpdate) {
        let kind = u.kind;
        let now = self.clock.now_ms();
        let mut inner = self.inner.lock().unwrap();
        inner.settle(now);
        if let Some(t) = inner.tasks.iter_mut().find(|t| t.update.kind == kind && t.finished_ms.is_none()) {
            if t.update.state == RunState::Running {
                t.update = TaskUpdate { state: RunState::Running, ..u };
                t.ending_since.get_or_insert(now);
            } else {
                t.update = u;
                t.end(now);
            }
        }
    }

    /// Forget everything: every running task and every finished run (the library they were about has been wiped).
    pub fn clear(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.tasks.clear();
        inner.history.clear();
    }

    /// Say which file a running task is on now (none: it is not on a slow one). Ignored for a task that is not running.
    pub fn set_current(&self, kind: TaskKind, item: Option<CurrentItem>) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(t) = inner.tasks.iter_mut().find(|t| t.update.kind == kind && t.finished_ms.is_none()) {
            t.current = item;
        }
    }

    /// Give a running task the list of files it is waiting on (empty: none). Ignored for a task that is not running.
    pub fn set_items(&self, kind: TaskKind, items: Vec<CurrentItem>) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(t) = inner.tasks.iter_mut().find(|t| t.update.kind == kind && t.finished_ms.is_none()) {
            t.items = items;
        }
    }

    /// Say which watched folder a running task is in (`None`: not known). Ignored for a task that is not running.
    pub fn set_place(&self, kind: TaskKind, place: Option<String>) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(t) = inner.tasks.iter_mut().find(|t| t.update.kind == kind && t.finished_ms.is_none()) {
            t.place = place;
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        let now = self.clock.now_ms();
        let mut inner = self.inner.lock().unwrap();
        inner.settle(now);
        let appear = self.appear_ms;
        let shown = |t: &TaskSnapshot| {
            appear == 0
                || match (t.finished_ms, t.state) {
                    (Some(f), _) => f - t.started_ms >= MIN_LISTED_SECS as i64 * 1000,
                    (None, RunState::Running | RunState::Waiting) => now - t.started_ms >= appear,
                    _ => true,
                }
        };
        let tasks: Vec<TaskSnapshot> = inner.tasks.iter().map(|t| t.snapshot(now)).filter(shown).collect();
        let finished = inner.history.clone();
        let running: Vec<&TaskSnapshot> = tasks.iter().filter(|t| t.state == RunState::Running).collect();
        let eta_secs = if running.is_empty() { None } else { running.iter().map(|t| t.eta_secs).try_fold(0, |m, e| e.map(|e| m.max(e))) };
        Snapshot { tasks, finished, eta_secs, now_ms: now }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicI64, Ordering};
    use std::sync::Arc;

    struct TestClock(AtomicI64);
    impl Clock for TestClock {
        fn now_ms(&self) -> i64 {
            self.0.load(Ordering::SeqCst)
        }
    }

    fn setup() -> (Arc<TestClock>, Activity) {
        let clock = Arc::new(TestClock(AtomicI64::new(0)));
        (clock.clone(), Activity::new(clock))
    }
    fn at(clock: &TestClock, secs: i64) {
        clock.0.store(secs * 1000, Ordering::SeqCst);
    }
    fn upd(kind: TaskKind, state: RunState, done: u64, total: Option<u64>) -> TaskUpdate {
        TaskUpdate { kind, state, done, total, bytes_done: 0, bytes_total: None, note: None }
    }
    fn task(a: &Activity, kind: TaskKind) -> TaskSnapshot {
        a.snapshot().tasks.into_iter().find(|t| t.kind == kind).expect("task is in the snapshot")
    }

    #[test]
    fn time_spent_waiting_for_a_cloud_drive_is_not_counted_in_the_speed() {
        let (clock, a) = setup();
        let mut done = 0u64;
        for s in (0..=140).step_by(10) {
            at(&clock, s);
            // 10 a second, except while a cloud download holds everything up (20 s to 130 s)
            done += if (30..=120).contains(&s) { 0 } else { 100 };
            a.report(upd(TaskKind::AutoTags, RunState::Running, done, Some(done + 1000)));
            if (20..130).contains(&s) {
                a.report(upd(TaskKind::CloudWait, RunState::Waiting, 0, Some(3)));
            } else if s == 130 {
                a.finish(TaskKind::CloudWait);
            }
        }
        let left = task(&a, TaskKind::AutoTags).eta_secs.unwrap();
        assert!(left < 150, "1000 left at 10 a second is 100 s, not {left} s");
    }

    #[test]
    fn a_slow_half_minute_after_minutes_of_steady_work_does_not_multiply_the_time_left() {
        let (clock, a) = setup();
        let mut done = 0u64;
        for s in (0..=150).step_by(10) {
            at(&clock, s);
            done += if s <= 120 { 100 } else { 10 }; // 10 a second, then 1 a second for the last 30 seconds
            a.report(upd(TaskKind::AutoTags, RunState::Running, done, Some(done + 1000)));
        }
        let left = task(&a, TaskKind::AutoTags).eta_secs.unwrap();
        assert!(left < 200, "1000 left at about 8 a second is a couple of minutes, not {left} s");
    }

    #[test]
    fn rate_is_done_per_second_over_the_window() {
        let (clock, a) = setup();
        for s in [0, 10, 20] {
            at(&clock, s);
            a.report(upd(TaskKind::AutoTags, RunState::Running, (s * 10) as u64, Some(1000)));
        }
        let t = task(&a, TaskKind::AutoTags);
        assert_eq!(t.per_sec, Some(10.0));
        assert_eq!(t.eta_secs, Some(80), "800 left at 10 a second");
    }

    #[test]
    fn no_rate_or_eta_before_ten_seconds_of_samples() {
        let (clock, a) = setup();
        for s in [0, 5, 9] {
            at(&clock, s);
            a.report(upd(TaskKind::Faces, RunState::Running, (s * 10) as u64, Some(1000)));
        }
        let t = task(&a, TaskKind::Faces);
        assert_eq!((t.per_sec, t.eta_secs), (None, None));
    }

    #[test]
    fn no_eta_when_the_total_is_unknown() {
        let (clock, a) = setup();
        for s in [0, 10, 20] {
            at(&clock, s);
            a.report(upd(TaskKind::Scan, RunState::Running, (s * 10) as u64, None));
        }
        let t = task(&a, TaskKind::Scan);
        assert_eq!(t.per_sec, Some(10.0), "the speed is still known");
        assert_eq!(t.eta_secs, None);
    }

    #[test]
    fn a_total_that_grows_moves_the_eta_out_and_an_overshoot_is_never_negative() {
        let (clock, a) = setup();
        for s in [0, 10, 20] {
            at(&clock, s);
            a.report(upd(TaskKind::Read, RunState::Running, (s * 10) as u64, Some(1000)));
        }
        assert_eq!(task(&a, TaskKind::Read).eta_secs, Some(80));
        a.report(upd(TaskKind::Read, RunState::Running, 200, Some(2000)));
        assert_eq!(task(&a, TaskKind::Read).eta_secs, Some(180));
        a.report(upd(TaskKind::Read, RunState::Running, 2500, Some(2000)));
        assert_eq!(task(&a, TaskKind::Read).eta_secs, Some(0));
    }

    #[test]
    fn a_gap_over_sixty_seconds_drops_old_samples() {
        let (clock, a) = setup();
        for s in [0, 10, 20] {
            at(&clock, s);
            a.report(upd(TaskKind::Thumbnails, RunState::Running, (s * 10) as u64, Some(5000)));
        }
        at(&clock, 4000); // the computer slept
        a.report(upd(TaskKind::Thumbnails, RunState::Running, 300, Some(5000)));
        assert_eq!(task(&a, TaskKind::Thumbnails).per_sec, None, "one sample is not a rate");
        at(&clock, 4010);
        a.report(upd(TaskKind::Thumbnails, RunState::Running, 400, Some(5000)));
        let t = task(&a, TaskKind::Thumbnails);
        assert_eq!(t.per_sec, Some(10.0), "the sleep is not counted in the speed");
    }

    #[test]
    fn paused_and_error_tasks_have_no_rate_or_eta() {
        let (clock, a) = setup();
        for s in [0, 10, 20] {
            at(&clock, s);
            a.report(upd(TaskKind::Duplicates, RunState::Running, (s * 10) as u64, Some(1000)));
        }
        a.report(upd(TaskKind::Duplicates, RunState::Paused, 200, Some(1000)));
        let t = task(&a, TaskKind::Duplicates);
        assert_eq!((t.state, t.per_sec, t.eta_secs), (RunState::Paused, None, None));
        let mut e = upd(TaskKind::Models, RunState::Error, 0, None);
        e.note = Some("no network".into());
        a.report(e);
        let t = task(&a, TaskKind::Models);
        assert_eq!((t.state, t.per_sec, t.eta_secs), (RunState::Error, None, None));
        assert_eq!(t.note.as_deref(), Some("no network"));
    }

    #[test]
    fn waiting_counts_up_and_gives_no_eta() {
        let (clock, a) = setup();
        at(&clock, 100);
        a.report(upd(TaskKind::CloudWait, RunState::Waiting, 0, Some(10)));
        at(&clock, 190);
        a.report(upd(TaskKind::CloudWait, RunState::Waiting, 1, Some(10)));
        let t = task(&a, TaskKind::CloudWait);
        assert_eq!((t.waiting_secs, t.eta_secs), (Some(90), None));
        a.report(upd(TaskKind::CloudWait, RunState::Running, 2, Some(10)));
        assert_eq!(task(&a, TaskKind::CloudWait).waiting_secs, None, "no longer waiting");
    }

    #[test]
    fn finish_keeps_the_task_with_its_totals_and_duration() {
        let (clock, a) = setup();
        at(&clock, 5);
        a.report(upd(TaskKind::Faces, RunState::Running, 40, Some(100)));
        at(&clock, 65);
        a.report(upd(TaskKind::Faces, RunState::Running, 100, Some(100)));
        a.finish(TaskKind::Faces);
        let t = task(&a, TaskKind::Faces);
        assert_eq!((t.state, t.done, t.total), (RunState::Done, 100, Some(100)));
        assert_eq!((t.started_ms, t.finished_ms), (5000, Some(65000)));
        assert_eq!((t.per_sec, t.eta_secs), (None, None));
    }

    #[test]
    fn overall_eta_is_the_longest_running_eta_and_none_if_any_running_task_has_none() {
        let (clock, a) = setup();
        for s in [0, 10, 20] {
            at(&clock, s);
            a.report(upd(TaskKind::AutoTags, RunState::Running, (s * 10) as u64, Some(1000))); // 80 s left
            a.report(upd(TaskKind::Faces, RunState::Running, (s * 5) as u64, Some(1000))); // 180 s left
        }
        assert_eq!(a.snapshot().eta_secs, Some(180));
        a.report(upd(TaskKind::Scan, RunState::Running, 5, None));
        assert_eq!(a.snapshot().eta_secs, None, "a running task with no estimate means no overall estimate");
        a.finish(TaskKind::Scan);
        assert_eq!(a.snapshot().eta_secs, Some(180));
    }

    #[test]
    fn a_second_report_for_a_finished_kind_starts_a_new_run() {
        let (clock, a) = setup();
        at(&clock, 1);
        a.report(upd(TaskKind::CopyToMaster, RunState::Running, 5, Some(5)));
        a.finish(TaskKind::CopyToMaster);
        at(&clock, 50);
        a.report(upd(TaskKind::CopyToMaster, RunState::Running, 1, Some(9)));
        let t = task(&a, TaskKind::CopyToMaster);
        assert_eq!((t.state, t.done, t.total, t.started_ms, t.finished_ms), (RunState::Running, 1, Some(9), 50000, None));
        assert_eq!(a.snapshot().tasks.len(), 1, "still one row for the kind");
    }

    #[test]
    fn bytes_decide_the_eta_when_the_total_bytes_are_known() {
        let (clock, a) = setup();
        for s in [0, 10, 20] {
            at(&clock, s);
            let mut u = upd(TaskKind::Read, RunState::Running, s as u64, Some(100)); // 1 file a second
            u.bytes_done = (s as u64) * 1_000_000; // 1 MB a second
            u.bytes_total = Some(500_000_000); // 480 MB left: 480 s, though only 80 files are left
            a.report(u);
        }
        let t = task(&a, TaskKind::Read);
        assert_eq!(t.bytes_per_sec, Some(1_000_000.0));
        assert_eq!(t.eta_secs, Some(480));
    }

    #[test]
    fn an_idle_source_leaves_its_task_running_for_a_short_grace_and_then_it_is_done() {
        let (clock, a) = setup();
        at(&clock, 1);
        a.finish_with(upd(TaskKind::Duplicates, RunState::Done, 9, Some(9)));
        assert!(a.snapshot().tasks.is_empty(), "an idle source does not make a row");
        a.report(upd(TaskKind::Duplicates, RunState::Running, 4, Some(9)));
        at(&clock, 8);
        a.finish_with(upd(TaskKind::Duplicates, RunState::Done, 9, Some(9)));
        at(&clock, 10);
        let t = task(&a, TaskKind::Duplicates);
        assert_eq!((t.state, t.done, t.total, t.finished_ms), (RunState::Running, 9, Some(9), None), "inside the grace it is not done yet, and it has the final counts");
        at(&clock, 8 + FINISH_GRACE_SECS as i64);
        let t = task(&a, TaskKind::Duplicates);
        assert_eq!((t.state, t.done, t.finished_ms), (RunState::Done, 9, Some(8000)), "done as of when the source went idle");
        at(&clock, 60);
        a.finish_with(upd(TaskKind::Duplicates, RunState::Done, 9, Some(9)));
        assert_eq!(task(&a, TaskKind::Duplicates).finished_ms, Some(8000), "an old run is not finished again");
    }

    #[test]
    fn work_that_starts_again_inside_the_grace_is_the_same_run() {
        let (clock, a) = setup();
        at(&clock, 1);
        a.report(upd(TaskKind::Duplicates, RunState::Running, 4, Some(9)));
        at(&clock, 10);
        a.finish_with(upd(TaskKind::Duplicates, RunState::Done, 9, Some(9)));
        at(&clock, 12);
        a.report(upd(TaskKind::Duplicates, RunState::Running, 9, Some(20))); // more files arrived
        at(&clock, 40);
        let t = task(&a, TaskKind::Duplicates);
        assert_eq!((t.state, t.started_ms, t.finished_ms, t.total), (RunState::Running, 1000, None, Some(20)));
        assert!(a.snapshot().finished.is_empty(), "nothing finished, so nothing is listed as finished");
    }

    #[test]
    fn a_failed_or_paused_task_that_the_source_calls_done_is_done_at_once() {
        let (clock, a) = setup();
        at(&clock, 1);
        a.report(upd(TaskKind::Models, RunState::Error, 0, None));
        at(&clock, 2);
        a.finish_with(upd(TaskKind::Models, RunState::Done, 0, None));
        assert_eq!(task(&a, TaskKind::Models).state, RunState::Done, "no grace for something that was not running");
    }

    #[test]
    fn a_finished_run_stays_in_the_finished_list_when_the_kind_runs_again() {
        let (clock, a) = setup();
        at(&clock, 5);
        a.report(upd(TaskKind::Faces, RunState::Running, 40, Some(100)));
        at(&clock, 65);
        a.finish(TaskKind::Faces);
        assert_eq!(task(&a, TaskKind::Faces).state, RunState::Done);
        at(&clock, 100);
        a.report(upd(TaskKind::Faces, RunState::Running, 1, Some(5)));
        let s = a.snapshot();
        assert_eq!(s.tasks.len(), 1);
        assert_eq!((s.tasks[0].state, s.tasks[0].started_ms), (RunState::Running, 100_000));
        assert_eq!(s.finished.len(), 1, "the earlier run is still listed as finished");
        assert_eq!((s.finished[0].kind, s.finished[0].started_ms, s.finished[0].finished_ms), (TaskKind::Faces, 5000, Some(65_000)));
    }

    #[test]
    fn a_run_of_a_few_seconds_is_not_worth_listing_and_the_list_is_capped() {
        let (clock, a) = setup();
        at(&clock, 10);
        a.report(upd(TaskKind::Email, RunState::Running, 1, Some(1)));
        at(&clock, 11);
        a.finish(TaskKind::Email);
        at(&clock, 20);
        a.report(upd(TaskKind::Email, RunState::Running, 1, Some(1)));
        assert!(a.snapshot().finished.is_empty(), "a one second run is dropped");
        a.finish(TaskKind::Email);
        for i in 0..(HISTORY_MAX as i64 + 5) {
            at(&clock, 100 + i * 100);
            a.report(upd(TaskKind::Scan, RunState::Running, 1, None));
            at(&clock, 150 + i * 100);
            a.finish(TaskKind::Scan);
        }
        at(&clock, 5000);
        a.report(upd(TaskKind::Scan, RunState::Running, 1, None));
        let f = a.snapshot().finished;
        assert_eq!(f.len(), HISTORY_MAX);
        assert!(f.windows(2).all(|w| w[0].finished_ms >= w[1].finished_ms), "newest first");
    }

    fn slow_to_show() -> (Arc<TestClock>, Activity) {
        let clock = Arc::new(TestClock(AtomicI64::new(0)));
        (clock.clone(), Activity::with_appear_delay(clock, APPEAR_AFTER_SECS))
    }

    #[test]
    fn a_row_shows_only_once_it_has_been_going_for_a_few_seconds_but_a_problem_shows_at_once() {
        let (clock, a) = slow_to_show();
        at(&clock, 10);
        a.report(upd(TaskKind::Duplicates, RunState::Running, 1, Some(9)));
        at(&clock, 10 + APPEAR_AFTER_SECS as i64 - 1);
        let s = a.snapshot();
        assert!(s.tasks.is_empty() && s.eta_secs.is_none(), "too new to show");
        at(&clock, 10 + APPEAR_AFTER_SECS as i64);
        assert_eq!(a.snapshot().tasks.len(), 1);
        at(&clock, 30);
        a.report(upd(TaskKind::Models, RunState::Error, 0, None));
        assert_eq!(a.snapshot().tasks.len(), 2, "an error is not held back");
    }

    #[test]
    fn a_short_burst_that_ends_is_never_shown_at_all() {
        let (clock, a) = slow_to_show();
        at(&clock, 10);
        a.report(upd(TaskKind::Duplicates, RunState::Running, 1, Some(9)));
        at(&clock, 11);
        a.finish(TaskKind::Duplicates);
        at(&clock, 12);
        assert!(a.snapshot().tasks.is_empty(), "a second of work is not worth a row, running or finished");
        at(&clock, 20);
        a.report(upd(TaskKind::Faces, RunState::Running, 1, Some(9)));
        at(&clock, 20 + MIN_LISTED_SECS as i64 + 1);
        a.finish(TaskKind::Faces);
        assert_eq!(a.snapshot().tasks.len(), 1, "a longer run stays as finished");
    }

    #[test]
    fn clearing_forgets_every_run_running_and_finished() {
        let (clock, a) = setup();
        at(&clock, 5);
        a.report(upd(TaskKind::Faces, RunState::Running, 40, Some(100)));
        at(&clock, 65);
        a.finish(TaskKind::Faces);
        at(&clock, 100);
        a.report(upd(TaskKind::Faces, RunState::Running, 1, Some(5))); // the first run goes into the finished list
        a.report(upd(TaskKind::Scan, RunState::Running, 3, None));
        assert!(!a.snapshot().finished.is_empty() && a.snapshot().tasks.len() == 2);
        a.clear();
        let s = a.snapshot();
        assert!(s.tasks.is_empty() && s.finished.is_empty() && s.eta_secs.is_none(), "nothing of the old library is left");
        a.report(upd(TaskKind::Faces, RunState::Running, 1, Some(5)));
        assert_eq!(task(&a, TaskKind::Faces).started_ms, 100_000, "and a kind starts fresh afterwards");
    }

    #[test]
    fn a_task_can_list_the_files_it_is_waiting_on_and_the_list_goes_when_it_ends() {
        let (_clock, a) = setup();
        a.set_items(TaskKind::CloudWait, vec![item("early.mp4", 1, 1)]);
        assert!(a.snapshot().tasks.is_empty(), "a source that is not running makes no row");
        a.report(upd(TaskKind::CloudWait, RunState::Waiting, 0, None));
        a.set_items(TaskKind::CloudWait, vec![item("a.mp4", 3_000_000_000, 130), item("b.mov", 5, 2)]);
        assert_eq!(task(&a, TaskKind::CloudWait).items.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(), ["a.mp4", "b.mov"]);
        a.set_items(TaskKind::CloudWait, Vec::new());
        assert!(task(&a, TaskKind::CloudWait).items.is_empty());
        a.set_items(TaskKind::CloudWait, vec![item("c.mp4", 1, 1)]);
        a.finish(TaskKind::CloudWait);
        assert!(task(&a, TaskKind::CloudWait).items.is_empty(), "a finished task lists nothing");
    }

    #[test]
    fn a_running_task_says_which_watched_folder_it_is_in_and_it_goes_when_the_task_ends() {
        let (_clock, a) = setup();
        a.set_place(TaskKind::Analysing, Some("early".into()));
        assert!(a.snapshot().tasks.is_empty(), "a task that is not running makes no row");
        a.report(upd(TaskKind::Analysing, RunState::Running, 1, Some(5)));
        a.set_place(TaskKind::Analysing, Some("P:/Pictures".into()));
        assert_eq!(task(&a, TaskKind::Analysing).place.as_deref(), Some("P:/Pictures"));
        a.finish(TaskKind::Analysing);
        assert_eq!(task(&a, TaskKind::Analysing).place, None);
    }

    fn item(name: &str, bytes: u64, secs: u64) -> CurrentItem {
        CurrentItem { name: name.into(), bytes, secs, source: String::new() }
    }

    #[test]
    fn a_task_can_say_which_file_it_is_on_and_stop_saying_so() {
        let (_clock, a) = setup();
        a.set_current(TaskKind::Duplicates, Some(item("early.mp4", 1, 9)));
        assert!(a.snapshot().tasks.is_empty(), "a source that is not running makes no row");
        a.report(upd(TaskKind::Duplicates, RunState::Running, 1, Some(5)));
        a.set_current(TaskKind::Duplicates, Some(item("big.mp4", 5_800_000_000, 130)));
        assert_eq!(task(&a, TaskKind::Duplicates).current, Some(item("big.mp4", 5_800_000_000, 130)));
        a.set_current(TaskKind::Duplicates, None);
        assert_eq!(task(&a, TaskKind::Duplicates).current, None);
    }

    #[test]
    fn a_finished_task_has_no_current_file() {
        let (_clock, a) = setup();
        a.report(upd(TaskKind::Read, RunState::Running, 1, Some(5)));
        a.set_current(TaskKind::Read, Some(item("x.jpg", 10, 4)));
        a.finish(TaskKind::Read);
        assert_eq!(task(&a, TaskKind::Read).current, None);
        a.set_current(TaskKind::Read, Some(item("y.jpg", 10, 4)));
        assert_eq!(task(&a, TaskKind::Read).current, None, "and one that is over does not get one back");
    }

    #[test]
    fn the_snapshot_serialises_with_snake_case_names() {
        let (clock, a) = setup();
        at(&clock, 3);
        a.report(upd(TaskKind::CloudWait, RunState::Waiting, 0, None));
        let json = serde_json::to_string(&a.snapshot()).unwrap();
        assert!(json.contains("\"kind\":\"cloud_wait\"") && json.contains("\"state\":\"waiting\""), "{json}");
    }
}
