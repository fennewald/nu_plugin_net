use nu_plugin::{serve_plugin, MsgPackSerializer};
use nu_plugin_net::Plugin;

fn main() {
    serve_plugin(&Plugin::new(), MsgPackSerializer);
}
