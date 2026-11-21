//! Thread-based background task runner with cancellation and a shell waker.

use alloy_ui_core::runtime::BoxFuture;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::thread::{self, Thread};

/// Shared cancellation flag, passed to blocking closures so they can stop early.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    /// Whether cancellation was requested.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Handle to a spawned task. Dropping it detaches the task (it keeps running).
#[derive(Debug, Clone)]
pub struct TaskHandle {
    token: CancelToken,
    done: Arc<AtomicBool>,
    thread: Arc<Mutex<Option<Thread>>>,
}

impl TaskHandle {
    /// Request cancellation. A cancelled task never delivers a message.
    /// Futures are dropped at their next wake-up; blocking closures must poll
    /// their [`CancelToken`] (their result is discarded regardless).
    pub fn cancel(&self) {
        self.token.0.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.lock_ok().as_ref() {
            t.unpark();
        }
    }

    /// Whether cancellation was requested.
    pub fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }

    /// Whether the task thread has finished (completed or cancelled).
    pub fn is_finished(&self) -> bool {
        self.done.load(Ordering::SeqCst)
    }
}

struct ThreadWaker(Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

/// Runs tasks off the UI thread and queues their messages for the UI thread.
///
/// The platform shell supplies a `waker` that wakes its event loop; the loop
/// then calls [`TaskRunner::drain`] to collect messages.
pub struct TaskRunner<M> {
    tx: Sender<M>,
    rx: Receiver<M>,
    waker: Arc<dyn Fn() + Send + Sync>,
}

impl<M: Send + 'static> TaskRunner<M> {
    /// Create a runner; `waker` is invoked from worker threads after each message is queued.
    pub fn new(waker: impl Fn() + Send + Sync + 'static) -> Self {
        let (tx, rx) = channel();
        Self {
            tx,
            rx,
            waker: Arc::new(waker),
        }
    }

    fn start(
        &self,
        body: impl FnOnce(CancelToken, Thread) -> Option<M> + Send + 'static,
    ) -> TaskHandle {
        let token = CancelToken::default();
        let done = Arc::new(AtomicBool::new(false));
        let slot = Arc::new(Mutex::new(None));
        let (tx, waker) = (self.tx.clone(), self.waker.clone());
        let (t2, d2, s2) = (token.clone(), done.clone(), slot.clone());
        let join = thread::spawn(move || {
            let msg = body(t2.clone(), thread::current());
            if let Some(m) = msg {
                if !t2.is_cancelled() && tx.send(m).is_ok() {
                    waker();
                }
            }
            d2.store(true, Ordering::SeqCst);
        });
        *s2.lock_ok() = Some(join.thread().clone());
        TaskHandle {
            token,
            done,
            thread: slot,
        }
    }

    /// Drive a future to completion on a worker thread and deliver its message.
    pub fn spawn(&self, fut: BoxFuture<M>) -> TaskHandle {
        self.start(move |token, me| {
            let mut fut = fut;
            let waker = Waker::from(Arc::new(ThreadWaker(me)));
            let mut cx = Context::from_waker(&waker);
            loop {
                if token.is_cancelled() {
                    return None;
                }
                if let Poll::Ready(m) = fut.as_mut().poll(&mut cx) {
                    return Some(m);
                }
                thread::park();
            }
        })
    }

    /// Run a blocking closure on a worker thread and deliver its message.
    pub fn spawn_blocking(&self, f: impl FnOnce(&CancelToken) -> M + Send + 'static) -> TaskHandle {
        self.start(move |token, _| Some(f(&token)))
    }

    /// Take one queued message, if any.
    pub fn try_recv(&self) -> Option<M> {
        self.rx.try_recv().ok()
    }

    /// Take all queued messages.
    pub fn drain(&self) -> Vec<M> {
        self.rx.try_iter().collect()
    }

    /// Block up to `timeout` for the next message (mostly useful in tests).
    pub fn recv_timeout(&self, timeout: std::time::Duration) -> Option<M> {
        self.rx.recv_timeout(timeout).ok()
    }
}

/// `Mutex::lock` that survives poisoning (the guarded slots stay valid if a task panics).
trait LockExt<T> {
    fn lock_ok(&self) -> std::sync::MutexGuard<'_, T>;
}

impl<T> LockExt<T> for std::sync::Mutex<T> {
    fn lock_ok(&self) -> std::sync::MutexGuard<'_, T> {
        self.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::time::Duration;

    const T: Duration = Duration::from_secs(5);

    #[test]
    fn future_round_trip_and_waker() {
        let wakes = Arc::new(AtomicUsize::new(0));
        let w = wakes.clone();
        let r = TaskRunner::new(move || {
            w.fetch_add(1, Ordering::SeqCst);
        });
        let h = r.spawn(Box::pin(async { 41 + 1 }));
        assert_eq!(r.recv_timeout(T), Some(42));
        while !h.is_finished() {
            thread::yield_now();
        }
        assert_eq!(wakes.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn blocking_round_trip() {
        let r = TaskRunner::new(|| {});
        r.spawn_blocking(|_| "hi".to_string());
        assert_eq!(r.recv_timeout(T).as_deref(), Some("hi"));
    }

    #[test]
    fn many_tasks_all_deliver() {
        let r = TaskRunner::new(|| {});
        for i in 0..20 {
            r.spawn_blocking(move |_| i);
        }
        let mut got: Vec<i32> = (0..20).filter_map(|_| r.recv_timeout(T)).collect();
        got.sort();
        assert_eq!(got, (0..20).collect::<Vec<_>>());
    }

    #[test]
    fn cancel_pending_future() {
        let r = TaskRunner::new(|| {});
        let h = r.spawn(Box::pin(std::future::pending::<u8>()));
        h.cancel();
        while !h.is_finished() {
            thread::yield_now();
        }
        assert!(h.is_cancelled());
        assert_eq!(r.try_recv(), None);
    }

    #[test]
    fn cancel_blocking_stops_and_discards() {
        let r = TaskRunner::new(|| {});
        let h = r.spawn_blocking(|tok| {
            while !tok.is_cancelled() {
                thread::sleep(Duration::from_millis(1));
            }
            7
        });
        h.cancel();
        while !h.is_finished() {
            thread::yield_now();
        }
        assert!(r.drain().is_empty());
    }

    #[test]
    fn future_woken_from_another_thread() {
        use std::sync::atomic::AtomicBool;
        struct Flag(Arc<AtomicBool>, Arc<Mutex<Option<Waker>>>);
        impl std::future::Future for Flag {
            type Output = u8;
            fn poll(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<u8> {
                if self.0.load(Ordering::SeqCst) {
                    Poll::Ready(9)
                } else {
                    *self.1.lock_ok() = Some(cx.waker().clone());
                    Poll::Pending
                }
            }
        }
        let flag = Arc::new(AtomicBool::new(false));
        let slot = Arc::new(Mutex::new(None));
        let r = TaskRunner::new(|| {});
        r.spawn(Box::pin(Flag(flag.clone(), slot.clone())));
        thread::sleep(Duration::from_millis(50));
        flag.store(true, Ordering::SeqCst);
        if let Some(w) = slot.lock_ok().take() {
            w.wake();
        }
        assert_eq!(r.recv_timeout(T), Some(9));
    }

    #[test]
    fn drain_empty() {
        let r: TaskRunner<u8> = TaskRunner::new(|| {});
        assert!(r.drain().is_empty());
        assert!(r.try_recv().is_none());
    }
}
