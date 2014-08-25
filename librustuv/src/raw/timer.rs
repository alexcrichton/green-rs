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

pub struct Timer {
    handle: *mut uvll::uv_timer_t,
}

impl Timer {
    /// Create a new uv_timer_t handle.
    ///
    /// This function is unsafe as a successful return value is not
    /// automatically deallocated.
    pub unsafe fn new(uv_loop: &Loop) -> UvResult<Timer> {
        let raw = Raw::new();
        try!(call!(uvll::uv_timer_init(uv_loop.raw(), raw.get())));
        Ok(Timer { handle: raw.unwrap() })
    }

    pub fn start(&mut self, timeout: u64, repeat: u64,
                 cb: uvll::uv_timer_cb) -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_timer_start(self.handle, cb, timeout, repeat)));
        }
        Ok(())
    }

    pub fn stop(&mut self) -> UvResult<()> {
        unsafe { try!(call!(uvll::uv_timer_stop(self.handle))); }
        Ok(())
    }

    pub fn set_repeat(&mut self, repeat: u64) {
        unsafe { uvll::uv_timer_set_repeat(self.handle, repeat) }
    }

    pub fn get_repeat(&mut self) -> u64 {
        unsafe { uvll::uv_timer_get_repeat(&*self.handle) }
    }

    pub fn again(&mut self) -> UvResult<()> {
        unsafe { try!(call!(uvll::uv_timer_again(self.handle))); }
        Ok(())
    }
}

impl Allocated for uvll::uv_timer_t {
    fn size(_self: Option<uvll::uv_timer_t>) -> uint {
        unsafe { uvll::uv_handle_size(uvll::UV_ASYNC) as uint }
    }
}

impl Handle<uvll::uv_timer_t> for Timer {
    fn raw(&self) -> *mut uvll::uv_timer_t { self.handle }
    fn from_raw(t: *mut uvll::uv_timer_t) -> Timer { Timer { handle: t } }
}

