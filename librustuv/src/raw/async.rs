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

use raw::{Loop, Handle, Allocated, Raw};
use UvResult;

pub struct Async {
    handle: *mut uvll::uv_async_t,
}

impl Async {
    /// Create a new uv_async_t handle.
    ///
    /// This function is unsafe as a successful return value is not
    /// automatically deallocated.
    pub unsafe fn new(uv_loop: &Loop, cb: uvll::uv_async_cb) -> UvResult<Async> {
        let raw = Raw::new();
        try!(call!(uvll::uv_async_init(uv_loop.raw(), raw.get(), cb)));
        Ok(Async { handle: raw.unwrap() })
    }

    pub fn send(&self) {
        unsafe { uvll::uv_async_send(self.handle) }
    }
}

impl Allocated for uvll::uv_async_t {
    fn size(_self: Option<uvll::uv_async_t>) -> uint {
        unsafe { uvll::uv_handle_size(uvll::UV_ASYNC) as uint }
    }
}

impl Handle<uvll::uv_async_t> for Async {
    fn raw(&self) -> *mut uvll::uv_async_t { self.handle }
    fn from_raw(t: *mut uvll::uv_async_t) -> Async { Async { handle: t } }
}
