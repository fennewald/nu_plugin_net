use nu_protocol::{LabeledError, Value};

type ByteStreamData = Result<Vec<u8>, LabeledError>;
type ListStreamData = Value;

mod consumer;
pub(super) use consumer::ConsumerManager;
pub use consumer::{ByteConsumer, InputDataHeader, ListConsumer};

mod producer;
pub(super) use producer::ProducerManager;
pub use producer::{ByteProducer, ListProducer};
