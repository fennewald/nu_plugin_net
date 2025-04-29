#![feature(new_zeroed_alloc)]

mod ping;
pub use ping::PingCommand;

mod cat;
pub use cat::CatCommand;

mod plugin;
pub use plugin::Plugin;

// traceroute
// netstat
// netcat
// route
// interface statistics
