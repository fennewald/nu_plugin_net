use nu_plugin_protocol::PluginOutput;

use crate::channel::Sender;

use super::{ByteProducer, ListProducer, StateRef};

/// An opaque type that allows the user to create a new output stream
pub(in crate::plugin) struct ProducerHandle {
    state: StateRef,
    tx: Sender<PluginOutput>,
}

impl ProducerHandle {
    pub(super) fn new(state: StateRef, tx: Sender<PluginOutput>) -> Self {
        Self { state, tx }
    }

    pub(in crate::plugin) fn into_list_stream(self, max_unnack: usize) -> ListProducer {
        self.state.borrow_mut().alloc(self.tx, max_unnack)
    }

    pub(in crate::plugin) fn into_byte_stream(self, max_unnack: usize) -> ByteProducer {
        self.state.borrow_mut().alloc(self.tx, max_unnack)
    }
}
