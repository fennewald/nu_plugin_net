use super::io_driver::with_driver;

mod core;
pub(crate) use core::EventedSource;

mod stdio;
pub(crate) use stdio::{Stdin, Stdout, stdin, stdout};
