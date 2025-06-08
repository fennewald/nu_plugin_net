mod core;
use core::{Core, CoreRef};

mod adapter;
use adapter::{Adapter, GenericAdapter};

mod client;
pub use client::{ByteConsumer, ListConsumer};

mod manager;
pub(in crate::plugin) use manager::ConsumerManager;

use nu_protocol::{PipelineMetadata, Value};

/// A `PipelineDataHeader` that's been integrated with our manager already
pub enum InputDataHeader {
    Empty,
    Value(Value, Option<PipelineMetadata>),
    List(ListConsumer),
    Byte(ByteConsumer),
}

/// The number of messages to early-ack in incoming stream. Setting this to zero disables the behavior
const STREAM_EAGERNESS: usize = 4;
