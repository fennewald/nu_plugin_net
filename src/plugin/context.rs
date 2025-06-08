use std::{cell::RefCell, rc::Rc, task::LocalWaker};

use nu_plugin_protocol::{CallInfo, PipelineDataHeader, PluginCallId};
use nu_protocol::{PipelineMetadata, ShellError, Value};

use crate::rt::JoinHandle;

use super::{
    CommandExt, CoreRef as ManagerCoreRef, EngineInterface, InputDataHeader, Plugin, Result,
};

enum Status {
    Nominal,
    Cancelled,
    Reset,
    Done,
}

struct Core {
    id: PluginCallId,
    status: Status,
    /// A handle to the local waker, currently waiting on a status change
    status_waker: Option<LocalWaker>,
}

impl Core {
    fn new(id: PluginCallId) -> Self {
        Self {
            id,
            status: Status::Nominal,
            status_waker: None,
        }
    }

    fn set_status(&mut self, status: Status) {
        self.status = status;
        if let Some(waker) = self.status_waker.take() {
            waker.wake();
        }
    }
}

type CoreRef = Rc<RefCell<Core>>;

/// A task-side handle to the engine context.
/// When dropped, the command is considered 'complete' from the manager's perspective
pub struct Context<P: Plugin> {
    manager: ManagerCoreRef<P>,
    core: CoreRef,
    responded: bool,
}

impl<P: Plugin> Context<P> {
    fn new(manager: ManagerCoreRef<P>, core: CoreRef) -> Self {
        Self {
            manager,
            core,
            responded: false,
        }
    }

    /// A handle so that other types can get access to our manager ref
    pub(super) fn manager(&self) -> &ManagerCoreRef<P> {
        &self.manager
    }

    pub fn id(&self) -> PluginCallId {
        self.core.borrow().id
    }

    fn respond_data(&mut self, data: PipelineDataHeader) -> Result<()> {
        if self.responded {
            return Err(ShellError::NushellFailed {
                msg: "Tried to respond twice to the same command".to_string(),
            });
        }

        let res = self.manager.respond_data(self.id(), data);
        if res.is_ok() {
            self.responded = true;
        }
        res
    }

    /// Sends an empty response
    pub fn respond_empty(&mut self) -> Result<()> {
        self.respond_data(PipelineDataHeader::Empty)
    }

    /// Sends a single value as response
    pub fn respond_value(&mut self, val: Value, meta: Option<PipelineMetadata>) -> Result<()> {
        self.respond_data(PipelineDataHeader::Value(val, meta))
    }

    pub fn engine(&mut self) -> EngineInterface<'_, P> {
        EngineInterface::new(self)
    }
}

impl<P: Plugin> Drop for Context<P> {
    fn drop(&mut self) {
        let id = self.id();
        let manager = self.manager.clone();
        crate::rt::spawn("cleanup", async move {
            manager.finish_invocation(id);
        });
    }
}

/// The engine-side counterpart
pub(super) struct ContextHandle {
    core: CoreRef,
    /// A handle to the running task
    handle: JoinHandle<Result<()>>,
}

impl ContextHandle {
    pub(super) fn spawn<P>(
        manager: ManagerCoreRef<P>,
        cmd: &Box<dyn CommandExt<Plugin = P>>,
        id: PluginCallId,
        call: CallInfo<InputDataHeader>,
    ) -> ContextHandle
    where
        P: Plugin,
    {
        let core = Rc::new(RefCell::new(Core::new(id)));
        let ctx = Context::new(manager, core.clone());

        let handle = cmd.spawn(call, ctx);
        Self { core, handle }
    }

    /// Consumes the context handle. Called when the command has been completed
    pub(super) fn finish(self) {
        let mut core = self.core.borrow_mut();
        let id = core.id;
        core.set_status(Status::Done);

        match self.handle.complete_sync() {
            Ok(Ok(Ok(()))) => log::info!(id; "command invocation exited successfully"),
            Ok(Ok(Err(e))) => log::error!(id; "command invocation exited with {e}"),
            Ok(Err(e)) => log::error!(id; "failed to rejoin command after completion: {e}"),
            Err(_) => {
                log::error!(id; "failed to rejoin command after completion: it wasn't done yet")
            }
        }
    }

    /// Handles an interrupt signal being sent
    pub(super) fn interrupt(&mut self) {}

    /// Handles a reset signal being sen
    pub(super) fn reset(&mut self) {}
}
