mod core;
use core::{Core, CoreRef};

mod adapter;
use adapter::{Adapter, GenericAdapter};

mod client;
pub use client::{ByteConsumer, ListConsumer};

mod manager;
pub(in crate::plugin) use manager::ConsumerManager;
