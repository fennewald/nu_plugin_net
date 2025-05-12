use std::{
    cell::RefCell,
    fmt,
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, ContextBuilder, LocalWaker, Poll, Waker},
};

use super::JoinHandle;

/// Spawn a future onto the runtime
pub fn spawn<T: 'static>(f: impl Future<Output = T> + 'static) -> JoinHandle<T> {
    let (handle, fut) = JoinHandle::wrap(f);
    crate::rt::executor::ready(TaskRef::new(fut));
    handle
}

pub(super) enum TaskResult<T> {
    /// The task is not yet complete, and no one is `await`ing it
    Unawaited,
    /// The task is actively being `await`ed
    Awaited(LocalWaker),
    /// The task is complete. Here is it's result
    Complete(T),
    Taken,
}

struct Task {
    future: Pin<Box<dyn Future<Output = ()>>>,
}

impl Task {
    fn new(f: impl Future<Output = ()> + 'static) -> Self {
        Task {
            future: Box::pin(f),
        }
    }

    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        self.future.as_mut().poll(cx)
    }
}

mod local_waker {
    use std::{
        rc::Rc,
        task::{RawWaker, RawWakerVTable},
    };

    // TODO: create eager wakers for timers

    pub const VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);

    unsafe fn clone(data: *const ()) -> RawWaker {
        log::trace!("cloning task {:?}", data);
        Rc::increment_strong_count(data as super::TaskRefPtr);
        RawWaker::new(data, &VTABLE)
    }

    unsafe fn wake(data: *const ()) {
        log::trace!("waking task {:?}", data);
        let this = super::TaskRef::from_raw(data as _);
        this.enqueue();
    }

    unsafe fn wake_by_ref(data: *const ()) {
        // log::trace!("waking task {:?} by ref", data);
        Rc::increment_strong_count(data as super::TaskRefPtr);
        wake(data);
    }

    unsafe fn drop(data: *const ()) {
        // log::trace!("dropping task {:?}", data);
        let this = super::TaskRef::from_raw(data as _);
        std::mem::drop(this);
    }
}

type TaskRefPtr = *const RefCell<Task>;

impl TaskRef {}

#[derive(Clone)]
pub(super) struct TaskRef {
    inner: Rc<RefCell<Task>>,
}

impl From<Task> for TaskRef {
    fn from(value: Task) -> Self {
        Self {
            inner: Rc::new(RefCell::new(value)),
        }
    }
}

impl fmt::Debug for TaskRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Task@{:?}", self.inner.as_ptr())
    }
}

impl TaskRef {
    pub(super) fn new(f: impl Future<Output = ()> + 'static) -> Self {
        Task::new(f).into()
    }

    unsafe fn from_raw(ptr: TaskRefPtr) -> Self {
        Self {
            inner: Rc::from_raw(ptr),
        }
    }

    fn into_raw(self) -> TaskRefPtr {
        Rc::into_raw(self.inner)
    }

    fn into_waker(self) -> LocalWaker {
        unsafe { LocalWaker::new(self.into_raw() as _, &local_waker::VTABLE) }
    }

    pub(super) fn enqueue(self) {
        super::executor::ready(self);
    }

    pub(super) fn poll(&self) -> Poll<()> {
        let waker = self.clone().into_waker();
        let mut cx = ContextBuilder::from_waker(Waker::noop())
            .local_waker(&waker)
            .build();
        self.inner.borrow_mut().poll(&mut cx)
    }
}
