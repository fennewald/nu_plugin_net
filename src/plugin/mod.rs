use nu_plugin_protocol::PluginOutput;
use nu_protocol::Signature;

pub mod entry;
mod io;
mod manager;

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
}
