// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::c_str::CString;
use std::io;
use std::mem;
use std::rt::task::BlockedTask;
use std::sync::Arc;
use std::time::Duration;
use libc;

use access::Access;
use homing::{HomingIO, HomeHandle};
use raw::Handle;
use stream::Stream;
use timeout::{Pusher, AcceptTimeout, ConnectCtx, AccessTimeout};
use {raw, uvll, net, EventLoop, UvResult, UvError};

pub struct Pipe {
    data: Arc<PipeData>,
    stream: Stream<raw::Pipe>,

    // see comments in TcpWatcher for why these exist
    write_access: Access<()>,
    read_access: AccessTimeout<()>,
}

struct PipeData {
    handle: raw::Pipe,
    home: HomeHandle,
}

pub struct PipeListener {
    handle: raw::Pipe,
    home: HomeHandle,
}

#[deriving(Clone)]
pub struct PipeAcceptor {
    data: Arc<AcceptorData>,
    access: AcceptTimeout<Pipe>,
}

struct AcceptorData {
    listener: PipeListener,
    pusher: Pusher<Pipe>,
}

// Pipe implementation and traits

impl Pipe {
    // Creates an uninitialized pipe watcher. The underlying uv pipe is ready to
    // get bound to some other source (this is normally a helper method paired
    // with another call).
    unsafe fn new(uv_loop: &raw::Loop, home: HomeHandle) -> UvResult<Pipe> {
        let raw = try!(raw::Pipe::new(uv_loop, false));
        Ok(Pipe {
            write_access: Access::new(()),
            read_access: AccessTimeout::new(()),
            stream: Stream::new(raw, false),
            data: Arc::new(PipeData {
                home: home,
                handle: raw,
            })
        })
    }

    pub fn open(file: libc::c_int) -> UvResult<Pipe> {
        Pipe::open_on(&mut *try!(EventLoop::borrow()), file)
    }

    pub fn open_on(eloop: &mut EventLoop, file: libc::c_int) -> UvResult<Pipe> {
        let pipe = unsafe {
            try!(Pipe::new(&eloop.uv_loop(), eloop.make_handle()))
        };
        let mut handle = pipe.data.handle;
        try!(handle.open(file));
        Ok(pipe)
    }

    pub fn connect<T: ToCStr>(name: &T) -> UvResult<Pipe> {
        Pipe::connect_on(&mut *try!(EventLoop::borrow()), name.to_c_str(), None)
    }

    pub fn connect_timeout<T: ToCStr>(name: &T, timeout: Duration)
                                      -> UvResult<Pipe> {
        Pipe::connect_on(&mut *try!(EventLoop::borrow()), name.to_c_str(),
                         Some(timeout))
    }

    pub fn connect_on(eloop: &mut EventLoop, name: CString,
                      timeout: Option<Duration>) -> UvResult<Pipe> {
        let pipe = unsafe {
            try!(Pipe::new(&eloop.uv_loop(), eloop.make_handle()))
        };
        let cx = ConnectCtx::new();
        cx.connect(pipe, timeout, eloop, |mut req, pipe, cb| {
            req.pipe_connect(pipe.stream.handle, &name, cb);
            Ok(())
        })
    }

    /// Gain access to the underlying raw pipe object.
    ///
    /// This function is unsafe as there is no guarantee that any safe
    /// modifications to the pipe handle are actually safe to perform given the
    /// assumptions of this object.
    pub unsafe fn raw(&self) -> raw::Pipe { self.data.handle }

    pub fn uv_read(&mut self, buf: &mut [u8]) -> UvResult<uint> {
        let m = self.data.fire_homing_missile();
        let guard = try!(self.read_access.grant(m));

        // see comments in close_read about this check
        if guard.access.is_closed() {
            return Err(UvError(uvll::EOF))
        }

        self.stream.read(buf)
    }

    pub fn uv_write(&mut self, buf: &[u8]) -> UvResult<()> {
        let m = self.data.fire_homing_missile();
        let _guard = self.write_access.grant(0, m);
        self.stream.write(buf)
    }

    pub fn close_read(&mut self) -> UvResult<()> {
        // The current uv_shutdown method only shuts the writing half of the
        // connection, and no method is provided to shut down the reading half
        // of the connection. With a lack of method, we emulate shutting down
        // the reading half of the connection by manually returning early from
        // all future calls to `read`.
        //
        // Note that we must be careful to ensure that *all* cloned handles see
        // the closing of the read half, so we stored the "is closed" bit in the
        // Access struct, not in our own personal watcher. Additionally, the
        // homing missile is used as a locking mechanism to ensure there is no
        // contention over this bit.
        //
        // To shutdown the read half, we must first flag the access as being
        // closed, and then afterwards we cease any pending read. Note that this
        // ordering is crucial because we could in theory be rescheduled during
        // the uv_read_stop which means that another read invocation could leak
        // in before we set the flag.
        let task = {
            let m = self.data.fire_homing_missile();
            self.read_access.access().close(&m);
            Stream::cancel_read(self.stream.handle, uvll::EOF as libc::ssize_t)
        };
        let _ = task.map(|t| t.reawaken());
        Ok(())
    }

    pub fn close_write(&mut self) -> UvResult<()> {
        let _m = self.data.fire_homing_missile();
        net::shutdown(self.stream.handle)
    }

    pub fn set_read_timeout(&mut self, dur: Option<Duration>) {
        let _m = self.data.fire_homing_missile();
        let uv_loop = self.stream.handle.uv_loop();
        self.read_access.set_timeout(dur, uv_loop, cancel_read,
                                     self.stream.handle.raw() as uint);

        fn cancel_read(stream: uint) -> Option<BlockedTask> {
            let stream = stream as *mut uvll::uv_pipe_t;
            let raw: raw::Pipe = unsafe { Handle::from_raw(stream) };
            Stream::cancel_read(raw, uvll::ECANCELED as libc::ssize_t)
        }
    }
}

impl HomingIO for PipeData {
    fn home(&self) -> &HomeHandle { &self.home }
}

impl Clone for Pipe {
    fn clone(&self) -> Pipe {
        Pipe {
            read_access: self.read_access.clone(),
            write_access: self.write_access.clone(),
            stream: Stream::new(self.data.handle, false),
            data: self.data.clone(),
        }
    }
}

impl Reader for Pipe {
    fn read(&mut self, into: &mut [u8]) -> io::IoResult<uint> {
        self.uv_read(into).map_err(|e| e.to_io_error())
    }
}

impl Writer for Pipe {
    fn write(&mut self, buf: &[u8]) -> io::IoResult<()> {
        self.uv_write(buf).map_err(|e| e.to_io_error())
    }
}

impl Drop for PipeData {
    fn drop(&mut self) {
        let _m = self.fire_homing_missile();
        unsafe { self.handle.close_and_free(); }
    }
}

// PipeListener implementation and traits

impl PipeListener {
    pub fn bind<T: ToCStr>(name: &T) -> UvResult<PipeListener> {
        PipeListener::bind_on(&mut *try!(EventLoop::borrow()), name.to_c_str())
    }

    pub fn bind_on(eloop: &mut EventLoop,
                   name: CString) -> UvResult<PipeListener> {
        unsafe {
            let mut ret = PipeListener {
                handle: try!(raw::Pipe::new(&eloop.uv_loop(), false)),
                home: eloop.make_handle(),
            };
            try!(ret.handle.bind(name));
            Ok(ret)
        }
    }

    pub fn listen(self) -> UvResult<PipeAcceptor> {
        use raw::Stream;

        let _m = self.fire_homing_missile();

        // create the acceptor object from ourselves
        let timeout = AcceptTimeout::new();
        let acceptor = PipeAcceptor {
            data: Arc::new(AcceptorData {
                listener: self,
                pusher: timeout.pusher(),
            }),
            access: timeout,
        };
        let mut handle = acceptor.data.listener.handle;
        handle.set_data(&*acceptor.data as *const _ as *mut _);

        // FIXME: the 128 backlog should be configurable
        try!(handle.listen(128, listen_cb));
        Ok(acceptor)
    }

    /// Gain access to the underlying raw pipe object.
    ///
    /// This function is unsafe as there is no guarantee that any safe
    /// modifications to the pipe handle are actually safe to perform given the
    /// assumptions of this object.
    pub unsafe fn raw(&self) -> raw::Pipe { self.handle }
}

impl io::Listener<Pipe, PipeAcceptor> for PipeListener {
    fn listen(self) -> io::IoResult<PipeAcceptor> {
        self.listen().map_err(|e| e.to_io_error())
    }
}

impl HomingIO for PipeListener {
    fn home(&self) -> &HomeHandle { &self.home }
}

extern fn listen_cb(server: *mut uvll::uv_stream_t, status: libc::c_int) {
    assert!(status != uvll::ECANCELED);

    unsafe {
        let pipe: raw::Pipe = Handle::from_raw(server as *mut uvll::uv_pipe_t);
        let data: &AcceptorData = mem::transmute(pipe.get_data());
        let msg = match status {
            0 => accept(&data.listener),
            n => Err(UvError(n)),
        };

        // If we're running then we have exclusive access, so the unsafe_get()
        // is ok
        data.pusher.push(msg);
    }

    unsafe fn accept(listener: &PipeListener) -> UvResult<Pipe> {
        use raw::Stream;
        let mut handle = listener.handle;
        let client = try!(Pipe::new(&handle.uv_loop(), listener.home.clone()));
        try!(handle.accept(client.data.handle));
        Ok(client)
    }
}

impl Drop for PipeListener {
    fn drop(&mut self) {
        let _m = self.fire_homing_missile();
        unsafe { self.handle.close_and_free() }
    }
}

// PipeAcceptor implementation and traits

impl PipeAcceptor {
    pub fn accept(&mut self) -> UvResult<Pipe> {
        let m = self.fire_homing_missile();
        let uv_loop = self.data.listener.handle.uv_loop();
        self.access.accept(m, uv_loop)
    }

    pub fn set_timeout(&mut self, dur: Option<Duration>) {
        let _m = self.fire_homing_missile();
        let uv_loop = self.data.listener.handle.uv_loop();
        self.access.set_timeout(dur, uv_loop)
    }

    pub fn close_accept(&mut self) -> UvResult<()> {
        let m = self.fire_homing_missile();
        self.access.close(m);
        Ok(())
    }

    /// Gain access to the underlying raw pipe object.
    ///
    /// This function is unsafe as there is no guarantee that any safe
    /// modifications to the pipe handle are actually safe to perform given the
    /// assumptions of this object.
    pub unsafe fn raw(&self) -> raw::Pipe { self.data.listener.handle }
}

impl io::Acceptor<Pipe> for PipeAcceptor {
    fn accept(&mut self) -> io::IoResult<Pipe> {
        self.accept().map_err(|e| e.to_io_error())
    }
}

impl HomingIO for PipeAcceptor {
    fn home(&self) -> &HomeHandle { &self.data.listener.home }
}
