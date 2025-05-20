mod errs;
pub use errs::{Closed, NoReceiver, NoSender};

pub mod oneshot;
pub use oneshot::{Receiver as OneshotReceiver, Sender as OneshotSender};

mod core;
pub use core::{new, with_capacity, Receiver, Sender};
