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

use green::Callback;

use {raw, uvll, EventLoop, UvResult};
use raw::Handle;
use homing::{HomingIO, HomeHandle};

pub struct Signal {
    handle: raw::Signal,
    home: HomeHandle,
}

struct Data {
    callback: Option<Box<Callback + Send>>,
}

impl Signal {
    pub fn new() -> UvResult<Signal> {
        Signal::new_on(&mut *try!(EventLoop::borrow()))
    }

    pub fn new_on(eloop: &mut EventLoop) -> UvResult<Signal> {
        unsafe {
            let mut ret = Signal {
                handle: try!(raw::Signal::new(&eloop.uv_loop())),
                home: eloop.make_handle(),
            };
            let data = box Data { callback: None };
            ret.handle.set_data(mem::transmute(data));
            Ok(ret)
        }
    }

    /// Attempts to start listening for the signal `signal`.
    ///
    /// When the process receives the specified signal, the callback `cb` will
    /// be invoked on the event loop. This function will cancel any previous
    /// signal being listened for.
    ///
    /// For more information, see `uv_signal_start`.
    pub fn start(&mut self, signal: libc::c_int,
                 cb: Box<Callback + Send>) -> UvResult<()> {
        // Be sure to run user destructors outside the homing missile, not
        // inside.
        let _prev = {
            let _m = self.fire_homing_missile();
            try!(self.handle.start(signal, signal_cb));
            let data: &mut Data = unsafe {
                mem::transmute(self.handle.get_data())
            };
            mem::replace(&mut data.callback, Some(cb))
        };
        Ok(())
    }

    /// Stop listening for the signal previously registered in `start`.
    pub fn stop(&mut self) -> UvResult<()> {
        let _prev = {
            let _m = self.fire_homing_missile();
            try!(self.handle.stop());
            let data: &mut Data = unsafe {
                mem::transmute(self.handle.get_data())
            };
            data.callback.take()
        };
        Ok(())
    }
}

extern fn signal_cb(handle: *mut uvll::uv_signal_t, _signum: libc::c_int) {
    unsafe {
        let raw: raw::Signal = Handle::from_raw(handle);
        let data: &mut Data = mem::transmute(raw.get_data());
        assert!(data.callback.is_some());
        data.callback.as_mut().unwrap().call();
    }
}

impl HomingIO for Signal {
    fn home<'r>(&'r mut self) -> &'r mut HomeHandle { &mut self.home }
}

impl Drop for Signal {
    fn drop(&mut self) {
        let _data: Box<Data> = unsafe {
            let _m = self.fire_homing_missile();
            self.handle.stop().unwrap();
            self.handle.close_and_free();
            mem::transmute(self.handle.get_data())
        };
    }
}
