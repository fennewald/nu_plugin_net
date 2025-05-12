use std::{
    collections::BTreeMap,
    future::Future,
    pin::Pin,
    task::{Context, LocalWaker, Poll},
    time::{Duration, Instant},
};

use futures::Stream;

// Public interface

/// Sleep for the provided duration
pub async fn sleep(dur: Duration) {
    sleep_until(Instant::now() + dur).await
}

/// Sleep until the provided moment
pub async fn sleep_until(deadline: Instant) {
    TimerFuture { deadline }.await
}

struct TimerFuture {
    deadline: Instant,
}

impl Future for TimerFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let now = Instant::now();
        if self.deadline <= now {
            log::trace!("triggered timer, {:?} off", now - self.deadline);
            Poll::Ready(())
        } else {
            log::trace!("registering new timer, duration {:?}", self.deadline - now);
            with_driver(|driver| driver.register(self.deadline, cx.local_waker()));
            Poll::Pending
        }
    }
}

/// Returns a `futures::Stream` where each future is spaced by the requested duration
/// The first item will yield immediately. `skip(1)` if you'd like the first trigger to be delayed
pub fn interval(interval: Duration) -> impl Stream<Item = ()> {
    Interval {
        target: None,
        interval,
    }
}

struct Interval {
    /// The target for the next emission, or `None` if this future has never been polled
    target: Option<Instant>,
    interval: Duration,
}

impl Unpin for Interval {}

impl Stream for Interval {
    type Item = ();

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        let now = Instant::now();

        match this.target.as_mut() {
            Some(tgt) if *tgt <= now => {
                // The requested interval has elapsed
                *tgt += this.interval;
                Poll::Ready(Some(()))
            }
            Some(tgt) => {
                with_driver(|driver| driver.register(*tgt, cx.local_waker()));
                Poll::Pending
            }
            None => {
                this.target = Some(now + this.interval);
                Poll::Ready(Some(()))
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (usize::MAX, None)
    }
}

/// Gets a handle to the driver, and then runs the requested function with that handle
fn with_driver<F, O>(f: F) -> O
where
    F: FnOnce(&mut TimeDriver) -> O,
{
    super::reactor::with_reactor(|r| (f)(&mut r.time))
}

/// The driver for timer futures
pub(super) struct TimeDriver {
    tasks: BTreeMap<Instant, Vec<LocalWaker>>,
}

impl TimeDriver {
    pub(super) fn new() -> Self {
        Self {
            tasks: BTreeMap::new(),
        }
    }

    /// Returns the deadline of the earliest timer, or `None` if no timers are enqueued
    pub(super) fn deadline(&self) -> Option<Instant> {
        self.tasks.iter().next().map(|(&t, _)| t)
    }

    /// Awaken any already-elapsed timers. Returns the number of tasks woken
    pub(super) fn wake_elapsed(&mut self) -> usize {
        let mut awoken = 0;
        let now = Instant::now();

        while let Some((time, wakers)) = self
            .tasks
            .first_entry()
            .filter(|e| *e.key() <= now)
            .map(|e| e.remove_entry())
        {
            let late = time - now;
            if late >= Duration::from_millis(1) {
                log::warn!("{:?} late for {} tasks: {:?}", late, wakers.len(), wakers);
            }
            for waker in wakers {
                awoken += 1;
                waker.wake();
            }
        }

        awoken
    }

    fn register(&mut self, deadline: Instant, waker: &LocalWaker) {
        let tasks = self.tasks.entry(deadline).or_default();
        if tasks.iter().find(|other| other.will_wake(waker)).is_none() {
            tasks.push(waker.clone());
        }
    }
}
