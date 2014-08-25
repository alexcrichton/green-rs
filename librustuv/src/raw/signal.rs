// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use libc;

use raw::{Loop, Handle, Allocated, Raw};
use {uvll, UvResult};

pub struct Signal {
    handle: *mut uvll::uv_signal_t,
}

impl Signal {
    /// Create a new uv_signal_t handle.
    ///
    /// This function is unsafe as a successful return value is not
    /// automatically deallocated.
    pub unsafe fn new(uv_loop: &Loop) -> UvResult<Signal> {
        let raw = Raw::new();
        try!(call!(uvll::uv_signal_init(uv_loop.raw(), raw.get())));
        Ok(Signal { handle: raw.unwrap() })
    }

    pub fn start(&mut self, signum: libc::c_int,
                 cb: uvll::uv_signal_cb) -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_signal_start(self.handle, cb, signum)));
            Ok(())
        }
    }

    pub fn stop(&mut self) -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_signal_stop(self.handle)));
            Ok(())
        }
    }
}

impl Allocated for uvll::uv_signal_t {
    fn size(_self: Option<uvll::uv_signal_t>) -> uint {
        unsafe { uvll::uv_handle_size(uvll::UV_SIGNAL) as uint }
    }
}

impl Handle<uvll::uv_signal_t> for Signal {
    fn raw(&self) -> *mut uvll::uv_signal_t { self.handle }
    fn from_raw(t: *mut uvll::uv_signal_t) -> Signal { Signal { handle: t } }
}

