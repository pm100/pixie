//! Model sessions that several workers use at once. A session runs one input at a time, so each worker needs its
//! own; they are kept in a pool and a worker takes whichever is free.

use ort::session::Session;
use std::sync::{Mutex, MutexGuard, TryLockError};

/// A session nobody is using, starting the search at `start`; if all are busy, waits for the one `start` points at.
pub fn pick<'a>(pool: &'a [Mutex<Session>], start: usize) -> MutexGuard<'a, Session> {
    for k in 0..pool.len() {
        match pool[(start + k) % pool.len()].try_lock() {
            Ok(g) => return g,
            Err(TryLockError::Poisoned(p)) => return p.into_inner(),
            Err(TryLockError::WouldBlock) => {}
        }
    }
    pool[start % pool.len()].lock().unwrap_or_else(|p| p.into_inner())
}
