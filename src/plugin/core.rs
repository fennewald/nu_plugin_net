// Flirting with the idea of _one_ giant context struct
//

use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    task::{LocalWaker, Poll},
};

use nu_plugin_protocol::{
    CallInfo, EngineCall, EngineCallId, EngineCallResponse, PipelineDataHeader, PluginCallId,
    PluginCallResponse, PluginOutput, StreamData, StreamId,
};
use nu_protocol::{LabeledError, PluginMetadata, PluginSignature, ShellError, Span, Value};

use crate::channel::{OneshotReceiver, Sender};

use super::{
    ByteProducer, CommandExt, ConsumerManager, ContextHandle, EngineResponse, EngineState,
    InputDataHeader, ListProducer, Plugin, ProducerManager, Result,
};

#[repr(transparent)]
pub(super) struct CoreRef<P: Plugin>(Rc<RefCell<Core<P>>>);

impl<P: Plugin> Clone for CoreRef<P> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

/// The mutable state of the plugin manager
pub(super) struct Core<P: Plugin> {
    plugin: P,

    commands: HashMap<&'static str, Box<dyn CommandExt<Plugin = P>>>,

    tx: Sender<PluginOutput>,
    err_tx: Sender<ShellError>,

    consumers: ConsumerManager,
    producers: ProducerManager,

    /// The number of current running commands
    n_running: usize,
    /// A waker for when the number of commands reaches zero
    complete_waker: Option<LocalWaker>,
    /// A list of all currently running commands
    running: HashMap<PluginCallId, ContextHandle>,

    engine: EngineState,
}

impl<P: Plugin> Core<P> {
    fn acutalize(&mut self, data: PipelineDataHeader) -> Result<InputDataHeader> {
        self.consumers.actualize(data, &self.tx, &self.err_tx)
    }

    /// Decrement the number of running tasks
    fn dec_running(&mut self) {
        self.n_running -= 1;
        if self.n_running == 0 {
            if let Some(waker) = self.complete_waker.take() {
                waker.wake();
            }
        }
    }

    /// Respond to a given plugin call id
    fn respond(
        &mut self,
        id: PluginCallId,
        res: PluginCallResponse<PipelineDataHeader>,
    ) -> Result<()> {
        self.tx
            .send(PluginOutput::CallResponse(id, res))
            .map_err(|e| ShellError::NushellFailed {
                msg: format!("failed to send reply to plugin call {id}: {e}"),
            })
    }

    fn engine_call(
        &mut self,
        id: PluginCallId,
        call: EngineCall<PipelineDataHeader>,
    ) -> Result<OneshotReceiver<EngineResponse>> {
        self.engine.call(&self.tx, id, call)
    }
}

impl<P: Plugin> CoreRef<P> {
    pub(super) fn new(plugin: P, tx: Sender<PluginOutput>, err_tx: Sender<ShellError>) -> Self {
        let commands = plugin.commands().map(|c| (c.name(), c)).collect();

        Self(Rc::new(RefCell::new(Core {
            plugin,

            commands,

            tx,
            err_tx,

            consumers: ConsumerManager::new(),
            producers: ProducerManager::new(),

            n_running: 0,
            complete_waker: None,
            running: HashMap::new(),

            engine: EngineState::new(),
        })))
    }

    pub(super) fn data(&self, id: StreamId, data: StreamData) -> Result<()> {
        self.0.borrow_mut().consumers.data(id, data)
    }

    pub(super) fn end(&self, id: StreamId) -> Result<()> {
        self.0.borrow_mut().consumers.end(id)
    }

    pub(super) fn ack(&self, id: StreamId) -> Result<()> {
        self.0.borrow_mut().producers.ack(id)
    }

    pub(super) fn drop(&self, id: StreamId) -> Result<()> {
        self.0.borrow_mut().producers.drop(id)
    }

    pub(super) fn new_list_stream(&self, max_unnack: usize) -> ListProducer {
        let mut this = self.0.borrow_mut();
        let tx = this.tx.clone();
        this.producers.new_list_stream(tx, max_unnack)
    }

    pub(super) fn new_byte_stream(&self, max_unnack: usize) -> ByteProducer {
        let mut this = self.0.borrow_mut();
        let tx = this.tx.clone();
        this.producers.new_byte_stream(tx, max_unnack)
    }

    /// Handles an interrupt signal
    pub(super) fn interrupt(&self) {
        self.0
            .borrow_mut()
            .running
            .values_mut()
            .for_each(|task| task.interrupt())
    }

    /// Handles a reset signal
    pub(super) fn reset(&self) {
        self.0
            .borrow_mut()
            .running
            .values_mut()
            .for_each(|task| task.reset())
    }

    /// A metadata call on the plugin
    pub(super) fn metadata(&self, id: PluginCallId) -> Result<()> {
        let mut this = self.0.borrow_mut();
        let version = this.plugin.version();
        this.respond(
            id,
            PluginCallResponse::Metadata(PluginMetadata::new().with_version(version)),
        )
    }

    /// A signature call on the plugin
    pub(super) fn signature(&self, id: PluginCallId) -> Result<()> {
        let mut this = self.0.borrow_mut();

        let signatures = this
            .commands
            .values()
            .map(|c| PluginSignature::new(c.signature(), c.examples()))
            .collect();

        this.respond(id, PluginCallResponse::Signature(signatures))
    }

    /// Run a command
    pub(super) fn run(&self, id: PluginCallId, info: CallInfo<PipelineDataHeader>) -> Result<()> {
        let mut this = self.0.borrow_mut();
        let info = info.map_data(|d| this.acutalize(d))?;

        if let Some(cmd) = this.commands.get(info.name.as_str()) {
            let handle = ContextHandle::spawn(self.clone(), cmd, id, info);
            this.n_running += 1;
            this.running.insert(id, handle);
            Ok(())
        } else {
            log::error!("unrecognized command {}", info.name);
            this.respond(
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

    /// Called by `EngineInterface`, makes an engine_call
    pub(super) fn engine_call(
        &self,
        call_id: PluginCallId,
        call: EngineCall<PipelineDataHeader>,
    ) -> Result<OneshotReceiver<EngineResponse>> {
        self.0.borrow_mut().engine_call(call_id, call)
    }

    /// Handle a engine call response
    pub(super) fn engine_response(
        &self,
        id: EngineCallId,
        res: EngineCallResponse<PipelineDataHeader>,
    ) -> Result<()> {
        let mut this = self.0.borrow_mut();
        let res = res.map_data(|d| this.acutalize(d))?;
        this.engine.response(id, res)
    }

    /// Called by the drop impl of `Context`, stops tracking this command
    pub(super) fn finish_invocation(&self, id: PluginCallId) {
        let mut this = self.0.borrow_mut();

        if let Some(handle) = this.running.remove(&id) {
            handle.finish();
            // Decrement the number of running tasks
            this.dec_running();
        } else {
            log::warn!("received invocation finished event for unrecognized call id {id}");
        }
    }

    /// Returns a future that waits for all currently running tasks to be complete
    pub(super) async fn all_done(&self) {
        std::future::poll_fn(move |cx| {
            let mut this = self.0.borrow_mut();

            if this.n_running == 0 {
                Poll::Ready(())
            } else {
                if let Some(ref mut old) = this.complete_waker {
                    old.clone_from(cx.local_waker());
                } else {
                    this.complete_waker = Some(cx.local_waker().clone());
                }
                Poll::Pending
            }
        })
        .await
    }

    pub(super) fn respond_data(&self, id: PluginCallId, data: PipelineDataHeader) -> Result<()> {
        self.0
            .borrow_mut()
            .respond(id, PluginCallResponse::PipelineData(data))
    }

    /// Send raw output
    pub(super) fn send(&self, output: PluginOutput) -> Result<()> {
        self.0.borrow_mut().tx.send(output).map_err(|e| e.into())
    }
}
