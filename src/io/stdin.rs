use std::{
    io::{ErrorKind, Read, Result},
    pin::Pin,
    task::{Context, Poll},
};

use futures::AsyncRead;

use crate::rt::EventedSource;

pub enum Handle<T> {
    Present(T),
}

pub fn open() -> Result<Stdin> {
    Stdin::open()
}

#[repr(transparent)]
pub struct Stdin(EventedSource<std::io::StdinLock<'static>>);

impl Unpin for Stdin {}

impl Stdin {
    fn open() -> Result<Self> {
        let mut handle = std::io::stdin().lock();
        use nix::fcntl::{fcntl, FcntlArg::F_SETFL, OFlag};
        fcntl(&mut handle, F_SETFL(OFlag::O_NONBLOCK))
            .map_err(|_| std::io::Error::last_os_error())?;
        Ok(Self(EventedSource::new(handle)))
    }
}

impl AsyncRead for Stdin {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize>> {
        let this = self.get_mut();
        match this.0.read(buf) {
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                this.0.register_read(cx.local_waker());
                Poll::Pending
            }
            anything_else => Poll::Ready(anything_else),
        }
    }
}
