use std::collections::HashMap;

use nu_plugin_protocol::{
    ByteStreamInfo, ListStreamInfo, PipelineDataHeader, PluginOutput, StreamData, StreamId,
};
use nu_protocol::ShellError;

use crate::{channel::Sender, plugin::Result};

use super::{ByteConsumer, GenericAdapter, InputDataHeader, ListConsumer, STREAM_EAGERNESS};

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

    /// Accepts a pipeline data header and tracks any relevant state
    pub(in crate::plugin) fn actualize(
        &mut self,
        data: PipelineDataHeader,
        tx: &Sender<PluginOutput>,
        err_tx: &Sender<ShellError>,
    ) -> Result<InputDataHeader> {
        match data {
            PipelineDataHeader::Empty => Ok(InputDataHeader::Empty),
            PipelineDataHeader::Value(value, meta) => Ok(InputDataHeader::Value(value, meta)),
            PipelineDataHeader::ListStream(info) => self
                .track_list_consumer(info, tx, err_tx)
                .map(InputDataHeader::List),
            PipelineDataHeader::ByteStream(info) => self
                .track_byte_consumer(info, tx, err_tx)
                .map(InputDataHeader::Byte),
        }
    }

    /// Accepts a `ListStreamInfo` supplied to the plugin, and integrates it with our stream tracking
    pub(in crate::plugin) fn track_list_consumer(
        &mut self,
        info: ListStreamInfo,
        tx: &Sender<PluginOutput>,
        err_tx: &Sender<ShellError>,
    ) -> Result<ListConsumer> {
        let id = info.id;

        if self.streams.contains_key(&id) {
            return Err(ShellError::NushellFailed {
                msg: format!("CallInfo contained an already-in-use stream id {id}"),
            });
        }

        let (adapter, consumer) = ListConsumer::new(
            id,
            info.span,
            info.metadata,
            tx.clone(),
            err_tx.clone(),
            STREAM_EAGERNESS,
        );

        self.streams.insert(id, adapter.into());
        Ok(consumer)
    }

    /// Accepts a `ListStreamInfo` supplied to the plugin, and integrates it with our stream tracking
    pub(in crate::plugin) fn track_byte_consumer(
        &mut self,
        info: ByteStreamInfo,
        tx: &Sender<PluginOutput>,
        err_tx: &Sender<ShellError>,
    ) -> Result<ByteConsumer> {
        let id = info.id;

        if self.streams.contains_key(&id) {
            return Err(ShellError::NushellFailed {
                msg: format!("CallInfo contained an already-in-use stream id {id}"),
            });
        }

        let (adapter, consumer) = ByteConsumer::new(
            id,
            info.span,
            info.metadata,
            info.type_,
            tx.clone(),
            err_tx.clone(),
            STREAM_EAGERNESS,
        );

        self.streams.insert(id, adapter.into());
        Ok(consumer)
    }
}

/// Returns a formatted shell error reporting that an invalid consumer stream id was found
fn invalid_stream_id(id: StreamId) -> ShellError {
    ShellError::NushellFailed {
        msg: format!("Received a message addressed to non-existant stream {id}."),
    }
}
