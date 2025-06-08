use super::io_driver::with_driver;

mod core;
pub use core::EventedSource;

mod stdio;
pub use stdio::{stdin, stdout, Stdin, Stdout};
