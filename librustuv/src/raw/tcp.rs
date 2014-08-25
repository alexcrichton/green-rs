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
use libc;

use raw::{mod, Loop, Handle, Allocated, Raw, Stream};
use {uvll, UvResult};

pub struct Tcp {
    handle: *mut uvll::uv_tcp_t,
}

impl Tcp {
    /// Create a new uv_tcp_t handle.
    ///
    /// This function is unsafe as a successful return value is not
    /// automatically deallocated.
    pub unsafe fn new(uv_loop: &Loop) -> UvResult<Tcp> {
        let raw = Raw::new();
        try!(call!(uvll::uv_tcp_init(uv_loop.raw(), raw.get())));
        Ok(Tcp { handle: raw.unwrap() })
    }

    pub fn open(&mut self, sock: uvll::uv_os_socket_t) -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_tcp_open(self.handle, sock)));
            Ok(())
        }
    }

    pub fn nodelay(&mut self, enable: bool) -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_tcp_nodelay(self.handle,
                                            enable as libc::c_int)));
            Ok(())
        }
    }

    pub fn keepalive(&mut self, delay: Option<uint>) -> UvResult<()> {
        let (enable, delay) = match delay {
            Some(n) => (1, n),
            None => (0, 0),
        };
        unsafe {
            try!(call!(uvll::uv_tcp_keepalive(self.handle, enable,
                                              delay as libc::c_uint)));
            Ok(())
        }
    }

    pub fn simultaneous_accepts(&mut self, enable: bool) -> UvResult<()> {
        unsafe {
            let enable = enable as libc::c_int;
            try!(call!(uvll::uv_tcp_simultaneous_accepts(self.handle, enable)));
            Ok(())
        }
    }

    pub fn bind(&mut self, addr: ip::SocketAddr) -> UvResult<()> {
        unsafe {
            let mut raw_addr: libc::sockaddr_storage = mem::zeroed();
            raw::addr_to_sockaddr(addr, &mut raw_addr);
            try!(call!(uvll::uv_tcp_bind(self.handle,
                                         &raw_addr as *const _ as *const _,
                                         0)));
            Ok(())
        }
    }

    pub fn getsockname(&mut self) -> UvResult<ip::SocketAddr> {
        unsafe { raw::socket_name(&*self.handle, uvll::uv_tcp_getsockname) }
    }

    pub fn getpeername(&mut self) -> UvResult<ip::SocketAddr> {
        unsafe { raw::socket_name(&*self.handle, uvll::uv_tcp_getpeername) }
    }
}

impl Allocated for uvll::uv_tcp_t {
    fn size(_self: Option<uvll::uv_tcp_t>) -> uint {
        unsafe { uvll::uv_handle_size(uvll::UV_TCP) as uint }
    }
}

impl Handle<uvll::uv_tcp_t> for Tcp {
    fn raw(&self) -> *mut uvll::uv_tcp_t { self.handle }
    fn from_raw(t: *mut uvll::uv_tcp_t) -> Tcp { Tcp { handle: t } }
}

impl Stream<uvll::uv_tcp_t> for Tcp {}

