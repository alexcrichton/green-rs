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

use raw::{mod, Request, Allocated, Stream};
use UvResult;

pub struct Write {
    handle: *mut uvll::uv_write_t,
}

impl Write {
    pub fn send<T, U>(&mut self,
                      handle: &mut U,
                      buf: &[u8],
                      cb: uvll::uv_write_cb) -> UvResult<()>
                      where T: Allocated, U: Stream<T> {
        unsafe {
            let buf = raw::slice_to_uv_buf(buf);
            try!(call!(uvll::uv_write(self.handle, handle.raw() as *mut _,
                                      &buf, 1, cb)));
            Ok(())
        }
    }
}

impl Allocated for uvll::uv_write_t {
    fn size(_self: Option<uvll::uv_write_t>) -> uint {
        unsafe { uvll::uv_req_size(uvll::UV_WRITE) as uint }
    }
}

impl Request<uvll::uv_write_t> for Write {
    fn raw(&self) -> *mut uvll::uv_write_t { self.handle }
    fn from_raw(t: *mut uvll::uv_write_t) -> Write {
        Write { handle: t }
    }
}

