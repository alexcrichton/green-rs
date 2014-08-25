// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::c_str::CString;
use libc;

use raw::{Loop, Handle, Allocated, Raw, Stream};
use {uvll, UvResult};

pub struct Pipe {
    handle: *mut uvll::uv_pipe_t,
}

impl Pipe {
    /// Create a new uv_pipe_t handle.
    ///
    /// This function is unsafe as a successful return value is not
    /// automatically deallocated.
    pub unsafe fn new(uv_loop: &Loop, ipc: bool) -> UvResult<Pipe> {
        let raw = Raw::new();
        try!(call!(uvll::uv_pipe_init(uv_loop.raw(), raw.get(),
                                      ipc as libc::c_int)));
        Ok(Pipe { handle: raw.unwrap() })
    }

    pub fn open(&mut self, file: libc::c_int) -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_pipe_open(self.handle, file)));
            Ok(())
        }
    }

    pub fn bind(&mut self, name: CString) -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_pipe_bind(self.handle, name.as_ptr())));
            Ok(())
        }
    }
}

impl Allocated for uvll::uv_pipe_t {
    fn size(_self: Option<uvll::uv_pipe_t>) -> uint {
        unsafe { uvll::uv_handle_size(uvll::UV_NAMED_PIPE) as uint }
    }
}

impl Handle<uvll::uv_pipe_t> for Pipe {
    fn raw(&self) -> *mut uvll::uv_pipe_t { self.handle }
    fn from_raw(t: *mut uvll::uv_pipe_t) -> Pipe { Pipe { handle: t } }
}

impl Stream<uvll::uv_pipe_t> for Pipe {}
