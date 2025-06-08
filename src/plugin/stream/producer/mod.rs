// TODO: validate behavior if stream is dropped early

mod core;
use core::{Core, CoreRef};

mod adapter;
use adapter::{Adapter, GenericAdapter};

mod client;
pub use client::{ByteProducer, ListProducer, Producer};

mod manager;
pub(in crate::plugin) use manager::ProducerManager;
