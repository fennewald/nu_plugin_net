use std::{
    cell::RefCell,
    collections::HashMap,
    num::{NonZero, NonZeroUsize},
    rc::Rc,
};

use nu_plugin_protocol::{PluginOutput, StreamData, StreamId};
use nu_protocol::ShellError;

use crate::{channel::Sender, plugin::Result};

use super::{Adapter, Core, GenericAdapter, Producer};

/// Shared state of the producer management
pub(super) struct State {
    next_id: StreamId,
    streams: HashMap<StreamId, GenericAdapter>,
}

pub(super) type StateRef = Rc<RefCell<State>>;

impl State {
    pub(super) fn new() -> Self {
        Self {
            next_id: 0,
            streams: HashMap::new(),
        }
    }

    fn next_id(&mut self) -> StreamId {
        let res = self.next_id;
        self.next_id += 1;
        res
    }

    pub(super) fn ack(&mut self, id: StreamId) -> Result<()> {
        self.streams
            .get_mut(&id)
            .ok_or_else(|| invalid_stream_id(id))?
            .ack()
    }

    pub(super) fn drop(&mut self, id: StreamId) -> Result<()> {
        self.streams
            .remove(&id)
            .ok_or_else(|| invalid_stream_id(id))?
            .drop()
    }

    pub(super) fn alloc<D>(&mut self, tx: Sender<PluginOutput>, max_unnack: usize) -> Producer<D>
    where
        D: Into<StreamData>,
        Adapter<D>: Into<GenericAdapter>,
    {
        const ONE: NonZeroUsize = NonZeroUsize::new(1).unwrap();
        let id = self.next_id();
        let max_unnack = NonZero::new(max_unnack).unwrap_or_else(|| {
            log::warn!("tried to create channel with max unnack of 0. defaulting to 1");
            ONE
        });

        let core = Core::new(id, tx.clone(), max_unnack);
        let adapter = Adapter::new(core.clone()).into();
        let client = Producer::new(core);

        self.streams.insert(id, adapter);

        client
    }
}

/// Returns a formatted shell error reporting that an invalid consumer stream id was found
fn invalid_stream_id(id: StreamId) -> ShellError {
    ShellError::NushellFailed {
        msg: format!("Received a message addressed to non-existant stream {id}."),
    }
}
