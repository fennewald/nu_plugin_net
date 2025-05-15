use super::JoinHandle;

use std::future::Future;

/// Spawns a new future onto the runtime
pub(crate) fn spawn<F: Future>(name: &'static str, future: F) -> JoinHandle<F::Output> {
    let (handle, task) = super::task::alloc(name, future);
    super::schedule(task);
    handle
}
