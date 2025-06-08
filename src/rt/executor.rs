use std::{ops::ControlFlow, task::Poll};

use super::reactor;

pub(crate) fn run() -> std::io::Result<()> {
    loop {
        if let ControlFlow::Break(res) = step() {
            return res;
        }
    }
}

fn step() -> ControlFlow<std::io::Result<()>> {
    reactor::wake_elapsed();

    if let Some(task) = super::queue::pop_task() {
        if task.is_cancelled() {
            log::debug!("Noticed task {:?} is cancelled. Removing it!", task);
            task.cancel();
        } else {
            match task.poll() {
                Ok(Poll::Ready(())) => log::trace!("task completed"),
                Ok(Poll::Pending) => {}
                Err(e) => log::trace!("Failed to poll task: {e}"),
            }
        }

        ControlFlow::Continue(())
    } else {
        // No tasks are ready, go to the reactor
        reactor::block()
    }
}
