use nu_plugin_protocol::CallInfo;
use nu_protocol::{PluginExample, Signature};

use crate::rt::JoinHandle;

use super::{Context, InputDataHeader, Result};

pub trait Plugin: 'static {
    const NAME: &str;

    fn version(&self) -> String;
    fn commands(&self) -> impl Iterator<Item = Box<dyn CommandExt<Plugin = Self>>>;
}

pub trait Command: Clone + 'static {
    type Plugin: Plugin;

    const NAME: &str;

    fn description(&self) -> &'static str;
    fn signature(&self) -> Signature;

    fn examples(&self) -> impl Iterator<Item = PluginExample> {
        std::iter::empty()
    }

    #[allow(async_fn_in_trait)]
    async fn run(self, call: CallInfo<InputDataHeader>, ctx: Context<Self::Plugin>) -> Result<()>;
}

pub trait CommandExt {
    type Plugin: Plugin;

    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn signature(&self) -> Signature;
    fn examples(&self) -> Vec<PluginExample>;

    fn spawn(
        &self,
        call: CallInfo<InputDataHeader>,
        ctx: Context<Self::Plugin>,
    ) -> JoinHandle<Result<()>>;
}

impl<T> CommandExt for T
where
    T: Command,
{
    type Plugin = T::Plugin;

    fn name(&self) -> &'static str {
        T::NAME
    }

    fn description(&self) -> &'static str {
        self.description()
    }

    fn signature(&self) -> Signature {
        self.signature()
    }

    fn examples(&self) -> Vec<PluginExample> {
        self.examples().collect()
    }

    fn spawn(
        &self,
        call: CallInfo<InputDataHeader>,
        ctx: Context<Self::Plugin>,
    ) -> JoinHandle<Result<()>> {
        let this = self.clone();
        crate::rt::spawn(self.name(), async move { this.run(call, ctx).await })
    }
}
