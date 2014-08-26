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

mod macros;

mod access;
mod timeout;
pub mod homing;
mod queue;
// mod rc;

pub mod uvll;

pub mod raw;
mod event_loop;

mod addrinfo;
mod async;
pub mod fs;
mod idle;
mod net;
mod pipe;
// mod process;
mod signal;
mod stream;
mod tcp;
mod timer;
mod tty;

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

// A type that wraps a uv handle
// trait UvHandle<T> {
//     fn uv_handle(&self) -> *mut T;
//
//     fn uv_loop(&self) -> Loop {
//         Loop::wrap(unsafe { uvll::get_loop_for_uv_handle(self.uv_handle()) })
//     }
//
//     // FIXME(#8888) dummy self
//     fn alloc(_: Option<Self>, ty: uvll::uv_handle_type) -> *mut T {
//         unsafe {
//             let handle = uvll::malloc_handle(ty);
//             assert!(!handle.is_null());
//             handle as *mut T
//         }
//     }
//
//     unsafe fn from_uv_handle<'a>(h: &'a *mut T) -> &'a mut Self {
//         mem::transmute(uvll::get_data_for_uv_handle(*h))
//     }
//
//     fn install(self: Box<Self>) -> Box<Self> {
//         unsafe {
//             let myptr = mem::transmute::<&Box<Self>, &*mut u8>(&self);
//             uvll::set_data_for_uv_handle(self.uv_handle(), *myptr);
//         }
//         self
//     }
//
//     fn close_async_(&mut self) {
//         // we used malloc to allocate all handles, so we must always have at
//         // least a callback to free all the handles we allocated.
//         extern fn close_cb(handle: *mut uvll::uv_handle_t) {
//             unsafe { uvll::free_handle(handle) }
//         }
//
//         unsafe {
//             uvll::set_data_for_uv_handle(self.uv_handle(), ptr::mut_null::<()>());
//             uvll::uv_close(self.uv_handle() as *mut uvll::uv_handle_t, close_cb)
//         }
//     }
//
//     fn close(&mut self) {
//         let mut slot = None;
//
//         unsafe {
//             uvll::uv_close(self.uv_handle() as *mut uvll::uv_handle_t, close_cb);
//             uvll::set_data_for_uv_handle(self.uv_handle(),
//                                          ptr::mut_null::<()>());
//
//             wait_until_woken_after(&mut slot, &self.uv_loop(), || {
//                 uvll::set_data_for_uv_handle(self.uv_handle(), &mut slot);
//             })
//         }
//
//         extern fn close_cb(handle: *mut uvll::uv_handle_t) {
//             unsafe {
//                 let data = uvll::get_data_for_uv_handle(handle);
//                 uvll::free_handle(handle);
//                 if data == ptr::mut_null() { return }
//                 let slot: &mut Option<BlockedTask> = mem::transmute(data);
//                 wakeup(slot);
//             }
//         }
//     }
// }

// struct ForbidSwitch {
//     msg: &'static str,
//     io: uint,
// }
//
// impl ForbidSwitch {
//     fn new(s: &'static str) -> ForbidSwitch {
//         ForbidSwitch {
//             msg: s,
//             io: homing::local_id(),
//         }
//     }
// }
//
// impl Drop for ForbidSwitch {
//     fn drop(&mut self) {
//         assert!(self.io == homing::local_id(),
//                 "didn't want a scheduler switch: {}",
//                 self.msg);
//     }
// }
//
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

// // struct Request {
// //     pub handle: *mut uvll::uv_req_t,
// //     defused: bool,
// // }
// //
// // impl Request {
// //     pub fn new(ty: uvll::uv_req_type) -> Request {
// //         unsafe {
// //             let handle = uvll::malloc_req(ty);
// //             uvll::set_data_for_req(handle, ptr::mut_null::<()>());
// //             Request::wrap(handle)
// //         }
// //     }
// //
// //     pub fn wrap(handle: *mut uvll::uv_req_t) -> Request {
// //         Request { handle: handle, defused: false }
// //     }
// //
// //     pub fn set_data<T>(&self, t: *mut T) {
// //         unsafe { uvll::set_data_for_req(self.handle, t) }
// //     }
// //
// //     pub unsafe fn get_data<T>(&self) -> &'static mut T {
// //         let data = uvll::get_data_for_req(self.handle);
// //         assert!(data != ptr::mut_null());
// //         mem::transmute(data)
// //     }
// //
// //     // This function should be used when the request handle has been given to an
// //     // underlying uv function, and the uv function has succeeded. This means
// //     // that uv will at some point invoke the callback, and in the meantime we
// //     // can't deallocate the handle because libuv could be using it.
// //     //
// //     // This is still a problem in blocking situations due to linked failure. In
// //     // the connection callback the handle should be re-wrapped with the `wrap`
// //     // function to ensure its destruction.
// //     pub fn defuse(&mut self) {
// //         self.defused = true;
// //     }
// // }
// //
// // impl Drop for Request {
// //     fn drop(&mut self) {
// //         if !self.defused {
// //             unsafe { uvll::free_req(self.handle) }
// //         }
// //     }
// // }
//
// pub struct Loop {
//     handle: *mut uvll::uv_loop_t
// }
//
// impl Loop {
//     pub fn new() -> UvResult<Loop> {
//         unsafe {
//             let size = uvll::uv_loop_size() as uint;
//             let handle: *mut uvll::uv_loop_t =
//                 std::rt::libc_heap::malloc_raw(size) as *mut _;
//             match uvll::uv_loop_init(handle) {
//                 0 => Ok(Loop { handle: handle }),
//                 n => {
//                     libc::free(handle as *mut libc::c_void);
//                     Err(UvError(n))
//                 }
//             }
//         }
//     }
//
//     pub fn wrap(handle: *mut uvll::uv_loop_t) -> Loop { Loop { handle: handle } }
//
//     pub fn run(&mut self) {
//         assert_eq!(unsafe { uvll::uv_run(self.handle, uvll::RUN_DEFAULT) }, 0);
//     }
//
//     pub fn close(&mut self) -> UvResult<()> {
//         unsafe {
//             match uvll::uv_loop_close(self.handle) {
//                 0 => {
//                     libc::free(self.handle as *mut libc::c_void);
//                     Ok(())
//                 }
//                 n => Err(UvError(n))
//             }
//         }
//     }
//
//     // The 'data' field of the uv_loop_t is used to count the number of tasks
//     // that are currently blocked waiting for I/O to complete.
//     fn modify_blockers(&self, amt: uint) {
//         unsafe {
//             let cur = uvll::get_data_for_uv_loop(self.handle) as uint;
//             uvll::set_data_for_uv_loop(self.handle, (cur + amt) as *mut c_void)
//         }
//     }
//
//     fn get_blockers(&self) -> uint {
//         unsafe { uvll::get_data_for_uv_loop(self.handle) as uint }
//     }
// }

pub type UvResult<T> = Result<T, UvError>;
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
