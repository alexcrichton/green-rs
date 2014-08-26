// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::io::net::ip;
use std::io;
use std::mem;
use std::rt::task::BlockedTask;
use std::sync::Arc;
use std::time::Duration;
use libc;

use access::Access;
use homing::{HomingIO, HomeHandle};
use raw::{Handle, Request};
use stream::Stream;
use timeout::{Pusher, AcceptTimeout, ConnectCtx, AccessTimeout};
use {raw, uvll, EventLoop, UvResult, UvError};

pub struct Tcp {
    data: Arc<TcpData>,
    stream: Stream<raw::Tcp>,

    // libuv can't support concurrent reads and concurrent writes of the same
    // stream object, so we use these access guards in order to arbitrate among
    // multiple concurrent reads and writes. Note that libuv *can* read and
    // write simultaneously, it just can't read and read simultaneously.
    write_access: Access<()>,
    read_access: AccessTimeout<()>,
}

struct TcpData {
    handle: raw::Tcp,
    home: HomeHandle,
}

pub struct TcpListener {
    handle: raw::Tcp,
    home: HomeHandle,
}

#[deriving(Clone)]
pub struct TcpAcceptor {
    data: Arc<AcceptorData>,
    access: AcceptTimeout<Tcp>,
}

struct AcceptorData {
    listener: TcpListener,
    pusher: Pusher<Tcp>,
}

// Tcp implementation and traits

impl Tcp {
    // Creates an uninitialized tcp watcher. The underlying uv tcp is ready to
    // get bound to some other source (this is normally a helper method paired
    // with another call).
    unsafe fn new(uv_loop: &raw::Loop, home: HomeHandle) -> UvResult<Tcp> {
        let raw = try!(raw::Tcp::new(uv_loop));
        Ok(Tcp {
            write_access: Access::new(()),
            read_access: AccessTimeout::new(()),
            stream: Stream::new(raw, true),
            data: Arc::new(TcpData {
                home: home,
                handle: raw,
            })
        })
    }

    pub fn open(file: libc::c_int) -> UvResult<Tcp> {
        Tcp::open_on(&mut *try!(EventLoop::borrow()), file)
    }

    pub fn open_on(eloop: &mut EventLoop, file: libc::c_int) -> UvResult<Tcp> {
        let tcp = unsafe {
            try!(Tcp::new(&eloop.uv_loop(), eloop.make_handle()))
        };
        let mut handle = tcp.data.handle;
        try!(handle.open(file));
        Ok(tcp)
    }

    pub fn connect(addr: ip::SocketAddr) -> UvResult<Tcp> {
        Tcp::connect_on(&mut *try!(EventLoop::borrow()), addr, None)
    }

    pub fn connect_timeout(addr: ip::SocketAddr, timeout: Duration)
                           -> UvResult<Tcp> {
        Tcp::connect_on(&mut *try!(EventLoop::borrow()), addr, Some(timeout))
    }

    pub fn connect_on(eloop: &mut EventLoop, addr: ip::SocketAddr,
                      timeout: Option<Duration>) -> UvResult<Tcp> {
        let tcp = unsafe {
            try!(Tcp::new(&eloop.uv_loop(), eloop.make_handle()))
        };
        let cx = ConnectCtx::new();
        cx.connect(tcp, timeout, eloop, |mut req, tcp, cb| {
            req.tcp_connect(tcp.stream.handle, addr, cb)
        })
    }

    /// Gain access to the underlying raw tcp object.
    ///
    /// This function is unsafe as there is no guarantee that any safe
    /// modifications to the tcp handle are actually safe to perform given the
    /// assumptions of this object.
    pub unsafe fn raw(&self) -> raw::Tcp { self.data.handle }

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
        // See comments in Pipe::close_read
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
        shutdown(self.stream.handle)
    }

    pub fn set_read_timeout(&mut self, dur: Option<Duration>) {
        let _m = self.data.fire_homing_missile();
        let uv_loop = self.stream.handle.uv_loop();
        self.read_access.set_timeout(dur, uv_loop, cancel_read,
                                     self.stream.handle.raw() as uint);

        fn cancel_read(stream: uint) -> Option<BlockedTask> {
            let stream = stream as *mut uvll::uv_tcp_t;
            let raw: raw::Tcp = unsafe { Handle::from_raw(stream) };
            Stream::cancel_read(raw, uvll::ECANCELED as libc::ssize_t)
        }
    }

    pub fn socket_name(&mut self) -> UvResult<ip::SocketAddr> {
        let _m = self.data.fire_homing_missile();
        self.stream.handle.getsockname()
    }

    pub fn peer_name(&mut self) -> UvResult<ip::SocketAddr> {
        let _m = self.data.fire_homing_missile();
        self.stream.handle.getpeername()
    }

    pub fn nodelay(&mut self, enabled: bool) -> UvResult<()> {
        let _m = self.data.fire_homing_missile();
        self.stream.handle.nodelay(enabled)
    }

    pub fn keepalive(&mut self, ttl: Option<uint>) -> UvResult<()> {
        let _m = self.data.fire_homing_missile();
        self.stream.handle.keepalive(ttl)
    }
}

impl HomingIO for TcpData {
    fn home(&self) -> &HomeHandle { &self.home }
}

impl Clone for Tcp {
    fn clone(&self) -> Tcp {
        Tcp {
            read_access: self.read_access.clone(),
            write_access: self.write_access.clone(),
            stream: Stream::new(self.data.handle, false),
            data: self.data.clone(),
        }
    }
}

impl Reader for Tcp {
    fn read(&mut self, into: &mut [u8]) -> io::IoResult<uint> {
        self.uv_read(into).map_err(|e| e.to_io_error())
    }
}

impl Writer for Tcp {
    fn write(&mut self, buf: &[u8]) -> io::IoResult<()> {
        self.uv_write(buf).map_err(|e| e.to_io_error())
    }
}

impl Drop for TcpData {
    fn drop(&mut self) {
        let _m = self.fire_homing_missile();
        unsafe { self.handle.close_and_free(); }
    }
}

// TcpListener implementation and traits

impl TcpListener {
    pub fn bind(addr: ip::SocketAddr) -> UvResult<TcpListener> {
        TcpListener::bind_on(&mut *try!(EventLoop::borrow()), addr)
    }

    pub fn bind_on(eloop: &mut EventLoop,
                   addr: ip::SocketAddr) -> UvResult<TcpListener> {
        unsafe {
            let mut ret = TcpListener {
                handle: try!(raw::Tcp::new(&eloop.uv_loop())),
                home: eloop.make_handle(),
            };
            try!(ret.handle.bind(addr));
            Ok(ret)
        }
    }

    pub fn listen(self) -> UvResult<TcpAcceptor> {
        use raw::Stream;

        let _m = self.fire_homing_missile();

        // create the acceptor object from ourselves
        let timeout = AcceptTimeout::new();
        let acceptor = TcpAcceptor {
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

    /// Gain access to the underlying raw tcp object.
    ///
    /// This function is unsafe as there is no guarantee that any safe
    /// modifications to the tcp handle are actually safe to perform given the
    /// assumptions of this object.
    pub unsafe fn raw(&self) -> raw::Tcp { self.handle }

    pub fn socket_name(&mut self) -> UvResult<ip::SocketAddr> {
        let _m = self.fire_homing_missile();
        self.handle.getsockname()
    }
}

impl io::Listener<Tcp, TcpAcceptor> for TcpListener {
    fn listen(self) -> io::IoResult<TcpAcceptor> {
        self.listen().map_err(|e| e.to_io_error())
    }
}

impl HomingIO for TcpListener {
    fn home(&self) -> &HomeHandle { &self.home }
}

extern fn listen_cb(server: *mut uvll::uv_stream_t, status: libc::c_int) {
    assert!(status != uvll::ECANCELED);

    unsafe {
        let tcp: raw::Tcp = Handle::from_raw(server as *mut uvll::uv_tcp_t);
        let data: &AcceptorData = mem::transmute(tcp.get_data());
        let msg = match status {
            0 => accept(&data.listener),
            n => Err(UvError(n)),
        };

        // If we're running then we have exclusive access, so the unsafe_get()
        // is ok
        data.pusher.push(msg);
    }

    unsafe fn accept(listener: &TcpListener) -> UvResult<Tcp> {
        use raw::Stream;
        let mut handle = listener.handle;
        let client = try!(Tcp::new(&handle.uv_loop(), listener.home.clone()));
        try!(handle.accept(client.data.handle));
        Ok(client)
    }
}

impl Drop for TcpListener {
    fn drop(&mut self) {
        let _m = self.fire_homing_missile();
        unsafe { self.handle.close_and_free() }
    }
}

// TcpAcceptor implementation and traits

impl TcpAcceptor {
    pub fn accept(&mut self) -> UvResult<Tcp> {
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

    /// Gain access to the underlying raw tcp object.
    ///
    /// This function is unsafe as there is no guarantee that any safe
    /// modifications to the tcp handle are actually safe to perform given the
    /// assumptions of this object.
    pub unsafe fn raw(&self) -> raw::Tcp { self.data.listener.handle }
}

impl io::Acceptor<Tcp> for TcpAcceptor {
    fn accept(&mut self) -> io::IoResult<Tcp> {
        self.accept().map_err(|e| e.to_io_error())
    }
}

impl HomingIO for TcpAcceptor {
    fn home(&self) -> &HomeHandle { &self.data.listener.home }
}

////////////////////////////////////////////////////////////////////////////////
// Shutdown helper
////////////////////////////////////////////////////////////////////////////////

pub fn shutdown<T, U>(mut handle: U) -> UvResult<()>
                      where T: raw::Allocated, U: raw::Stream<T> {
    struct Ctx {
        slot: Option<BlockedTask>,
        status: libc::c_int,
    }
    unsafe {
        let mut req: raw::Shutdown = raw::Request::alloc();
        let mut cx = Ctx { slot: None, status: 0 };
        req.set_data(&mut cx as *mut _ as *mut _);

        let ret = match req.send(&mut handle, shutdown_cb) {
            Ok(()) => {
                ::block(handle.uv_loop(), |task| {
                    cx.slot = Some(task);
                });
                if cx.status < 0 {Err(UvError(cx.status))} else {Ok(())}
            }
            Err(e) => Err(e),
        };
        req.free();
        return ret;
    }

    extern fn shutdown_cb(req: *mut uvll::uv_shutdown_t, status: libc::c_int) {
        unsafe {
            assert!(status != uvll::ECANCELED);
            let req: raw::Shutdown = raw::Request::from_raw(req);
            let cx: &mut Ctx = mem::transmute(req.get_data());
            cx.status = status;
            ::wakeup(&mut cx.slot);
        }
    }
}
