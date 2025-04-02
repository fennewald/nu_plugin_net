use nu_plugin::{EngineInterface, EvaluatedCall, PluginCommand};
use nu_protocol::{LabeledError, PipelineData, Signature, Span, Value};

pub struct PingCommand;

impl PluginCommand for PingCommand {
    type Plugin = crate::Plugin;

    fn name(&self) -> &str {
        "net ping"
    }

    fn description(&self) -> &str {
        "Pings a host"
    }

    fn signature(&self) -> Signature {
        Signature::new(self.name()).description(self.description())
    }

    fn run(
        &self,
        plugin: &Self::Plugin,
        engine: &EngineInterface,
        call: &EvaluatedCall,
        input: PipelineData,
    ) -> Result<PipelineData, LabeledError> {
        if !input.is_nothing() {
            return Err(LabeledError::new("ambiguous input").with_label(
                "No pipeline input expected",
                input.span().unwrap_or(Span::unknown()),
            ));
        }

        todo!()
    }
}
