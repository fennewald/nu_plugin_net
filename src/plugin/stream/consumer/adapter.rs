use nu_plugin_protocol::StreamData;
use nu_protocol::ShellError;

use crate::plugin::{
    stream::{ByteStreamData, ListStreamData},
    Result,
};

use super::CoreRef;

/// A generalized form of the type-specific adapter
pub(super) enum GenericAdapter {
    List(Adapter<ListStreamData>),
    Byte(Adapter<ByteStreamData>),
}

impl From<Adapter<ByteStreamData>> for GenericAdapter {
    fn from(v: Adapter<ByteStreamData>) -> Self {
        Self::Byte(v)
    }
}

impl From<Adapter<ListStreamData>> for GenericAdapter {
    fn from(v: Adapter<ListStreamData>) -> Self {
        Self::List(v)
    }
}

impl GenericAdapter {
    /// Called by the manager to handle an incoming `StreamData` message
    pub(super) fn data(&mut self, msg: StreamData) -> Result<()> {
        match (self, msg) {
            (Self::List(a), StreamData::List(it)) => a.data(it),
            (Self::Byte(a), StreamData::Raw(it)) => a.data(it),
            (Self::Byte(_), StreamData::List(v)) => Err(ShellError::PluginFailedToDecode {
                msg: "Stream was created with the 'raw' type, but received list data instead"
                    .into(),
            }
            .into_chainned(v.span())),
            (Self::List(_), StreamData::Raw(_)) => Err(ShellError::PluginFailedToDecode {
                msg: "Stream was created with the 'list' type, but received raw data".into(),
            }),
        }
    }

    /// Used by the manager to signal the `end` message has been received. The adapter will be
    /// dropped in this function
    pub(super) fn end(self) {
        match self {
            Self::List(a) => a.end(),
            Self::Byte(a) => a.end(),
        }
    }
}

/// The manager-side handle for an incoming stream.
/// It's lifetime corresponds to the stream lifetime. When the stream is `ended`, this handle is
/// removed. This handle may still exist while the corresponding consumer has been `drop`ed
#[repr(transparent)]
pub(super) struct Adapter<D>(CoreRef<D>);

impl<D> Adapter<D> {
    pub(super) fn new(core: CoreRef<D>) -> Self {
        Self(core)
    }

    fn data(&mut self, msg: D) -> Result<()> {
        self.0.borrow_mut().push(msg)
    }

    fn end(self) {
        self.0.borrow_mut().end()
    }
}
