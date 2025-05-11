use std::{
    cell::{RefCell, RefMut},
    collections::{hash_map::Entry, BTreeMap, HashMap},
    future::Future,
    io,
    ops::{ControlFlow, Deref, DerefMut},
    os::fd::{AsFd, AsRawFd, BorrowedFd, RawFd},
    pin::Pin,
    sync::atomic::{AtomicBool, Ordering},
    task::{Context, LocalWaker, Poll},
    time::{Duration, Instant},
};

use nix::poll::{ppoll, PollFd, PollFlags};

thread_local! {
    static REACTOR: &'static RefCell<Reactor> = Reactor::new();
}

fn reactor_cell() -> &'static RefCell<Reactor> {
    REACTOR.with(|v| *v)
}

fn reactor() -> RefMut<'static, Reactor> {
    reactor_cell().borrow_mut()
}

/// Marks any timer tasks that are expired as ready
pub(super) fn enqueue_elapsed() {
    reactor().enqueue_elapsed();
}

pub(super) fn block() -> ControlFlow<io::Result<()>> {
    reactor().block()
}

#[repr(transparent)]
pub struct EventedSource<T: AsFd> {
    src: T,
}

impl<T: AsFd> Drop for EventedSource<T> {
    fn drop(&mut self) {
        if let Some(task) = reactor().io_awaited.remove(&self.as_raw_fd()) {
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

impl<T: AsFd> AsRawFd for EventedSource<T> {
    fn as_raw_fd(&self) -> RawFd {
        self.src.as_fd().as_raw_fd()
    }
}

impl<T: AsFd> EventedSource<T> {
    pub fn new(src: T) -> Self {
        Self { src }
    }

    pub fn register_read(&self, waker: &LocalWaker) {
        let interest = PollFlags::POLLIN | PollFlags::POLLERR;
        match reactor().io_awaited.entry(self.as_raw_fd()) {
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
                slot.insert(IoReg {
                    interest,
                    read_waker: Some(waker.clone()),
                    write_waker: None,
                });
            }
        }
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

// type IoRef = Rc<RefCell<IoResource>>;

// pub fn register_io(fd: impl Into<OwnedFd>) -> EventedSource {
//     EventedSource(Rc::new(RefCell::new(IoResource {
//         fd: fd.into(),
//         interest: PollFlags::empty(),
//         waker: None,
//     })))
// }

struct IoReg {
    interest: PollFlags,
    read_waker: Option<LocalWaker>,
    write_waker: Option<LocalWaker>,
}

struct Reactor {
    timed_tasks: BTreeMap<Instant, Vec<LocalWaker>>,
    io_awaited: HashMap<RawFd, IoReg>,
}

impl Reactor {
    /// Creates a new reactor. May only be called _once_ per thread
    fn new() -> &'static RefCell<Self> {
        static INIT: AtomicBool = AtomicBool::new(false);

        if !INIT.fetch_or(true, Ordering::Relaxed) {
            Box::leak(Box::new(RefCell::new(Self {
                timed_tasks: BTreeMap::new(),
                io_awaited: HashMap::new(),
            })))
        } else {
            panic!("Tried to re-initalize already initalized reactor");
        }
    }

    /// Enqueues elapsed tasks, returning the number of tasks awoken
    fn enqueue_elapsed(&mut self) -> usize {
        let now = Instant::now();

        let mut count = 0;

        for (time, wakers) in std::iter::from_fn(|| {
            self.timed_tasks
                .first_entry()
                .filter(|e| *e.key() <= now)
                .map(|e| e.remove_entry())
        }) {
            let late = time - now;
            if late >= Duration::from_millis(1) {
                log::warn!("{:?} late for {} tasks: {:?}", late, wakers.len(), wakers);
            }
            for waker in wakers {
                count += 1;
                waker.wake();
            }
        }

        count
    }

    /// Polls internal file events. Returns the number of events woken up
    fn poll(&mut self, deadline: Option<Instant>) -> io::Result<usize> {
        let mut fds = self
            .io_awaited
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
            let Some(entry) = self.io_awaited.get_mut(&fd) else {
                log::error!("somehow got results on an fd I didn't ask about");
                continue;
            };

            if !events
                .intersection(PollFlags::POLLIN | PollFlags::POLLERR)
                .is_empty()
            {
                entry.interest ^= PollFlags::POLLIN;
                if let Some(waker) = entry.read_waker.take() {
                    woken += 1;
                    waker.wake();
                }
            }
            if !events
                .intersection(PollFlags::POLLOUT | PollFlags::POLLERR)
                .is_empty()
            {
                entry.interest ^= PollFlags::POLLOUT;
                if let Some(waker) = entry.write_waker.take() {
                    woken += 1;
                    waker.wake();
                }
            }

            if entry.read_waker.is_none() && entry.write_waker.is_none() {
                self.io_awaited.remove(&fd);
            }
        }

        Ok(woken)
    }

    /// Test if there's IO work to be done
    fn has_io(&self) -> bool {
        !self.io_awaited.is_empty()
    }

    fn block(&mut self) -> ControlFlow<io::Result<()>> {
        let deadline = self.timed_tasks.first_entry().map(|ent| *ent.key());
        if self.has_io() {
            match self.poll(deadline) {
                Ok(n) => {
                    log::debug!("io poll awoke {} events", n);
                    ControlFlow::Continue(())
                }
                Err(e) => ControlFlow::Break(Err(e)),
            }
        } else if let Some(deadline) = deadline {
            log::trace!("waiting for timer");
            std::thread::sleep_until(deadline);
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(Ok(()))
        }
    }

    fn register_timer(&mut self, time: Instant, waker: &LocalWaker) {
        if let Some(timers) = self.timed_tasks.get_mut(&time) {
            if let Some(sibling) = timers.iter_mut().find(|sib| waker.will_wake(sib)) {
                sibling.clone_from(waker);
            } else {
                timers.push(waker.clone());
            }
        } else {
            self.timed_tasks.insert(time, vec![waker.clone()]);
        }
    }
}

pub struct Timer {
    deadline: Instant,
}

impl Future for Timer {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let now = Instant::now();
        if self.deadline <= now {
            log::trace!("triggered timer, {:?} off", now - self.deadline);
            Poll::Ready(())
        } else {
            log::trace!("registering new timer, duration {:?}", self.deadline - now);
            reactor().register_timer(self.deadline, cx.local_waker());
            Poll::Pending
        }
    }
}

impl Timer {
    pub fn new(dur: Duration) -> Self {
        Self {
            deadline: Instant::now() + dur,
        }
    }
}
