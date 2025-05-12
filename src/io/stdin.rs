use std::io::Result;

use crate::rt::io::EventedSource;

pub type Stdin = EventedSource<std::io::StdinLock<'static>>;

pub fn open() -> Result<Stdin> {
    let mut handle = std::io::stdin().lock();
    use nix::fcntl::{fcntl, FcntlArg::F_SETFL, OFlag};
    fcntl(&mut handle, F_SETFL(OFlag::O_NONBLOCK)).map_err(|_| std::io::Error::last_os_error())?;
    Ok(EventedSource::new(handle))
}
