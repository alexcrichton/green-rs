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

pub struct Idle {
    handle: *mut uvll::uv_idle_t,
}

impl Idle {
    /// Create a new uv_idle_t handle.
    ///
    /// This function is unsafe as a successful return value is not
    /// automatically deallocated.
    pub unsafe fn new(uv_loop: &Loop) -> UvResult<Idle> {
        let raw = Raw::new();
        try!(call!(uvll::uv_idle_init(uv_loop.raw(), raw.get())));
        Ok(Idle { handle: raw.unwrap() })
    }

    pub fn start(&mut self, f: uvll::uv_idle_cb) -> UvResult<()> {
        unsafe { try!(call!(uvll::uv_idle_start(self.handle, f))); }
        Ok(())
    }

    pub fn stop(&mut self) -> UvResult<()> {
        unsafe { try!(call!(uvll::uv_idle_stop(self.handle))); }
        Ok(())
    }
}

impl Allocated for uvll::uv_idle_t {
    fn size(_self: Option<uvll::uv_idle_t>) -> uint {
        unsafe { uvll::uv_handle_size(uvll::UV_IDLE) as uint }
    }
}

impl Handle<uvll::uv_idle_t> for Idle {
    fn raw(&self) -> *mut uvll::uv_idle_t { self.handle }
    fn from_raw(t: *mut uvll::uv_idle_t) -> Idle { Idle { handle: t } }
}
