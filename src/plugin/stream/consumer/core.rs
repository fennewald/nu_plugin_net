use std::{cell::RefCell, collections::VecDeque, rc::Rc, task::LocalWaker};

use nu_plugin_protocol::{PluginOutput, StreamId};
use nu_protocol::{PipelineMetadata, ShellError, Span};

use crate::{channel::Sender, plugin::Result};

/// The shared state of a single consumer stream
pub(super) struct Core<D> {
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
pub(super) type CoreRef<D> = Rc<RefCell<Core<D>>>;

impl<D> Core<D> {
    /// Allocates a new core instance
    pub(super) fn new(
        id: StreamId,
        span: Span,
        meta: Option<PipelineMetadata>,
        tx: Sender<PluginOutput>,
        errs: Sender<ShellError>,
        tgt_buffer: usize,
    ) -> CoreRef<D> {
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

    pub(super) fn meta(&self) -> Option<PipelineMetadata> {
        self.meta.clone()
    }

    /// Tests if the consumer has ended
    pub(super) fn ended(&self) -> bool {
        self.ended
    }

    /// Tests if the consumer has been dropped
    pub(super) fn dropped(&self) -> bool {
        self.dropped
    }

    pub(super) fn len(&self) -> usize {
        self.items.len()
    }

    pub(super) fn set_waker(&mut self, waker: &LocalWaker) {
        if let Some(old) = self.waker.as_mut() {
            old.clone_from(waker);
        } else {
            self.waker = Some(waker.clone());
        }
    }

    fn ack(&mut self) -> Result<()> {
        self.tx
            .send(PluginOutput::Ack(self.id))
            .map_err(|e| ShellError::NushellFailedSpanned {
                msg: format!("Failed to send `Ack` message for stream {}: {}", self.id, e),
                label: "While processing this call".into(),
                span: self.span,
            })
    }

    /// Push a new item into the stream. Called by the manager when a new message has been received.
    pub(super) fn push(&mut self, item: D) -> Result<()> {
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

    /// Pops an item from the internal queue, perhaps sending `Ack` if needed
    pub(super) fn pop(&mut self) -> Option<D> {
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

    /// Marks the stream as 'ended'
    pub(super) fn end(&mut self) {
        self.ended = true;
    }

    /// Marks the stream as 'dropped'
    pub(super) fn drop(&mut self) -> Result<()> {
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

    /// Reports an error to the engine manager. If this fails, logs a warning and moves on.
    pub(super) fn report_err(&self, err: ShellError) {
        if let Err(e) = self.errs.send(err) {
            log::error!("failed to notify the manager of error: {e}")
        }
    }
}
