use nu_protocol::{Signature, Value};

use super::{ByteConsumer, ListConsumer, Plugin, Result};

pub enum Input {
    Empty,
    Value(Value),
    List(ListConsumer),
    Byte(ByteConsumer),
}

trait Command {
    const NAME: &'static str;
    type Plugin: Plugin;

    fn description(&self) -> &'static str;
    fn signature(&self) -> Signature;

    async fn call(self, data: Input);
}

/// A dyn-compatible extension trait for commands
trait CommandExt {
    type Plugin: Plugin;

    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn signature(&self) -> Signature;
}

impl<T: Command> CommandExt for T {
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
}

fn foo<P: Plugin>() {
    let b: Box<dyn CommandExt<Plugin = P>> = todo!();
}
