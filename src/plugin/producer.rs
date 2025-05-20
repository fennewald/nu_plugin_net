use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use nu_plugin_protocol::StreamId;
use nu_protocol::{LabeledError, ShellError, Value};

pub type RawStreamData = Result<Vec<u8>, LabeledError>;
pub type ListStreamData = Value;

struct State<D> {
    /// A flag indicating that the `End` message has been sent
    ended: bool,
    /// A flag indicating that the `Drop` message has been received
    dropped: bool,
    /// The items to be sent
    items: VecDeque<D>,
    /// The current Ack debt.
    /// 0 means that all sent messages have been acked.
    /// 1 means that we've sent 1 message that hasn't been acked yet
    ack_debt: u16,
    /// The target Ack debt. If we're more than 1 over this, we'll wait for an `Ack` to send data.
    tgt_debt: u16,
}

type StateRef<D> = Rc<RefCell<State<D>>>;

pub struct Producer {
    id: StreamId,
}

impl Producer {}

pub(super) struct ProducerAdapter {
    id: StreamId,
}

impl ProducerAdapter {
    pub(super) fn ack(&mut self) -> Result<(), ShellError> {
        todo!()
    }

    pub(super) fn drop(&mut self) -> Result<(), ShellError> {
        todo!()
    }
}
