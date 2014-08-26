// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/*!

Bindings to libuv, along with the default implementation of `std::rt::rtio`.

UV types consist of the event loop (Loop), Watchers, Requests and
Callbacks.

Watchers and Requests encapsulate pointers to uv *handles*, which have
subtyping relationships with each other.  This subtyping is reflected
in the bindings with explicit or implicit coercions. For example, an
upcast from TcpWatcher to StreamWatcher is done with
`tcp_watcher.as_stream()`. In other cases a callback on a specific
type of watcher will be passed a watcher of a supertype.

Currently all use of Request types (connect/write requests) are
encapsulated in the bindings and don't need to be dealt with by the
caller.

# Safety note

Due to the complex lifecycle of uv handles, as well as compiler bugs,
this module is not memory safe and requires explicit memory management,
via `close` and `delete` methods.

*/

#![license = "MIT/ASL2"]
#![doc(html_logo_url = "http://www.rust-lang.org/logos/rust-logo-128x128-blk-v2.png",
       html_favicon_url = "http://www.rust-lang.org/favicon.ico")]

#![feature(macro_rules, unsafe_destructor, phase)]
#![allow(visible_private_types)]

#[cfg(test)] extern crate debug;
#[cfg(test)] extern crate native;
extern crate green;
extern crate libc;
#[phase(plugin, link)] extern crate tls;

use std::fmt;
use std::io;
use std::rt::local::Local;
use std::rt::task::{BlockedTask, Task};
use std::str;
use std::task;
use libc::c_int;

pub use addrinfo::get_host_addresses;
pub use async::Async;
pub use event_loop::EventLoop;
pub use fs::File;
pub use idle::Idle;
pub use pipe::{Pipe, PipeListener, PipeAcceptor};
pub use signal::Signal;
pub use tcp::{Tcp, TcpListener, TcpAcceptor};
pub use timer::Timer;
pub use tty::Tty;
pub use udp::Udp;

mod macros;

mod access;
mod timeout;
pub mod homing;
mod queue;

pub mod uvll;

pub mod raw;
mod event_loop;

mod addrinfo;
mod async;
pub mod fs;
mod idle;
mod pipe;
// mod process;
mod signal;
mod stream;
mod tcp;
mod timer;
mod tty;
mod udp;

// /// Creates a new event loop which is powered by libuv
// ///
// /// This function is used in tandem with libgreen's `PoolConfig` type as a value
// /// for the `event_loop_factory` field. Using this function as the event loop
// /// factory will power programs with libuv and enable green threading.
// ///
// /// # Example
// ///
// /// ```
// /// extern crate rustuv;
// /// extern crate green;
// ///
// /// #[start]
// /// fn start(argc: int, argv: *const *const u8) -> int {
// ///     green::start(argc, argv, rustuv::event_loop, main)
// /// }
// ///
// /// fn main() {
// ///     // this code is running inside of a green task powered by libuv
// /// }
// /// ```
// pub fn event_loop() -> Box<green::EventLoop + Send> {
//     box uvio::UvEventLoop::new() as Box<green::EventLoop + Send>
// }

struct ForbidUnwind {
    msg: &'static str,
    failing_before: bool,
}

impl ForbidUnwind {
    fn new(s: &'static str) -> ForbidUnwind {
        ForbidUnwind {
            msg: s, failing_before: task::failing(),
        }
    }
}

impl Drop for ForbidUnwind {
    fn drop(&mut self) {
        assert!(self.failing_before == task::failing(),
                "didn't want an unwind during: {}", self.msg);
    }
}

fn block(mut uv_loop: raw::Loop, f: |BlockedTask|) {
    let _f = ForbidUnwind::new("wait_until_woken_after");
    let task: Box<Task> = Local::take();
    let cnt = uv_loop.get_data() as uint;
    uv_loop.set_data((cnt + 1) as *mut _);
    task.deschedule(1, |task| {
        f(task);
        Ok(())
    });
    uv_loop.set_data(cnt as *mut _);
}

fn wakeup(slot: &mut Option<BlockedTask>) {
    assert!(slot.is_some());
    slot.take().unwrap().reawaken();
}

pub type UvResult<T> = Result<T, UvError>;

#[deriving(Eq, PartialEq, Clone)]
pub struct UvError(c_int);

impl UvError {
    /// Creates a new uv error for a particular code
    pub fn new(code: c_int) -> UvError { UvError(code) }

    /// Return the name of this error
    pub fn name(&self) -> &'static str {
        unsafe {
            let name_str = uvll::uv_err_name(self.code());
            assert!(name_str.is_not_null());
            str::raw::c_str_to_static_slice(name_str)
        }
    }

    /// Return a textual description of this error
    pub fn desc(&self) -> &'static str {
        unsafe {
            let name_str = uvll::uv_strerror(self.code());
            assert!(name_str.is_not_null());
            str::raw::c_str_to_static_slice(name_str)
        }
    }

    /// Gain access to the raw code in this error
    pub fn code(&self) -> c_int { let UvError(code) = *self; code }

    /// Convert this libuv-based error to a std IoError instance
    #[cfg(unix)]
    pub fn to_io_error(&self) -> io::IoError {
        let code = if self.code() == uvll::EOF {
            libc::EOF as uint
        } else {
            -self.code() as uint
        };
        io::IoError::from_errno(code, true)
    }

    #[cfg(windows)]
    pub fn to_io_error(&self) -> io::IoError {
        let code = match self.code() {
            uvll::EOF => libc::EOF,
            uvll::EACCES => libc::ERROR_ACCESS_DENIED,
            uvll::ECONNREFUSED => libc::WSAECONNREFUSED,
            uvll::ECONNRESET => libc::WSAECONNRESET,
            uvll::ENOTCONN => libc::WSAENOTCONN,
            uvll::ENOENT => libc::ERROR_FILE_NOT_FOUND,
            uvll::EPIPE => libc::ERROR_NO_DATA,
            uvll::ECONNABORTED => libc::WSAECONNABORTED,
            uvll::EADDRNOTAVAIL => libc::WSAEADDRNOTAVAIL,
            uvll::ECANCELED => libc::ERROR_OPERATION_ABORTED,
            uvll::EADDRINUSE => libc::WSAEADDRINUSE,
            uvll::EPERM => libc::ERROR_ACCESS_DENIED,
            err => {
                uvdebug!("uverr.code {}", err as int);
                // FIXME: Need to map remaining uv error types
                -1
            }
        };
        io::IoError::from_errno(code, true)
    }
}

impl fmt::Show for UvError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {}", self.name(), self.desc())
    }
}

#[test]
fn error_smoke_test() {
    let err: UvError = UvError(uvll::EOF);
    assert_eq!(err.to_string(), "EOF: end of file".to_string());
}
