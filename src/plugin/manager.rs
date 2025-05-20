use std::collections::HashMap;

use anyhow::Context;
use futures::{AsyncRead, AsyncWrite, Stream, StreamExt};
use nu_plugin_protocol::{
    ByteStreamInfo, CallInfo, EngineCallId, EngineCallResponse, ListStreamInfo, PipelineDataHeader,
    PluginCall, PluginCallId, PluginCallResponse, PluginInput, PluginOutput, ProtocolInfo,
    StreamData, StreamId,
};
use nu_protocol::{
    LabeledError, PipelineMetadata, PluginMetadata, PluginSignature, ShellError, SignalAction,
    Value,
};

use crate::{channel::Sender, rt::JoinHandle};

use super::{
    engine::EngineContext, io::AsyncEncoder, producer::ProducerAdapter, ByteConsumer, Command,
    GenericConsumerAdapter, ListConsumer, Plugin, ShellResult,
};

/// The number of messages to early-ack in incoming stream. Setting this to zero disables the behavior
const STREAM_EAGERNESS: usize = 4;

pub(super) type Input = Result<PluginInput, ShellError>;

pub(super) struct Manager<P, R> {
    plugin: P,
    tx: Sender<PluginOutput>,
    err_tx: Sender<ShellError>,
    incoming: R,
    commands: HashMap<&'static str, Box<dyn Command<Plugin = P>>>,
    /// A map of currently-running commands
    running: HashMap<PluginCallId, JoinHandle<ShellResult<()>>>,
    consumers: HashMap<StreamId, GenericConsumerAdapter>,
    producers: HashMap<StreamId, ProducerAdapter>,
    /// Engine call state
    engine_ctx: EngineContext,
    should_exit: bool,
}

pub(super) async fn open<E, P, W, R>(
    plugin: P,
    tx: W,
    rx: R,
) -> Result<Manager<P, impl Stream<Item = Input> + Unpin>, ShellError>
where
    E: AsyncEncoder,
    P: Plugin,
    W: AsyncWrite + Unpin + 'static,
    R: AsyncRead + Unpin,
{
    let (err_tx, err_rx) = crate::channel::with_capacity(16);
    let (tx, rx) = super::io::consume::<E, W, R>(tx, rx, err_tx.clone()).await?;
    let incoming = futures::stream::select(rx, err_rx.map(Err));

    Ok(Manager::new(plugin, tx, err_tx, incoming))
}

impl<P, R> Manager<P, R>
where
    P: Plugin,
    R: Stream<Item = Input> + Unpin,
{
    fn new(plugin: P, tx: Sender<PluginOutput>, err_tx: Sender<ShellError>, incoming: R) -> Self {
        let commands = plugin.commands().map(|c| (c.name(), c)).collect();

        let engine_ctx = EngineContext::new(tx.clone());

        Self {
            plugin,
            tx,
            err_tx,
            incoming,
            commands,

            running: HashMap::new(),

            consumers: HashMap::new(),
            producers: HashMap::new(),

            engine_ctx,

            should_exit: false,
        }
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        let proto = ProtocolInfo {
            protocol: nu_plugin_protocol::Protocol::NuPlugin,
            version: "0.104.1".to_string(),
            features: Vec::new(),
        };
        self.tx
            .send(PluginOutput::Hello(proto))
            .context("failed to say hello")?;

        while let Some(it) = self.next().await {
            let input = it?;
            self.handle(input)?;
            if self.should_exit {
                break;
            }
        }
        log::info!("fell off the end");
        Ok(())
    }

    /// Returns the next input to be handled
    async fn next(&mut self) -> Option<Result<PluginInput, ShellError>> {
        self.incoming.next().await
    }

    fn handle(&mut self, input: PluginInput) -> Result<(), ShellError> {
        log::info!("got input: {:?}", input);
        match input {
            PluginInput::Hello(_) => Ok(()), // nop
            PluginInput::Call(id, call) => self.handle_call(id, call),
            PluginInput::EngineCallResponse(id, res) => self.handle_engine_call_response(id, res),
            PluginInput::Data(id, data) => self.handle_data(id, data),
            PluginInput::End(id) => self.handle_end(id),
            PluginInput::Drop(id) => self.handle_drop(id),
            PluginInput::Ack(id) => self.handle_ack(id),
            PluginInput::Signal(sig) => self.handle_signal(sig),
            PluginInput::Goodbye => {
                log::info!("saw goodbye message, exiting");
                self.should_exit = true;
                Ok(())
            }
        }
    }

    fn handle_call(
        &mut self,
        id: PluginCallId,
        call: PluginCall<PipelineDataHeader>,
    ) -> Result<(), ShellError> {
        match call {
            PluginCall::Metadata => self.metadata(id),
            PluginCall::Signature => self.signature(id),
            PluginCall::Run(call_info) => self.handle_run(id, call_info),
            PluginCall::CustomValueOp(..) => {
                log::info!("ignoring customvalue operation");
                Ok(())
            }
        }
    }

    fn respond(
        &mut self,
        id: PluginCallId,
        res: PluginCallResponse<PipelineDataHeader>,
    ) -> Result<(), ShellError> {
        self.tx
            .send(PluginOutput::CallResponse(id, res))
            .map_err(|e| ShellError::NushellFailed {
                msg: format!("failed to send reply to plugin call {id}: {e}"),
            })
    }

    fn metadata(&mut self, id: PluginCallId) -> Result<(), ShellError> {
        use PluginCallResponse::Metadata;
        self.respond(
            id,
            Metadata(PluginMetadata::new().with_version(self.plugin.version())),
        )
    }

    fn signature(&mut self, id: PluginCallId) -> Result<(), ShellError> {
        use PluginCallResponse::Signature;
        let signatures = self
            .commands
            .values()
            .map(|c| c.signature())
            .map(|sig| PluginSignature::new(sig, Vec::new()))
            .collect();

        self.respond(id, Signature(signatures))
    }

    fn handle_run(
        &mut self,
        id: PluginCallId,
        info: CallInfo<PipelineDataHeader>,
    ) -> Result<(), ShellError> {
        let info = info.map_data(|data| self.actualize_data_header(data))?;
        if let Some(cmd) = self.commands.get(info.name.as_str()) {
            let handle = cmd.spawn(info)?;
            self.running.insert(id, handle);
            Ok(())
        } else {
            log::error!("unrecognized command {}", info.name);
            self.respond(
                id,
                PluginCallResponse::Error(
                    LabeledError::new(format!("Unrecognized command {}", info.name)).with_inner(
                        ShellError::NotFound {
                            span: info.call.head,
                        },
                    ),
                ),
            )
        }
    }

    /// Accepts a `PipelineDataHeader`, and updates the manager state to track any streams
    fn actualize_data_header(
        &mut self,
        data: PipelineDataHeader,
    ) -> Result<ActualizedPipelineDataHeader, ShellError> {
        use ActualizedPipelineDataHeader::*;
        match data {
            PipelineDataHeader::Empty => Ok(Empty),
            PipelineDataHeader::Value(val, meta) => Ok(Value(val, meta)),
            PipelineDataHeader::ListStream(info) => Ok(List(self.track_list_consumer(info)?)),
            PipelineDataHeader::ByteStream(info) => Ok(Byte(self.track_byte_consumer(info)?)),
        }
    }

    fn track_list_consumer(&mut self, info: ListStreamInfo) -> Result<ListConsumer, ShellError> {
        let id = info.id;

        if self.consumers.contains_key(&id) {
            return Err(ShellError::NushellFailed {
                msg: format!("CallInfo contained an already-in-use stream id {id}"),
            });
        }

        let (adapter, consumer) = ListConsumer::new(
            id,
            info.span,
            info.metadata,
            self.tx.clone(),
            self.err_tx.clone(),
            STREAM_EAGERNESS,
        );

        self.consumers.insert(id, adapter.into());
        Ok(consumer)
    }

    fn track_byte_consumer(&mut self, info: ByteStreamInfo) -> Result<ByteConsumer, ShellError> {
        let id = info.id;

        if self.consumers.contains_key(&id) {
            return Err(ShellError::NushellFailed {
                msg: format!("CallInfo contained an already-in-use stream id {id}"),
            });
        }

        let (adapter, consumer) = ByteConsumer::new(
            id,
            info.span,
            info.metadata,
            info.type_,
            self.tx.clone(),
            self.err_tx.clone(),
            STREAM_EAGERNESS,
        );

        self.consumers.insert(id, adapter.into());
        Ok(consumer)
    }

    /// Handles an incoming piece of stream data
    fn handle_data(&mut self, id: StreamId, data: StreamData) -> Result<(), ShellError> {
        self.consumers
            .get_mut(&id)
            .ok_or_else(|| invalid_stream_id(id))?
            .data(data)
    }

    fn handle_end(&mut self, id: StreamId) -> Result<(), ShellError> {
        self.consumers
            .remove(&id)
            .map(|c| c.end())
            .ok_or_else(|| invalid_stream_id(id))
    }

    fn get_producer(&mut self, id: StreamId) -> Result<&mut ProducerAdapter, ShellError> {
        self.producers
            .get_mut(&id)
            .ok_or_else(|| ShellError::NushellFailed {
                msg: format!("Referenced non-existent producer stream {id}"),
            })
    }

    fn handle_ack(&mut self, id: StreamId) -> Result<(), ShellError> {
        self.get_producer(id)?.ack()
    }

    fn handle_drop(&mut self, id: StreamId) -> Result<(), ShellError> {
        self.get_producer(id)?.drop()
    }

    fn handle_signal(&mut self, signal: SignalAction) -> Result<(), ShellError> {
        todo!()
    }

    fn handle_engine_call_response(
        &mut self,
        id: EngineCallId,
        res: EngineCallResponse<PipelineDataHeader>,
    ) -> Result<(), ShellError> {
        let res = res.map_data(|d| self.actualize_data_header(d))?;
        self.engine_ctx.handle_response(id, res)
    }
}

/// Returns a formatted shell error reporting that an invalid stream id was found
fn invalid_stream_id(id: StreamId) -> ShellError {
    ShellError::NushellFailed {
        msg: format!("Received a message addressed to non-existant stream {id}."),
    }
}

/// A `PipelineDataHeader` that's been integrated with our manager already
pub enum ActualizedPipelineDataHeader {
    Empty,
    Value(Value, Option<PipelineMetadata>),
    List(ListConsumer),
    Byte(ByteConsumer),
}
