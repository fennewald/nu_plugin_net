//! rt/task/mod.rs
//!
//! This is the task model for our runtime.
//!
//! # Task Life Cycle
//!
//! 0. Let's say we've got some `T: Future` that we'd like to spawn onto our runtime.
//! 1. At first, a `Box<Slot<T>>` is allocated. Notably, this struct is a per-future monomorphization.
//! 2. `Box::into_raw` our `Box<Slot<T>>` into a `NonNull<Slot<T>>`. Easy enough.
//! 3. `NonNull::cast` our `NonNull<Slot<T>>` into a `NonNull<Header>`. This is interesting for two
//!    reasons:
//!      3.1. `Slot<T>` is `repr(C)`, and has `Header` as its first field. That makes this cast
//!           legal, if a bit scary.
//!      3.2. `Header` has a field `vtable`, that contains function-pointers to all of the
//!           interesting member function of `Slot<T>`. This means we can call them from `Header`
//!           alone. (Yes, you read that right. We re-created c++-style vtables. Bjarne Stroustrup,
//!           eat your heart out).
//! 4. Wrap our `NonNull<Header>` into a smart pointer that will track reference counts, and free
//!    when nessecary (a `RawTask`).
//! 5. Hand out these wrapped pointers to anyone who wants to work with our runtime. In practice,
//!    this is just the executor and wakers.
//! 6. When ref counts drop to zero, free the allocation.
//!
//! # This looks kind of familiar...
//! Most of the task design is stolen from `tokio`. I wrote my own task system, and then I read
//! `tokio`'s, and theres was a lot better. That being said, there were several opportunities to
//! simplify structures, as we don't need to worry about concurrent access to the tasks.
//!
//! Namely, state transitions are much cheaper, as are refcount bumps. We can do away with fancy
//! task budget tracking, which sounds hard, and also stop tracking `JoinHandle` lifecycles as
//! anything more than simple `RawTask`s.
//!
//! There operations are so much cheaper, in fact, that it no longer makes sense to manually manage
//! refs outside of objects (which, in tokio's case, allows them to combine 'expensive' atomic
//! operations). To whit, `RawTask` is a Drop-guarded smart-pointer. This makes reference management
//! much easier to work with, and will likely compile to the same code.

mod meta;
pub(crate) use meta::Metadata;

mod state;
use state::State;
pub(super) use state::{PollError, ScheduleError};

mod vtable;
use vtable::VTable;

mod raw;
use raw::RawPtr;
pub(super) use raw::RawTask;

mod waker;

mod core;
use core::{Header, Slot};
use std::future::Future;

mod join_handle;
pub(crate) use join_handle::{JoinError, JoinHandle, JoinResult};

/// Allocates a new task
pub(super) fn alloc<F: Future>(name: &'static str, future: F) -> (JoinHandle<F::Output>, RawTask) {
    let slot = Slot::new(name, future);
    let task = RawTask::from_box(slot);
    let handle = JoinHandle::new(task.clone());
    (handle, task)
}
