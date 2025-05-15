use std::{
    io::ErrorKind,
    ops::{Deref, DerefMut},
    os::fd::{AsFd, AsRawFd, RawFd},
    pin::Pin,
    task::{Context, Poll},
};

use futures::{AsyncRead, AsyncWrite};

use super::with_driver;

type IoResult<T> = std::io::Result<T>;

#[repr(transparent)]
pub(crate) struct EventedSource<T: AsFd> {
    src: T,
}

impl<T: AsFd + Unpin> Unpin for EventedSource<T> {}

impl<T: AsFd> Drop for EventedSource<T> {
    fn drop(&mut self) {
        with_driver(|d| d.remove(&self.as_raw_fd()))
    }
}

impl<T: AsFd> AsRawFd for EventedSource<T> {
    fn as_raw_fd(&self) -> RawFd {
        self.src.as_fd().as_raw_fd()
    }
}

impl<T> AsyncRead for EventedSource<T>
where
    T: AsFd + std::io::Read + Unpin,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<IoResult<usize>> {
        let this = self.get_mut();
        let res = this.src.read(buf);
        if res
            .as_ref()
            .is_err_and(|e| e.kind() == ErrorKind::WouldBlock)
        {
            // Read isn't ready yet
            with_driver(|d| d.register_read(this.as_raw_fd(), cx.local_waker()));
            Poll::Pending
        } else {
            Poll::Ready(res)
        }
    }
}

impl<T> AsyncWrite for EventedSource<T>
where
    T: AsFd + std::io::Write + Unpin,
{
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<IoResult<usize>> {
        let this = self.get_mut();
        let res = this.src.write(buf);
        if res
            .as_ref()
            .is_err_and(|e| e.kind() == ErrorKind::WouldBlock)
        {
            with_driver(|d| d.register_write(this.as_raw_fd(), cx.local_waker()));
            Poll::Pending
        } else {
            Poll::Ready(res)
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        let this = self.get_mut();

        let res = this.src.flush();

        if res
            .as_ref()
            .is_err_and(|e| e.kind() == ErrorKind::WouldBlock)
        {
            with_driver(|d| d.register_write(this.as_raw_fd(), cx.local_waker()));
            Poll::Pending
        } else {
            Poll::Ready(res)
        }
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        let _ = cx;
        todo!()
    }
}

impl<T: AsFd> EventedSource<T> {
    pub(crate) fn new(src: T) -> Self {
        Self { src }
    }
}

impl<T: AsFd> Deref for EventedSource<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.src
    }
}

impl<T: AsFd> DerefMut for EventedSource<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.src
    }
}
