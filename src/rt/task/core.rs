use std::{
    cell::{Cell, UnsafeCell},
    future::Future,
    pin::Pin,
    task::{Context, LocalWaker, Poll},
};

use super::{JoinError, JoinResult, Metadata, PollError, State, VTable};

/// The core task cell
#[repr(C)]
pub(super) struct Slot<T: Future> {
    pub(super) header: Header,
    core: Core<T>,
}

#[repr(C)]
pub(super) struct Header {
    /// The state the task is in
    pub(super) state: State,
    /// The vtable for this task
    /// All this trouble just to re-invent C++ virtual classes -_-
    pub(super) vtable: &'static VTable,
    /// Task metadata
    pub(super) meta: Metadata,
}

#[repr(C)]
struct Core<T: Future> {
    stage: UnsafeCell<Stage<T>>,
    waker: Cell<Option<LocalWaker>>,
}

enum Stage<T: Future> {
    InProgress(T),
    Complete(T::Output),
    Cancelled,
    Consumed,
}

impl<T: Future> Stage<T> {
    /// Consumes the result of the stage, and replaces `self` with `Consumed`. `panic`s if `self` is
    /// not Complete.
    unsafe fn take(&mut self) -> T::Output {
        match std::mem::replace(self, Stage::Consumed) {
            Stage::Complete(v) => v,
            _ => unreachable!(),
        }
    }
}

impl<T: Future> Slot<T> {
    pub(super) fn new(name: &'static str, future: T) -> Box<Self> {
        Box::new(Self {
            header: Header {
                state: State::init(),
                vtable: VTable::new::<T>(),
                meta: Metadata::new(name),
            },
            core: Core {
                stage: UnsafeCell::new(Stage::InProgress(future)),
                waker: Cell::new(None),
            },
        })
    }

    pub(super) fn poll(&self, cx: &mut Context<'_>) -> Result<Poll<()>, PollError> {
        self.poll_inner(cx).map(|p| p.map(|v| self.complete(v)))
    }

    fn poll_inner(&self, cx: &mut Context<'_>) -> Result<Poll<T::Output>, PollError> {
        match unsafe { self.core.stage.get().as_mut().unwrap() } {
            Stage::Cancelled => Err(PollError::Cancelled),
            Stage::Complete(_) | Stage::Consumed => Err(PollError::Complete),
            Stage::InProgress(f) => {
                self.header.state.mark_running()?;
                // SAFETY: we're always in a non-moving box, so this is safe
                let p = unsafe { Pin::new_unchecked(f) };
                let res = p.poll(cx);
                self.header.state.unset_running();
                Ok(res)
            }
        }
    }

    /// Completes this task, storing the result, and maybe waking the future
    fn complete(&self, val: T::Output) {
        self.header.state.mark_complete();
        let future = unsafe { self.core.stage.get().as_mut().unwrap() };
        *future = Stage::Complete(val);
        self.wake_waiter();
    }

    pub(super) fn try_take(&self, dst: &mut Poll<JoinResult<T::Output>>, waker: &LocalWaker) {
        let stage = unsafe { self.core.stage.get().as_mut().unwrap() };

        match stage {
            Stage::InProgress(_) => self.update_waker(waker),
            Stage::Cancelled => *dst = Poll::Ready(Err(JoinError::cancelled(self.header.meta))),
            Stage::Consumed => *dst = Poll::Ready(Err(JoinError::consumed(self.header.meta))),
            Stage::Complete(_) => *dst = Poll::Ready(Ok(unsafe { stage.take() })),
        }
    }

    /// Cancel ourselves
    pub(super) fn cancel(&self) {
        self.header.state.mark_complete();
        let future = unsafe { self.core.stage.get().as_mut().unwrap() };
        *future = Stage::Cancelled;
        self.wake_waiter();
    }

    fn update_waker(&self, waker: &LocalWaker) {
        let out = if let Some(mut old) = self.core.waker.take() {
            old.clone_from(waker);
            old
        } else {
            waker.clone()
        };
        self.core.waker.set(Some(out));
    }

    /// Wakes the waker (if one exists)
    fn wake_waiter(&self) {
        if let Some(waker) = self.core.waker.take() {
            waker.wake();
        }
    }
}

impl Header {
    pub(super) const fn ref_count(&self) -> usize {
        self.state.ref_count()
    }
}
