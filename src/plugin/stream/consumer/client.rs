use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures::Stream;
use nu_plugin_protocol::{PluginOutput, StreamId};
use nu_protocol::{ByteStreamType, PipelineMetadata, ShellError, Span};
use pin_project_lite::pin_project;

use crate::{
    channel::Sender,
    plugin::stream::{ByteStreamData, ListStreamData},
};

use super::{Adapter, Core, CoreRef};

/// The command-side handle for an incoming stream
/// It's lifetime correponds to 'drop' notion of the stream. When this handle is dropped, the `drop`
///  message is sent to the engine.
#[repr(transparent)]
struct Inner<D>(CoreRef<D>);

impl<D> Unpin for Inner<D> {}

impl<D> Inner<D> {
    /// Tests if we've received a `StreamMessage::End` yet.
    fn ended(&self) -> bool {
        self.0.borrow().ended()
    }

    fn len(&self) -> usize {
        self.0.borrow().len()
    }
}

impl<D> Drop for Inner<D> {
    fn drop(&mut self) {
        let mut this = self.0.borrow_mut();

        if let Err(e) = this.drop() {
            log::error!("failed to drop consumer handle: {e}");
            this.report_err(e);
        }
    }
}

impl<D> Stream for Inner<D> {
    type Item = D;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut inner = self.0.borrow_mut();

        if inner.dropped() {
            // We've dropped the stream
            Poll::Ready(None)
        } else if let Some(it) = inner.pop() {
            Poll::Ready(Some(it))
        } else if inner.ended() {
            // The stream has ended, and we've consumed all items
            Poll::Ready(None)
        } else {
            inner.set_waker(cx.local_waker());
            Poll::Pending
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let inner = self.0.borrow_mut();
        let lower = inner.len();

        if inner.dropped() {
            (0, Some(0))
        } else if inner.ended() {
            (lower, Some(lower))
        } else {
            (lower, None)
        }
    }
}

pin_project! {
    #[repr(transparent)]
    pub struct ListConsumer {
        #[pin]
        inner: Inner<ListStreamData>,
    }
}

impl Stream for ListConsumer {
    type Item = ListStreamData;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().inner.poll_next(cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl ListConsumer {
    pub(super) fn new(
        id: StreamId,
        span: Span,
        meta: Option<PipelineMetadata>,
        tx: Sender<PluginOutput>,
        errs: Sender<ShellError>,
        tgt_buffer: usize,
    ) -> (Adapter<ListStreamData>, Self) {
        let core = Core::new(id, span, meta, tx, errs, tgt_buffer);
        let inner = Inner(core.clone());
        (Adapter::new(core), Self { inner })
    }

    pub fn ended(&self) -> bool {
        self.inner.ended()
    }

    pub fn meta(&self) -> Option<PipelineMetadata> {
        self.inner.0.borrow().meta()
    }

    pub fn queued(&self) -> usize {
        self.inner.len()
    }
}

pin_project! {
    pub struct ByteConsumer {
        #[pin]
        inner: Inner<ByteStreamData>,
        color: ByteStreamType,
    }
}

impl Stream for ByteConsumer {
    type Item = ByteStreamData;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().inner.poll_next(cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl ByteConsumer {
    pub(super) fn new(
        id: StreamId,
        span: Span,
        meta: Option<PipelineMetadata>,
        color: ByteStreamType,
        tx: Sender<PluginOutput>,
        errs: Sender<ShellError>,
        tgt_buffer: usize,
    ) -> (Adapter<ByteStreamData>, Self) {
        let core = Core::new(id, span, meta, tx, errs, tgt_buffer);
        let inner = Inner(core.clone());
        (Adapter::new(core), Self { inner, color })
    }

    pub fn ended(&self) -> bool {
        self.inner.ended()
    }

    pub fn color(&self) -> ByteStreamType {
        self.color
    }

    pub fn meta(&self) -> Option<PipelineMetadata> {
        self.inner.0.borrow().meta()
    }

    pub fn queued(&self) -> usize {
        self.inner.len()
    }
}
