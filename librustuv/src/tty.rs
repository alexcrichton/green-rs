// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::io;
use libc;

use {raw, uvll, EventLoop, UvResult, UvError};
use stream::Stream;
use raw::Handle;
use homing::{HomingIO, HomeHandle};

pub struct Tty {
    home: HomeHandle,
    stream: Stream<raw::Tty>,
}

impl Tty {
    /// Create a new TTY instance.
    pub fn new(fd: libc::c_int, readable: bool) -> UvResult<Tty> {
        Tty::new_on(&mut *try!(EventLoop::borrow()), fd, readable)
    }

    /// Same as `new`, but specifies what event loop to be created on.
    pub fn new_on(eloop: &mut EventLoop, fd: libc::c_int, readable: bool)
                  -> UvResult<Tty> {
        // libuv may succeed in giving us a handle (via uv_tty_init), but if the
        // handle isn't actually connected to a terminal there are frequently
        // many problems in using it with libuv. To get around this, always
        // return a failure if the specified file descriptor isn't actually a
        // TTY.
        //
        // Related:
        // - https://github.com/joyent/libuv/issues/982
        // - https://github.com/joyent/libuv/issues/988
        let guess = raw::Tty::guess_handle(fd);
        if guess != uvll::UV_TTY {
            return Err(UvError(uvll::EBADF));
        }

        // libuv was recently changed to not close the stdio file descriptors,
        // but it did not change the behavior for windows. Until this issue is
        // fixed, we need to dup the stdio file descriptors because otherwise
        // uv_close will close them
        let fd = if cfg!(windows) && fd <= libc::STDERR_FILENO {
            unsafe { libc::dup(fd) }
        } else { fd };

        unsafe {
            let handle = try!(raw::Tty::new(&eloop.uv_loop(), fd, readable));
            Ok(Tty {
                stream: Stream::new(handle, false),
                home: eloop.make_handle(),
            })
        }
    }

    pub fn uv_read(&mut self, buf: &mut [u8]) -> UvResult<uint> {
        let _m = self.fire_homing_missile();
        self.stream.read(buf)
    }

    pub fn uv_write(&mut self, buf: &[u8]) -> UvResult<()> {
        let _m = self.fire_homing_missile();
        self.stream.write(buf)
    }

    pub fn set_raw(&mut self, raw: bool) -> UvResult<()> {
        let _m = self.fire_homing_missile();
        self.stream.handle.set_mode(raw)
    }

    pub fn winsize(&mut self) -> UvResult<(int, int)> {
        let _m = self.fire_homing_missile();
        self.stream.handle.winsize()
    }

    // One day we may support creating instances of a tty which don't
    // correspond to an actual underlying TTY, so this is a method.
    pub fn isatty(&self) -> bool { true }

    /// Gain access to the underlying raw tty object.
    ///
    /// This function is unsafe as there is no guarantee that any safe
    /// modifications to the tty handle are actually safe to perform given the
    /// assumptions of this object.
    pub unsafe fn raw(&self) -> raw::Tty { self.stream.handle }
}

impl HomingIO for Tty {
    fn home(&self) -> &HomeHandle { &self.home }
}

impl Reader for Tty {
    fn read(&mut self, buf: &mut [u8]) -> io::IoResult<uint> {
        self.uv_read(buf).map_err(|e| e.to_io_error())
    }
}

impl Writer for Tty {
    fn write(&mut self, buf: &[u8]) -> io::IoResult<()> {
        self.uv_write(buf).map_err(|e| e.to_io_error())
    }
}

impl Drop for Tty {
    fn drop(&mut self) {
        unsafe {
            let _m = self.fire_homing_missile();
            self.stream.handle.close_and_free();
        }
    }
}
