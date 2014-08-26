// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use raw::{Request, Allocated, Stream};
use {uvll, UvResult};

pub struct Shutdown {
    handle: *mut uvll::uv_shutdown_t,
}

impl Shutdown {
    pub fn send<T, U>(&mut self,
                      handle: &mut U,
                      cb: uvll::uv_shutdown_cb) -> UvResult<()>
                      where T: Allocated, U: Stream<T> {
        unsafe {
            try!(call!(uvll::uv_shutdown(self.handle, handle.raw() as *mut _,
                                         cb)));
            Ok(())
        }
    }
}

impl Allocated for uvll::uv_shutdown_t {
    fn size(_self: Option<uvll::uv_shutdown_t>) -> uint {
        unsafe { uvll::uv_req_size(uvll::UV_SHUTDOWN) as uint }
    }
}

impl Request<uvll::uv_shutdown_t> for Shutdown {
    fn raw(&self) -> *mut uvll::uv_shutdown_t { self.handle }
    fn from_raw(t: *mut uvll::uv_shutdown_t) -> Shutdown {
        Shutdown { handle: t }
    }
}

