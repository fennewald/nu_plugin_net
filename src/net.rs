pub struct Net;

impl crate::plugin::Plugin for Net {
    const NAME: &str = "net";

    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    fn commands(&self) -> impl Iterator<Item = Box<dyn crate::plugin::Command<Plugin = Self>>> {
        std::iter::empty()
    }
}
