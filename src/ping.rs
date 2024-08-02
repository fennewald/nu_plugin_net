use nu_plugin::{EngineInterface, EvaluatedCall, PluginCommand};
use nu_protocol::{LabeledError, PipelineData, Record, Signature, Span, Type, Value};

pub struct PingCommand {}

impl PluginCommand for PingCommand {
    type Plugin = crate::Plugin;

    fn name(&self) -> &str {
        "net ping"
    }

    fn usage(&self) -> &str {
        "pings a host"
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
    }

    fn run(
        &self,
        plugin: &Self::Plugin,
        engine: &EngineInterface,
        call: &EvaluatedCall,
        input: PipelineData,
    ) -> Result<PipelineData, LabeledError> {
        todo!()
    }
}
