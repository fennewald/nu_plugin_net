use std::{
    fmt,
    future::Future,
    ptr::NonNull,
    task::{ContextBuilder, LocalWaker, Poll, Waker},
};

use super::{Header, Metadata, PollError, ScheduleError, Slot, State, VTable};

pub(super) type RawPtr = NonNull<Header>;

pub(crate) struct RawTask {
    /// SAFETY: `ptr` must always reference a valid Cell
    ptr: RawPtr,
}

impl Clone for RawTask {
    fn clone(&self) -> Self {
        // SAFETY: We are constructing a raw waker which will decrement it's reference count
        // as-needed when freed
        unsafe { self.inc_ref() };
        Self { ptr: self.ptr }
    }
}

impl Drop for RawTask {
    fn drop(&mut self) {
        // SAFETY: we were given this reference when we were clone'd or created, so we have the
        // right to do this.
        unsafe {
            if self.state().dec_ref() {
                log::trace!("freeing task {:?}", self.meta());
                (self.vtable().drop)(self.ptr)
            }
        }
    }
}

impl fmt::Debug for RawTask {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // TODO: enhance
        self.meta().fmt(f)
    }
}

impl RawTask {
    // Constructors
    ////////////////////////////////////////////////////////////////////////////////////////////////

    /// Creates a new RawTask from an owned Box
    pub(super) fn from_box<T: Future>(bx: Box<Slot<T>>) -> Self {
        let ptr = Box::into_raw(bx);
        let ptr = unsafe { NonNull::new_unchecked(ptr.cast()) };
        Self { ptr }
    }

    /// Creates an owned handle from `raw`
    ///
    /// # Safety
    /// `raw` _*must*_ be a valid, _owning_ pointer to a slot
    pub(super) const unsafe fn from_raw(raw: *mut Header) -> Self {
        Self {
            ptr: NonNull::new(raw).unwrap(),
        }
    }

    /// Creates an owned handle from a _borrowed_ raw pointer
    /// Distinct from `from_raw` in that it does _not_ take ownership of the provided pointer
    ///
    /// # Safety
    /// `raw` must be a pointer to a valid Slot.
    pub(super) unsafe fn from_raw_ref(raw: *mut Header) -> Self {
        let this = unsafe { Self::from_raw(raw) };
        unsafe { this.state().inc_ref() };
        this
    }

    // Consumers
    ////////////////////////////////////////////////////////////////////////////////////////////////

    /// Convert `self` into it's raw pointer. This takes ownership of the reference count. The caller
    /// must ensure it is properly tracked
    pub(super) unsafe fn into_raw(self) -> RawPtr {
        let ptr = self.ptr;
        std::mem::forget(self);
        ptr
    }

    // Getters
    ////////////////////////////////////////////////////////////////////////////////////////////////

    /// Returns a reference to the header of the task
    const fn header(&self) -> &Header {
        // SAFETY: This is always valid, as a precondition of this struct existing
        unsafe { self.ptr.as_ref() }
    }

    /// Returns a reference to the task's metadata
    pub(crate) const fn meta(&self) -> &Metadata {
        &self.header().meta
    }

    /// Returns a reference to the task's `state` field
    pub(super) const fn state(&self) -> &State {
        &self.header().state
    }

    /// Retruns a reference to the task's vtable
    const fn vtable(&self) -> &'static VTable {
        self.header().vtable
    }

    /// Tests if this task is cancelled
    pub(crate) const fn is_cancelled(&self) -> bool {
        self.state().is_cancelled()
    }

    // Helpers
    ////////////////////////////////////////////////////////////////////////////////////////////////

    /// Increments the reference count by 1
    /// # Safety
    /// The resulting ref count must be tracked, and eventually relased. When released, it should be
    /// freed, if the resulting refcount is 0.
    unsafe fn inc_ref(&self) {
        unsafe { self.state().inc_ref() }
    }

    fn into_waker(self) -> LocalWaker {
        super::waker::into_waker(self)
    }

    // Behaviors
    ////////////////////////////////////////////////////////////////////////////////////////////////

    /// Poll the underlying future
    pub(crate) fn poll(self) -> Result<Poll<()>, PollError> {
        let ptr = self.ptr;
        let pol = self.vtable().poll;
        let wak = self.into_waker();
        let mut cx = ContextBuilder::from_waker(Waker::noop())
            .local_waker(&wak)
            .build();

        unsafe { (pol)(ptr, &mut cx) }
    }

    /// Attempt to steal the result of the task. If not ready, stores the provided waker
    ///
    /// `dst` _must_ be a pointer to a `Poll<JoinResult<T::Output>>`
    pub(super) fn try_take(&self, dst: *mut (), waker: &LocalWaker) {
        unsafe { (self.vtable().take)(self.ptr, dst, waker) }
    }

    /// Cancel this task
    /// Should be called by the executor if it notices a task is cancelled
    pub(crate) fn cancel(self) {
        unsafe { (self.vtable().cancel)(self.ptr) }
    }

    /// Request this task be cancelled
    pub(crate) fn request_cancel(&self) {
        self.state().mark_cancelled();
        if !self.state().is_scheduled() {
            crate::rt::schedule(self.clone());
        }
    }

    pub(crate) fn mark_scheduled(&self) -> Result<(), ScheduleError> {
        self.state().mark_scheduled()
    }

    pub(crate) fn unset_scheduled(&self) {
        self.state().unset_scheduled();
    }
}
