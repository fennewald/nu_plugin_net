use std::collections::HashMap;

use anyhow::Context;
use futures::{
    stream::{Map, Select},
    AsyncRead, AsyncWrite, Stream, StreamExt,
};
use nu_plugin_protocol::{
    CallInfo, PipelineDataHeader, PluginCall, PluginCallId, PluginCallResponse, PluginInput,
    PluginOutput, ProtocolInfo,
};
use nu_protocol::{LabeledError, PluginMetadata, PluginSignature, ShellError};

use crate::channel::{Receiver, Sender};

use super::{io::AsyncEncoder, Command, Plugin};

pub(super) type Input = Result<PluginInput, ShellError>;

pub(super) struct Manager<P, R> {
    plugin: P,
    tx: Sender<PluginOutput>,
    incoming: R,
    commands: HashMap<&'static str, Box<dyn Command<Plugin = P>>>,
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
    let (tx, rx) = super::io::consume::<E, W, R>(tx, rx, err_tx).await?;
    let incoming = futures::stream::select(rx, err_rx.map(Err));

    Ok(Manager::new(plugin, tx, incoming))
}

impl<P, R> Manager<P, R>
where
    P: Plugin,
    R: Stream<Item = Input> + Unpin,
{
    fn new(plugin: P, tx: Sender<PluginOutput>, incoming: R) -> Self {
        let commands = plugin.commands().map(|c| (c.name(), c)).collect();

        Self {
            plugin,
            tx,
            incoming,
            commands,
            should_exit: false,
        }
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        let proto = ProtocolInfo {
            protocol: nu_plugin_protocol::Protocol::NuPlugin,
            version: "0.104.1".to_string(),
            features: Vec::new(),
        };
        self.tx.send(PluginOutput::Hello(proto));

        log::info!("send hello");

        while let Some(it) = self.next().await {
            match it {
                Ok(i) => self.handle(i),
                Err(e) => log::error!("saw error: {e}"),
            }

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

    fn handle(&mut self, input: PluginInput) {
        log::info!("got input: {:?}", input);
        match input {
            PluginInput::Hello(info) => {
                log::debug!("Saw protocol info: {:?}", info);
            }
            PluginInput::Call(id, call) => self.handle_call(id, call),
            PluginInput::Goodbye => {
                log::info!("saw goodbye message, exiting");
                self.should_exit = true;
            }
            PluginInput::EngineCallResponse(_, engine_call_response) => todo!(),
            PluginInput::Data(_, stream_data) => todo!(),
            PluginInput::End(_) => todo!(),
            PluginInput::Drop(_) => todo!(),
            PluginInput::Ack(_) => todo!(),
            PluginInput::Signal(signal_action) => todo!(),
        }
    }

    fn handle_call(&mut self, id: PluginCallId, call: PluginCall<PipelineDataHeader>) {
        match call {
            PluginCall::Metadata => self.metadata(id),
            PluginCall::Signature => self.signature(id),
            PluginCall::Run(call_info) => self.handle_run(id, call_info),
            PluginCall::CustomValueOp(..) => {
                log::info!("ignoring customvalue operation");
                Ok(())
            }
        };
    }

    fn respond(
        &mut self,
        id: PluginCallId,
        res: PluginCallResponse<PipelineDataHeader>,
    ) -> Result<(), ShellError> {
        self.tx.send(PluginOutput::CallResponse(id, res));
        Ok(())
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
        if let Some(cmd) = self.commands.get(info.name.as_str()) {
            todo!()
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
}
