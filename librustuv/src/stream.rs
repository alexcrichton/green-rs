// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use libc::{c_int, size_t, ssize_t};
use std::mem;
use std::rt::task::BlockedTask;

// use Loop;
// use super::{UvError, Buf, slice_to_uv_buf, Request, wait_until_woken_after,
//             ForbidUnwind, wakeup};
use raw::{mod, Handle, Request};
use {uvll, UvResult, UvError};

// This is a helper structure which is intended to get embedded into other
// structures. This structure will retain a handle to the underlying
// uv_stream_t instance, and all I/O operations assume that it's already located
// on the appropriate scheduler.
pub struct Stream<T> {
    pub handle: T,

    // Cache the last used uv_write_t so we don't have to allocate a new one on
    // every call to uv_write(). Ideally this would be a stack-allocated
    // structure, but currently we don't have mappings for all the structures
    // defined in libuv, so we're forced to malloc this.
    last_write_req: Option<Write>,
}

struct Write {
    handle: raw::Write,
}

struct ReadContext {
    buf: Option<uvll::uv_buf_t>,
    result: ssize_t,
    task: Option<BlockedTask>,
}

struct WriteContext {
    result: c_int,
    task: Option<BlockedTask>,
}

impl<T: raw::Allocated, U: raw::Stream<T>> Stream<U> {
    // Creates a new helper structure which should be then embedded into another
    // watcher. This provides the generic read/write methods on streams.
    //
    // This structure will *not* close the stream when it is dropped. It is up
    // to the enclosure structure to be sure to call the close method (which
    // will block the task). Note that this is also required to prevent memory
    // leaks.
    //
    // It should also be noted that the `data` field of the underlying uv handle
    // will be manipulated on each of the methods called on this watcher.
    // Wrappers should ensure to always reset the field to an appropriate value
    // if they rely on the field to perform an action.
    pub fn new(mut stream: U, init: bool) -> Stream<U> {
        if init {
            stream.set_data(0 as *mut _);
        }
        Stream {
            handle: stream,
            last_write_req: None,
        }
    }

    pub fn read(&mut self, buf: &mut [u8]) -> UvResult<uint> {
        let mut rcx = ReadContext {
            buf: Some(raw::slice_to_uv_buf(buf)),
            result: 0,
            task: None,
        };

        self.handle.set_data(&mut rcx as *mut _ as *mut _);

        // Send off the read request, but don't block until we're sure that the
        // read request is queued.
        let ret = match self.handle.read_start(alloc_cb::<T, U>,
                                               read_cb::<T, U>) {
            Ok(()) => {
                ::block(self.handle.uv_loop(), |task| {
                    rcx.task = Some(task);
                });
                match rcx.result {
                    n if n < 0 => Err(UvError(n as c_int)),
                    n => Ok(n as uint),
                }
            }
            Err(e) => Err(e),
        };
        // Make sure a read cancellation sees that there's no pending read
        self.handle.set_data(0 as *mut _);
        return ret;
    }

    pub fn cancel_read(&mut self, reason: ssize_t) -> Option<BlockedTask> {
        // When we invoke uv_read_stop, it cancels the read and alloc
        // callbacks. We need to manually wake up a pending task (if one was
        // present).
        self.handle.read_stop().unwrap();
        let data = self.handle.get_data();
        if data.is_null() { return None }

        unsafe {
            self.handle.set_data(0 as *mut _);
            let data: &mut ReadContext = &mut *(data as *mut ReadContext);
            data.result = reason;
            data.task.take()
        }
    }

    pub fn write(&mut self, buf: &[u8]) -> Result<(), UvError> {
        // Prepare the write request, either using a cached one or allocating a
        // new one
        let mut req = match self.last_write_req.take() {
            Some(req) => req,
            None => unsafe { Write { handle: raw::Request::alloc() } },
        };
        req.handle.set_data(0 as *mut _);
        try!(req.handle.send(&mut self.handle, buf, write_cb));

        let mut wcx = WriteContext {
            result: 0,
            task: None,
        };
        req.handle.set_data(&mut wcx as *mut _ as *mut _);
        ::block(self.handle.uv_loop(), |task| {
            wcx.task = Some(task);
        });

        self.last_write_req = Some(req);
        return match wcx.result {
            0 => Ok(()),
            n => Err(UvError(n)),
        }
    }
}

// This allocation callback expects to be invoked once and only once. It will
// unwrap the buffer in the ReadContext stored in the stream and return it. This
// will fail if it is called more than once.
extern fn alloc_cb<T, U>(stream: *mut uvll::uv_stream_t, _hint: size_t,
                         buf: *mut uvll::uv_buf_t)
                         where T: raw::Allocated, U: raw::Stream<T> {
    uvdebug!("alloc_cb");
    unsafe {
        let raw: U = raw::Handle::from_raw(stream as *mut T);
        let rcx: &mut ReadContext = mem::transmute(raw.get_data());
        *buf = rcx.buf.take().expect("stream alloc_cb called more than once");
    }
}

// When a stream has read some data, we will always forcibly stop reading and
// return all the data read (even if it didn't fill the whole buffer).
extern fn read_cb<T, U>(stream: *mut uvll::uv_stream_t, nread: ssize_t,
                        _buf: *const uvll::uv_buf_t)
                        where T: raw::Allocated, U: raw::Stream<T> {
    use raw::Stream;
    use raw::Handle;
    assert!(nread != uvll::ECANCELED as ssize_t);

    unsafe {
        let mut raw: U = raw::Handle::from_raw(stream as *mut T);
        let rcx: &mut ReadContext = mem::transmute(raw.get_data());

        // Stop reading so that no read callbacks are
        // triggered before the user calls `read` again.
        raw.read_stop().unwrap();
        rcx.result = nread;

        ::wakeup(&mut rcx.task);
    }
}

// Unlike reading, the WriteContext is stored in the uv_write_t request. Like
// reading, however, all this does is wake up the blocked task after squirreling
// away the error code as a result.
extern fn write_cb(req: *mut uvll::uv_write_t, status: c_int) {
    unsafe {
        let raw: raw::Write = raw::Request::from_raw(req);
        let wcx: &mut WriteContext = mem::transmute(raw.get_data());
        wcx.result = status;
        ::wakeup(&mut wcx.task);
    }
}

impl Drop for Write {
    fn drop(&mut self) {
        unsafe { self.handle.free(); }
    }
}
