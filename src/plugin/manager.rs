use anyhow::Context;
use futures::{AsyncRead, AsyncWrite, Stream, StreamExt};
use nu_plugin_protocol::{PluginCall, PluginInput, PluginOutput, ProtocolInfo};
use nu_protocol::{ShellError, SignalAction};

use crate::channel::Sender;

use super::{io::AsyncEncoder, CoreRef, Plugin};

/// The number of messages to early-ack in incoming stream. Setting this to zero disables the behavior
const STREAM_EAGERNESS: usize = 4;

pub(super) type Input = Result<PluginInput, ShellError>;

pub(super) struct Manager<P: Plugin, R> {
    core: CoreRef<P>,

    tx: Sender<PluginOutput>,
    err_tx: Sender<ShellError>,
    incoming: R,
    /// A map of currently-running commands
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
        Self {
            core: CoreRef::new(plugin, tx.clone(), err_tx.clone()),

            tx,
            err_tx,
            incoming,

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

        loop {
            if self.should_exit {
                // TODO: still handle input in this state
                self.core.all_done().await;
                log::info!("all tasks exited");
                return Ok(());
            } else if let Some(it) = self.next().await {
                match it {
                    Ok(input) => self.handle(input),
                    Err(e) => {
                        log::error!("got in error: {e}");
                    }
                }
            } else {
                // Input is closed
                log::info!("input socket was closed");
                self.should_exit = true;
                // Wait for running commands to finish
                self.core.all_done().await;
                return Ok(());
            }
        }
    }

    /// Returns the next input to be handled
    async fn next(&mut self) -> Option<Result<PluginInput, ShellError>> {
        self.incoming.next().await
    }

    fn handle(&mut self, input: PluginInput) {
        if let Err(e) = self.do_handle(input) {
            log::error!("failed to handle input: {e}");
        }
    }

    /// Handle a single `PluginInput` object
    fn do_handle(&mut self, input: PluginInput) -> Result<(), ShellError> {
        // log::info!("got input: {:?}", input);
        match input {
            // Lifetime
            ////////////////////////////////////////////////////////////////////
            PluginInput::Hello(info) => {
                log::debug!("recv hello: {:?}", info);
                Ok(()) // nop
            }
            PluginInput::Goodbye => {
                log::info!("saw goodbye message, exiting");
                self.should_exit = true;
                Ok(())
            }

            // Stream messages
            ////////////////////////////////////////////////////////////////////
            // Consumer messages
            PluginInput::Data(id, data) => {
                log::trace!(id; "recv stream data: {:?}", data);
                self.core.data(id, data)
            }
            PluginInput::End(id) => {
                log::trace!(id; "recv stream end");
                self.core.end(id)
            }
            // Producer messages
            PluginInput::Ack(id) => {
                log::trace!(id; "recv ack");
                self.core.ack(id)
            }
            PluginInput::Drop(id) => {
                log::trace!(id; "recv drop");
                self.core.drop(id)
            }

            // Calls
            ////////////////////////////////////////////////////////////////////
            PluginInput::Call(id, PluginCall::Metadata) => self.core.metadata(id),
            PluginInput::Call(id, PluginCall::Signature) => self.core.signature(id),
            PluginInput::Call(id, PluginCall::CustomValueOp(..)) => {
                log::info!(id; "ignorning customvalueop");
                Ok(())
            }
            PluginInput::Call(id, PluginCall::Run(call)) => self.core.run(id, call),

            // Engine Interactions
            ////////////////////////////////////////////////////////////////////
            PluginInput::EngineCallResponse(id, res) => self.core.engine_response(id, res),
            PluginInput::Signal(SignalAction::Interrupt) => {
                log::trace!("received interrupt signal");
                self.core.interrupt();
                Ok(())
            }
            PluginInput::Signal(SignalAction::Reset) => {
                log::trace!("received reset signal");
                self.core.reset();
                Ok(())
            }
        }
    }
}
