use std::{cell::RefCell, collections::HashMap, rc::Rc};

use nu_plugin_protocol::{
    EngineCall, EngineCallId, EngineCallResponse, PipelineDataHeader, PluginCallId, PluginOutput,
};
use nu_protocol::{ShellError, Span, Value};

use crate::channel::{oneshot, OneshotReceiver, OneshotSender, Sender};

use super::{ActualizedPipelineDataHeader, ShellResult};

type Response = EngineCallResponse<ActualizedPipelineDataHeader>;

/// The core of the engine state
struct Core {
    next_id: usize,
    tx: Sender<PluginOutput>,
    active: HashMap<EngineCallId, OneshotSender<Response>>,
}

impl Core {
    fn new(tx: Sender<PluginOutput>) -> Self {
        Self {
            next_id: 0,
            tx,
            active: HashMap::new(),
        }
    }

    fn get_id(&mut self) -> EngineCallId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn call(
        &mut self,
        context: PluginCallId,
        it: EngineCall<PipelineDataHeader>,
    ) -> ShellResult<OneshotReceiver<Response>> {
        let id = self.get_id();

        // TODO: add spans and shit, make error better
        self.tx.send(PluginOutput::EngineCall {
            context,
            id,
            call: it,
        })?;

        let (tx, rx) = oneshot::channel();
        self.active.insert(id, tx);

        Ok(rx)
    }
}

#[derive(Clone)]
pub struct EngineInterface {
    context: PluginCallId,
    core: Rc<RefCell<Core>>,
}

impl EngineInterface {
    async fn call(&self, it: EngineCall<PipelineDataHeader>) -> ShellResult<Response> {
        let rx = self.core.borrow_mut().call(self.context, it)?;
        rx.recv().await.map_err(|_| ShellError::NushellFailed {
            msg: "The engine shut down before the call was completed".into(),
        })
    }

    pub async fn get_config(&self) -> ShellResult<()> {
        todo!()
    }

    pub async fn get_plugin_config(&self) -> ShellResult<Option<Value>> {
        todo!()
    }

    pub async fn get_env_var(&self, name: impl Into<String>) -> ShellResult<Option<Value>> {
        use ActualizedPipelineDataHeader::*;
        match self.call(EngineCall::GetEnvVar(name.into())).await? {
            EngineCallResponse::PipelineData(Value(val, _)) => Ok(Some(val)),
            EngineCallResponse::PipelineData(Empty) => Ok(None),
            EngineCallResponse::Error(shell_error) => Err(shell_error),
            _ => Err(ShellError::TypeMismatch {
                err_message: "Received a non-value response to a GetEnv request".into(),
                span: Span::unknown(),
            }),
        }
    }

    pub async fn get_current_dir(&self) -> ShellResult<String> {
        use ActualizedPipelineDataHeader::*;
        match self.call(EngineCall::GetCurrentDir).await? {
            EngineCallResponse::PipelineData(Value(v, _)) => v.into_string(),
            EngineCallResponse::Error(shell_error) => Err(shell_error),
            _ => Err(ShellError::TypeMismatch {
                err_message: "Received a non-value response to a GetCurrentDir request".into(),
                span: Span::unknown(),
            }),
        }
    }
}

/// The manager-side handle for engine context management
pub(super) struct EngineContext {
    core: Rc<RefCell<Core>>,
}

impl EngineContext {
    pub(super) fn new(tx: Sender<PluginOutput>) -> Self {
        Self {
            core: Rc::new(RefCell::new(Core::new(tx))),
        }
    }

    pub(super) fn handle_response(&mut self, id: EngineCallId, res: Response) -> ShellResult<()> {
        self.core
            .borrow_mut()
            .active
            .remove(&id)
            .ok_or_else(|| ShellError::NushellFailed {
                msg: format!("Received engine call response for unrecognized id {id}"),
            })
            .map(|chan| {
                if chan.send(res).is_err() {
                    log::info!("received engine call response for already-dropped future")
                }
            })
    }
}
