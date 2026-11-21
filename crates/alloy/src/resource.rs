//! Async data in one call: `resource(fetch)` gives loading, error, retry and refresh for free.
//!
//! The future runs on a background thread driven by a tiny built-in executor (std only),
//! so plain `async fn`s and [`sleep`] work with no runtime. Futures that need a specific
//! runtime (tokio sockets) should be wrapped by the app. Results are applied on the UI
//! thread by [`Pump`], which `sketch` and `app` apps install automatically.

use crate::{
    cx::{self, auto_id, LockExt},
    dsl::{button, caption, heading, text, Act},
    view::{col, Node},
};
use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    task::{Context, Poll, Wake, Waker},
    time::{Duration, Instant},
};

type Fut<T> = Pin<Box<dyn Future<Output = Result<T, String>> + Send>>;
type Job = Box<dyn FnOnce() + Send>;

static INFLIGHT: AtomicUsize = AtomicUsize::new(0);
static DONE: Mutex<Vec<Job>> = Mutex::new(Vec::new());
static TICKERS: Mutex<Vec<Arc<dyn Fn() + Send + Sync>>> = Mutex::new(Vec::new());

struct Parker(std::thread::Thread);
impl Wake for Parker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

/// Run a future to completion on the current thread.
pub fn block_on<F: Future>(fut: F) -> F::Output {
    let mut fut = Box::pin(fut);
    let waker = Waker::from(Arc::new(Parker(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    loop {
        if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
            return v;
        }
        std::thread::park();
    }
}

/// Lets the UI thread stop a fetch: sets a flag and wakes the worker so it drops the future.
struct Cancel {
    flag: AtomicBool,
    worker: Mutex<Option<std::thread::Thread>>,
}

impl Cancel {
    fn cancel(&self) {
        self.flag.store(true, Ordering::Release);
        if let Some(t) = self.worker.lock_ok().as_ref() {
            t.unpark();
        }
    }
}

/// Like [`block_on`], but returns `None` (dropping the future) once `cancel` fires.
fn block_on_cancellable<F: Future>(fut: F, cancel: &Cancel) -> Option<F::Output> {
    let mut fut = Box::pin(fut);
    let waker = Waker::from(Arc::new(Parker(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    loop {
        if cancel.flag.load(Ordering::Acquire) {
            return None;
        }
        if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
            return Some(v);
        }
        std::thread::park();
    }
}

/// An async sleep that needs no runtime.
pub async fn sleep(d: Duration) {
    struct Sleep(Arc<Mutex<(bool, Option<Waker>)>>, Duration, bool);
    impl Future for Sleep {
        type Output = ();
        fn poll(mut self: Pin<&mut Self>, c: &mut Context<'_>) -> Poll<()> {
            let state = self.0.clone();
            let mut g = state.lock_ok();
            if g.0 {
                return Poll::Ready(());
            }
            g.1 = Some(c.waker().clone());
            if !self.2 {
                self.2 = true;
                let (s, d) = (self.0.clone(), self.1);
                std::thread::spawn(move || {
                    std::thread::sleep(d);
                    let mut g = s.lock_ok();
                    g.0 = true;
                    if let Some(w) = g.1.take() {
                        w.wake();
                    }
                });
            }
            Poll::Pending
        }
    }
    Sleep(Arc::new(Mutex::new((false, None))), d, false).await
}

/// Apply finished background work and fire due refresh timers. Runs on the UI thread.
pub(crate) fn pump() {
    let done: Vec<Job> = std::mem::take(&mut *DONE.lock_ok());
    for job in done {
        job();
    }
    let ticks: Vec<_> = TICKERS.lock_ok().clone();
    for t in ticks {
        t();
    }
}

/// True when no fetch is running.
pub(crate) fn idle() -> bool {
    INFLIGHT.load(Ordering::Relaxed) == 0 && DONE.lock_ok().is_empty()
}

/// How often the UI should call [`pump`], if anything is pending.
pub(crate) fn pump_period() -> Option<Duration> {
    if INFLIGHT.load(Ordering::Relaxed) > 0 {
        Some(Duration::from_millis(50))
    } else if !TICKERS.lock_ok().is_empty() {
        Some(Duration::from_millis(500))
    } else {
        None
    }
}

struct Inner<T> {
    data: Option<T>,
    error: Option<String>,
    busy: bool,
    fetch: Arc<dyn Fn() -> Fut<T> + Send + Sync>,
    every: Option<(Duration, Instant)>,
    ticking: bool,
    auto: Option<crate::Local<bool>>,
    /// Bumped by every cancel or restart; a finished fetch applies only if it still matches.
    generation: u64,
    running: Option<Arc<Cancel>>,
    key: Option<Box<dyn std::any::Any + Send>>,
}

impl<T> Inner<T> {
    fn new(fetch: Arc<dyn Fn() -> Fut<T> + Send + Sync>) -> Self {
        Inner {
            data: None,
            error: None,
            busy: false,
            fetch,
            every: None,
            ticking: false,
            auto: None,
            generation: 0,
            running: None,
            key: None,
        }
    }
}

fn boxed<T, E, F, Fut2>(f: F) -> Arc<dyn Fn() -> Fut<T> + Send + Sync>
where
    E: std::fmt::Display,
    F: Fn() -> Fut2 + Send + Sync + 'static,
    Fut2: Future<Output = Result<T, E>> + Send + 'static,
    T: 'static,
{
    Arc::new(move || {
        let f = f();
        Box::pin(async move { f.await.map_err(|e| e.to_string()) })
    })
}

/// Data fetched in the background. Made by [`resource`].
pub struct Resource<T>(Arc<Mutex<Inner<T>>>);

impl<T> Clone for Resource<T> {
    fn clone(&self) -> Self {
        Resource(self.0.clone())
    }
}

/// Load `fetch()` in the background the first time this runs, keep the result between
/// frames, and read it with [`Resource::show`]. `fetch` is any `async fn() -> Result<T, E: Display>`.
#[track_caller]
pub fn resource<T, E, F, Fut2>(fetch: F) -> Resource<T>
where
    T: Send + 'static,
    E: std::fmt::Display,
    F: Fn() -> Fut2 + Send + Sync + 'static,
    Fut2: Future<Output = Result<T, E>> + Send + 'static,
{
    let fresh = std::cell::Cell::new(false);
    let slot = cx::slot(auto_id(30), || {
        fresh.set(true);
        Inner::<T>::new(boxed(fetch))
    });
    let r = Resource(slot);
    if fresh.get() {
        r.reload();
    }
    r
}

/// Like [`resource`], but the data depends on `key` (a search query, a selected id): whenever
/// `key` differs from the last frame the running fetch is **cancelled** (its future is dropped,
/// and a late result can never overwrite newer data) and `fetch(key)` starts. The previous data
/// stays visible while the new fetch runs.
///
/// For search-as-you-type, begin the fetch with `sleep(secs(0.3)).await`: a key that changes
/// again within that time cancels the fetch before it does any work, which is a debounce.
#[track_caller]
pub fn resource_for<K, T, E, F, Fut2>(key: K, fetch: F) -> Resource<T>
where
    K: PartialEq + Clone + Send + Sync + 'static,
    T: Send + 'static,
    E: std::fmt::Display,
    F: Fn(K) -> Fut2 + Send + Sync + 'static,
    Fut2: Future<Output = Result<T, E>> + Send + 'static,
{
    let slot = cx::slot(auto_id(31), || {
        Inner::<T>::new(boxed(|| async { Err::<T, String>("no key".into()) }))
    });
    let r = Resource(slot);
    let changed = {
        let g = r.0.lock_ok();
        g.key.as_ref().and_then(|k| k.downcast_ref::<K>()) != Some(&key)
    };
    if changed {
        {
            let mut g = r.0.lock_ok();
            g.fetch = boxed({
                let key = key.clone();
                move || fetch(key.clone())
            });
            g.key = Some(Box::new(key));
        }
        r.restart();
    }
    r
}

impl<T: Send + 'static> Resource<T> {
    /// Fetch again now (ignored while a fetch is running). The old data stays visible meanwhile.
    pub fn reload(&self) {
        let (fut, generation, cancel) = {
            let mut g = self.0.lock_ok();
            if g.busy {
                return;
            }
            g.busy = true;
            let cancel = Arc::new(Cancel {
                flag: AtomicBool::new(false),
                worker: Mutex::new(None),
            });
            g.running = Some(cancel.clone());
            ((g.fetch)(), g.generation, cancel)
        };
        INFLIGHT.fetch_add(1, Ordering::Relaxed);
        let inner = self.0.clone();
        let token = cancel.clone();
        let worker = std::thread::spawn(move || {
            let result = block_on_cancellable(fut, &token);
            DONE.lock_ok().push(Box::new(move || {
                let mut g = inner.lock_ok();
                // A cancelled or superseded fetch applies nothing.
                if let (Some(result), true) = (result, g.generation == generation) {
                    match result {
                        Ok(v) => (g.data, g.error) = (Some(v), None),
                        Err(e) => g.error = Some(e),
                    }
                    g.busy = false;
                    g.running = None;
                    if let Some((d, next)) = &mut g.every {
                        *next = Instant::now() + *d;
                    }
                }
                INFLIGHT.fetch_sub(1, Ordering::Relaxed);
            }));
        });
        *cancel.worker.lock_ok() = Some(worker.thread().clone());
    }

    /// Stop the running fetch, if any, and keep what was shown before. The future is dropped
    /// at its next suspension point and its result, should it still arrive, is ignored.
    pub fn cancel(&self) {
        let mut g = self.0.lock_ok();
        g.generation += 1;
        g.busy = false;
        if let Some(c) = g.running.take() {
            c.cancel();
        }
    }

    /// [`cancel`](Self::cancel) the running fetch and start a new one now.
    pub fn restart(&self) {
        self.cancel();
        self.reload();
    }

    /// Refetch every `period` while `enabled`: `.every_if(auto.get(), secs(10.0))`.
    pub fn every_if(self, enabled: bool, period: Duration) -> Self {
        {
            let mut g = self.0.lock_ok();
            match (enabled, g.every) {
                (true, None) => g.every = Some((period, Instant::now() + period)),
                (false, Some(_)) => g.every = None,
                _ => {}
            }
            if !g.ticking {
                g.ticking = true;
                let me = self.clone();
                TICKERS.lock_ok().push(Arc::new(move || {
                    let due = matches!(me.0.lock_ok().every, Some((_, at)) if Instant::now() >= at);
                    if due {
                        me.reload();
                    }
                }));
            }
        }
        self
    }

    /// Let the user switch periodic refetching on and off (starts off): shows an
    /// "Auto refresh" switch in [`header`](Self::header) and refetches every `period` while it is on.
    #[track_caller]
    pub fn auto(self, period: Duration) -> Self {
        let on = crate::local(false);
        self.0.lock_ok().auto = Some(on);
        self.every_if(on.get(), period)
    }

    /// A title row for a page that shows this data: the heading, then the auto-refresh switch
    /// (if [`auto`](Self::auto) was used) and a Refresh button, right-aligned.
    pub fn header<S: 'static>(&self, title: &str) -> Node<Act<S>> {
        let mut items: Vec<Node<Act<S>>> = vec![heading(title).into(), crate::space()];
        if let Some(on) = self.0.lock_ok().auto {
            use crate::FlagCell;
            items.push(on.switch("Auto refresh").into());
        }
        items.push(self.refresh_button());
        crate::row(items).align_center().gap(12.0).into()
    }

    /// Like [`every_if`](Self::every_if), always on.
    pub fn every(self, period: Duration) -> Self {
        self.every_if(true, period)
    }

    /// Whether a fetch is running.
    pub fn busy(&self) -> bool {
        self.0.lock_ok().busy
    }

    /// A copy of the latest data, if any has arrived.
    pub fn get(&self) -> Option<T>
    where
        T: Clone,
    {
        self.0.lock_ok().data.clone()
    }

    /// A Refresh button that is disabled while loading.
    pub fn refresh_button<S: 'static>(&self) -> Node<Act<S>> {
        let me = self.clone();
        let b = button("Refresh");
        if self.busy() {
            return b.into();
        }
        b.act(move |_: &mut S| me.reload()).into()
    }

    /// A whole page for this data: [`header`](Self::header) over [`show`](Self::show).
    pub fn page<S: 'static, N: Into<Node<Act<S>>>>(
        &self,
        title: &str,
        view: impl FnOnce(&T) -> N,
    ) -> Node<Act<S>> {
        col([self.header(title), self.show(|d| view(d).into())])
            .gap(16.0)
            .pad(24.0)
            .fill()
            .into()
    }

    /// The view for the current state: "Loading…" before the first data, an error card with
    /// Retry if the first load failed, otherwise `view(data)` (with a note above it if a
    /// later refresh failed).
    pub fn show<S: 'static>(&self, view: impl FnOnce(&T) -> Node<Act<S>>) -> Node<Act<S>> {
        let g = self.0.lock_ok();
        let retry = self.clone();
        match (&g.data, &g.error) {
            (Some(d), err) => {
                let body = view(d);
                match err {
                    Some(e) => col([
                        caption(format!("Refresh failed: {e}")).danger().into(),
                        body,
                    ])
                    .gap(12.0)
                    .fill()
                    .into(),
                    None => body,
                }
            }
            (None, Some(e)) => col([
                Node::from(heading("Could not load")),
                caption(e.clone()).danger().into(),
                button("Retry").act(move |_: &mut S| retry.reload()).into(),
            ])
            .gap(10.0)
            .pad(40.0)
            .into(),
            (None, None) => col([Node::<Act<S>>::from(text("Loading…").dim())])
                .pad(40.0)
                .into(),
        }
    }
}

/// The timer that applies finished fetches and fires refreshes, for `Act` apps.
pub(crate) fn pump_sub<S: 'static>() -> Option<crate::Sub<Act<S>>> {
    pump_period().map(|p| crate::every(p, || Act::new(|_: &mut S| pump())))
}
