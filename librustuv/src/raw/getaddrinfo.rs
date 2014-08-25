// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use uvll;

use raw::{Request, Allocated, Loop};
use UvResult;

pub struct GetAddrInfo {
    handle: *mut uvll::uv_getaddrinfo_t,
}

impl GetAddrInfo {
    pub fn send(&mut self,
                uv_loop: &Loop,
                node: Option<&str>,
                service: Option<&str>,
                cb: uvll::uv_getaddrinfo_cb) -> UvResult<()> {
        let node = node.map(|s| s.to_c_str());
        let service = service.map(|s| s.to_c_str());
        let node = node.as_ref().map(|c| c.as_ptr()).unwrap_or(0 as *const _);
        let service = service.as_ref().map(|c| c.as_ptr()).unwrap_or(0 as *const _);
        unsafe {
            try!(call!(uvll::uv_getaddrinfo(uv_loop.raw(),
                                            self.handle,
                                            cb,
                                            node,
                                            service,
                                            0 as *const _)));
        }
        Ok(())
    }
}

impl Allocated for uvll::uv_getaddrinfo_t {
    fn size(_self: Option<uvll::uv_getaddrinfo_t>) -> uint {
        unsafe { uvll::uv_req_size(uvll::UV_GETADDRINFO) as uint }
    }
}

impl Request<uvll::uv_getaddrinfo_t> for GetAddrInfo {
    fn raw(&self) -> *mut uvll::uv_getaddrinfo_t { self.handle }
    fn from_raw(t: *mut uvll::uv_getaddrinfo_t) -> GetAddrInfo {
        GetAddrInfo { handle: t }
    }
}
