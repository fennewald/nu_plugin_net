use std::io::{Result, StdoutLock};

use crate::rt::io::EventedSource;

pub type Stdin = EventedSource<std::io::StdinLock<'static>>;
pub type Stdout = EventedSource<StdoutLock<'static>>;

pub fn stdin() -> Result<Stdin> {
    let mut handle = std::io::stdin().lock();
    use nix::fcntl::{fcntl, FcntlArg::F_SETFL, OFlag};
    fcntl(&mut handle, F_SETFL(OFlag::O_NONBLOCK)).map_err(|_| std::io::Error::last_os_error())?;
    Ok(EventedSource::new(handle))
}

pub fn stdout() -> Result<Stdout> {
    let mut handle = std::io::stdout().lock();
    use nix::fcntl::{fcntl, FcntlArg::F_SETFL, OFlag};
    fcntl(&mut handle, F_SETFL(OFlag::O_NONBLOCK)).map_err(|_| std::io::Error::last_os_error())?;
    Ok(EventedSource::new(handle))
}
