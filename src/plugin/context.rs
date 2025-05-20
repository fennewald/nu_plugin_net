use nu_plugin_protocol::PluginOutput;

use crate::channel::Sender;

/// A task-side handle to the engine context.
pub(crate) struct Context {
    tx: Sender<PluginOutput>,
}

/// The engine-side counterpart
pub(super) struct ContextHandle {
    
}
