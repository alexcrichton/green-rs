// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::rt::heap;
use libc;

use {UvResult, UvError, uvll};

pub struct Loop {
    handle: *mut uvll::uv_loop_t,
}

impl Loop {
    /// Create a new uv event loop.
    ///
    /// This function is unsafe becuase it will leak the event loop as there is
    /// no destructor on the returned value.
    pub unsafe fn new() -> UvResult<Loop> {
        unsafe {
            let size = uvll::uv_loop_size() as uint;
            let handle: *mut uvll::uv_loop_t = heap::allocate(size, 8) as *mut _;
            match call!(uvll::uv_loop_init(handle)) {
                Ok(_) => Ok(Loop { handle: handle }),
                Err(e) => {
                    heap::deallocate(handle as *mut u8, size, 8);
                    Err(e)
                }
            }
        }
    }

    /// Wrap an existing event loop.
    ///
    /// This function is unsafe because there is no guarantee that the
    /// underlying pointer is valid.
    pub unsafe fn wrap(raw: *mut uvll::uv_loop_t) -> Loop {
        Loop { handle: raw }
    }

    pub fn raw(&self) -> *mut uvll::uv_loop_t { self.handle }

    pub fn run(&mut self, mode: uvll::uv_run_mode) -> UvResult<()> {
        try!(call!(unsafe { uvll::uv_run(self.handle, mode) }));
        Ok(())
    }

    pub fn get_data(&mut self) -> *mut libc::c_void {
        unsafe { uvll::rust_uv_get_data_for_uv_loop(self.handle) }
    }

    pub fn set_data(&mut self, data: *mut libc::c_void) {
        unsafe { uvll::rust_uv_set_data_for_uv_loop(self.handle, data) }
    }

    /// Close an event loop.
    ///
    /// This function is unsafe because there is no guarantee that the event
    /// loop is not currently active elsewhere.
    ///
    /// If the event loops fails to close, it will not be deallocated and this
    /// function should be called in the future to deallocate it.
    pub unsafe fn close(&mut self) -> UvResult<()> {
        try!(call!(uvll::uv_loop_close(self.handle)));
        let size = uvll::uv_loop_size() as uint;
        heap::deallocate(self.handle as *mut u8, size, 8);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Loop;
    use uvll;

    #[test]
    fn smoke() {
        unsafe {
            let mut l = Loop::new().unwrap();
            l.run(uvll::RUN_DEFAULT).unwrap();
            l.close().unwrap();
        }
    }
}
