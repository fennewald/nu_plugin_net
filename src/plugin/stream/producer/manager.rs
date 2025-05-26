use nu_plugin_protocol::{PluginOutput, StreamId};

use crate::{channel::Sender, plugin::Result};

use super::{ProducerHandle, StateRef};

#[repr(transparent)]
pub(in crate::plugin) struct Manager {
    state: StateRef,
}

impl Manager {
    pub(in crate::plugin) fn ack(&mut self, id: StreamId) -> Result<()> {
        self.state.borrow_mut().ack(id)
    }

    pub(in crate::plugin) fn drop(&mut self, id: StreamId) -> Result<()> {
        self.state.borrow_mut().drop(id)
    }

    pub(in crate::plugin) fn make_handle(&mut self, tx: Sender<PluginOutput>) -> ProducerHandle {
        let state = self.state.clone();
        ProducerHandle::new(state, tx)
    }
}
