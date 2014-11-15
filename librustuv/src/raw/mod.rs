// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::io::net::ip;
use std::mem;
use std::num::Int;
use std::rt::heap;
use libc;

use {uvll, UvResult};

pub use self::async::Async;
pub use self::connect::Connect;
pub use self::fs::Fs;
pub use self::getaddrinfo::GetAddrInfo;
pub use self::idle::Idle;
pub use self::loop_::Loop;
pub use self::pipe::Pipe;
pub use self::process::Process;
pub use self::shutdown::Shutdown;
pub use self::signal::Signal;
pub use self::tcp::Tcp;
pub use self::timer::Timer;
pub use self::tty::Tty;
pub use self::udp::Udp;
pub use self::udp_send::UdpSend;
pub use self::write::Write;

macro_rules! call( ($e:expr) => (
    match $e {
        n if n < 0 => Err(::UvError(n)),
        n => Ok(n),
    }
) )

mod async;
mod connect;
mod fs;
mod getaddrinfo;
mod idle;
mod loop_;
mod pipe;
mod process;
mod shutdown;
mod signal;
mod tcp;
mod timer;
mod tty;
mod udp;
mod udp_send;
mod write;

pub trait Allocated {
    fn size(_self: Option<Self>) -> uint;
}

pub struct Raw<T> {
    ptr: *mut T,
}

// FIXME: this T should be an associated type
pub trait Handle<T: Allocated> {
    fn raw(&self) -> *mut T;
    unsafe fn from_raw(t: *mut T) -> Self;

    fn uv_loop(&self) -> Loop {
        unsafe {
            let loop_ = uvll::rust_uv_get_loop_for_uv_handle(self.raw() as *mut _);
            Loop::from_raw(loop_)
        }
    }

    fn get_data(&self) -> *mut libc::c_void {
        unsafe { uvll::rust_uv_get_data_for_uv_handle(self.raw() as *mut _) }
    }

    fn set_data(&mut self, data: *mut libc::c_void) {
        unsafe {
            uvll::rust_uv_set_data_for_uv_handle(self.raw() as *mut _, data)
        }
    }

    /// Invokes uv_close
    ///
    /// This is unsafe as there is no guarantee that this handle is not actively
    /// being used by other objects.
    unsafe fn close(&mut self, thunk: Option<uvll::uv_close_cb>) {
        uvll::uv_close(self.raw() as *mut _, thunk)
    }

    /// Deallocate this handle.
    ///
    /// This is unsafe as there is no guarantee that no one else is using this
    /// handle currently.
    unsafe fn free(&mut self) { drop(Raw::wrap(self.raw())) }

    /// Invoke uv_close, and then free the handle when the close operation is
    /// done.
    ///
    /// This is unsafe for the same reasons as `close` and `free`.
    unsafe fn close_and_free(&mut self) {
        extern fn done<T: Allocated>(t: *mut uvll::uv_handle_t) {
            unsafe { drop(Raw::wrap(t as *mut T)) }
        }
        self.close(Some(done::<T>))
    }

    fn uv_ref(&self) { unsafe { uvll::uv_ref(self.raw() as *mut _) } }
    fn uv_unref(&self) { unsafe { uvll::uv_unref(self.raw() as *mut _) } }
}

// FIXME: this T should be an associated type
pub trait Request<T: Allocated> {
    fn raw(&self) -> *mut T;
    unsafe fn from_raw(t: *mut T) -> Self;

    fn get_data(&self) -> *mut libc::c_void {
        unsafe { uvll::rust_uv_get_data_for_req(self.raw() as *mut _) }
    }

    fn set_data(&mut self, data: *mut libc::c_void) {
        unsafe {
            uvll::rust_uv_set_data_for_req(self.raw() as *mut _, data)
        }
    }

    /// Allocate a new uninitialized request.
    ///
    /// This function is unsafe as there is no scheduled destructor for the
    /// returned value.
    unsafe fn alloc() -> Self {
        Request::from_raw(Raw::<T>::new().unwrap())
    }

    /// Invokes uv_close
    ///
    /// This is unsafe as there is no guarantee that this handle is not actively
    /// being used by other objects.
    fn cancel(&mut self) -> UvResult<()> {
        unsafe { try!(call!(uvll::uv_cancel(self.raw() as *mut _))); }
        Ok(())
    }

    /// Deallocate this handle.
    ///
    /// This is unsafe as there is no guarantee that no one else is using this
    /// handle currently.
    unsafe fn free(&mut self) { drop(Raw::wrap(self.raw())) }
}

pub trait Stream<T: Allocated>: Handle<T> {
    fn listen(&mut self, backlog: libc::c_int,
              cb: uvll::uv_connection_cb) -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_listen(self.raw() as *mut _, backlog, cb)));
            Ok(())
        }
    }

    fn accept(&mut self, other: Self) -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_accept(self.raw() as *mut _,
                                       other.raw() as *mut _)));
            Ok(())
        }
    }

    fn read_start(&mut self, alloc_cb: uvll::uv_alloc_cb,
                  read_cb: uvll::uv_read_cb) -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_read_start(self.raw() as *mut _, alloc_cb,
                                           read_cb)));
            Ok(())
        }
    }

    fn read_stop(&mut self) -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_read_stop(self.raw() as *mut _)));
            Ok(())
        }
    }
}

impl<T: Allocated> Raw<T> {
    /// Allocates a new instance of the underlying pointer.
    fn new() -> Raw<T> {
        let size = Allocated::size(None::<T>);
        unsafe {
            Raw { ptr: heap::allocate(size as uint, 8) as *mut T }
        }
    }

    /// Wrap a pointer, scheduling it for deallocation when the returned value
    /// goes out of scope.
    unsafe fn wrap(ptr: *mut T) -> Raw<T> { Raw { ptr: ptr } }

    fn get(&self) -> *mut T { self.ptr }

    /// Unwrap this raw pointer, cancelling its deallocation.
    ///
    /// This method is unsafe because it will leak the returned pointer.
    unsafe fn unwrap(mut self) -> *mut T {
        let ret = self.ptr;
        self.ptr = 0 as *mut T;
        return ret;
    }
}

#[unsafe_destructor]
impl<T: Allocated> Drop for Raw<T> {
    fn drop(&mut self) {
        if self.ptr.is_null() { return }

        let size = Allocated::size(None::<T>);
        unsafe {
            heap::deallocate(self.ptr as *mut u8, size as uint, 8)
        }
    }
}

pub fn slice_to_uv_buf(v: &[u8]) -> uvll::uv_buf_t {
    let data = v.as_ptr();
    uvll::uv_buf_t { base: data as *mut u8, len: v.len() as uvll::uv_buf_len_t }
}

fn socket_name<T>(handle: *const T,
                  f: unsafe extern fn(*const T, *mut libc::sockaddr,
                                      *mut libc::c_int) -> libc::c_int)
                  -> UvResult<ip::SocketAddr> {
    // Allocate a sockaddr_storage since we don't know if it's ipv4 or ipv6
    let mut sockaddr: libc::sockaddr_storage = unsafe { mem::zeroed() };
    let mut namelen = mem::size_of::<libc::sockaddr_storage>() as libc::c_int;

    let sockaddr_p = &mut sockaddr as *mut libc::sockaddr_storage;
    unsafe {
        try!(call!(f(&*handle, sockaddr_p as *mut _, &mut namelen)));
    }
    Ok(sockaddr_to_addr(&sockaddr, namelen as uint))
}


pub fn sockaddr_to_addr(storage: &libc::sockaddr_storage,
                        len: uint) -> ip::SocketAddr {
    fn ntohs(u: u16) -> u16 { Int::from_be(u) }

    match storage.ss_family as libc::c_int {
        libc::AF_INET => {
            assert!(len as uint >= mem::size_of::<libc::sockaddr_in>());
            let storage: &libc::sockaddr_in = unsafe {
                mem::transmute(storage)
            };
            let ip = (storage.sin_addr.s_addr as u32).to_be();
            let a = (ip >> 24) as u8;
            let b = (ip >> 16) as u8;
            let c = (ip >>  8) as u8;
            let d = (ip >>  0) as u8;
            ip::SocketAddr {
                ip: ip::Ipv4Addr(a, b, c, d),
                port: ntohs(storage.sin_port),
            }
        }
        libc::AF_INET6 => {
            assert!(len as uint >= mem::size_of::<libc::sockaddr_in6>());
            let storage: &libc::sockaddr_in6 = unsafe {
                mem::transmute(storage)
            };
            let a = ntohs(storage.sin6_addr.s6_addr[0]);
            let b = ntohs(storage.sin6_addr.s6_addr[1]);
            let c = ntohs(storage.sin6_addr.s6_addr[2]);
            let d = ntohs(storage.sin6_addr.s6_addr[3]);
            let e = ntohs(storage.sin6_addr.s6_addr[4]);
            let f = ntohs(storage.sin6_addr.s6_addr[5]);
            let g = ntohs(storage.sin6_addr.s6_addr[6]);
            let h = ntohs(storage.sin6_addr.s6_addr[7]);
            ip::SocketAddr {
                ip: ip::Ipv6Addr(a, b, c, d, e, f, g, h),
                port: ntohs(storage.sin6_port),
            }
        }
        n => {
            panic!("unknown family {}", n);
        }
    }
}

pub fn addr_to_sockaddr(addr: ip::SocketAddr,
                        storage: &mut libc::sockaddr_storage)
                        -> libc::socklen_t {
    fn htons(u: u16) -> u16 { u.to_be() }

    unsafe {
        let len = match addr.ip {
            ip::Ipv4Addr(a, b, c, d) => {
                let ip = (a as u32 << 24) |
                         (b as u32 << 16) |
                         (c as u32 <<  8) |
                         (d as u32 <<  0);
                let storage = storage as *mut _ as *mut libc::sockaddr_in;
                (*storage).sin_family = libc::AF_INET as libc::sa_family_t;
                (*storage).sin_port = htons(addr.port);
                (*storage).sin_addr = libc::in_addr {
                    s_addr: Int::from_be(ip),

                };
                mem::size_of::<libc::sockaddr_in>()
            }
            ip::Ipv6Addr(a, b, c, d, e, f, g, h) => {
                let storage = storage as *mut _ as *mut libc::sockaddr_in6;
                (*storage).sin6_family = libc::AF_INET6 as libc::sa_family_t;
                (*storage).sin6_port = htons(addr.port);
                (*storage).sin6_port = htons(addr.port);
                (*storage).sin6_addr = libc::in6_addr {
                    s6_addr: [
                        htons(a),
                        htons(b),
                        htons(c),
                        htons(d),
                        htons(e),
                        htons(f),
                        htons(g),
                        htons(h),
                    ]
                };
                mem::size_of::<libc::sockaddr_in6>()
            }
        };
        return len as libc::socklen_t
    }
}
