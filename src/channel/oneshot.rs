use std::{
    future::Future,
    mem::ManuallyDrop,
    pin::Pin,
    task::{Context, Poll},
};

use super::{NoReceiver, NoSender};

/// Creates a new oneshot channel
pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let (lhs, rhs) = Ref::new();
    (Sender(ManuallyDrop::new(lhs)), Receiver(rhs))
}

use sealed::Ref;
mod sealed {
    //! All state management is inside this sealed module, to clearly enumerate all possible state transitions

    use std::{cell::UnsafeCell, rc::Rc, task::LocalWaker};

    use super::NoSender;

    /// The internal state of the channel. The reference count of the wrapping RC is also used as
    /// part of the state.
    ///
    /// State diagram:
    /// ```
    ///                                ┌──────────────────┐
    ///                  Sender::send─▶│   RC: 0, null    │
    ///                    │           └──────────────────┘
    ///                    │                     ▲
    ///          ┌─────────┴────────┐            │
    ///          │ RC: 1, Unawaited ├──Sender::drop
    ///          └──────────────────┘
    ///                    ▲
    ///             Receiver::drop
    ///                    │
    ///          ┌─────────┴────────┐                  ┌──────────────────┐
    ///  Init───▶│ RC: 2, Unawaited │──Receiver::poll─▶│  RC: 2, Awaited  │
    ///          └─────────┬────┬───┘                  └────┬────┬────────┘
    ///                    ├────┼───────────────────────────┘    │
    ///                    │    └────────────────────────────────┤
    ///              Sender::send                          Sender::drop
    ///                    │                                     │
    ///                    ▼                                     ▼
    ///          ┌──────────────────┐                  ┌──────────────────┐
    ///          │   RC: 1, Ready   │                  │  RC: 1, Closed   │
    ///          └─────────┬────────┘                  └────────┬─────────┘
    ///                    └────────────────┬───────────────────┘
    ///                              Receiver::poll
    ///                                     ▼
    ///                           ┌──────────────────┐
    ///                           │   RC: 0, null    │
    ///                           └──────────────────┘
    /// ```
    enum State<T> {
        /// The channel has just been created. No waiter is registered, and no value is present
        Unawaited,
        /// A future is waiting on this channel
        Waiting(LocalWaker),
        /// A value is ready on this channel
        Ready(T),
        /// The sender has been dropped
        Closed,
    }

    pub(super) struct Ref<T>(Rc<UnsafeCell<State<T>>>);

    impl<T> Ref<T> {
        pub(super) fn new() -> (Self, Self) {
            let inner = Rc::new(UnsafeCell::new(State::Unawaited));
            (Self(inner.clone()), Self(inner))
        }

        fn cell(&self) -> &mut State<T> {
            unsafe { &mut *self.0.get() }
        }

        fn replace(&self, new: State<T>) -> State<T> {
            std::mem::replace(self.cell(), new)
        }

        pub(super) fn refcount(&self) -> usize {
            Rc::strong_count(&self.0)
        }

        /// Consume the `Sender` reference, and mark the channel as `Closed`
        pub(super) fn close(self) {
            match self.replace(State::Closed) {
                State::Unawaited => {}
                State::Waiting(waker) => waker.wake(),
                State::Ready(_) | State::Closed => unreachable!(),
            }
        }

        /// Consume the `Sender` reference, and mark the channel as `Ready`
        pub(super) fn ready(self, val: T) {
            match self.replace(State::Ready(val)) {
                State::Unawaited => {}
                State::Waiting(waker) => waker.wake(),
                State::Ready(_) | State::Closed => unreachable!(),
            }
        }

        /// Consumes self and the wrapped cell, returning the underlying value
        fn into_inner(self) -> Option<State<T>> {
            Rc::into_inner(self.0).map(|cell| cell.into_inner())
        }

        /// Consumes `self`, and returns the 'result' of this channel.
        /// # Safety
        /// This method may _only_ be called when the strong count of the handle is 1.
        /// This method will panic if this is not the case.
        pub(super) unsafe fn into_poll_result(self) -> Result<T, NoSender> {
            match self.into_inner().unwrap() {
                State::Unawaited | State::Waiting(_) => unreachable!(),
                State::Ready(v) => Ok(v),
                State::Closed => Err(NoSender),
            }
        }

        /// Updates the cell to awaited, possibly updating the waker in-place
        pub(super) fn awaited(&self, waker: &LocalWaker) {
            let cell = self.cell();
            if let State::Waiting(ref mut old) = cell {
                old.clone_from(waker);
            } else {
                debug_assert!(matches!(*cell, State::Unawaited));
                *cell = State::Waiting(waker.clone())
            }
        }
    }
}

// The `Ref` inside this is dropped when the state transition occurs. This happens in one of two places:
// 1. In `send`, the reference is consumed when the channel transitions to the `Ready` state
// 2. In `drop`, the reference is consumed when the channel transitions to the `Closed` state
//
// To prevent the `drop` glue from running after send, `std::mem::forget` is used
#[repr(transparent)]
pub struct Sender<T>(ManuallyDrop<Ref<T>>);

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        // SAFETY: We're inside drop, and _we_ never touch `self` again, so this is safe
        let handle = unsafe { ManuallyDrop::take(&mut self.0) };
        handle.close();
    }
}

impl<T> Sender<T> {
    /// Send an item over the channel, consuming self
    pub fn send(mut self, item: T) -> Result<(), NoReceiver> {
        let count = self.0.refcount();
        if count == 2 {
            // SAFETY: we immediately forget handle, so this is safe
            let handle = unsafe { ManuallyDrop::take(&mut self.0) };
            std::mem::forget(self);
            handle.ready(item);
            Ok(())
        } else if count == 1 {
            Err(NoReceiver)
        } else {
            unreachable!()
        }
    }
}

#[repr(transparent)]
pub struct Receiver<T>(Ref<T>);

impl<T> Receiver<T> {
    pub fn into_future(self) -> impl Future<Output = Result<T, NoSender>> {
        ReceiverFuture(Some(self.0))
    }

    pub async fn recv(self) -> Result<T, NoSender> {
        self.into_future().await
    }

    /// Is the future ready to be read? This means it either has a value, or the sender has dropped
    pub fn complete(&self) -> bool {
        self.0.refcount() == 1
    }
}

#[repr(transparent)]
struct ReceiverFuture<T>(Option<Ref<T>>);

impl<T> ReceiverFuture<T> {
    fn strong_count(&self) -> Option<usize> {
        self.0.as_ref().map(|r| r.refcount())
    }
}

impl<T> Unpin for ReceiverFuture<T> {}

impl<T> Future for ReceiverFuture<T> {
    type Output = Result<T, NoSender>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.strong_count() == Some(1) {
            // SAFETY: We just verified that self.0 is `Some`, _and_ that the strong count is 1
            Poll::Ready(unsafe { self.get_mut().0.take().unwrap().into_poll_result() })
        } else {
            // We're not the only live reference, so we should store our waker and move on
            self.0.as_ref().unwrap().awaited(cx.local_waker());
            Poll::Pending
        }
    }
}
