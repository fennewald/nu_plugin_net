use super::JoinHandle;

use std::future::Future;

/// Spawns a new future onto the runtime
pub fn spawn<F: Future + 'static>(name: &'static str, future: F) -> JoinHandle<F::Output> {
    let (handle, task) = super::task::alloc(name, future);
    super::schedule(task);
    handle
}
