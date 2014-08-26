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
use std::mem;
use std::rt::task::BlockedTask;
use std::sync::Arc;
use std::time::Duration;
use libc;

use homing::{HomingIO, HomeHandle};
use access::Access;
use timeout::AccessTimeout;

use {raw, uvll, UvResult, UvError, EventLoop};
use raw::{Request, Handle};

pub struct Udp {
    data: Arc<Data>,

    // See tcp for what these fields are
    read_access: AccessTimeout<()>,
    write_access: Access<()>,
}

struct Data {
    handle: raw::Udp,
    home: HomeHandle,
}

struct UdpRecvCtx {
    task: Option<BlockedTask>,
    buf: Option<uvll::uv_buf_t>,
    result: Option<(libc::ssize_t, Option<ip::SocketAddr>)>,
}

struct UdpSendCtx {
    result: libc::c_int,
    task: Option<BlockedTask>,
}


impl Udp {
    pub fn bind(addr: ip::SocketAddr) -> UvResult<Udp> {
        Udp::bind_on(&mut *try!(EventLoop::borrow()), addr)
    }

    pub fn bind_on(eloop: &mut EventLoop, addr: ip::SocketAddr)
                   -> UvResult<Udp> {
        let mut udp = Data {
            home: eloop.make_handle(),
            handle: unsafe { try!(raw::Udp::new(&eloop.uv_loop())) }
        };
        try!(udp.handle.bind(addr));
        Ok(Udp {
            data: Arc::new(udp),
            read_access: AccessTimeout::new(()),
            write_access: Access::new(()),
        })
    }

    pub fn socket_name(&mut self) -> UvResult<ip::SocketAddr> {
        let _m = self.data.fire_homing_missile();
        let mut handle = self.data.handle;
        handle.getsockname()
    }

    pub fn recv_from(&mut self, buf: &mut [u8])
                     -> UvResult<(uint, ip::SocketAddr)> {
        let m = self.data.fire_homing_missile();
        let _guard = try!(self.read_access.grant(m));
        let mut handle = self.data.handle;
        let mut cx = UdpRecvCtx {
            task: None,
            buf: Some(raw::slice_to_uv_buf(buf)),
            result: None,
        };

        try!(handle.recv_start(alloc_cb, recv_cb));
        handle.set_data(&mut cx as *mut _ as *mut _);
        ::block(handle.uv_loop(), |task| {
            cx.task = Some(task);
        });
        handle.set_data(0 as *mut _);

        return match cx.result.take().unwrap() {
            (n, _) if n < 0 => Err(UvError(n as libc::c_int)),
            (n, addr) => Ok((n as uint, addr.unwrap()))
        };

        extern fn alloc_cb(handle: *mut uvll::uv_handle_t,
                           _suggested_size: libc::size_t,
                           buf: *mut uvll::uv_buf_t) {
            unsafe {
                let handle = handle as *mut uvll::uv_udp_t;
                let raw: raw::Udp = Handle::from_raw(handle);
                let cx: &mut UdpRecvCtx = mem::transmute(raw.get_data());
                *buf = cx.buf.take().expect("recv alloc_cb called more than once")
            }
        }

        extern fn recv_cb(handle: *mut uvll::uv_udp_t, nread: libc::ssize_t,
                          buf: *const uvll::uv_buf_t,
                          addr: *const libc::sockaddr, _flags: libc::c_uint) {
            assert!(nread != uvll::ECANCELED as libc::ssize_t);

            unsafe {
                let mut raw: raw::Udp = Handle::from_raw(handle);
                let cx: &mut UdpRecvCtx = mem::transmute(raw.get_data());

                // When there's no data to read the recv callback can be a
                // no-op.  This can happen if read returns EAGAIN/EWOULDBLOCK.
                // By ignoring this we just drop back to kqueue and wait for the
                // next callback.
                if nread == 0 {
                    cx.buf = Some(*buf);
                    return
                }

                raw.recv_stop().unwrap();
                let addr = if addr.is_null() {
                    None
                } else {
                    let len = mem::size_of::<libc::sockaddr_storage>();
                    Some(raw::sockaddr_to_addr(mem::transmute(addr), len))
                };
                cx.result = Some((nread, addr));
                ::wakeup(&mut cx.task);
            }
        }
    }

    pub fn send_to(&mut self, buf: &[u8], dst: ip::SocketAddr) -> UvResult<()> {
        let m = self.data.fire_homing_missile();
        let _guard = self.write_access.grant(0, m);
        let mut cx = UdpSendCtx {
            result: uvll::ECANCELED,
            task: None,
        };

        unsafe {
            let mut req: raw::UdpSend = Request::alloc();
            req.set_data(&mut cx as *mut _ as *mut _);
            match req.send(self.data.handle, buf, dst, send_cb) {
                Ok(()) => {}
                Err(e) => { req.free(); return Err(e) }
            }
            ::block(self.data.handle.uv_loop(), |task| {
                cx.task = Some(task);
            });
        }
        return if cx.result < 0 {Err(UvError(cx.result))} else {Ok(())};

        extern fn send_cb(req: *mut uvll::uv_udp_send_t, status: libc::c_int) {
            unsafe {
                let mut req: raw::UdpSend = Request::from_raw(req);
                let cx: &mut UdpSendCtx = mem::transmute(req.get_data());
                cx.result = status;
                ::wakeup(&mut cx.task);
                req.free();
            }
        }
    }

    pub fn join_multicast(&mut self, multi: ip::IpAddr) -> UvResult<()> {
        let _m = self.data.fire_homing_missile();
        let mut handle = self.data.handle;
        handle.set_membership(multi, uvll::UV_JOIN_GROUP)
    }

    pub fn leave_multicast(&mut self, multi: ip::IpAddr) -> UvResult<()> {
        let _m = self.data.fire_homing_missile();
        let mut handle = self.data.handle;
        handle.set_membership(multi, uvll::UV_LEAVE_GROUP)
    }

    pub fn multicast_locally(&mut self, enable: bool) -> UvResult<()> {
        let _m = self.data.fire_homing_missile();
        let mut handle = self.data.handle;
        handle.set_multicast_loop(enable)
    }

    pub fn multicast_time_to_live(&mut self, ttl: int) -> UvResult<()> {
        let _m = self.data.fire_homing_missile();
        let mut handle = self.data.handle;
        handle.set_multicast_ttl(ttl)
    }

    pub fn time_to_live(&mut self, ttl: int) -> UvResult<()> {
        let _m = self.data.fire_homing_missile();
        let mut handle = self.data.handle;
        handle.set_ttl(ttl)
    }

    pub fn broadcast(&mut self, enable: bool) -> UvResult<()> {
        let _m = self.data.fire_homing_missile();
        let mut handle = self.data.handle;
        handle.set_broadcast(enable)
    }

    pub fn set_read_timeout(&mut self, dur: Option<Duration>) {
        let _m = self.data.fire_homing_missile();
        self.read_access.set_timeout(dur, self.data.handle.uv_loop(),
                                     cancel_read,
                                     self.data.handle.raw() as uint);

        fn cancel_read(stream: uint) -> Option<BlockedTask> {
            // This method is quite similar to StreamWatcher::cancel_read, see
            // there for more information
            unsafe {
                let handle = stream as *mut uvll::uv_udp_t;
                let mut raw: raw::Udp = Handle::from_raw(handle);
                raw.recv_stop().unwrap();
                if raw.get_data().is_null() { return None }

                let cx: &mut UdpRecvCtx = mem::transmute(raw.get_data());
                cx.result = Some((uvll::ECANCELED as libc::ssize_t, None));
                cx.task.take()
            }
        }
    }
}

impl Clone for Udp {
    fn clone(&self) -> Udp {
        Udp {
            read_access: self.read_access.clone(),
            write_access: self.write_access.clone(),
            data: self.data.clone(),
        }
    }
}

impl HomingIO for Data {
    fn home(&self) -> &HomeHandle { &self.home }
}

impl Drop for Data {
    fn drop(&mut self) {
        let _m = self.fire_homing_missile();
        unsafe { self.handle.close_and_free(); }
    }
}
