use std::{
    cell::RefCell,
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, LocalWaker, Poll},
};

enum TaskResult<T> {
    /// The task is not yet complete, and no one is `await`ing it
    Unawaited,
    /// The task is actively being `await`ed
    Awaited(LocalWaker),
    /// The task is complete. Here is it's result
    Complete(T),
    Taken,
}

impl<T> Default for TaskResult<T> {
    fn default() -> Self {
        Self::Unawaited
    }
}

impl<T> TaskResult<T> {
    fn complete(&mut self, val: T) {
        match std::mem::replace(self, Self::Complete(val)) {
            TaskResult::Unawaited => {}
            TaskResult::Awaited(waker) => waker.wake(),
            TaskResult::Complete(_) => panic!("overwrote stored value in complete task result"),
            TaskResult::Taken => panic!("overwrote a taken result"),
        }
    }

    /// Basically poll, but notably, self is not `Pin`
    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<T> {
        let this = std::mem::replace(self, Self::Taken);
        match this {
            Self::Unawaited => {
                *self = Self::Awaited(cx.local_waker().clone());
                Poll::Pending
            }
            Self::Awaited(mut waker) => {
                waker.clone_from(cx.local_waker());
                *self = Self::Awaited(waker);
                Poll::Pending
            }
            Self::Complete(res) => {
                // self is already Self::Taken
                Poll::Ready(res)
            }
            Self::Taken => panic!("tried to poll an already-consumed task"),
        }
    }
}

// TODO: investigate using the Pin in the await to just move the pinned ptr to the future, and store the result in-place
#[repr(transparent)]
pub struct JoinHandle<T>(Rc<RefCell<TaskResult<T>>>);

impl<T> Future for JoinHandle<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Ok(mut res) = self.0.try_borrow_mut() {
            res.poll(cx)
        } else {
            log::warn!("tried to poll myself!?");
            // TODO: is this sound??
            Poll::Pending
        }
    }
}

impl<T: 'static> JoinHandle<T> {
    fn new() -> Self {
        Self(Rc::new(RefCell::new(TaskResult::Unawaited)))
    }

    pub(super) fn wrap(
        fut: impl Future<Output = T> + 'static,
    ) -> (JoinHandle<T>, impl Future<Output = ()> + 'static) {
        let handle = JoinHandle::new();
        let res = handle.0.clone();

        (handle, async move {
            fut.await;
            // res.borrow_mut().complete(fut.await);
        })
    }
}
