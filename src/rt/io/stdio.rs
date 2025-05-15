use std::io::{Result, StdoutLock};

use nix::fcntl::{FcntlArg::F_SETFL, OFlag, fcntl};

use super::EventedSource;

pub(crate) type Stdin = EventedSource<std::io::StdinLock<'static>>;
pub(crate) type Stdout = EventedSource<StdoutLock<'static>>;

pub(crate) fn stdin() -> Result<Stdin> {
    let mut handle = std::io::stdin().lock();
    fcntl(&mut handle, F_SETFL(OFlag::O_NONBLOCK)).map_err(|_| std::io::Error::last_os_error())?;
    Ok(EventedSource::new(handle))
}

pub(crate) fn stdout() -> Result<Stdout> {
    let mut handle = std::io::stdout().lock();
    fcntl(&mut handle, F_SETFL(OFlag::O_NONBLOCK)).map_err(|_| std::io::Error::last_os_error())?;
    Ok(EventedSource::new(handle))
}
