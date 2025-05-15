//! Contains the logic for the run-time task queue
#[cfg(debug_assertions)]
use std::cell::RefCell;
#[cfg(not(debug_assertions))]
use std::cell::UnsafeCell;
use std::collections::VecDeque;

use super::task::{RawTask, ScheduleError};

#[cfg(debug_assertions)]
thread_local! {
    static RUN_QUEUE: RefCell<VecDeque<RawTask>> = RefCell::new(VecDeque::new());
}

#[cfg(not(debug_assertions))]
thread_local! {
    static RUN_QUEUE: UnsafeCell<VecDeque<RawTask>> = UnsafeCell::new(VecDeque::new());
}

/// In debug mode, use a refcell
#[cfg(debug_assertions)]
fn with_runqueue<F, O>(f: F) -> O
where
    F: FnOnce(&mut VecDeque<RawTask>) -> O,
{
    RUN_QUEUE.with(|q| (f)(&mut q.borrow_mut()))
}

/// In release mode, use an unsafecell
#[cfg(not(debug_assertions))]
fn with_runqueue<F, O>(f: F) -> O
where
    F: FnOnce(&mut VecDeque<RawTask>) -> O,
{
    // SAFETY: We _only_ ever access RUN_QUEUE from this function and `try_schedule`, and since we're
    // in a thread-local context, this is always mutually exclusive
    RUN_QUEUE.with(|q| (f)(unsafe { q.get().as_mut().unwrap() }))
}

/// Returns the next task to be run from the run queue
pub(super) fn pop_task() -> Option<RawTask> {
    with_runqueue(|rq| rq.pop_front()).inspect(|task| task.unset_scheduled())
}

/// Attempts to schedule a task for processing
pub(super) fn try_schedule(task: RawTask) -> Result<(), ScheduleError> {
    task.mark_scheduled()?;
    with_runqueue(|q| q.push_back(task));
    Ok(())
}

/// Schedules a task for processing, `panic`ing if an error is encountered.
pub(super) fn schedule(task: RawTask) {
    let meta = *task.meta();
    if let Err(e) = try_schedule(task) {
        log::trace!("failed to wake task {}: {}", meta, e);
    }
}
