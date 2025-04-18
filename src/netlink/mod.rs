use zerocopy::{FromBytes, Immutable, IntoByteSlice, IntoBytes, KnownLayout, TryFromBytes};

use std::{
    ffi::c_int,
    io,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
        raw::c_void,
    },
    slice,
};

const NETLINK_BUFFER_SIZE: usize = 4096;
const N_BUFFERS: usize = 128;

type Buffer = [u8; NETLINK_BUFFER_SIZE];

fn open_raw(domain: c_int, ty: c_int, proto: c_int) -> io::Result<OwnedFd> {
    let sd = unsafe { libc::socket(domain, ty, proto) };

    if sd < 0 {
        Err(io::Error::last_os_error())
    } else {
        let raw_fd = unsafe { RawFd::from_raw_fd(sd) };
        let own_fd = unsafe { OwnedFd::from_raw_fd(raw_fd) };
        Ok(own_fd)
    }
}

const BUFF_LEN: usize = 32 * 1024;

/// This buffer has
#[repr(transparent)]
struct MsgBuf {
    iov: libc::iovec,
}

/*
/// A structure representing a buffered used for messages with the kernel. It owns it's own data
struct IoVec {
    iov: libc::iovec,
    full_len: usize,
}

impl Drop for IoVec {
    fn drop(&mut self) {
        unsafe { Box::from_raw(self.as_full_mut().as_mut_ptr()) };
    }
}

impl From<Box<[u8]>> for IoVec {
    fn from(value: Box<[u8]>) -> Self {
        let len = value.len();
        let ptr = Box::into_raw(value);

        Self {
            iov: libc::iovec {
                iov_base: ptr,
                iov_len: 0,
            },
            full_len: len,
        }
    }
}

impl Deref for IoVec {
    type Target = libc::iovec;

    fn deref(&self) -> &Self::Target {
        &self.iov
    }
}

impl DerefMut for IoVec {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.iov
    }
}

impl IoVec {
    /// Allocats a new IoVec
    fn new(size: usize) -> Self {
        unsafe { Box::new_zeroed_slice(size).assume_init().into() }
    }

    /// Returns the portion that the Kernel has written to, as a slice
    fn as_slice(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.iov.iov_base as *mut u8, self.iov.iov_len) }
    }

    fn as_full_mut(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self.iov.iov_base as *mut u8, self.full_len) }
    }
}

*/
struct RawSocket {
    sd: OwnedFd,
    pid: u32,
    seq: u32,
}

impl RawSocket {
    fn open() -> io::Result<Self> {
        let sd = open_raw(libc::AF_NETLINK, libc::SOCK_DGRAM, libc::NETLINK_GENERIC)?;

        Ok(RawSocket { sd, pid: 0, seq: 0 })
    }

    fn send_raw(&mut self, msg: &[u8]) -> io::Result<()> {
        let mut sent = 0;
        while sent < msg.len() {
            let to_send = &msg[sent..];
            let i =
                unsafe { libc::send(self.sd.as_raw_fd(), to_send.as_ptr() as _, to_send.len(), 0) };
            if i <= 0 {
                return Err(io::Error::last_os_error());
            } else {
                sent += i as usize;
            }
            println!("took another loop");
        }
        Ok(())
    }

    fn send<T: IntoBytes + Immutable>(&mut self, item: T) -> io::Result<()> {
        self.send_raw(item.as_bytes())
    }

    fn links(&mut self) -> io::Result<()> {
        let len = 32;
        self.send(NlMsgHdr {
            len,
            typ: libc::RTM_GETLINK,
            flags: NLM_F_REQUEST | libc::NLM_F_ECHO as u16,
            seq: self.seq,
            pid: self.pid,
        })?;

        self.send(IfInfoMsg {
            family: libc::AF_UNSPEC as u8,
            _pad: 0,
            typ: 0,
            index: 0,
            flags: 0,
            change: u32::MAX,
        })?;

        self.seq += 1;

        println!("sent");

        let res = self.recv();
        println!("got back: {:?}", res);

        todo!()
    }

    fn recv(&mut self) -> io::Result<Vec<u8>> {
        let mut buffer = vec![0; NETLINK_BUFFER_SIZE];

        let ret = unsafe {
            libc::recv(
                self.sd.as_raw_fd(),
                buffer.as_mut_ptr() as _,
                NETLINK_BUFFER_SIZE,
                0,
            )
        };

        if ret <= 0 {
            Err(io::Error::last_os_error())
        } else {
            buffer.truncate(ret as usize);
            Ok(buffer)
        }
    }

    /*
    /// Receive as many messages as we can on the internal socket
    fn recv_many(&mut self) -> io::Result<()> {
        let mut io_vecs = [const {
            libc::iovec {
                iov_base: std::ptr::null_mut(),
                iov_len: 0,
            }
        }; N_BUFFERS];

        let mut headers = [const {
            libc::mmsghdr {
                msg_len: 0,
                msg_hdr: unsafe { std::mem::zeroed() },
            }
        }; N_BUFFERS];

        let mut len = 0;

        for (i, free) in self.freelist().enumerate() {
            let hdr = &mut headers[i].msg_hdr;
            let iov = &mut io_vecs[i];

            iov.iov_base = free.as_mut_ptr() as _;
            iov.iov_len = free.len();

            hdr.msg_iov = iov as _;
            hdr.msg_iovlen = 1;

            len += 1;
        }

        // At this point, the entire freelist has been stored in headers

        let ret = unsafe {
            libc::recvmmsg(
                self.sd.as_raw_fd(),
                &mut headers as _,
                len,
                libc::MSG_WAITFORONE,
                std::ptr::null_mut(),
            )
        };

        if ret == -1 {
            return Err(io::Error::last_os_error());
        }

        // println!("successsfully received ")

        todo!()
    }

    fn freelist(&self) -> impl Iterator<Item = &mut Buffer> {
        let indexes = if self.l <= self.r {
            (self.r..self.buffers.len()).chain(0..self.l)
        } else {
            (self.r..self.l).chain(0..0)
        };
        indexes.map(|i| &mut self.buffers[i])
    }

    fn recv(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let mut io_vec = libc::iovec {
            iov_base: buffer.as_mut_ptr() as *mut c_void,
            iov_len: buffer.len(),
        };

        let mut hdr = libc::msghdr {
            msg_name: std::ptr::null_mut(),
            msg_namelen: 0,
            msg_iov: &mut io_vec as _,
            msg_iovlen: 1,
            msg_control: std::ptr::null_mut(),
            msg_controllen: 0,
            msg_flags: libc::MSG_TRUNC,
        };

        let size = unsafe { libc::recvmsg(self.sd, &mut hdr as _, 0) };
        if (1isize..=buffer.len() as isize).contains(&size) {
            Ok(size as usize)
        } else if size <= -1 {
            // Error encountered
            Err(io::Error::last_os_error())
        } else if size == 0 {
            // shutdown ordered
            panic!("shutdown")
        } else {
            // Message was too large
            panic!("truncated")
        }
    }

    fn send(&mut self, msg: &[u8]) -> io::Result<()> {
        let mut sent = 0;
        while sent < msg.len() {
            let to_send = &msg[sent..];
            let i = unsafe { libc::send(self.sd, to_send.as_ptr() as _, to_send.len(), 0) };
            if i <= 0 {
                return Err(io::Error::last_os_error());
            } else {
                sent += i as usize;
            }
        }
        Ok(())
    }

    fn links(&mut self) {}
    */
}

/// Type tag representing `NETLINK_ROUTE`
struct Route;

enum RouteMessageType {}

trait Family {
    /// An enum describing all valid types of messages
    type Typ;
}

#[derive(Debug, Default, TryFromBytes, IntoBytes, Immutable, KnownLayout)]
#[repr(C)]
struct NlMsgHdr {
    len: u32,
    typ: u16,
    flags: u16,
    seq: u32,
    pid: u32,
}

const NLM_F_REQUEST: u16 = libc::NLM_F_REQUEST as u16;

impl NlMsgHdr {
    /// Sets the request flag
    fn set_request(&mut self) {
        self.flags &= NLM_F_REQUEST
    }
    /// Tests the request flag
    fn request(&self) -> bool {
        self.flags & NLM_F_REQUEST == NLM_F_REQUEST
    }
}

#[derive(Debug, Default, TryFromBytes, IntoBytes, Immutable, KnownLayout)]
#[repr(C)]
struct IfInfoMsg {
    family: u8,
    _pad: u8,
    typ: u16,
    index: i32,
    flags: u32,
    change: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_socket() {
        let mut socket = RawSocket::open().unwrap();
        println!("opened socket");

        socket.links().unwrap();

        // println!("received {}: {:?}", len, &buffer[0..len]);
    }
}
