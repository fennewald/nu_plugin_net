use std::task::{LocalWaker, RawWaker, RawWakerVTable};

use super::{Header, RawTask};

const VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, dealloc);

pub(super) fn into_waker(task: RawTask) -> LocalWaker {
    let data = unsafe { task.into_raw().as_ptr() };
    unsafe { LocalWaker::new(data as _, &VTABLE) }
}

unsafe fn clone(raw: *const ()) -> RawWaker {
    let header = unsafe { (raw as *const Header).as_ref().unwrap() };
    unsafe { header.state.inc_ref() };
    RawWaker::new(raw, &VTABLE)
}

unsafe fn wake(raw: *const ()) {
    let task = unsafe { RawTask::from_raw(raw as _) };
    crate::rt::schedule(task);
}

unsafe fn wake_by_ref(raw: *const ()) {
    let task = unsafe { RawTask::from_raw_ref(raw as _) };
    crate::rt::schedule(task);
}

unsafe fn dealloc(raw: *const ()) {
    let task = unsafe { RawTask::from_raw(raw as _) };
    drop(task);
}
