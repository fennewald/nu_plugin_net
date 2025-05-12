use std::{
    collections::{hash_map::Entry, HashMap},
    io::{self, ErrorKind},
    ops::{Deref, DerefMut},
    os::fd::{AsFd, AsRawFd, BorrowedFd, RawFd},
    pin::Pin,
    task::{Context, LocalWaker, Poll},
    time::Instant,
};

use futures::AsyncRead;
use nix::poll::{ppoll, PollFd, PollFlags};

fn with_driver<F, O>(f: F) -> O
where
    F: FnOnce(&mut IoDriver) -> O,
{
    super::reactor::with_reactor(|r| (f)(&mut r.io))
}

#[repr(transparent)]
pub struct EventedSource<T: AsFd> {
    src: T,
}

impl<T: AsFd + Unpin> Unpin for EventedSource<T> {}

impl<T: AsFd> Drop for EventedSource<T> {
    fn drop(&mut self) {
        with_driver(|driver| {
            if let Some(task) = driver.tasks.remove(&self.as_raw_fd()) {
                if let Some(reader) = task.read_waker {
                    reader.wake();
                }
                if let Some(writer) = task.write_waker {
                    writer.wake();
                }
                log::warn!("dropped in-progress IO task");
            }
        })
    }
}

impl<T: AsFd> AsRawFd for EventedSource<T> {
    fn as_raw_fd(&self) -> RawFd {
        self.src.as_fd().as_raw_fd()
    }
}

impl<T> AsyncRead for EventedSource<T>
where
    T: AsFd + io::Read + Unpin,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        let res = this.src.read(buf);
        if res
            .as_ref()
            .is_err_and(|e| e.kind() == ErrorKind::WouldBlock)
        {
            // Read isn't ready yet
            with_driver(|driver| driver.register_read(this.as_raw_fd(), cx.local_waker()));
            Poll::Pending
        } else {
            Poll::Ready(res)
        }
    }
}

impl<T: AsFd> EventedSource<T> {
    pub fn new(src: T) -> Self {
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

struct Reg {
    interest: PollFlags,
    read_waker: Option<LocalWaker>,
    write_waker: Option<LocalWaker>,
}

pub(super) struct IoDriver {
    tasks: HashMap<RawFd, Reg>,
}

impl IoDriver {
    pub(super) fn new() -> Self {
        Self {
            tasks: HashMap::new(),
        }
    }

    /// Tests if there are any IO operations enqueued
    pub(super) fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    pub(super) fn poll(&mut self, deadline: Option<Instant>) -> io::Result<usize> {
        let mut fds = self
            .tasks
            .iter()
            // SAFETY: The resultant BorrowedFd lives only for the remainder of this function
            .map(|(raw, e)| unsafe { (BorrowedFd::borrow_raw(*raw), e) })
            .map(|(fd, e)| PollFd::new(fd, e.interest))
            .collect::<Vec<PollFd<'static>>>();

        let timeout = deadline.map(|t| (t - Instant::now()).into());

        ppoll(&mut fds, timeout, None)?;

        let mut woken = 0;

        for resp in fds.iter().filter(|e| e.any().unwrap_or_default()) {
            let fd = resp.as_fd().as_raw_fd();
            let Some(events) = resp.revents() else {
                log::error!("got unrecognized values in the poll response");
                continue;
            };
            let Some(entry) = self.tasks.get_mut(&fd) else {
                log::error!("somehow got results on an fd I didn't ask about");
                continue;
            };

            if !events
                .intersection(PollFlags::POLLIN | PollFlags::POLLERR)
                .is_empty()
            {
                entry.interest &= !PollFlags::POLLIN;
                if let Some(waker) = entry.read_waker.take() {
                    woken += 1;
                    waker.wake();
                }
            }

            if !events
                .intersection(PollFlags::POLLOUT | PollFlags::POLLERR)
                .is_empty()
            {
                entry.interest &= !PollFlags::POLLOUT;
                if let Some(waker) = entry.write_waker.take() {
                    woken += 1;
                    waker.wake();
                }
            }

            if entry.read_waker.is_none() && entry.write_waker.is_none() {
                self.tasks.remove(&fd);
            }
        }

        Ok(woken)
    }

    fn register_read(&mut self, fd: RawFd, waker: &LocalWaker) {
        let interest = PollFlags::POLLIN | PollFlags::POLLERR;
        match self.tasks.entry(fd) {
            Entry::Occupied(mut entry) => {
                log::debug!("IO entry event updated");
                let entry = entry.get_mut();
                entry.interest |= interest;
                if let Some(old) = entry.read_waker.as_mut() {
                    old.clone_from(waker);
                } else {
                    entry.read_waker = Some(waker.clone());
                }
            }
            Entry::Vacant(slot) => {
                log::debug!("new IO entry created");
                slot.insert(Reg {
                    interest,
                    read_waker: Some(waker.clone()),
                    write_waker: None,
                });
            }
        }
    }
}
