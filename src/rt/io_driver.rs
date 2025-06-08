use std::{
    collections::HashMap,
    os::fd::{AsFd, AsRawFd, BorrowedFd, RawFd},
    task::LocalWaker,
    time::Instant,
};

use nix::poll::{ppoll, PollFd, PollFlags};

pub(super) fn with_driver<F, O>(f: F) -> O
where
    F: FnOnce(&mut IoDriver) -> O,
{
    super::with_reactor(|r| (f)(&mut r.io))
}

enum Interest {
    Read,
    Write,
}

#[derive(Default)]
struct Reg {
    read_waker: Option<LocalWaker>,
    write_waker: Option<LocalWaker>,
}

impl Reg {
    const fn is_empty(&self) -> bool {
        self.read_waker.is_none() && self.write_waker.is_none()
    }

    fn update_waker(&mut self, interest: Interest, waker: &LocalWaker) {
        let slot = match interest {
            Interest::Read => &mut self.read_waker,
            Interest::Write => &mut self.write_waker,
        };

        if let Some(old) = slot.as_mut() {
            old.clone_from(waker);
        } else {
            *slot = Some(waker.clone());
        }
    }

    fn interest(&self) -> PollFlags {
        match (self.read_waker.is_some(), self.write_waker.is_some()) {
            (true, true) => PollFlags::POLLIN | PollFlags::POLLOUT | PollFlags::POLLERR,
            (true, false) => PollFlags::POLLIN | PollFlags::POLLERR,
            (false, true) => PollFlags::POLLOUT | PollFlags::POLLERR,
            (false, false) => PollFlags::empty(),
        }
    }

    /// Observe the supplied poll results, waking up any needed futures. Returns the number of tasks woken
    fn observe(&mut self, flags: PollFlags) -> usize {
        let mut n = 0;

        if !flags
            .intersection(PollFlags::POLLIN | PollFlags::POLLERR)
            .is_empty()
        {
            if let Some(waker) = self.read_waker.take() {
                waker.wake();
                n += 1;
            }
        }

        if !flags
            .intersection(PollFlags::POLLOUT | PollFlags::POLLERR)
            .is_empty()
        {
            if let Some(waker) = self.write_waker.take() {
                waker.wake();
                n += 1;
            }
        }

        n
    }
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

    pub(super) fn poll(&mut self, deadline: Option<Instant>) -> std::io::Result<usize> {
        let mut fds = self
            .tasks
            .iter()
            // SAFETY: The resultant BorrowedFd lives only for the remainder of this function
            .map(|(raw, e)| unsafe { (BorrowedFd::borrow_raw(*raw), e) })
            .map(|(fd, e)| PollFd::new(fd, e.interest()))
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

            woken += entry.observe(events);
            if entry.is_empty() {
                self.tasks.remove(&fd);
            }
        }

        Ok(woken)
    }

    fn register(&mut self, interest: Interest, fd: RawFd, waker: &LocalWaker) {
        self.tasks
            .entry(fd)
            .or_default()
            .update_waker(interest, waker);
    }

    pub(super) fn register_read(&mut self, fd: RawFd, waker: &LocalWaker) {
        self.register(Interest::Read, fd, waker);
    }

    pub(super) fn register_write(&mut self, fd: RawFd, waker: &LocalWaker) {
        self.register(Interest::Write, fd, waker);
    }

    pub(super) fn remove(&mut self, fd: &RawFd) {
        if let Some(task) = self.tasks.remove(fd) {
            if let Some(reader) = task.read_waker {
                reader.wake();
            }
            if let Some(writer) = task.write_waker {
                writer.wake();
            }
            log::warn!("dropped in-progress IO task");
        }
    }
}
