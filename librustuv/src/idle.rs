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
    pub fn new(eloop: &mut EventLoop,
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

#[cfg(test)]
mod test {
    use std::mem;
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::rt::task::{BlockedTask, Task};
    use std::rt::local::Local;

    use green::{Callback, PausableIdleCallback};
    use EventLoop;
    use super::Idle;

    type Chan = Rc<RefCell<(Option<BlockedTask>, uint)>>;

    struct MyCallback(Rc<RefCell<(Option<BlockedTask>, uint)>>, uint);
    impl Callback for MyCallback {
        fn call(&mut self) {
            let task = match *self {
                MyCallback(ref rc, n) => {
                    match *rc.borrow_mut().deref_mut() {
                        (ref mut task, ref mut val) => {
                            *val = n;
                            match task.take() {
                                Some(t) => t,
                                None => return
                            }
                        }
                    }
                }
            };
            let _ = task.wake().map(|t| t.reawaken());
        }
    }

    fn mk(v: uint, uv: &mut EventLoop) -> (Idle, Chan) {
        let rc = Rc::new(RefCell::new((None, 0)));
        let cb = box MyCallback(rc.clone(), v);
        let cb = cb as Box<Callback>;
        let cb = unsafe { mem::transmute(cb) };
        (Idle::new(uv, cb).unwrap(), rc)
    }

    fn sleep(chan: &Chan) -> uint {
        let task: Box<Task> = Local::take();
        task.deschedule(1, |task| {
            match *chan.borrow_mut().deref_mut() {
                (ref mut slot, _) => {
                    assert!(slot.is_none());
                    *slot = Some(task);
                }
            }
            Ok(())
        });

        match *chan.borrow() { (_, n) => n }
    }

    test!(fn not_used() {
        let (_idle, _chan) = mk(1, ::test::local_loop());
    })

    test!(fn smoke_test() {
        let (mut idle, chan) = mk(1, ::test::local_loop());
        idle.resume();
        assert_eq!(sleep(&chan), 1);
    })

    test!(fn smoke_drop() {
        let (mut idle, _chan) = mk(1, ::test::local_loop());
        idle.resume();
        fail!();
    })

    test!(fn fun_combinations_of_methods() {
        let (mut idle, chan) = mk(1, ::test::local_loop());
        idle.resume();
        assert_eq!(sleep(&chan), 1);
        idle.pause();
        idle.resume();
        idle.resume();
        assert_eq!(sleep(&chan), 1);
        idle.pause();
        idle.pause();
        idle.resume();
        assert_eq!(sleep(&chan), 1);
    })

    test!(fn pause_pauses() {
        let (mut idle1, chan1) = mk(1, ::test::local_loop());
        let (mut idle2, chan2) = mk(2, ::test::local_loop());
        idle2.resume();
        assert_eq!(sleep(&chan2), 2);
        idle2.pause();
        idle1.resume();
        assert_eq!(sleep(&chan1), 1);
    })
}
