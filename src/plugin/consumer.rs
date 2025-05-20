use std::{
    cell::RefCell,
    collections::VecDeque,
    pin::Pin,
    rc::Rc,
    task::{Context, LocalWaker, Poll},
};

use futures::Stream;
use nu_plugin_protocol::{PluginOutput, StreamData, StreamId};
use nu_protocol::{ByteStreamType, LabeledError, PipelineMetadata, ShellError, Span, Value};
use pin_project_lite::pin_project;

use crate::channel::Sender;

type ByteStreamData = Result<Vec<u8>, LabeledError>;
type ListStreamData = Value;

/// The shared state of a consumer
struct State<D> {
    /// The ID of this stream
    id: StreamId,
    /// Source span of this stream
    span: Span,
    /// Optional stream metadata
    meta: Option<PipelineMetadata>,
    /// A handle to send output data over
    tx: Sender<PluginOutput>,
    /// A flag indicating that the `End` message has been received
    ended: bool,
    /// A flag indicating that the `Drop` message has been sent
    dropped: bool,
    /// A channel to signal an out-of-band error to the engine
    errs: Sender<ShellError>,
    /// The target number of items in the queue. Upon receiving an item from the engine, it may be
    /// `Ack`ed before it has actually been processed by this mechanism, to help improve
    /// performance.
    tgt_buffer: usize,
    /// If the consumer is waiting on a new message, it can put a waker for itself here
    waker: Option<LocalWaker>,
    /// The actual items in the queue
    items: VecDeque<D>,
}

/// Forget `Arc<Mutex<T>>`! Enjoy the new-and-improved `Rc<RefCell<T>>`!
type StateRef<D> = Rc<RefCell<State<D>>>;

impl<D> State<D> {
    fn new(
        id: StreamId,
        span: Span,
        meta: Option<PipelineMetadata>,
        tx: Sender<PluginOutput>,
        errs: Sender<ShellError>,
        tgt_buffer: usize,
    ) -> StateRef<D> {
        Rc::new(RefCell::new(Self {
            id,
            span,
            meta,
            tx,
            ended: false,
            dropped: false,
            errs,
            tgt_buffer,
            waker: None,
            items: VecDeque::with_capacity(std::cmp::max(tgt_buffer, 1)),
        }))
    }

    /// Called by the command-side handle when it is dropped
    fn set_dropped(&mut self) -> Result<(), ShellError> {
        if self.dropped {
            log::warn!("tried to double-drop handle");
            return Ok(());
        }

        self.dropped = true;
        self.tx
            .send(PluginOutput::Drop(self.id))
            .map_err(|_| ShellError::NushellFailedSpanned {
                msg: "An internal channel was dropped while in-use".into(),
                label: "While processing this call".into(),
                span: self.span,
            })
    }

    fn ack(&mut self) -> Result<(), ShellError> {
        self.tx
            .send(PluginOutput::Ack(self.id))
            .map_err(|e| ShellError::NushellFailedSpanned {
                msg: format!("Failed to send `Ack` message for stream {}: {}", self.id, e),
                label: "While processing this call".into(),
                span: self.span,
            })
    }

    /// Pops an item from the internal queue, perhaps sending `Ack` if needed
    fn pop(&mut self) -> Option<D> {
        let it = self.items.pop_front();

        if it.is_some() && self.items.len() <= self.tgt_buffer {
            // We need to send `Ack`
            if let Err(e) = self.ack() {
                log::error!("{e}");
                if let Err(another) = self.errs.send(e) {
                    log::error!("failed to send error message to engine: {another}");
                }
            }
        }

        it
    }

    fn push(&mut self, item: D) -> Result<(), ShellError> {
        if self.dropped {
            log::trace!(
                "ignorning data received on already-dropped stream {}",
                self.id
            );
            return Ok(());
        }

        self.items.push_back(item);

        if self.items.len() <= self.tgt_buffer {
            self.ack()?;
        }

        if let Some(waker) = self.waker.take() {
            waker.wake();
        }

        Ok(())
    }
}

pin_project! {
    #[repr(transparent)]
    pub struct ListConsumer {
        #[pin]
        inner: ConsumerInner<ListStreamData>,
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
    ) -> (ConsumerAdapter<ListStreamData>, Self) {
        let state = State::new(id, span, meta, tx, errs, tgt_buffer);
        let inner = ConsumerInner(state.clone());
        (ConsumerAdapter(state), Self { inner })
    }

    pub fn ended(&self) -> bool {
        self.inner.ended()
    }
}

pin_project! {
    pub struct ByteConsumer {
        #[pin]
        inner: ConsumerInner<ByteStreamData>,
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
    ) -> (ConsumerAdapter<ByteStreamData>, Self) {
        let state = State::new(id, span, meta, tx, errs, tgt_buffer);
        let inner = ConsumerInner(state.clone());
        (ConsumerAdapter(state), Self { inner, color })
    }

    pub fn ended(&self) -> bool {
        self.inner.ended()
    }

    pub fn color(&self) -> ByteStreamType {
        self.color
    }
}

/// The command-side handle for an incoming stream
/// It's lifetime correponds to 'drop' notion of the stream. When this handle is dropped, the `drop`
///  message is sent to the engine.
#[repr(transparent)]
struct ConsumerInner<D>(StateRef<D>);

impl<D> Unpin for ConsumerInner<D> {}

impl<D> ConsumerInner<D> {
    /// Tests if we've received a `StreamMessage::End` yet.
    fn ended(&self) -> bool {
        self.0.borrow().ended
    }
}

impl<D> Drop for ConsumerInner<D> {
    fn drop(&mut self) {
        let mut this = self.0.borrow_mut();

        if let Err(e) = this.set_dropped() {
            log::error!("failed to drop consumer handle: {e}");
            if let Err(e) = this.errs.send(e) {
                log::error!("failed to notify the engine, too!: {e}")
            }
        }
    }
}

impl<D> Stream for ConsumerInner<D> {
    type Item = D;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut inner = self.0.borrow_mut();

        if inner.dropped {
            // We've dropped the stream
            Poll::Ready(None)
        } else if let Some(it) = inner.pop() {
            Poll::Ready(Some(it))
        } else if inner.ended {
            // The stream has ended, and we've consumed all items
            Poll::Ready(None)
        } else {
            // No items are ready yet. Register waker and wait
            if let Some(old) = inner.waker.as_mut() {
                old.clone_from(cx.local_waker());
            } else {
                inner.waker = Some(cx.local_waker().clone());
            }
            Poll::Pending
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let inner = self.0.borrow_mut();
        let lower = inner.items.len();

        if inner.dropped {
            (0, Some(0))
        } else if inner.ended {
            (lower, Some(lower))
        } else {
            (lower, None)
        }
    }
}

/// The manager-side handle for an incoming stream.
/// It's lifetime corresponds to the stream lifetime. When the stream is `ended`, this handle is
/// removed. This handle may still exist while the corresponding consumer has been `drop`ed
#[repr(transparent)]
pub(super) struct ConsumerAdapter<D>(StateRef<D>);

/// A generic wrapper for the manager
pub(super) enum GenericConsumerAdapter {
    List(ConsumerAdapter<ListStreamData>),
    Byte(ConsumerAdapter<ByteStreamData>),
}

impl From<ConsumerAdapter<ByteStreamData>> for GenericConsumerAdapter {
    fn from(v: ConsumerAdapter<ByteStreamData>) -> Self {
        Self::Byte(v)
    }
}

impl From<ConsumerAdapter<ListStreamData>> for GenericConsumerAdapter {
    fn from(v: ConsumerAdapter<ListStreamData>) -> Self {
        Self::List(v)
    }
}

impl GenericConsumerAdapter {
    /// Called by the manager to handle an incoming `StreamData` message
    pub(super) fn data(&mut self, msg: StreamData) -> Result<(), ShellError> {
        match (self, msg) {
            (Self::List(a), StreamData::List(it)) => a.data(it),
            (Self::Byte(a), StreamData::Raw(it)) => a.data(it),
            (Self::Byte(_), StreamData::List(v)) => Err(ShellError::PluginFailedToDecode {
                msg: "Stream was created with the 'raw' type, but received list data instead"
                    .into(),
            }
            .into_chainned(v.span())),
            (Self::List(_), StreamData::Raw(_)) => Err(ShellError::PluginFailedToDecode {
                msg: "Stream was created with the 'list' type, but received raw data".into(),
            }),
        }
    }

    /// Used by the manager to signal the `end` message has been received. The adapter will be
    /// dropped in this function
    pub(super) fn end(self) {
        match self {
            Self::List(a) => a.end(),
            Self::Byte(a) => a.end(),
        }
    }
}

impl<D> ConsumerAdapter<D> {
    pub(super) fn data(&mut self, msg: D) -> Result<(), ShellError> {
        self.0.borrow_mut().push(msg)
    }

    /// End the stream. Called by the manager when the `End` message is received.
    pub(super) fn end(self) {
        self.0.borrow_mut().ended = true;
    }
}
