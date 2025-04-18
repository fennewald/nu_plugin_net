#![feature(new_zeroed_alloc)]

mod inf;
pub use inf::InterfacesCommand;

mod ping;
pub use ping::PingCommand;

mod plugin;
pub use plugin::Plugin;

// traceroute
// netstat
// netcat
// route
// interface statistics

pub mod netlink;
