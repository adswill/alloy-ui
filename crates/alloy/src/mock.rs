//! Fake data for demos, prototypes and tests: a backend that answers after a delay and
//! fails on a schedule, and made-up chart series.

use crate::{resource, secs, sleep, Resource};
use std::{
    sync::{
        atomic::{AtomicU32, Ordering::Relaxed},
        Arc,
    },
    time::Duration,
};

/// A pretend server over a fixed list of rows. Made by [`Record::mock`](crate::Record::mock).
/// Every [`load`](Self::load) takes [`delay`](Self::delay) and fails on a
/// [`fail_every`](Self::fail_every) schedule, so loading, error and retry states can be seen.
pub struct Mock<T> {
    rows: Vec<T>,
    delay: Duration,
    fail: Option<(u32, String)>,
    calls: AtomicU32,
}

impl<T: Clone + Send + Sync + 'static> Mock<T> {
    pub(crate) fn new(rows: Vec<T>) -> Self {
        Mock {
            rows,
            delay: Duration::ZERO,
            fail: None,
            calls: AtomicU32::new(0),
        }
    }
    /// Answer after this many seconds.
    pub fn delay(mut self, seconds: f32) -> Self {
        self.delay = secs(seconds);
        self
    }
    /// Every `n`th request fails with `message` (the 3rd, 6th, ... for `n = 3`).
    pub fn fail_every(mut self, n: u32, message: &str) -> Self {
        self.fail = Some((n, message.into()));
        self
    }
    /// One request.
    pub async fn load(&self) -> Result<Vec<T>, String> {
        sleep(self.delay).await;
        let n = self.calls.fetch_add(1, Relaxed) + 1;
        match &self.fail {
            Some((k, msg)) if n % k == 0 => Err(msg.clone()),
            _ => Ok(self.rows.clone()),
        }
    }
    /// Load this in the background like [`resource`]: `.auto(secs(10.0)).board(..)`.
    #[track_caller]
    pub fn resource(self) -> Resource<Vec<T>> {
        let me = Arc::new(self);
        resource(move || {
            let me = me.clone();
            async move { me.load().await }
        })
    }
}

/// `n` made-up values in `0.0..1.0`, always the same for the same `seed`: sample chart data.
pub fn series(seed: &str, n: usize) -> Vec<f32> {
    let mut h = seed
        .bytes()
        .fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619));
    (0..n)
        .map(|_| {
            h = h.wrapping_mul(1664525).wrapping_add(1013904223);
            (h >> 16) as f32 / 65536.0
        })
        .collect()
}
