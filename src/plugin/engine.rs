use std::collections::HashMap;

use nu_plugin_protocol::{
    EngineCall, EngineCallId, EngineCallResponse, PipelineDataHeader, PluginCallId, PluginOption,
    PluginOutput,
};
use nu_protocol::{DeclId, ShellError, Span, Value};

use crate::channel::{oneshot, OneshotReceiver, OneshotSender, Sender};

use super::{Context, InputDataHeader, Plugin, Result};

pub(super) type EngineResponse = EngineCallResponse<InputDataHeader>;

/// The core engine state. Stored in the plugin core
pub(super) struct EngineState {
    /// The next id to use for an engine call
    next_id: EngineCallId,
    /// A map of active outstanding engine calls
    active: HashMap<EngineCallId, OneshotSender<EngineResponse>>,
}

impl EngineState {
    pub(super) fn new() -> Self {
        Self {
            next_id: 0,
            active: HashMap::new(),
        }
    }

    fn get_id(&mut self) -> EngineCallId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub(super) fn response(&mut self, id: EngineCallId, res: EngineResponse) -> Result<()> {
        self.active
            .remove(&id)
            .ok_or_else(|| ShellError::NushellFailed {
                msg: format!("received an engine call response th invalid id {id}"),
            })?
            .send(res)
            .map_err(|e| ShellError::NushellFailed {
                msg: format!("could not handle engine call response {id}: {e}"),
            })
    }

    pub(super) fn call(
        &mut self,
        tx: &Sender<PluginOutput>,
        call_id: PluginCallId,
        call: EngineCall<PipelineDataHeader>,
    ) -> Result<OneshotReceiver<EngineResponse>> {
        let id = self.get_id();

        tx.send(PluginOutput::EngineCall {
            context: call_id,
            id,
            call,
        })?;
        let (tx, rx) = oneshot::channel();
        self.active.insert(id, tx);

        Ok(rx)
    }
}

/// A temporary handle for interacting with the engine
#[repr(transparent)]
pub struct EngineInterface<'r, P: Plugin> {
    ctx: &'r mut Context<P>,
}

impl<'r, P: Plugin> EngineInterface<'r, P> {
    pub(super) fn new(ctx: &'r mut Context<P>) -> Self {
        Self { ctx }
    }

    async fn call(&self, call: EngineCall<PipelineDataHeader>) -> Result<EngineResponse> {
        let rx = self.ctx.manager().engine_call(self.ctx.id(), call)?;
        rx.recv().await.map_err(|e| ShellError::NushellFailed {
            msg: format!("could not receive engine call response: {e}"),
        })
    }

    pub async fn get_config(&self) -> Result<()> {
        todo!()
    }

    pub async fn get_plugin_config(&self) -> Result<Option<Value>> {
        todo!()
    }

    pub async fn get_env_var(&self, name: impl Into<String>) -> Result<Option<Value>> {
        use InputDataHeader::*;
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

    pub async fn get_current_dir(&self) -> Result<String> {
        use InputDataHeader::*;
        match self.call(EngineCall::GetCurrentDir).await? {
            EngineCallResponse::PipelineData(Value(v, _)) => v.into_string(),
            EngineCallResponse::Error(shell_error) => Err(shell_error),
            _ => Err(ShellError::TypeMismatch {
                err_message: "Received a non-value response to a GetCurrentDir request".into(),
                span: Span::unknown(),
            }),
        }
    }

    pub async fn get_env_vars(&self) -> Result<HashMap<String, Value>> {
        match self.call(EngineCall::GetEnvVars).await? {
            EngineCallResponse::ValueMap(map) => Ok(map),
            EngineCallResponse::Error(shell_error) => Err(shell_error),
            _ => Err(ShellError::TypeMismatch {
                err_message: "Received a non-value-map response to a GetEnvVars request".into(),
                span: Span::unknown(),
            }),
        }
    }

    pub async fn add_env_var(&self, name: impl Into<String>, value: Value) -> Result<()> {
        match self.call(EngineCall::AddEnvVar(name.into(), value)).await? {
            EngineCallResponse::PipelineData(_) => Ok(()),
            EngineCallResponse::Error(shell_error) => Err(shell_error),
            _ => Err(ShellError::TypeMismatch {
                err_message: "Received a non-pipeline data response to a AddEnvVar request".into(),
                span: Span::unknown(),
            }),
        }
    }

    pub async fn get_help(&self) -> Result<String> {
        use InputDataHeader::*;
        match self.call(EngineCall::GetHelp).await? {
            EngineCallResponse::PipelineData(Value(v, _)) => v.into_string(),
            EngineCallResponse::Error(shell_error) => Err(shell_error),
            _ => Err(ShellError::TypeMismatch {
                err_message: "Received a non-value response to a GetHelp request".into(),
                span: Span::unknown(),
            }),
        }
    }

    // TODO: figure out fg/bg stuff

    pub async fn get_span_contents(&self, span: Span) -> Result<Vec<u8>> {
        match self.call(EngineCall::GetSpanContents(span)).await? {
            EngineCallResponse::PipelineData(InputDataHeader::Value(
                Value::Binary { val, .. },
                _,
            )) => Ok(val),
            EngineCallResponse::Error(shell_error) => Err(shell_error),
            _ => Err(ShellError::TypeMismatch {
                err_message: "Received a non-binary value response to a GetSpan request".into(),
                span: Span::unknown(),
            }),
        }
    }

    // TODO: figure out eval_closure_with_stream
    // TODO: figure out eval_closure

    pub async fn find_decl(&self, name: impl Into<String>) -> Result<Option<DeclId>> {
        use InputDataHeader::*;
        match self.call(EngineCall::FindDecl(name.into())).await? {
            EngineCallResponse::Identifier(id) => Ok(Some(id)),
            EngineCallResponse::PipelineData(Empty) => Ok(None),
            EngineCallResponse::Error(shell_error) => Err(shell_error),
            _ => Err(ShellError::TypeMismatch {
                err_message: "Received a non-id/empty response to a FindDecl request".into(),
                span: Span::unknown(),
            }),
        }
    }

    // TODO: figure out call_decl

    pub fn set_gc_disabled(&self, disabled: bool) -> Result<()> {
        self.ctx
            .manager()
            .send(PluginOutput::Option(PluginOption::GcDisabled(disabled)))
    }
}
