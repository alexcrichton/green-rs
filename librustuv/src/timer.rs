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
use std::time::Duration;
use std::rt::task::BlockedTask;

use green::Callback;

use {raw, uvll, EventLoop, UvResult};
use homing::{HomeHandle, HomingIO, HomingMissile};
use raw::Handle;

/// A libuv-based timer to schedule callbacks to run on an event loop.
pub struct Timer {
    handle: raw::Timer,
    home: HomeHandle,
}

struct Data {
    action: Option<Action>,
    id: uint, // see comments in timer_cb
}

enum Action {
    WakeTask(BlockedTask),
    CallOnce(Box<Callback + Send>),
    CallMany(Box<Callback + Send>, uint),
}

impl Timer {
    /// Create a new timer on the local event loop.
    pub fn new() -> UvResult<Timer> {
        let mut eloop = try!(EventLoop::borrow());
        Timer::new_on(&mut *eloop)
    }

    /// Same as `new`, but specifies what event loop to be created on.
    pub fn new_on(eloop: &mut EventLoop) -> UvResult<Timer> {
        unsafe {
            let data = box Data {
                action: None,
                id: 0,
            };
            let mut ret = Timer {
                handle: try!(raw::Timer::new(&eloop.uv_loop())),
                home: eloop.make_handle(),
            };
            ret.handle.set_data(mem::transmute(data));
            Ok(ret)
        }
    }

    /// Sleep for a specified duration of time.
    ///
    /// See [`std::io::Timer::sleep`][1] for semantic information.
    ///
    /// [1]: http://doc.rust-lang.org/std/io/timer/struct.Timer.html#method.sleep
    pub fn sleep(&mut self, dur: Duration) {
        let mut ms = dur.num_milliseconds();
        if ms <= 0 { ms = 0; }

        let mut handle = self.handle;

        // As with all of the below functions, we must be extra careful when
        // destroying the previous action. If the previous action was a
        // callback, destroying it could invoke a context switch. For these
        // situations, we must temporarily un-home ourselves, then destroy the
        // action, and then re-home again.
        {
            let (m, data, _) = self.data();
            handle.stop().unwrap();
            match data.action.take() {
                Some(a) => { drop(m); drop(a); }
                None => {}
            }
        }
        let (_m, data, mut handle) = self.data();
        assert!(data.action.is_none());
        data.id += 1;
        ::block(handle.uv_loop(), |task| {
            data.action = Some(Action::WakeTask(task));
            handle.stop().unwrap();
            handle.start(ms as u64, 0, timer_cb).unwrap();
        });
    }

    /// Schedule a callback to be run once after the specified duration has
    /// elapsed.
    ///
    /// See [`std::io::Timer::oneshot`][1] for semantic information.
    ///
    /// [1]: http://doc.rust-lang.org/std/io/timer/struct.Timer.html#method.oneshot
    pub fn oneshot(&mut self, dur: Duration, cb: Box<Callback + Send>) {
        let mut ms = dur.num_milliseconds();
        if ms <= 0 { ms = 0; }

        let _prev = {
            let (_m, data, mut handle) = self.data();
            data.id += 1;
            handle.stop().unwrap();
            handle.start(ms as u64, 0, timer_cb).unwrap();
            mem::replace(&mut data.action, Some(Action::CallOnce(cb)))
        };
    }

    /// Schedule a callback to be run after each `dur` amount of time that
    /// passes.
    ///
    /// See [`std::io::Timer::periodic`][1] for semantic information.
    ///
    /// [1]: http://doc.rust-lang.org/std/io/timer/struct.Timer.html#method.periodic
    pub fn periodic(&mut self, dur: Duration, cb: Box<Callback + Send>) {
        let mut ms = dur.num_milliseconds();
        if ms <= 0 { ms = 1; }

        let _prev = {
            let (_m, data, mut handle) = self.data();
            data.id += 1;
            handle.stop().unwrap();
            handle.start(ms as u64, ms as u64, timer_cb).unwrap();
            mem::replace(&mut data.action, Some(Action::CallMany(cb, data.id)))
        };
    }

    fn data(&mut self) -> (HomingMissile, &mut Data, raw::Timer) {
        let m = self.fire_homing_missile();
        (m, unsafe { mem::transmute(self.handle.get_data()) }, self.handle)
    }

    /// Gain access to the underlying raw timer handle.
    ///
    /// This function is unsafe as there is no guarantee that any safe
    /// modifications to the timer handle are actually safe to perform given the
    /// assumptions of this object.
    pub unsafe fn raw(&self) -> raw::Timer { self.handle }
}

extern fn timer_cb(timer: *mut uvll::uv_timer_t) {
    let timer: raw::Timer = unsafe { Handle::from_raw(timer) };
    let data: &mut Data = unsafe { mem::transmute(timer.get_data()) };
    match data.action.take().unwrap() {
        Action::WakeTask(task) => task.reawaken(),
        Action::CallOnce(mut cb) => cb.call(),
        Action::CallMany(mut cb, id) => {
            cb.call();

            // Note that the above operation could have performed some form
            // of scheduling. This means that the timer may have decided to
            // insert some other action to happen. This 'id' keeps track of
            // the updates to the timer, so we only reset the action back to
            // sending on this channel if the id has remained the same. This
            // is essentially a bug in that we have mutably aliasable
            // memory, but that's libuv for you. We're guaranteed to all be
            // running on the same thread, so there's no need for any
            // synchronization here.
            if data.id == id {
                data.action = Some(Action::CallMany(cb, id));
            }
        }
    }
}

impl HomingIO for Timer {
    fn home(&self) -> &HomeHandle { &self.home }
}

impl Drop for Timer {
    fn drop(&mut self) {
        // note that this drop is a little subtle. Dropping a channel which is
        // held internally may invoke some scheduling operations. We can't take
        // the channel unless we're on the home scheduler, but once we're on the
        // home scheduler we should never move. Hence, we take the timer's
        // action item and then move it outside of the homing block.
        //
        // Furthermore, we can't actually free the extraneous `Data` here
        // because the `timer_cb` may actually be somewhere up on the stack in a
        // `Action::CallMany` which might use the data. To mitigate this, we bump the
        // `id` (so the `Action::CallMany` doesn't re-store its callback) and then
        // schedule a custom close callback to free both the timer and the data.
        let _action = unsafe {
            let _m = self.fire_homing_missile();
            self.handle.stop().unwrap();
            let data: &mut Data = mem::transmute(self.handle.get_data());
            data.id += 1;
            self.handle.close(Some(close_cb));
            data.action.take()
        };

        extern fn close_cb(handle: *mut uvll::uv_handle_t) {
            unsafe {
                let handle = handle as *mut uvll::uv_timer_t;
                let mut timer: raw::Timer = Handle::from_raw(handle);
                let data: Box<Data> = mem::transmute(timer.get_data());
                assert!(data.action.is_none());
                timer.free();
            }
        }
    }
}
