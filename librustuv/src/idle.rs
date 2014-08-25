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

use green::{Callback, PausableIdleCallback};
use raw::Handle;
use {raw, uvll, EventLoop, UvResult};

pub struct Idle { handle: raw::Idle }
struct Data { callback: Box<Callback + Send> }

impl Idle {
    pub fn new(cb: Box<Callback + Send>) -> UvResult<Idle> {
        let mut eloop = try!(EventLoop::borrow());
        Idle::new_on(&mut *eloop, cb)
    }

    pub fn new_on(eloop: &mut EventLoop,
                  cb: Box<Callback + Send>) -> UvResult<Idle> {
        unsafe {
            let mut ret = Idle { handle: try!(raw::Idle::new(&eloop.uv_loop())) };
            let data = box Data { callback: cb };
            ret.handle.set_data(mem::transmute(data));
            Ok(ret)
        }
    }

    /// Gain access to the underlying raw idle handle.
    ///
    /// This function is unsafe as there is no guarantee that any safe
    /// modifications to the idle handle are actually safe to perform given the
    /// assumptions of this object.
    pub unsafe fn raw(&self) -> raw::Idle { self.handle }
}

impl PausableIdleCallback for Idle {
    fn pause(&mut self) { self.handle.stop().unwrap() }
    fn resume(&mut self) { self.handle.start(idle_cb).unwrap() }
}

extern fn idle_cb(handle: *mut uvll::uv_idle_t) {
    unsafe {
        let raw: raw::Idle = Handle::from_raw(handle);
        let data: &mut Data = mem::transmute(raw.get_data());
        data.callback.call();
    }
}

impl Drop for Idle {
    fn drop(&mut self) {
        let _data: Box<Data> = unsafe { mem::transmute(self.handle.get_data()) };
        self.handle.stop().unwrap();
        unsafe { self.handle.close_and_free() }
    }
}
