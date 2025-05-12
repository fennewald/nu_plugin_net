use std::{cell::RefCell, io, ops::ControlFlow};

use super::{io::IoDriver, time::TimeDriver};

thread_local! {
    static REACTOR: RefCell<Reactor> = RefCell::new(Reactor::new());
}

pub(super) fn with_reactor<F, O>(f: F) -> O
where
    F: FnOnce(&mut Reactor) -> O,
{
    REACTOR.with_borrow_mut(f)
}

/// Marks any timer tasks that are expired as ready
pub(super) fn wake_elapsed() {
    with_reactor(|r| r.wake_elapsed());
}

pub(super) fn block() -> ControlFlow<io::Result<()>> {
    with_reactor(|r| r.block())
}

pub(super) struct Reactor {
    pub(super) io: IoDriver,
    pub(super) time: TimeDriver,
}

impl Reactor {
    fn new() -> Self {
        Self {
            io: IoDriver::new(),
            time: TimeDriver::new(),
        }
    }

    pub(super) fn wake_elapsed(&mut self) -> usize {
        self.time.wake_elapsed()
    }

    pub(super) fn block(&mut self) -> ControlFlow<io::Result<()>> {
        let deadline = self.time.deadline();
        if !self.io.is_empty() {
            match self.io.poll(deadline) {
                Ok(n) => {
                    log::debug!("io poll awoke {} events", n);
                    ControlFlow::Continue(())
                }
                Err(e) => ControlFlow::Break(Err(e)),
            }
        } else if let Some(deadline) = deadline {
            log::trace!("waiting for timer");
            std::thread::sleep_until(deadline);
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(Ok(()))
        }
    }
}
