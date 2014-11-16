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
use green;

use {uvll, UvResult, Idle, Async, UvError};
use raw::{mod, Loop, Handle};
use queue::QueuePool;
use homing::HomeHandle;

scoped_tls!(static local_loop: Cell<(*mut EventLoop, bool)>)

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
            local_loop.with(|local| {
                local.and_then(|c| {
                    match c.get() {
                        (_, true) => None,
                        (p, false) => { c.set((p, true)); Some(p) }
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

    /// Borrow an unsafe pointer to the local event loop
    pub unsafe fn borrow_raw() -> UvResult<*mut EventLoop> {
        let local = local_loop.with(|local| local.map(|c| c.get().val0()));
        match local {
            Some(eloop) => Ok(eloop),
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
        HomeHandle::new(id, &mut **self.pool.as_mut().unwrap())
    }
}

impl green::EventLoop for EventLoop {
    fn run(&mut self) {
        let tls = Cell::new((self as *mut _, false));
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
        unsafe { self.uv_loop().get_data() as uint > 0 }
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
        let mut handle = self.pool.as_ref().unwrap().handle();
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
            local_loop.with(|l| l.unwrap().set((self.local, false)))
        }
    }
}
