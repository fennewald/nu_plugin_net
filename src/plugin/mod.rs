use nu_plugin_protocol::{CallInfo, PipelineDataHeader, PluginOutput};
use nu_protocol::{ShellError, Signature};

pub type ShellResult<T> = Result<T, ShellError>;

pub mod producer;

pub mod entry;
mod io;

mod manager;
pub use manager::ActualizedPipelineDataHeader;

use crate::rt::JoinHandle;

mod consumer;
use consumer::GenericConsumerAdapter;
pub use consumer::{ByteConsumer, ListConsumer};

mod context;

mod engine;

pub type OutputStream = crate::channel::Sender<PluginOutput>;

/// Our own, async version of nushell's Plugin trait
pub trait Plugin {
    const NAME: &str;

    fn version(&self) -> String;

    fn commands(&self) -> impl Iterator<Item = Box<dyn Command<Plugin = Self>>>;
}

pub trait Command {
    type Plugin: Plugin;
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn signature(&self) -> Signature;
    fn spawn(
        &self,
        call_info: CallInfo<ActualizedPipelineDataHeader>,
    ) -> ShellResult<JoinHandle<ShellResult<()>>>;
}
