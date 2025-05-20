use std::{fmt, sync::atomic::AtomicU64};

/// Task metadata, used for identifying tasks
#[derive(Debug, Copy, Clone)]
pub struct Metadata {
    /// A _globally unique_ per-task ID
    id: u64,
    /// A human-readable name, for help
    name: &'static str,
}

impl fmt::Display for Metadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Task {}: '{}'", self.id(), self.name())
    }
}

impl Metadata {
    /// Returns new metadata for a task, auto-generating an ID
    pub(super) fn new(name: &'static str) -> Self {
        static ID: AtomicU64 = AtomicU64::new(0);
        let id = ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self { id, name }
    }

    pub const fn id(&self) -> u64 {
        self.id
    }

    pub const fn name(&self) -> &'static str {
        self.name
    }
}
