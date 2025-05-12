use std::{
    cell::RefCell,
    collections::VecDeque,
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, LocalWaker, Poll},
};

struct Inner<T> {
    items: VecDeque<T>,
    fut: Option<LocalWaker>,
}

impl<T> Inner<T> {
    fn wake(&mut self) {
        if let Some(w) = self.fut.take() {
            w.wake();
        }
    }
}

type InnerRef<T> = Rc<RefCell<Inner<T>>>;

pub fn with_capacity<T>(capacity: usize) -> (Sender<T>, Receiver<T>) {
    let lhs = Rc::new(RefCell::new(Inner {
        items: VecDeque::with_capacity(capacity),
        fut: None,
    }));

    let rhs = lhs.clone();

    (Sender(lhs), Receiver(rhs))
}

#[derive(Clone)]
pub struct Sender<T>(InnerRef<T>);

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        self.0.borrow_mut().wake();
    }
}

impl<T> Sender<T> {
    pub fn send(&self, item: T) {
        let mut inner = self.0.borrow_mut();
        inner.items.push_back(item);

        inner.wake();
    }

    pub fn len(&self) -> usize {
        self.0.borrow().items.len()
    }
}

pub struct Receiver<T>(InnerRef<T>);

impl<T> Receiver<T> {
    /// Tries to pop an item from the channel. Returns None if no items are available
    pub fn try_recv(&self) -> Option<T> {
        self.0.borrow_mut().items.pop_front()
    }

    /// Tests if the channel is closed
    pub fn closed(&self) -> bool {
        Rc::strong_count(&self.0) == 1
    }

    pub fn len(&self) -> usize {
        self.0.borrow().items.len()
    }

    pub async fn recv(&mut self) -> Option<T> {
        RecvFuture { chan: self }.await
    }
}

impl<T> futures::Stream for Receiver<T> {
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.closed() {
            return Poll::Ready(None);
        }

        let mut inner = self.0.borrow_mut();

        if let Some(it) = inner.items.pop_front() {
            inner.fut = None;
            Poll::Ready(Some(it))
        } else {
            if let Some(fut) = inner.fut.as_mut() {
                fut.clone_from(cx.local_waker());
            } else {
                inner.fut = Some(cx.local_waker().clone());
            }
            Poll::Pending
        }
    }
}

struct RecvFuture<'c, T> {
    chan: &'c mut Receiver<T>,
}

impl<'c, T> Unpin for RecvFuture<'c, T> {}

impl<'c, T> Future for RecvFuture<'c, T> {
    type Output = Option<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.chan.closed() {
            return Poll::Ready(None);
        }

        let mut inner = self.chan.0.borrow_mut();

        if let Some(it) = inner.items.pop_front() {
            inner.fut = None;
            Poll::Ready(Some(it))
        } else {
            if let Some(fut) = inner.fut.as_mut() {
                fut.clone_from(cx.local_waker());
            } else {
                inner.fut = Some(cx.local_waker().clone());
            }
            Poll::Pending
        }
    }
}
