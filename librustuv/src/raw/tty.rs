// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::mem;
use libc;

use raw::{Loop, Handle, Allocated, Raw, Stream};
use {uvll, UvResult};

pub struct Tty {
    handle: *mut uvll::uv_tty_t,
}

impl Tty {
    /// Create a new uv_tty_t handle.
    ///
    /// This function is unsafe as a successful return value is not
    /// automatically deallocated.
    pub unsafe fn new(uv_loop: &Loop, fd: libc::c_int,
                      readable: bool) -> UvResult<Tty> {
        let raw = Raw::new();
        try!(call!(uvll::uv_tty_init(uv_loop.raw(), raw.get(), fd,
                                     readable as libc::c_int)));
        Ok(Tty { handle: raw.unwrap() })
    }

    pub fn reset_mode() -> UvResult<()> {
        unsafe { try!(call!(uvll::uv_tty_reset_mode())); }
        Ok(())
    }

    pub fn guess_handle(fd: libc::c_int) -> uvll::uv_handle_type {
        unsafe { mem::transmute(uvll::rust_uv_guess_handle(fd)) }
    }

    pub fn set_mode(&mut self, raw: bool) -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_tty_set_mode(self.handle, raw as libc::c_int)));
            Ok(())
        }
    }

    pub fn winsize(&mut self) -> UvResult<(int, int)> {
        unsafe {
            let (mut width, mut height) = (0, 0);
            try!(call!(uvll::uv_tty_get_winsize(self.handle, &mut width,
                                                &mut height)));
            Ok((width as int, height as int))
        }
    }
}

impl Allocated for uvll::uv_tty_t {
    fn size(_self: Option<uvll::uv_tty_t>) -> uint {
        unsafe { uvll::uv_handle_size(uvll::UV_TTY) as uint }
    }
}

impl Handle<uvll::uv_tty_t> for Tty {
    fn raw(&self) -> *mut uvll::uv_tty_t { self.handle }
    fn from_raw(t: *mut uvll::uv_tty_t) -> Tty { Tty { handle: t } }
}

impl Stream<uvll::uv_tty_t> for Tty {}
