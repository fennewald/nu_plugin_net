use nu_plugin_protocol::StreamData;

use crate::plugin::{
    stream::{ByteStreamData, ListStreamData},
    Result,
};

use super::CoreRef;

pub(super) enum GenericAdapter {
    List(Adapter<ListStreamData>),
    Byte(Adapter<ByteStreamData>),
}

impl From<Adapter<ListStreamData>> for GenericAdapter {
    fn from(v: Adapter<ListStreamData>) -> Self {
        Self::List(v)
    }
}

impl From<Adapter<ByteStreamData>> for GenericAdapter {
    fn from(v: Adapter<ByteStreamData>) -> Self {
        Self::Byte(v)
    }
}

#[repr(transparent)]
pub(super) struct Adapter<D>(CoreRef<D>);

impl GenericAdapter {
    pub(super) fn ack(&mut self) -> Result<()> {
        match self {
            GenericAdapter::List(a) => a.ack(),
            GenericAdapter::Byte(a) => a.ack(),
        }
    }

    pub(super) fn drop(self) -> Result<()> {
        match self {
            GenericAdapter::List(a) => a.drop(),
            GenericAdapter::Byte(a) => a.drop(),
        }
    }
}

impl<D> Adapter<D>
where
    D: Into<StreamData>,
{
    pub(super) fn new(core: CoreRef<D>) -> Self {
        Self(core)
    }

    /// Handle an ack message
    fn ack(&mut self) -> Result<()> {
        self.0.borrow_mut().ack()
    }

    /// Handle a drop message
    fn drop(self) -> Result<()> {
        self.0.borrow_mut().drop()
    }
}
