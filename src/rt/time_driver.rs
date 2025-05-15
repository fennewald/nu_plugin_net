use std::{
    collections::BTreeMap,
    task::LocalWaker,
    time::{Duration, Instant},
};

pub(super) fn with_driver<F, O>(f: F) -> O
where
    F: FnOnce(&mut TimeDriver) -> O,
{
    super::with_reactor(|r| (f)(&mut r.time))
}

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

    pub(super) fn register(&mut self, deadline: Instant, waker: &LocalWaker) {
        let tasks = self.tasks.entry(deadline).or_default();
        if tasks.iter().find(|other| other.will_wake(waker)).is_none() {
            tasks.push(waker.clone());
        }
    }
}
