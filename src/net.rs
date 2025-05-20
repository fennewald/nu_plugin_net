use nu_plugin_protocol::{CallInfo, PipelineDataHeader};
use nu_protocol::{ShellError, Signature};

use crate::{
    plugin::{ActualizedPipelineDataHeader, Command},
    rt::JoinHandle,
};

pub struct Net;

impl crate::plugin::Plugin for Net {
    const NAME: &str = "net";

    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    fn commands(&self) -> impl Iterator<Item = Box<dyn Command<Plugin = Self>>> {
        [PingCommand::new()].into_iter()
    }
}

pub struct PingCommand;

impl PingCommand {
    fn new() -> Box<dyn Command<Plugin = Net>> {
        Box::new(PingCommand)
    }
}

impl Command for PingCommand {
    type Plugin = Net;

    fn name(&self) -> &'static str {
        "net ping"
    }

    fn description(&self) -> &'static str {
        "Ping description"
    }

    fn signature(&self) -> Signature {
        Signature::new(self.name())
    }

    fn spawn(
        &self,
        call_info: CallInfo<ActualizedPipelineDataHeader>,
    ) -> Result<JoinHandle<Result<(), ShellError>>, ShellError> {
        todo!()
    }
}
