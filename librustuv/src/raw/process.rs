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

pub struct Process {
    handle: *mut uvll::uv_process_t,
}

impl Process {
    pub unsafe fn spawn(uv_loop: &Loop, opts: *mut uvll::uv_process_options_t)
                        -> UvResult<Process> {
        let raw = Raw::new();
        try!(call!(uvll::uv_spawn(uv_loop.raw(), raw.get(), opts)));
        Ok(Process { handle: raw.unwrap() })
    }

    pub fn pid(&self) -> libc::c_int {
        unsafe { uvll::rust_uv_process_pid(self.handle) }
    }

    pub fn kill(pid: libc::c_int, signum: libc::c_int) -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_kill(pid, signum)));
            Ok(())
        }
    }

    pub fn kill_me(&mut self, signum: libc::c_int) -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_process_kill(self.handle, signum)));
            Ok(())
        }
    }
}

impl Allocated for uvll::uv_process_t {
    fn size(_self: Option<uvll::uv_process_t>) -> uint {
        unsafe { uvll::uv_handle_size(uvll::UV_PROCESS) as uint }
    }
}

impl Handle<uvll::uv_process_t> for Process {
    fn raw(&self) -> *mut uvll::uv_process_t { self.handle }
    fn from_raw(t: *mut uvll::uv_process_t) -> Process { Process { handle: t } }
}
