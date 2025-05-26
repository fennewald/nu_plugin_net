mod core;
use core::{Core, CoreRef};

mod adapter;
use adapter::{Adapter, GenericAdapter};

mod state;
use state::StateRef;

mod handle;
pub(in crate::plugin) use handle::ProducerHandle;

mod client;
use client::{ByteProducer, ListProducer, Producer};

mod manager;
