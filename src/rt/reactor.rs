#[cfg(debug_assertions)]
use std::cell::RefCell;
#[cfg(not(debug_assertions))]
use std::cell::UnsafeCell;
use std::ops::ControlFlow;

use super::{IoDriver, TimeDriver};

#[cfg(debug_assertions)]
thread_local! {
    static REACTOR: RefCell<Reactor> = RefCell::new(Reactor::new());
}

#[cfg(not(debug_assertions))]
thread_local! {
    static REACTOR: UnsafeCell<Reactor> = UnsafeCell::new(Reactor::new());
}

/// In debug mode, use a refcell
#[cfg(debug_assertions)]
pub(super) fn with_reactor<F, O>(f: F) -> O
where
    F: FnOnce(&mut Reactor) -> O,
{
    REACTOR.with(|r| (f)(&mut r.borrow_mut()))
}

/// In release mode, use an unsafecell
#[cfg(not(debug_assertions))]
pub(super) fn with_reactor<F, O>(f: F) -> O
where
    F: FnOnce(&mut Reactor) -> O,
{
    REACTOR.with(|r| (f)(unsafe { r.get().as_mut().unwrap() }))
}

// The two executor entries
pub(super) fn wake_elapsed() -> usize {
    with_reactor(|r| r.time.wake_elapsed())
}

pub(super) fn block() -> ControlFlow<std::io::Result<()>> {
    with_reactor(|r| r.block())
}

pub(super) struct Reactor {
    pub(super) time: TimeDriver,
    pub(super) io: IoDriver,
}

impl Reactor {
    fn new() -> Self {
        Self {
            time: TimeDriver::new(),
            io: IoDriver::new(),
        }
    }

    pub(super) fn block(&mut self) -> ControlFlow<std::io::Result<()>> {
        let deadline = self.time.deadline();

        if !self.io.is_empty() {
            match self.io.poll(deadline) {
                Ok(n) => {
                    log::debug!("io event woke {} tasks", n);
                    ControlFlow::Continue(())
                }
                Err(e) => ControlFlow::Break(Err(e)),
            }
        } else if let Some(deadline) = deadline {
            std::thread::sleep_until(deadline);
            ControlFlow::Continue(())
        } else {
            // There is nothing to do. We should break
            ControlFlow::Break(Ok(()))
        }
    }
}
