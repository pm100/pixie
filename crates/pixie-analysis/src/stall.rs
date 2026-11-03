//! Which file the analysis touched last, so a hang can be told from the log: when no file has started or finished for a while, the
//! last file started (reading it, and working on it) is named. Nothing is said while files keep moving.

use std::sync::Mutex;
use std::time::{Duration, Instant};

struct Inner {
    reading: String,
    analysing: String,
    moved: Instant,
    said: Option<Instant>,
}

pub struct Trail(Mutex<Inner>);

impl Trail {
    pub fn new() -> Self {
        Trail(Mutex::new(Inner { reading: String::new(), analysing: String::new(), moved: Instant::now(), said: None }))
    }

    fn with(&self, f: impl FnOnce(&mut Inner)) {
        f(&mut self.0.lock().unwrap_or_else(|e| e.into_inner()));
    }

    /// A reader is about to read this file.
    pub fn reading(&self, path: &str) {
        self.with(|t| {
            t.reading = path.to_string();
            t.moved = Instant::now();
        });
    }

    /// A worker is about to work on this file.
    pub fn analysing(&self, path: &str) {
        self.with(|t| {
            t.analysing = path.to_string();
            t.moved = Instant::now();
        });
    }

    /// A file's work came back.
    pub fn finished(&self) {
        self.with(|t| t.moved = Instant::now());
    }

    /// The line to log if nothing has moved for `limit` (and it was not already said within `limit`).
    pub fn stalled(&self, now: Instant, limit: Duration) -> Option<String> {
        let mut out = None;
        self.with(|t| {
            let quiet = now.saturating_duration_since(t.moved);
            if quiet < limit || t.said.is_some_and(|s| now.saturating_duration_since(s) < limit) {
                return;
            }
            t.said = Some(now);
            out = Some(format!(
                "[analysis] no file has started or finished for {} s. Last started reading: {}. Last started analysing: {}.",
                quiet.as_secs(),
                if t.reading.is_empty() { "none" } else { &t.reading },
                if t.analysing.is_empty() { "none" } else { &t.analysing },
            ));
        });
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIMIT: Duration = Duration::from_secs(30);

    #[test]
    fn nothing_is_said_while_files_keep_moving() {
        let t = Trail::new();
        t.reading("a.jpg");
        assert_eq!(t.stalled(Instant::now() + Duration::from_secs(5), LIMIT), None);
    }

    #[test]
    fn a_stall_names_the_last_file_read_and_the_last_one_worked_on() {
        let t = Trail::new();
        t.reading("P:/a.jpg");
        t.analysing("C:/b.jpg");
        let line = t.stalled(Instant::now() + Duration::from_secs(40), LIMIT).unwrap();
        assert!(line.contains("reading: P:/a.jpg"), "{line}");
        assert!(line.contains("analysing: C:/b.jpg"), "{line}");
    }

    #[test]
    fn a_stall_is_said_once_per_limit_not_every_second() {
        let t = Trail::new();
        let now = Instant::now();
        assert!(t.stalled(now + Duration::from_secs(40), LIMIT).is_some());
        assert_eq!(t.stalled(now + Duration::from_secs(41), LIMIT), None);
        assert!(t.stalled(now + Duration::from_secs(75), LIMIT).is_some());
    }

    #[test]
    fn a_file_finishing_ends_the_stall() {
        let t = Trail::new();
        t.finished();
        assert_eq!(t.stalled(Instant::now() + Duration::from_secs(10), LIMIT), None);
    }
}
