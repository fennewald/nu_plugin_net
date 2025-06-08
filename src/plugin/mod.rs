use nu_plugin_protocol::PluginOutput;
use nu_protocol::ShellError;

pub type Result<T> = std::result::Result<T, ShellError>;

mod traits;
pub use traits::{Command, CommandExt, Plugin};

mod stream;
pub use stream::{ByteConsumer, ByteProducer, InputDataHeader, ListConsumer, ListProducer};
use stream::{ConsumerManager, ProducerManager};

mod core;
use core::{Core, CoreRef};

mod manager;

mod context;
pub use context::Context;
use context::ContextHandle;

pub mod entry;
mod io;

mod engine;
pub use engine::EngineInterface;
use engine::{EngineResponse, EngineState};

pub type OutputStream = crate::channel::Sender<PluginOutput>;
