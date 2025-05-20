use nu_protocol::ShellError;

#[derive(Debug, thiserror::Error, Clone, Copy)]
#[error("This channel's receiver has been dropped")]
pub struct NoReceiver;

#[derive(Debug, thiserror::Error, Clone, Copy)]
#[error("This channel's sender has been dropped")]
pub struct NoSender;

#[derive(Debug, thiserror::Error, Clone, Copy)]
#[error("This channel's been closed")]
pub struct Closed;

impl From<Closed> for ShellError {
    fn from(_: Closed) -> Self {
        ShellError::NushellFailed {
            msg: "An internal channel was closed while still in-use".to_string(),
        }
    }
}
