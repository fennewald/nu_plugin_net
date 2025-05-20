use std::{
    cell::UnsafeCell,
    collections::VecDeque,
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, LocalWaker, Poll},
};

use futures::Stream;

use super::Closed;

/// Every TRIM_INTERVAL operations, maybe shrink the internal queue if it's being stressed
const TRIM_INTERVAL: usize = 100;

/// Creates an unbounded channel with a given pre-allocated capacity. The channel is still
/// infinitely buffered, the supplied capacity is just pre-allocated.
pub fn with_capacity<T>(capacity: usize) -> (Sender<T>, Receiver<T>) {
    let inner = Rc::new(UnsafeCell::new(Inner {
        closed: false,
        ticks_since_trim: 0,
        items: VecDeque::with_capacity(capacity),
        waker: None,
    }));

    (Sender(inner.clone()), Receiver(inner))
}

/// Creates a standard, unbounded, asynchronous channel
pub fn new<T>() -> (Sender<T>, Receiver<T>) {
    let inner = Rc::new(UnsafeCell::new(Inner {
        closed: false,
        ticks_since_trim: 0,
        items: VecDeque::new(),
        waker: None,
    }));

    (Sender(inner.clone()), Receiver(inner))
}

struct Inner<T> {
    /// Is the channel closed?
    closed: bool,
    /// Steps since we've done a trim check
    ticks_since_trim: usize,
    items: VecDeque<T>,
    /// A waker to turn-on this task
    waker: Option<LocalWaker>,
}

impl<T> Inner<T> {
    /// Tick once, maybe trimming
    fn tick(&mut self) {
        self.ticks_since_trim += 1;
        if self.ticks_since_trim >= TRIM_INTERVAL {
            self.trim();
        }
    }

    fn close(&mut self) {
        if !self.closed {
            self.closed = true;
            self.wake();
        }
    }

    fn trim(&mut self) {
        self.ticks_since_trim = 0;
        let unused = self.items.capacity() - self.items.len();
        // If the unused space is at least one quarter the total capacity, we should trim
        if unused >= self.items.capacity() / 4 && unused >= 32 {
            // Leave a few slack elements in there
            self.items.shrink_to(self.items.len() + 3);
        }
    }

    fn wake(&mut self) {
        if let Some(w) = self.waker.take() {
            w.wake();
        }
    }

    fn poll_next(&mut self, cx: &mut Context<'_>) -> Poll<Result<T, Closed>> {
        if let Some(it) = self.items.pop_front() {
            self.tick();
            Poll::Ready(Ok(it))
        } else if self.closed {
            Poll::Ready(Err(Closed))
        } else {
            // Channel is open, but there aren't any elements ready yet
            if let Some(ref mut old) = self.waker {
                old.clone_from(cx.local_waker());
            } else {
                self.waker = Some(cx.local_waker().clone());
            }
            Poll::Pending
        }
    }
}

type Ref<T> = Rc<UnsafeCell<Inner<T>>>;

#[derive(Clone)]
#[repr(transparent)]
pub struct Sender<T>(Ref<T>);

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        // For our purposes, we assume that the receiver is still alive, and one of these refs
        if Rc::strong_count(&self.0) == 2 {
            unsafe { self.inner().close() }
        }
    }
}

impl<T> Sender<T> {
    /// # Safety
    /// Caller must guarantee this reference never leaves this module
    unsafe fn inner(&self) -> &mut Inner<T> {
        &mut *self.0.get()
    }

    pub fn len(&self) -> usize {
        unsafe { self.inner().items.len() }
    }

    pub fn close(&self) {
        unsafe {
            self.inner().close();
        }
    }

    pub fn send(&self, msg: T) -> Result<(), Closed> {
        let this = unsafe { self.inner() };
        if this.closed {
            Err(Closed)
        } else {
            this.items.push_back(msg);
            this.tick();
            this.wake();
            Ok(())
        }
    }
}

#[repr(transparent)]
pub struct Receiver<T>(Ref<T>);

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        unsafe { self.inner().close() }
    }
}

impl<T> Receiver<T> {
    /// # Safety
    /// Caller must guarantee this reference never leaves this module
    unsafe fn inner(&self) -> &mut Inner<T> {
        &mut *self.0.get()
    }

    pub fn is_closed(&self) -> bool {
        unsafe { self.inner().closed }
    }

    pub async fn recv(&mut self) -> Result<T, Closed> {
        RecvFuture { receiver: self }.await
    }

    pub fn len(&self) -> usize {
        unsafe { self.inner().items.len() }
    }
}

struct RecvFuture<'c, T> {
    receiver: &'c mut Receiver<T>,
}

impl<'c, T> Unpin for RecvFuture<'c, T> {}

impl<'c, T> Future for RecvFuture<'c, T> {
    type Output = Result<T, Closed>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = unsafe { self.receiver.inner() };
        this.poll_next(cx)
    }
}

impl<T> Stream for Receiver<T> {
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = unsafe { self.inner() };
        this.poll_next(cx).map(|r| r.ok())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let lower = self.len();
        let upper = if self.is_closed() { Some(lower) } else { None };
        (lower, upper)
    }
}
