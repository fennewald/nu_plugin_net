use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use nu_plugin_protocol::{StreamData, StreamId};

use crate::plugin::{
    stream::{ByteStreamData, ListStreamData},
    Result,
};

use super::CoreRef;

/// The user-side handle to a currently active output pipeline
#[repr(transparent)]
pub struct Producer<D: Into<StreamData>>(CoreRef<D>);

pub type ByteProducer = Producer<ByteStreamData>;
pub type ListProducer = Producer<ListStreamData>;

impl<D> Producer<D>
where
    D: Into<StreamData>,
{
    pub(super) fn new(core: CoreRef<D>) -> Self {
        Self(core)
    }

    pub fn id(&self) -> StreamId {
        self.0.borrow().id()
    }

    /// Sets the maximum number of unacknowledged messages.
    pub fn set_max_unacked(&mut self, max: usize) {
        self.0.borrow_mut().set_max_unack(max);
    }
    /// Sends data into the pipeline. If the consumer has not `Ack`ed enough data messages, this
    /// will `await` until there is room in the stream, and then send the data. This is in
    /// contrast to `send_buffered`.
    pub async fn send(&mut self, data: D) -> Result<()> {
        SendFuture {
            core: &mut self.0,
            data: Some(data),
        }
        .await
    }

    /// Sends data into the pipeline
    pub fn send_buffered(&mut self, data: D) -> Result<()> {
        self.0.borrow_mut().push(data)
    }
}

impl<D> Drop for Producer<D>
where
    D: Into<StreamData>,
{
    fn drop(&mut self) {
        // TODO: forward error to main plugin
        if let Err(e) = self.0.borrow_mut().end() {
            log::error!("failed to end stream: {e}");
        }
    }
}

/// This future uses a borrowed mutable refernece to the adapters copy of the state
struct SendFuture<'p, D> {
    core: &'p mut CoreRef<D>,
    data: Option<D>,
}

impl<'p, D> SendFuture<'p, D>
where
    D: Into<StreamData>,
{
    // Written as a separate func to support a different self-type for the potential recursion
    fn do_poll(&mut self, cx: &mut Context<'_>) -> Poll<Result<()>> {
        let mut core = self.core.borrow_mut();

        if core.poll_room(cx.local_waker()).is_pending() {
            Poll::Pending
        } else {
            // SAFETY: this will panic if the future is called after returning `Poll::Ready`, which
            // is forbidden.
            let data = self.data.take().unwrap();
            match core.try_send(data) {
                Ok(v) => Poll::Ready(v),
                Err(data) => {
                    log::warn!("unexpected polling state. Expected room in channel.");
                    log::warn!("recovering and trying again");
                    self.data = Some(data);
                    drop(core);
                    self.do_poll(cx)
                }
            }
        }
    }
}

impl<'p, D> Unpin for SendFuture<'p, D> {}

impl<'p, D> Future for SendFuture<'p, D>
where
    D: Into<StreamData>,
{
    type Output = Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.get_mut().do_poll(cx)
    }
}
