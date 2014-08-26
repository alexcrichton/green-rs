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
use std::c_str::CString;
use libc;

use raw::{mod, Request, Allocated, Pipe, Handle, Tcp};
use {uvll, UvResult};

pub struct Connect {
    handle: *mut uvll::uv_connect_t,
}

impl Connect {
    pub fn pipe_connect(&mut self,
                        handle: Pipe,
                        name: &CString,
                        cb: uvll::uv_connect_cb) {
        unsafe {
            uvll::uv_pipe_connect(self.handle,
                                  handle.raw() as *mut _,
                                  name.as_ptr(),
                                  cb)
        }
    }

    pub fn tcp_connect(&mut self,
                       handle: Tcp,
                       addr: ip::SocketAddr,
                       cb: uvll::uv_connect_cb) -> UvResult<()> {
        unsafe {
            let mut raw_addr: libc::sockaddr_storage = mem::zeroed();
            raw::addr_to_sockaddr(addr, &mut raw_addr);
            try!(call!(uvll::uv_tcp_connect(self.handle,
                                            handle.raw() as *mut _,
                                            &raw_addr as *const _ as *const _,
                                            cb)));
            Ok(())
        }
    }
}

impl Allocated for uvll::uv_connect_t {
    fn size(_self: Option<uvll::uv_connect_t>) -> uint {
        unsafe { uvll::uv_req_size(uvll::UV_CONNECT) as uint }
    }
}

impl Request<uvll::uv_connect_t> for Connect {
    fn raw(&self) -> *mut uvll::uv_connect_t { self.handle }
    fn from_raw(t: *mut uvll::uv_connect_t) -> Connect {
        Connect { handle: t }
    }
}


