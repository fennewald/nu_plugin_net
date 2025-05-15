use std::cell::Cell;

pub(super) struct State(Cell<usize>);

/// The task is running
const RUNNING: usize = 0b0001;

/// The task is complete
const COMPLETE: usize = 0b0010;

/// Flag tracking if the task has been pushed into a run queue.
const SCHEDULED: usize = 0b0100;

/// The task has been forcibly cancelled.
const CANCELLED: usize = 0b1000;

/// All bits
const STATE_MASK: usize = CANCELLED | SCHEDULED | COMPLETE | RUNNING;

/// Bits used by the ref count
const REF_COUNT_MASK: usize = !STATE_MASK;

/// Number of positions to shift the ref count
const REF_COUNT_SHIFT: usize = REF_COUNT_MASK.count_zeros() as usize;

/// One ref count
const REF_ONE: usize = 1 << REF_COUNT_SHIFT;

/// Maximum ref count
const _MAX_REFS: usize = usize::MAX / REF_ONE;

/// The error returned when a schedule operation fails
#[derive(Debug, thiserror::Error)]
pub(crate) enum ScheduleError {
    #[error("The task is already scheduled")]
    AlreadyScheduled,
    #[error("The task has already finished")]
    Done,
}

/// The error returned when a `mark_running` operation fails
#[derive(Debug, thiserror::Error)]
pub(crate) enum PollError {
    #[error("The task is already marked as running")]
    AlreadyRunning,
    #[error("The task is marked for cancellation")]
    Cancelled,
    #[error("The task is marked as complete")]
    Complete,
}

impl State {
    /// Returns the initial state of the task. One live reference, and no state bits set
    pub(super) const fn init() -> Self {
        Self(Cell::new(REF_ONE))
    }

    const fn get(&self) -> usize {
        self.0.get()
    }

    pub(super) const fn ref_count(&self) -> usize {
        self.get() >> REF_COUNT_SHIFT
    }

    /// Increments the reference count by 1
    /// # Safety
    /// Caller guarantees that this reference count will eventually be responsibly freed
    pub(super) unsafe fn inc_ref(&self) {
        self.0.update(|s| s + REF_ONE);
    }

    /// Decrements the reference count by 1
    /// Returns `true` if the reference count is now zero
    /// # Safety
    /// Caller guarantees that this reference count was responsibly attained
    pub(super) unsafe fn dec_ref(&self) -> bool {
        assert!(self.ref_count() > 0);
        self.0.update(|s| s - REF_ONE);
        self.ref_count() == 0
    }

    pub(super) const fn is_running(&self) -> bool {
        self.get() & RUNNING != 0
    }

    pub(super) const fn is_complete(&self) -> bool {
        self.get() & COMPLETE != 0
    }

    pub(super) const fn is_scheduled(&self) -> bool {
        self.get() & SCHEDULED != 0
    }

    pub(super) const fn is_cancelled(&self) -> bool {
        self.get() & CANCELLED != 0
    }

    /// Mark a task as complete
    pub(super) fn mark_complete(&self) {
        self.0.update(|s| s | COMPLETE);
    }

    /// Mark a task as scheduled
    pub(super) fn mark_scheduled(&self) -> Result<(), ScheduleError> {
        if self.is_complete() {
            Err(ScheduleError::Done)
        } else if self.is_scheduled() {
            Err(ScheduleError::AlreadyScheduled)
        } else {
            self.0.update(|s| s | SCHEDULED);
            Ok(())
        }
    }

    /// Sets the running flag,
    pub(super) fn mark_running(&self) -> Result<(), PollError> {
        if self.is_running() {
            Err(PollError::AlreadyRunning)
        } else if self.is_complete() {
            Err(PollError::Complete)
        } else if self.is_cancelled() {
            Err(PollError::Cancelled)
        } else {
            self.0.update(|s| s | RUNNING);
            Ok(())
        }
    }

    pub(super) fn unset_running(&self) {
        self.0.update(|s| s & !RUNNING);
    }

    pub(super) fn unset_scheduled(&self) {
        self.0.update(|s| s & !SCHEDULED)
    }

    pub(super) fn mark_cancelled(&self) {
        self.0.update(|s| s | CANCELLED)
    }
}
