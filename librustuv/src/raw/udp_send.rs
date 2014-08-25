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

use raw::{mod, Request, Allocated, Udp, Handle};
use {uvll, UvResult};

pub struct UdpSend {
    handle: *mut uvll::uv_udp_send_t,
}

impl UdpSend {
    pub fn send(&mut self,
                handle: Udp,
                buf: &[u8],
                addr: ip::SocketAddr,
                cb: uvll::uv_udp_send_cb) -> UvResult<()> {
        unsafe {
            let mut raw_addr: libc::sockaddr_storage = mem::zeroed();
            raw::addr_to_sockaddr(addr, &mut raw_addr);
            let buf = raw::slice_to_uv_buf(buf);
            try!(call!(uvll::uv_udp_send(self.handle, handle.raw() as *mut _,
                                         &buf, 1,
                                         &raw_addr as *const _ as *const _,
                                         cb)));
            Ok(())
        }
    }
}

impl Allocated for uvll::uv_udp_send_t {
    fn size(_self: Option<uvll::uv_udp_send_t>) -> uint {
        unsafe { uvll::uv_req_size(uvll::UV_UDP_SEND) as uint }
    }
}

impl Request<uvll::uv_udp_send_t> for UdpSend {
    fn raw(&self) -> *mut uvll::uv_udp_send_t { self.handle }
    fn from_raw(t: *mut uvll::uv_udp_send_t) -> UdpSend {
        UdpSend { handle: t }
    }
}


