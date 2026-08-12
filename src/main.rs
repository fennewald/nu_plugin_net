use nu_plugin::{MsgPackSerializer, serve_plugin};
use nu_plugin_net::Plugin;

fn main() {
    serve_plugin(&Plugin::new(), MsgPackSerializer);
}
