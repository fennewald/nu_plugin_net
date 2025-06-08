use std::{
    collections::HashMap,
    num::{NonZero, NonZeroUsize},
};

use nu_plugin_protocol::{PluginOutput, StreamData, StreamId};
use nu_protocol::ShellError;

use crate::{channel::Sender, plugin::Result};

use super::{
    Adapter, ByteProducer, Core, GenericAdapter, ListProducer, Producer, ProducerHandle, State,
    StateRef,
};

pub(in crate::plugin) struct ProducerManager {
    /// The next id to be used for a stream
    next_id: StreamId,
    streams: HashMap<StreamId, GenericAdapter>,
}

impl ProducerManager {
    pub(in crate::plugin) fn new() -> Self {
        Self {
            next_id: 0,
            streams: HashMap::new(),
        }
    }

    pub(in crate::plugin) fn ack(&mut self, id: StreamId) -> Result<()> {
        self.streams
            .get_mut(&id)
            .ok_or_else(|| invalid_stream_id(id))?
            .ack()
    }

    pub(in crate::plugin) fn drop(&mut self, id: StreamId) -> Result<()> {
        self.streams
            .remove(&id)
            .ok_or_else(|| invalid_stream_id(id))?
            .drop()
    }

    fn new_producer<D>(&mut self, tx: Sender<PluginOutput>, max_unnack: usize) -> Producer<D>
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

        let core = Core::new(id, tx, max_unnack);
        let adapter = Adapter::new(core.clone()).into();
        let client = Producer::new(core);

        self.streams.insert(id, adapter);

        client
    }

    // Hides private types in the generic bounds
    pub(in crate::plugin) fn new_list_stream(
        &mut self,
        tx: Sender<PluginOutput>,
        max_unnack: usize,
    ) -> ListProducer {
        self.new_producer(tx, max_unnack)
    }

    pub(in crate::plugin) fn new_byte_stream(
        &mut self,
        tx: Sender<PluginOutput>,
        max_unnack: usize,
    ) -> ByteProducer {
        self.new_producer(tx, max_unnack)
    }

    fn next_id(&mut self) -> StreamId {
        let res = self.next_id;
        self.next_id += 1;
        res
    }
}

/// Returns a formatted shell error reporting that an invalid consumer stream id was found
fn invalid_stream_id(id: StreamId) -> ShellError {
    ShellError::NushellFailed {
        msg: format!("Received a message addressed to non-existant stream {id}."),
    }
}
