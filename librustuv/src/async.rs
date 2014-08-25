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
use std::rt::exclusive::Exclusive;
use green::{Callback, RemoteCallback};

use {raw, uvll, EventLoop, UvResult};
use raw::Handle;

/// An asynchronous handle which is used to send notifications to the event
/// loop.
pub struct Async { handle: raw::Async }

struct Data {
    // A flag to tell the callback to exit, set from the dtor. This is
    // almost never contested - only in rare races with the dtor.
    exit_flag: Exclusive<bool>,
    callback: Box<Callback + Send>,
}

impl Async {
    /// Create a new asynchronous handle to the local event loop.
    ///
    /// The When this handle is `fire`d, the given callback will be invoked on
    /// the event loop. This corresponds to `uv_async_t`.
    pub fn new(cb: Box<Callback + Send>) -> UvResult<Async> {
        let mut eloop = try!(EventLoop::borrow());
        Async::new_on(&mut *eloop, cb)
    }

    /// Same as `new`, but specifies what event loop to be created on.
    pub fn new_on(eloop: &mut EventLoop,
                  cb: Box<Callback + Send>) -> UvResult<Async> {
        unsafe {
            let mut ret = Async {
                handle: try!(raw::Async::new(&eloop.uv_loop(), async_cb)),
            };
            let data = box Data {
                exit_flag: Exclusive::new(false),
                callback: cb,
            };
            ret.handle.set_data(mem::transmute(data));
            Ok(ret)
        }
    }

    /// Gain access to the underlying raw async handle.
    ///
    /// This function is unsafe as there is no guarantee that any safe
    /// modifications to the async handle are actually safe to perform given the
    /// assumptions of this object.
    pub unsafe fn raw(&self) -> raw::Async { self.handle }
}

extern fn async_cb(handle: *mut uvll::uv_async_t) {
    unsafe {
        let mut handle: raw::Async = Handle::from_raw(handle);
        let data: &mut Data = mem::transmute(handle.get_data());

        // The synchronization logic here is subtle. To review,
        // the uv async handle type promises that, after it is
        // triggered the remote callback is definitely called at
        // least once. UvRemoteCallback needs to maintain those
        // semantics while also shutting down cleanly from the
        // dtor. In our case that means that, when the
        // UvRemoteCallback dtor calls `async.send()`, here `f` is
        // always called later.

        // In the dtor both the exit flag is set and the async
        // callback fired under a lock.  Here, before calling `f`,
        // we take the lock and check the flag. Because we are
        // checking the flag before calling `f`, and the flag is
        // set under the same lock as the send, then if the flag
        // is set then we're guaranteed to call `f` after the
        // final send.

        // If the check was done after `f()` then there would be a
        // period between that call and the check where the dtor
        // could be called in the other thread, missing the final
        // callback while still destroying the handle.

        let should_exit = *data.exit_flag.lock();

        data.callback.call();

        if should_exit {
            handle.close(close_cb);
        }
    }
}

extern fn close_cb(handle: *mut uvll::uv_handle_t) {
    let handle = handle as *mut uvll::uv_async_t;
    unsafe {
        let mut handle: raw::Async = Handle::from_raw(handle);
        // drop the payload
        let _data: Box<Data> = mem::transmute(handle.get_data());
        // and then free the handle
        handle.free();
    }
}


impl RemoteCallback for Async {
    fn fire(&mut self) { self.handle.send() }
}

impl Drop for Async {
    fn drop(&mut self) {
        unsafe {
            let data: &Data = mem::transmute(self.handle.get_data());
            let mut should_exit = data.exit_flag.lock();
            // NB: These two things need to happen atomically. Otherwise
            // the event handler could wake up due to a *previous*
            // signal and see the exit flag, destroying the handle
            // before the final send.
            *should_exit = true;
            self.handle.send();
        }
    }
}

#[cfg(test)]
mod test_remote {
    use green::{Callback, RemoteCallback};

    use super::Async;

    // Make sure that we can fire watchers in remote threads and that they
    // actually trigger what they say they will.
    test!(fn smoke_test() {
        struct MyCallback(Option<Sender<int>>);
        impl Callback for MyCallback {
            fn call(&mut self) {
                // this can get called more than once, but we only want to send
                // once
                let MyCallback(ref mut s) = *self;
                match s.take() {
                    Some(s) => s.send(1),
                    None => {}
                }
            }
        }

        let (tx, rx) = channel();
        let cb = box MyCallback(Some(tx));
        let watcher = Async::new(cb).unwrap();

        spawn(proc() {
            let mut watcher = watcher;
            watcher.fire();
        });
        assert_eq!(rx.recv(), 1);
    })
}
