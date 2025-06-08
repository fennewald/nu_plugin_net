use nu_plugin_protocol::{PipelineDataHeader, PluginCallId, PluginCallResponse, PluginOutput};
use nu_protocol::{ShellError, Span};

use crate::channel::Sender;

use super::Result;

/// A type that represents the response to a command
pub struct Response {
    /// id of the plugin call
    id: PluginCallId,
    /// span of the plugin call, for error reporting
    tx: Sender<PluginOutput>,
}

impl Response {
    /// Respond to this command invocation, returning `Empty`
    pub fn respond_empty(self) -> Result<()> {
        // self.tx.send(PluginOutput::CallResponse(
        //     self.id,
        //     PluginCallResponse::PipelineData(nu_plugin_protocol::PipelineDataHeader::Empty),
        // ))
        todo!()
    }
}

pub struct Context {
    /// id of the plugin call
    id: PluginCallId,
    /// Span of the command invocation
    span: Span,
    /// A handle to the pipeline output
    tx: Sender<PluginOutput>,
    /// Flag indicating a response has been sent
    responded: bool,
}

impl Context {
    pub(super) fn new(id: PluginCallId, span: Span, tx: Sender<PluginOutput>) -> Self {
        todo!()
    }

    /// Returns the span of the command invocation
    pub fn span(&self) -> Span {
        self.span
    }

    fn nushell_failed(&self, msg: impl Into<String>, label: impl Into<String>) -> ShellError {
        ShellError::NushellFailedSpanned {
            msg: msg.into(),
            label: label.into(),
            span: self.span,
        }
    }

    fn take_response(&mut self) -> Result<()> {
        if !self.responded {
            self.responded = true;
            Ok(())
        } else {
            Err(self.nushell_failed(
                format!("Tried to respond to a command invocation {} twice", self.id),
                "here",
            ))
        }
    }

    pub fn respond(&mut self, resp: PluginCallResponse<PipelineDataHeader>) -> Result<()> {
        self.take_response()?;
        self.tx
            .send(PluginOutput::CallResponse(self.id, resp))
            .map_err(|e| {
                self.responded = false;
                self.nushell_failed(
                    format!(
                        "failed to send output for command invocation {}: {}",
                        self.id, e
                    ),
                    "here",
                )
            })
    }

    /// Sends an empty response
    pub fn empty_response(&mut self) -> Result<()> {
        self.take_response()?;
        todo!()
    }
}
