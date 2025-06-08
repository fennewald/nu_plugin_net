use nu_plugin_protocol::CallInfo;
use nu_protocol::Signature;

use crate::plugin::{Command, CommandExt, Context, InputDataHeader, Result};

pub struct Net;

impl crate::plugin::Plugin for Net {
    const NAME: &str = "net";

    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    fn commands(&self) -> impl Iterator<Item = Box<dyn CommandExt<Plugin = Self>>> {
        [PingCommand::new()].into_iter()
    }
}

#[derive(Clone)]
pub struct PingCommand;

impl PingCommand {
    fn new() -> Box<dyn CommandExt<Plugin = Net>> {
        Box::new(PingCommand)
    }
}

impl Command for PingCommand {
    type Plugin = Net;

    const NAME: &str = "net ping";

    fn description(&self) -> &'static str {
        "Ping description"
    }

    fn signature(&self) -> Signature {
        Signature::new(self.name())
    }

    async fn run(
        self,
        call: CallInfo<InputDataHeader>,
        mut ctx: Context<Self::Plugin>,
    ) -> Result<()> {
        log::info!("inside task id {}", ctx.id());

        let decl_id = ctx.engine().find_decl(self.name()).await?;
        log::info!("my decl id: {:?}", decl_id);

        ctx.respond_empty()
    }
}
