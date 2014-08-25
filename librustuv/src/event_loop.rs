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
use std::kinds::marker;
use std::cell::Cell;

// use std::c_str::CString;
// use std::mem;
// use std::rt::rtio;
// use std::rt::rtio::{ProcessConfig, IoFactory, IoResult};
// use libc::c_int;
// use libc::{O_CREAT, O_APPEND, O_TRUNC, O_RDWR, O_RDONLY, O_WRONLY, S_IRUSR,
//                 S_IWUSR};
// use libc;
use green;

use {UvResult, Idle, Async, UvError};
use raw::{mod, Loop, Handle};
// use green::EventLoop;
//
// use super::{uv_error_to_io_error, Loop};

// use addrinfo::GetAddrInfoRequest;
// use async::AsyncWatcher;
// use file::{FsRequest, FileWatcher};
use queue::QueuePool;
use homing::HomeHandle;
// use idle::IdleWatcher;
// use net::{TcpWatcher, TcpListener, UdpWatcher};
// use pipe::{PipeWatcher, PipeListener};
// use process::Process;
// use signal::SignalWatcher;
// use timer::TimerWatcher;
// use tty::TtyWatcher;
use uvll;

tls!(local_loop: Cell<*mut EventLoop>)

pub struct EventLoop {
    uv_loop: Loop,
    pool: Option<Box<QueuePool>>,
}

pub struct BorrowedEventLoop {
    local: *mut EventLoop,
    marker1: marker::NoSend,
    marker2: marker::NoSync,
}

impl EventLoop {
    pub fn new() -> UvResult<EventLoop> {
        let mut uv_loop = try!(unsafe { Loop::new() });
        uv_loop.set_data(0 as *mut _);
        let pool = try!(QueuePool::new(&uv_loop));
        Ok(EventLoop {
            pool: Some(pool),
            uv_loop: uv_loop,
        })
    }

    /// Borrow a reference to the local event loop.
    ///
    /// If there is no local event loop, or the local event loop is already
    /// borrowed, then an error is returned.
    pub fn borrow() -> UvResult<BorrowedEventLoop> {
        let local = unsafe {
            local_loop.get(|local| {
                local.and_then(|c| {
                    match c.get() as uint {
                        0 => None,
                        n => { c.set(0 as *mut _); Some(n as *mut EventLoop) }
                    }
                })
            })
        };
        match local {
            Some(eloop) => Ok(BorrowedEventLoop {
                local: eloop,
                marker1: marker::NoSend,
                marker2: marker::NoSync,
            }),
            None => Err(UvError(uvll::UNKNOWN))
        }
    }

    /// Gain access to the underlying event loop.
    ///
    /// This method is unsafe as there is no guarantee that further safe methods
    /// called on the `Loop` will be valid as this event loop will deallocate it
    /// when it goes out of scope.
    pub unsafe fn uv_loop(&self) -> Loop { self.uv_loop }

    /// Create a new handle to this event loop.
    ///
    /// This handle can be used to return the current task home, assuming that
    /// it is a green task.
    pub fn make_handle(&mut self) -> HomeHandle {
        // It's understood by the homing code that the "local id" is just the
        // pointer of the local I/O factory cast to a uint.
        let id: uint = self as *mut _ as uint;
        HomeHandle::new(id, &mut **self.pool.get_mut_ref())
    }
}

impl green::EventLoop for EventLoop {
    fn run(&mut self) {
        let tls = Cell::new(self as *mut _);
        unsafe {
            local_loop.set(&tls, || {
                self.uv_loop.run(uvll::RUN_DEFAULT).unwrap();
            });
        }
    }

    fn callback(&mut self, f: proc()) {
        // Create a new idle handle, put the procedure into the custom data, and
        // then clean it up when the idle handle fires.
        unsafe {
            let mut idle = raw::Idle::new(&self.uv_loop).unwrap();
            idle.set_data(mem::transmute(box f));
            idle.start(onetime).unwrap();
        }

        extern fn onetime(handle: *mut uvll::uv_idle_t) {
            unsafe {
                let mut idle: raw::Idle = Handle::from_raw(handle);
                let f: Box<proc()> = mem::transmute(idle.get_data());
                idle.close_and_free();
                (*f)();
            }
        }
    }

    fn pausable_idle_callback(&mut self, cb: Box<green::Callback + Send>)
                              -> Box<green::PausableIdleCallback + Send> {
        box Idle::new_on(self, cb).unwrap()
            as Box<green::PausableIdleCallback + Send>
    }

    fn remote_callback(&mut self, f: Box<green::Callback + Send>)
                       -> Box<green::RemoteCallback + Send> {
        box Async::new_on(self, f).unwrap() as Box<green::RemoteCallback + Send>
    }

    fn has_active_io(&self) -> bool {
        true
        // self.uvio.loop_.get_blockers() > 0
    }
}

impl Drop for EventLoop {
    fn drop(&mut self) {
        use green::EventLoop;

        // Must first destroy the pool of handles before we destroy the loop
        // because otherwise the contained async handle will be destroyed after
        // the loop is free'd (use-after-free). We also must free the uv handle
        // after the loop has been closed because during the closing of the loop
        // the handle is required to be used apparently.
        //
        // Lastly, after we've closed the pool of handles we pump the event loop
        // one last time to run any closing callbacks to make sure the loop
        // shuts down cleanly.
        let mut handle = self.pool.get_ref().handle();
        drop(self.pool.take());
        self.run();

        unsafe {
            self.uv_loop.close().unwrap();
            handle.free();
            self.uv_loop.free();
        }
    }
}

impl Deref<EventLoop> for BorrowedEventLoop {
    fn deref<'a>(&'a self) -> &'a EventLoop { unsafe { &*self.local } }
}

impl DerefMut<EventLoop> for BorrowedEventLoop {
    fn deref_mut<'a>(&'a mut self) -> &'a mut EventLoop {
        unsafe { &mut *self.local }
    }
}

#[unsafe_destructor]
impl Drop for BorrowedEventLoop {
    fn drop(&mut self) {
        unsafe {
            local_loop.get(|l| l.unwrap().set(self.local))
        }
    }
}

// }

// impl IoFactory for IoFactory {
//     // Connect to an address and return a new stream
//     // NB: This blocks the task waiting on the connection.
//     // It would probably be better to return a future
//     fn tcp_connect(&mut self, addr: rtio::SocketAddr, timeout: Option<u64>)
//                    -> IoResult<Box<rtio::RtioTcpStream + Send>> {
//         match TcpWatcher::connect(self, addr, timeout) {
//             Ok(t) => Ok(box t as Box<rtio::RtioTcpStream + Send>),
//             Err(e) => Err(uv_error_to_io_error(e)),
//         }
//     }
//
//     fn tcp_bind(&mut self, addr: rtio::SocketAddr)
//                 -> IoResult<Box<rtio::RtioTcpListener + Send>> {
//         match TcpListener::bind(self, addr) {
//             Ok(t) => Ok(t as Box<rtio::RtioTcpListener + Send>),
//             Err(e) => Err(uv_error_to_io_error(e)),
//         }
//     }
//
//     fn udp_bind(&mut self, addr: rtio::SocketAddr)
//                 -> IoResult<Box<rtio::RtioUdpSocket + Send>> {
//         match UdpWatcher::bind(self, addr) {
//             Ok(u) => Ok(box u as Box<rtio::RtioUdpSocket + Send>),
//             Err(e) => Err(uv_error_to_io_error(e)),
//         }
//     }
//
//     fn timer_init(&mut self) -> IoResult<Box<rtio::RtioTimer + Send>> {
//         Ok(TimerWatcher::new(self) as Box<rtio::RtioTimer + Send>)
//     }
//
//     fn get_host_addresses(&mut self, host: Option<&str>, servname: Option<&str>,
//                           hint: Option<rtio::AddrinfoHint>)
//         -> IoResult<Vec<rtio::AddrinfoInfo>>
//     {
//         let r = GetAddrInfoRequest::run(&self.loop_, host, servname, hint);
//         r.map_err(uv_error_to_io_error)
//     }
//
//     fn fs_from_raw_fd(&mut self, fd: c_int, close: rtio::CloseBehavior)
//                       -> Box<rtio::RtioFileStream + Send> {
//         box FileWatcher::new(self, fd, close) as
//             Box<rtio::RtioFileStream + Send>
//     }
//
//     fn fs_open(&mut self, path: &CString, fm: rtio::FileMode,
//                fa: rtio::FileAccess)
//         -> IoResult<Box<rtio::RtioFileStream + Send>>
//     {
//         let flags = match fm {
//             rtio::Open => 0,
//             rtio::Append => libc::O_APPEND,
//             rtio::Truncate => libc::O_TRUNC,
//         };
//         // Opening with a write permission must silently create the file.
//         let (flags, mode) = match fa {
//             rtio::Read => (flags | libc::O_RDONLY, 0),
//             rtio::Write => (flags | libc::O_WRONLY | libc::O_CREAT,
//                             libc::S_IRUSR | libc::S_IWUSR),
//             rtio::ReadWrite => (flags | libc::O_RDWR | libc::O_CREAT,
//                                 libc::S_IRUSR | libc::S_IWUSR),
//         };
//
//         match FsRequest::open(self, path, flags as int, mode as int) {
//             Ok(fs) => Ok(box fs as Box<rtio::RtioFileStream + Send>),
//             Err(e) => Err(uv_error_to_io_error(e))
//         }
//     }
//
//     fn fs_unlink(&mut self, path: &CString) -> IoResult<()> {
//         let r = FsRequest::unlink(&self.loop_, path);
//         r.map_err(uv_error_to_io_error)
//     }
//     fn fs_lstat(&mut self, path: &CString) -> IoResult<rtio::FileStat> {
//         let r = FsRequest::lstat(&self.loop_, path);
//         r.map_err(uv_error_to_io_error)
//     }
//     fn fs_stat(&mut self, path: &CString) -> IoResult<rtio::FileStat> {
//         let r = FsRequest::stat(&self.loop_, path);
//         r.map_err(uv_error_to_io_error)
//     }
//     fn fs_mkdir(&mut self, path: &CString, perm: uint) -> IoResult<()> {
//         let r = FsRequest::mkdir(&self.loop_, path, perm as c_int);
//         r.map_err(uv_error_to_io_error)
//     }
//     fn fs_rmdir(&mut self, path: &CString) -> IoResult<()> {
//         let r = FsRequest::rmdir(&self.loop_, path);
//         r.map_err(uv_error_to_io_error)
//     }
//     fn fs_rename(&mut self, path: &CString, to: &CString) -> IoResult<()> {
//         let r = FsRequest::rename(&self.loop_, path, to);
//         r.map_err(uv_error_to_io_error)
//     }
//     fn fs_chmod(&mut self, path: &CString, perm: uint) -> IoResult<()> {
//         let r = FsRequest::chmod(&self.loop_, path, perm as c_int);
//         r.map_err(uv_error_to_io_error)
//     }
//     fn fs_readdir(&mut self, path: &CString, flags: c_int)
//         -> IoResult<Vec<CString>>
//     {
//         let r = FsRequest::readdir(&self.loop_, path, flags);
//         r.map_err(uv_error_to_io_error)
//     }
//     fn fs_link(&mut self, src: &CString, dst: &CString) -> IoResult<()> {
//         let r = FsRequest::link(&self.loop_, src, dst);
//         r.map_err(uv_error_to_io_error)
//     }
//     fn fs_symlink(&mut self, src: &CString, dst: &CString) -> IoResult<()> {
//         let r = FsRequest::symlink(&self.loop_, src, dst);
//         r.map_err(uv_error_to_io_error)
//     }
//     fn fs_chown(&mut self, path: &CString, uid: int, gid: int) -> IoResult<()> {
//         let r = FsRequest::chown(&self.loop_, path, uid, gid);
//         r.map_err(uv_error_to_io_error)
//     }
//     fn fs_readlink(&mut self, path: &CString) -> IoResult<CString> {
//         let r = FsRequest::readlink(&self.loop_, path);
//         r.map_err(uv_error_to_io_error)
//     }
//     fn fs_utime(&mut self, path: &CString, atime: u64, mtime: u64)
//         -> IoResult<()>
//     {
//         let r = FsRequest::utime(&self.loop_, path, atime, mtime);
//         r.map_err(uv_error_to_io_error)
//     }
//
//     fn spawn(&mut self, cfg: ProcessConfig)
//             -> IoResult<(Box<rtio::RtioProcess + Send>,
//                          Vec<Option<Box<rtio::RtioPipe + Send>>>)>
//     {
//         match Process::spawn(self, cfg) {
//             Ok((p, io)) => {
//                 Ok((p as Box<rtio::RtioProcess + Send>,
//                     io.move_iter().map(|i| i.map(|p| {
//                         box p as Box<rtio::RtioPipe + Send>
//                     })).collect()))
//             }
//             Err(e) => Err(uv_error_to_io_error(e)),
//         }
//     }
//
//     fn kill(&mut self, pid: libc::pid_t, signum: int) -> IoResult<()> {
//         Process::kill(pid, signum).map_err(uv_error_to_io_error)
//     }
//
//     fn unix_bind(&mut self, path: &CString)
//                  -> IoResult<Box<rtio::RtioUnixListener + Send>> {
//         match PipeListener::bind(self, path) {
//             Ok(p) => Ok(p as Box<rtio::RtioUnixListener + Send>),
//             Err(e) => Err(uv_error_to_io_error(e)),
//         }
//     }
//
//     fn unix_connect(&mut self, path: &CString, timeout: Option<u64>)
//                     -> IoResult<Box<rtio::RtioPipe + Send>> {
//         match PipeWatcher::connect(self, path, timeout) {
//             Ok(p) => Ok(box p as Box<rtio::RtioPipe + Send>),
//             Err(e) => Err(uv_error_to_io_error(e)),
//         }
//     }
//
//     fn tty_open(&mut self, fd: c_int, readable: bool)
//             -> IoResult<Box<rtio::RtioTTY + Send>> {
//         match TtyWatcher::new(self, fd, readable) {
//             Ok(tty) => Ok(box tty as Box<rtio::RtioTTY + Send>),
//             Err(e) => Err(uv_error_to_io_error(e))
//         }
//     }
//
//     fn pipe_open(&mut self, fd: c_int)
//         -> IoResult<Box<rtio::RtioPipe + Send>>
//     {
//         match PipeWatcher::open(self, fd) {
//             Ok(s) => Ok(box s as Box<rtio::RtioPipe + Send>),
//             Err(e) => Err(uv_error_to_io_error(e))
//         }
//     }
//
//     fn signal(&mut self, signum: int, cb: Box<rtio::Callback + Send>)
//         -> IoResult<Box<rtio::RtioSignal + Send>>
//     {
//         match SignalWatcher::new(self, signum, cb) {
//             Ok(s) => Ok(s as Box<rtio::RtioSignal + Send>),
//             Err(e) => Err(uv_error_to_io_error(e)),
//         }
//     }
// }

// #[cfg(test)]
// mod tests {
//     use green::EventLoop;
//     use super::EventLoop;
//
//     #[test]
//     fn test_callback_run_once() {
//         let mut event_loop = EventLoop::new();
//         let mut count = 0;
//         let count_ptr: *mut int = &mut count;
//         event_loop.callback(proc() {
//             unsafe { *count_ptr += 1 }
//         });
//         event_loop.run();
//         assert_eq!(count, 1);
//     }
// }
//
