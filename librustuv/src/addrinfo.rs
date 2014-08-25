// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::io::net::ip::IpAddr;
use std::mem;
use std::rt::task::BlockedTask;
use libc::c_int;
use libc;

use {uvll, raw, UvResult, UvError, EventLoop};
use raw::Request;

struct Data {
    blocker: Option<BlockedTask>,
    status: libc::c_int,
    addrinfo: Option<AddrInfo>,
}

struct AddrInfo {
    handle: *const libc::addrinfo,
}

struct GetAddrInfo {
    handle: raw::GetAddrInfo,
}

/// Synchronous DNS resolution
///
/// See [`std::io::net::get_host_addresses`][1]
///
/// [1]: http://doc.rust-lang.org/std/io/net/addrinfo/fn.get_host_addresses.html
pub fn get_host_addresses(host: &str) -> UvResult<Vec<IpAddr>> {
    let mut eloop = try!(EventLoop::borrow());
    get_host_addresses_on(&mut *eloop, host)
}

/// Same as `get_host_addresses`, but specifies what event loop to run on.
pub fn get_host_addresses_on(eloop: &mut EventLoop,
                             host: &str) -> UvResult<Vec<IpAddr>> {
    let mut req = unsafe { GetAddrInfo { handle: Request::alloc() } };
    let mut data = Data {
        blocker: None,
        status: 0,
        addrinfo: None,
    };
    req.handle.set_data(&mut data as *mut _ as *mut _);
    unsafe {
        try!(req.handle.send(&eloop.uv_loop(), Some(host), None, callback));
        ::block(eloop.uv_loop(), |task| {
            data.blocker = Some(task);
        });
    }

    if data.status < 0 { return Err(UvError(data.status)) }

    let addrinfo = data.addrinfo.unwrap();
    unsafe {
        let mut addr = addrinfo.handle;

        let mut addrs = Vec::new();
        loop {
            let rustaddr = raw::sockaddr_to_addr(mem::transmute((*addr).ai_addr),
                                                 (*addr).ai_addrlen as uint);

            addrs.push(rustaddr.ip);
            if (*addr).ai_next.is_not_null() {
                addr = (*addr).ai_next as *const _;
            } else {
                break;
            }
        }

        Ok(addrs)
    }
}

extern fn callback(req: *mut uvll::uv_getaddrinfo_t,
                   status: libc::c_int,
                   res: *const libc::addrinfo) {
    assert!(status != uvll::ECANCELED);

    let req: raw::GetAddrInfo = unsafe { Request::from_raw(req) };
    let data: &mut Data = unsafe { mem::transmute(req.get_data()) };
    data.status = status;
    data.addrinfo = Some(AddrInfo { handle: res });

    ::wakeup(&mut data.blocker);
}

impl Drop for AddrInfo {
    fn drop(&mut self) {
        unsafe { uvll::uv_freeaddrinfo(self.handle as *mut _) }
    }
}

impl Drop for GetAddrInfo {
    fn drop(&mut self) {
        unsafe { self.handle.free() }
    }
}
