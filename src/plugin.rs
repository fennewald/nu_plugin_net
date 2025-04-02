use crate::{InterfacesCommand, PingCommand};

pub struct Plugin {
    rt: tokio::runtime::Runtime,
}

impl Plugin {
    pub fn new() -> Plugin {
        Plugin {
            rt: tokio::runtime::Runtime::new().unwrap(),
        }
    }
}

impl nu_plugin::Plugin for Plugin {
    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    fn commands(&self) -> Vec<Box<dyn nu_plugin::PluginCommand<Plugin = Self>>> {
        vec![Box::new(InterfacesCommand), Box::new(PingCommand)]
    }
}
