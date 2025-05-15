use std::{
    future::Future,
    task::{Context, LocalWaker, Poll},
};

use super::{JoinResult, PollError, RawPtr, Slot};

/// A static table defining all of the type-dependent behavior for a task
pub(super) struct VTable {
    /// Poll the underlying task.
    /// The first argument is a borrowed, pinned, pointer. It's ref-count must not be consumed
    pub(super) poll: unsafe fn(RawPtr, cx: &mut Context<'_>) -> Result<Poll<()>, PollError>,
    /// Poll the output of the task
    pub(super) take: unsafe fn(RawPtr, dst: *mut (), waker: &LocalWaker),
    /// Drop the future. This should be called once the ref-count has reached 0
    pub(super) drop: unsafe fn(RawPtr),
    /// Cancel the future
    pub(super) cancel: unsafe fn(RawPtr),
}

impl VTable {
    pub(super) const fn new<T: Future>() -> &'static VTable {
        &VTable {
            poll: poll::<T>,
            take: take::<T>,
            drop: dealloc::<T>,
            cancel: cancel::<T>,
        }
    }
}

fn poll<T: Future>(ptr: RawPtr, cx: &mut Context<'_>) -> Result<Poll<()>, PollError> {
    let slot = unsafe { ptr.cast::<Slot<T>>().as_ref() };
    slot.poll(cx)
}

fn take<T: Future>(ptr: RawPtr, dst: *mut (), waker: &LocalWaker) {
    let slot = unsafe { ptr.cast::<Slot<T>>().as_ref() };
    let dst = unsafe { (dst as *mut Poll<JoinResult<T::Output>>).as_mut().unwrap() };
    slot.try_take(dst, waker)
}

fn dealloc<T: Future>(ptr: RawPtr) {
    let slot = unsafe { Box::from_non_null(ptr.cast::<Slot<T>>()) };
    assert_eq!(slot.header.ref_count(), 0);
    drop(slot);
}

fn cancel<T: Future>(ptr: RawPtr) {
    let slot = unsafe { ptr.cast::<Slot<T>>().as_ref() };
    slot.cancel();
}
