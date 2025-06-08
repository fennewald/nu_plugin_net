//! rt/mod.rs
//!
//! Contained herein is a custom asynchronous runtime for evaluating this plugin.
//!
//! The highlights
//!
//! ### Always Single-Threaded: !Send + !Sync
//! By enforcing a single thread for all execution, we get to just not think about concurrency. As
//! someone who spends a lot of time thinking about concurrency, this is a _huge_ win. A `RefCell`
//! (or `UnsafeCell` *wink*) wrapped with a few transaction methods is safe (despite what
//! `UnsafeCell` may tell you), and easy to reason about.
//!
//! ### Dirt-Simple Scheduling
//! It's simple. _*Never*_ yield to IO unless you can't help it. This means:
//! - :)  Easy to reason about
//! - :(  A single future that never yields can lock the runtime
//!
//! ### `poll(2)`
//! Forget `epoll` or `kqueue`, or whatever the hell windows does. Just use `poll` (and `ppoll`, if
//! available). This does mean we would run into performance issues scaling to more than a few
//! hundred open files. Thankfully, we can fix this issue by just _not doing that_.
//!
//! In practice, using `poll` everywhere does have several ancilliary benefits. We don't have to
//! worry about TOCTOU readiness issues, which means we can only register reads in the happy case.
//! This means the 'base case', i.e. reading from a file with data already-ready, incurs _zero_
//! penalty for being done inside the runtime. Something something zero-cost abstractions are cool.
//!
//! ### Just Steal Some of Tokio's Guts
//! Good artists borrow, I just steal. Tokio's task internals are great. They nicely limit
//! per-future allocations to 1, allow for ergonomic `Waker` object creation (dyn point'er? I hardly
//! know'er!), and they keep nitty gritty state transition details meaningfully isolated from nitty
//! gritty memory management details.
//!
//! The `task` module is a single-threaded implementation of a tokio-like system. See the docs
//! therein for more details.

mod task;
pub(crate) use task::JoinHandle;

mod executor;
pub(crate) use executor::run;

mod queue;
use queue::schedule;

mod spawn;
pub use spawn::spawn;

mod reactor;
use reactor::with_reactor;

mod time_driver;
use time_driver::TimeDriver;

pub mod time;

mod io_driver;
use io_driver::IoDriver;

pub mod io;
