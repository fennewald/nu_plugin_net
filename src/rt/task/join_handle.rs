use std::{
    fmt,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use super::{Metadata, RawTask};

pub(crate) type JoinResult<T> = std::result::Result<T, JoinError>;

#[derive(Debug, thiserror::Error)]
pub(crate) struct JoinError {
    meta: Metadata,
    reason: Reason,
}

impl fmt::Display for JoinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Failed to join task {}: {}", self.meta, self.reason)
    }
}

impl JoinError {
    pub(super) fn cancelled(meta: Metadata) -> Self {
        Self {
            meta,
            reason: Reason::Cancelled,
        }
    }
    pub(super) fn consumed(meta: Metadata) -> Self {
        Self {
            meta,
            reason: Reason::Consumed,
        }
    }
}

#[derive(Debug, thiserror::Error)]
enum Reason {
    #[error("the task was cancelled")]
    Cancelled,
    #[error("the response has already been consumed")]
    Consumed,
}

pub(crate) struct JoinHandle<T> {
    task: RawTask,
    _tag: PhantomData<T>,
}

impl<T> JoinHandle<T> {
    pub(super) fn new(task: RawTask) -> Self {
        Self {
            task,
            _tag: PhantomData,
        }
    }

    /// Abort the task associated with this handle
    pub(crate) fn cancel(&self) {
        self.task.request_cancel();
    }

    /// Tests if the task has finished
    pub(crate) const fn is_finished(&self) -> bool {
        self.task.state().is_complete()
    }

    pub(crate) const fn meta(&self) -> &Metadata {
        self.task.meta()
    }

    pub(crate) const fn name(&self) -> &'static str {
        self.meta().name()
    }

    pub(crate) const fn id(&self) -> u64 {
        self.meta().id()
    }
}

impl<T> Unpin for JoinHandle<T> {}

impl<T> Future for JoinHandle<T> {
    type Output = JoinResult<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut res = Poll::Pending;
        self.task
            .try_take(&mut res as *mut _ as *mut _, cx.local_waker());
        res
    }
}
