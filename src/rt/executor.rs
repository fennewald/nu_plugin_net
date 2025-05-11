use std::{
    cell::{RefCell, RefMut},
    collections::VecDeque,
    io,
    ops::ControlFlow,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::rt::TaskRef;

thread_local! {
    static READY_QUEUE: &'static RefCell<VecDeque<TaskRef>> = make_queue();
}

/// Returns the threads ready queue. Panics if called from the non-executor thread
fn ready_queue_cell() -> &'static RefCell<VecDeque<TaskRef>> {
    READY_QUEUE.with(|v| *v)
}

/// Short-hand for a mutable ref to the ready queeu
fn ready_queue() -> RefMut<'static, VecDeque<TaskRef>> {
    ready_queue_cell().borrow_mut()
}

fn next_task() -> Option<TaskRef> {
    ready_queue().pop_front()
}

/// Returns a new leaked task queue
/// `panic`s if the function is called twice
fn make_queue() -> &'static RefCell<VecDeque<TaskRef>> {
    static INIT: AtomicBool = AtomicBool::new(false);

    if !INIT.fetch_or(true, Ordering::Relaxed) {
        Box::leak(Box::new(RefCell::new(VecDeque::new())))
    } else {
        panic!("Tried to re-initalize already initalized runtime");
    }
}

/// Signal to the executor that the provided task is ready for processing
pub(super) fn ready(fut: TaskRef) {
    ready_queue().push_back(fut);
}

/// Mark a task as ready _urgently_, which moves the task to the front of the list to be processed
pub(super) fn ready_urgent(fut: TaskRef) {
    ready_queue().push_front(fut);
}

/// Advance the global executor
pub fn run() -> io::Result<()> {
    loop {
        match step() {
            ControlFlow::Continue(()) => {}
            ControlFlow::Break(v) => return v,
        }
    }
}

/// Step the global executor forwards once
fn step() -> ControlFlow<io::Result<()>> {
    super::reactor::enqueue_elapsed();

    if let Some(task) = next_task() {
        log::trace!("polling task {:?}", task);
        let res = task.poll();
        log::trace!("poll task {:?}: {:?}", task, res);
        ControlFlow::Continue(())
    } else {
        super::reactor::block()
    }
}
