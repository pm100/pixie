//! Getting folders scanned when a scan job may or may not be running (see `scan_folders`).

use pixie_index::{ScanGate, ScanGuard, ScanQueue};
use std::time::Duration;

#[derive(Debug, PartialEq)]
pub enum Outcome {
    /// No job was running: one was started over the folders.
    Started,
    /// A running job took all the folders through its queue.
    Queued,
    /// A job is ending: it no longer takes folders but still holds the gate. These folders are not placed yet and must
    /// be started as soon as the gate frees.
    Waiting(Vec<i64>),
}

pub struct Placed {
    pub outcome: Outcome,
    /// Some of the folders joined a running job.
    pub queued: bool,
}

/// Try, up to `attempts` times with `pause` between, to start a job over `ids` or to hand them to the running one.
/// `start` is called at most once, with the folders still unplaced and the gate guard.
pub fn place_folders(
    ids: &[i64],
    gate: &ScanGate,
    queue: &ScanQueue,
    attempts: usize,
    pause: Duration,
    start: &mut dyn FnMut(Vec<i64>, ScanGuard),
) -> Placed {
    let mut rest = ids.to_vec();
    let mut queued = false;
    for _ in 0..attempts {
        if rest.is_empty() {
            return Placed { outcome: Outcome::Queued, queued };
        }
        if let Some(guard) = gate.try_start() {
            start(rest, guard);
            return Placed { outcome: Outcome::Started, queued };
        }
        let before = rest.len();
        rest.retain(|id| !queue.push(*id));
        queued |= rest.len() < before;
        if !rest.is_empty() {
            std::thread::sleep(pause);
        }
    }
    let outcome = if rest.is_empty() { Outcome::Queued } else { Outcome::Waiting(rest) };
    Placed { outcome, queued }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixie_index::{ScanGate, ScanQueue};
    use std::time::Duration;

    const PAUSE: Duration = Duration::from_millis(5);

    #[test]
    fn a_free_gate_starts_a_job_over_all_the_folders() {
        let (gate, queue) = (ScanGate::default(), ScanQueue::default());
        let mut started = Vec::new();
        let placed = place_folders(&[1, 2], &gate, &queue, 10, PAUSE, &mut |ids, guard| {
            started = ids;
            drop(guard);
        });
        assert_eq!((placed.outcome, placed.queued), (Outcome::Started, false));
        assert_eq!(started, vec![1, 2]);
    }

    #[test]
    fn a_running_job_takes_the_folders_through_the_queue() {
        let (gate, queue) = (ScanGate::default(), ScanQueue::default());
        let _running = gate.try_start().unwrap();
        queue.start_job();
        let placed = place_folders(&[7], &gate, &queue, 10, PAUSE, &mut |_, _| panic!("no second job"));
        assert_eq!((placed.outcome, placed.queued), (Outcome::Queued, true));
        assert_eq!(queue.next_batch(), vec![7]);
    }

    #[test]
    fn a_job_that_is_ending_but_still_holds_the_gate_leaves_the_folders_waiting_not_lost() {
        // The job has asked for its last (empty) batch, so the queue refuses folders, but the gate is held until it
        // has finished its closing steps: neither route is open yet.
        let (gate, queue) = (ScanGate::default(), ScanQueue::default());
        let _ending = gate.try_start().unwrap();
        let placed = place_folders(&[3, 4], &gate, &queue, 5, PAUSE, &mut |_, _| panic!("the gate is held"));
        assert_eq!(placed.outcome, Outcome::Waiting(vec![3, 4]));
        assert!(!placed.queued);
    }

    #[test]
    fn a_gate_that_frees_during_the_attempts_starts_the_job() {
        let (gate, queue) = (ScanGate::default(), ScanQueue::default());
        let guard = gate.try_start().unwrap();
        let releaser = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(40));
            drop(guard);
        });
        let mut started = Vec::new();
        let placed = place_folders(&[9], &gate, &queue, 200, PAUSE, &mut |ids, guard| {
            started = ids;
            drop(guard);
        });
        releaser.join().unwrap();
        assert_eq!(placed.outcome, Outcome::Started);
        assert_eq!(started, vec![9]);
    }
}
