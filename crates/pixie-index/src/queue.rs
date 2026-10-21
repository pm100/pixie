//! Folders added while a scan job is running join that job instead of being refused or forgotten.

use pixie_core::FolderId;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct State {
    running: bool,
    pending: Vec<FolderId>,
}

/// The folders waiting for the scan job that is running. The job calls `start_job` before it begins, `next_batch`
/// whenever it has finished its current folders (an empty answer means the job is over), and `finish` if it ends any
/// other way. Anyone adding a folder calls `push`: true means the running job will scan it; false means no job will,
/// so the caller starts one.
#[derive(Clone, Default)]
pub struct ScanQueue(Arc<Mutex<State>>);

impl ScanQueue {
    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn start_job(&self) {
        let mut s = self.state();
        s.running = true;
        s.pending.clear();
    }

    /// Queue a folder for the running job; false (and nothing queued) when none is running or it is ending.
    pub fn push(&self, id: FolderId) -> bool {
        let mut s = self.state();
        if !s.running {
            return false;
        }
        if !s.pending.contains(&id) {
            s.pending.push(id);
        }
        true
    }

    /// The folders queued since the last call. When there are none the job is over: the queue stops accepting, in the
    /// same step, so a `push` is either in this batch or refused, never lost.
    pub fn next_batch(&self) -> Vec<FolderId> {
        let mut s = self.state();
        let batch = std::mem::take(&mut s.pending);
        if batch.is_empty() {
            s.running = false;
        }
        batch
    }

    /// The job ended without asking (it failed): stop accepting and forget what was queued.
    pub fn finish(&self) {
        let mut s = self.state();
        s.running = false;
        s.pending.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_is_refused_when_no_job_runs() {
        let q = ScanQueue::default();
        assert!(!q.push(1));
        assert!(q.next_batch().is_empty());
    }

    #[test]
    fn pushed_ids_come_back_in_batches_and_an_empty_batch_ends_the_job() {
        let q = ScanQueue::default();
        q.start_job();
        assert!(q.push(1));
        assert!(q.push(2));
        assert_eq!(q.next_batch(), vec![1, 2]);
        assert!(q.push(3)); // still running: the job has not asked for an empty batch yet
        assert_eq!(q.next_batch(), vec![3]);
        assert!(q.next_batch().is_empty()); // nothing queued: the job ends here...
        assert!(!q.push(4), "...and a late folder is refused, so its caller starts a job of its own");
    }

    #[test]
    fn an_id_pushed_around_the_end_of_a_job_is_either_returned_or_refused_never_lost() {
        for i in 0..1000 {
            let q = ScanQueue::default();
            q.start_job();
            let pusher = {
                let q = q.clone();
                std::thread::spawn(move || q.push(i))
            };
            let mut returned = q.next_batch();
            while !{
                let more = q.next_batch();
                let empty = more.is_empty();
                returned.extend(more);
                empty
            } {}
            let accepted = pusher.join().unwrap();
            assert_eq!(accepted, returned.contains(&i), "i = {i}: accepted {accepted}, returned {returned:?}");
        }
    }

    #[test]
    fn finish_stops_a_job_that_died() {
        let q = ScanQueue::default();
        q.start_job();
        assert!(q.push(1));
        q.finish();
        assert!(!q.push(2));
        assert!(q.next_batch().is_empty());
    }
}
