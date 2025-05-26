use std::collections::HashMap;

use nu_plugin_protocol::{StreamData, StreamId};
use nu_protocol::ShellError;

use crate::plugin::Result;

use super::GenericAdapter;

/// A global struct that manages all active consumers
pub(in crate::plugin) struct ConsumerManager {
    streams: HashMap<StreamId, GenericAdapter>,
}

impl ConsumerManager {
    pub(in crate::plugin) fn new() -> Self {
        Self {
            streams: HashMap::new(),
        }
    }

    /// Called by the manager to handle an incoming piece of StreamData
    pub(in crate::plugin) fn data(&mut self, id: StreamId, data: StreamData) -> Result<()> {
        self.streams
            .get_mut(&id)
            .ok_or_else(|| invalid_stream_id(id))?
            .data(data)
    }

    /// Called by the manager to handle a received End message
    pub(in crate::plugin) fn end(&mut self, id: StreamId) -> Result<()> {
        self.streams
            .remove(&id)
            .map(|c| c.end())
            .ok_or_else(|| invalid_stream_id(id))
    }
}

/// Returns a formatted shell error reporting that an invalid consumer stream id was found
fn invalid_stream_id(id: StreamId) -> ShellError {
    ShellError::NushellFailed {
        msg: format!("Received a message addressed to non-existant stream {id}."),
    }
}
