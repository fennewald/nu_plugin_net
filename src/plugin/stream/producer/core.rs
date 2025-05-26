use std::{
    cell::RefCell,
    collections::VecDeque,
    num::NonZeroUsize,
    rc::Rc,
    task::{LocalWaker, Poll},
};

use nu_plugin_protocol::{PluginOutput, StreamData, StreamId};
use nu_protocol::ShellError;

use crate::{channel::Sender, plugin::Result};

/// The shared state of a single producer stream
pub(super) struct Core<D> {
    /// The ID of this stream
    id: StreamId,
    /// A handle for the plugin output
    tx: Sender<PluginOutput>,
    /// A flag indicating that the `End` message has been sent
    ended: bool,
    /// A flag indicating that the `Drop` message has been received
    dropped: bool,
    /// The acutal items in queue. Note, if there's room in the stream, messages may never end up
    /// here
    queue: VecDeque<D>,
    /// If the producer is waiting on space in the queue, it will store it's waker here
    waker: Option<LocalWaker>,
    /// Number of sent messages that are currently not acknowledged by the receiver
    n_unack: usize,
    /// The maximum number of unacknowledged messages to permit. Note, this can be changed
    /// dynamically during the stream's lifetime.
    max_unack: NonZeroUsize,
}

/// Lock'er? I hardly know'er!
pub(super) type CoreRef<D> = Rc<RefCell<Core<D>>>;

impl<D> Core<D>
where
    D: Into<StreamData>,
{
    pub(super) fn new(
        id: StreamId,
        tx: Sender<PluginOutput>,
        max_unack: NonZeroUsize,
    ) -> CoreRef<D> {
        Rc::new(RefCell::new(Self {
            id,
            tx,

            ended: false,
            dropped: false,

            queue: VecDeque::new(),
            waker: None,

            n_unack: 0,
            max_unack,
        }))
    }
    pub(super) fn set_max_unack(&mut self, max: usize) {
        const ONE: NonZeroUsize = NonZeroUsize::new(1).unwrap();
        let n = match NonZeroUsize::new(max) {
            Some(n) => n,
            None => {
                log::warn!("Tried to set max unacked to 0. Overriding to 1");
                ONE
            }
        };
        self.max_unack = n;
    }

    /// Tests if there's room for a new message to be sent
    fn has_room(&self) -> bool {
        self.n_unack < self.max_unack.get()
    }

    fn wake(&mut self) {
        if let Some(waker) = self.waker.take() {
            waker.wake();
        }
    }

    fn do_send(&mut self, val: D) -> Result<()> {
        self.tx.send(PluginOutput::Data(self.id, val.into()))?;
        self.n_unack += 1;
        Ok(())
    }

    /// Attempts to push the provided item into the channel. If there's no room, `Err(val)` is
    /// returned
    pub(super) fn try_send(&mut self, val: D) -> std::result::Result<Result<()>, D> {
        if self.has_room() {
            Ok(self.do_send(val))
        } else {
            Err(val)
        }
    }

    /// Push an item into the producer stream, buffering the value and returning if the pipeline is
    /// currently full
    pub(super) fn push(&mut self, val: D) -> Result<()> {
        self.try_send(val).unwrap_or_else(|val| {
            self.queue.push_back(val);
            Ok(())
        })
    }

    fn set_waker(&mut self, waker: &LocalWaker) {
        if let Some(old) = self.waker.as_mut() {
            old.clone_from(waker);
        } else {
            self.waker = Some(waker.clone());
        }
    }

    /// Polls if there's room to send data on this channel. If there is room, `Poll::Ready(())` is
    /// returned. If there is not room, `Poll::Pending` is returned, and the waker is recorded. The
    /// waker will be woken when there's room
    pub(super) fn poll_room(&mut self, waker: &LocalWaker) -> Poll<()> {
        if self.has_room() {
            Poll::Ready(())
        } else {
            self.set_waker(waker);
            Poll::Pending
        }
    }

    /// Handle an ack event
    pub(super) fn ack(&mut self) -> Result<()> {
        if self.n_unack == 0 {
            return Err(ShellError::NushellFailed {
                msg: "Received an `Ack` message for which no data message exists".into(),
            });
        }
        self.n_unack -= 1;

        if self.n_unack == self.max_unack.get() - 1 {
            // We just opened up room to push from the queue and/or wake up a waker
            if let Some(item) = self.queue.pop_front() {
                // First, try to pull something out of the queue
                self.do_send(item)?;
            } else {
                self.wake();
            }
        }

        Ok(())
    }
}
