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

pub struct Udp {
    handle: *mut uvll::uv_udp_t,
}

impl Udp {
    /// Create a new uv_udp_t handle.
    ///
    /// This function is unsafe as a successful return value is not
    /// automatically deallocated.
    pub unsafe fn new(uv_loop: &Loop) -> UvResult<Udp> {
        let raw = Raw::new();
        try!(call!(uvll::uv_udp_init(uv_loop.raw(), raw.get())));
        Ok(Udp { handle: raw.unwrap() })
    }

    pub fn open(&mut self, sock: uvll::uv_os_socket_t) -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_udp_open(self.handle, sock)));
            Ok(())
        }
    }

    pub fn bind(&mut self, addr: ip::SocketAddr) -> UvResult<()> {
        unsafe {
            let mut raw_addr: libc::sockaddr_storage = mem::zeroed();
            raw::addr_to_sockaddr(addr, &mut raw_addr);
            try!(call!(uvll::uv_udp_bind(self.handle,
                                         &raw_addr as *const _ as *const _,
                                         0)));
            Ok(())
        }
    }

    pub fn getsockname(&mut self) -> UvResult<ip::SocketAddr> {
        unsafe { raw::socket_name(&*self.handle, uvll::uv_udp_getsockname) }
    }

    pub fn set_membership(&mut self, addr: ip::IpAddr,
                          membership: uvll::uv_membership) -> UvResult<()> {
        let addr = addr.to_string().to_c_str();
        unsafe {
            try!(call!(uvll::uv_udp_set_membership(self.handle, addr.as_ptr(),
                                                   0 as *const _,
                                                   membership)));
            Ok(())
        }
    }

    pub fn set_multicast_loop(&mut self, on: bool) -> UvResult<()> {
        unsafe {
            let on = on as libc::c_int;
            try!(call!(uvll::uv_udp_set_multicast_loop(self.handle, on)));
            Ok(())
        }
    }

    pub fn set_multicast_ttl(&mut self, ttl: int) -> UvResult<()> {
        unsafe {
            let ttl = ttl as libc::c_int;
            try!(call!(uvll::uv_udp_set_multicast_ttl(self.handle, ttl)));
            Ok(())
        }
    }

    pub fn set_broadcast(&mut self, on: bool) -> UvResult<()> {
        unsafe {
            let on = on as libc::c_int;
            try!(call!(uvll::uv_udp_set_broadcast(self.handle, on)));
            Ok(())
        }
    }

    pub fn set_ttl(&mut self, ttl: int) -> UvResult<()> {
        unsafe {
            let ttl = ttl as libc::c_int;
            try!(call!(uvll::uv_udp_set_ttl(self.handle, ttl)));
            Ok(())
        }
    }

    pub fn try_send(&mut self, buf: &[u8],
                    addr: ip::SocketAddr) -> UvResult<()> {
        unsafe {
            let mut raw_addr: libc::sockaddr_storage = mem::zeroed();
            raw::addr_to_sockaddr(addr, &mut raw_addr);
            try!(call!(uvll::uv_udp_try_send(self.handle,
                                             &raw::slice_to_uv_buf(buf),
                                             1,
                                             &raw_addr as *const _ as *const _)));
            Ok(())
        }
    }

    pub fn recv_start(&mut self, alloc: uvll::uv_alloc_cb,
                      recv: uvll::uv_udp_recv_cb) -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_udp_recv_start(self.handle, alloc, recv)));
            Ok(())
        }
    }

    pub fn recv_stop(&mut self) -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_udp_recv_stop(self.handle)));
            Ok(())
        }
    }
}

impl Allocated for uvll::uv_udp_t {
    fn size(_self: Option<uvll::uv_udp_t>) -> uint {
        unsafe { uvll::uv_handle_size(uvll::UV_UDP) as uint }
    }
}

impl Handle<uvll::uv_udp_t> for Udp {
    fn raw(&self) -> *mut uvll::uv_udp_t { self.handle }
    fn from_raw(t: *mut uvll::uv_udp_t) -> Udp { Udp { handle: t } }
}

impl Stream<uvll::uv_udp_t> for Udp {}


