use crate::InterfacesCommand;

pub struct Plugin;

impl Plugin {
    pub fn new() -> Plugin {
        Plugin {}
    }
}

impl nu_plugin::Plugin for Plugin {
    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    fn commands(&self) -> Vec<Box<dyn nu_plugin::PluginCommand<Plugin = Self>>> {
        vec![Box::new(InterfacesCommand)]
    }
}
