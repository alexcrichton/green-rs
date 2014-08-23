// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::rt::heap;
use libc;

use uvll;

pub use self::loop_::Loop;
pub use self::idle::Idle;

macro_rules! call( ($e:expr) => (
    match $e {
        n if n < 0 => Err(::UvError(n)),
        n => Ok(n),
    }
) )

mod loop_;
mod idle;

pub trait Allocated {
    fn size(_self: Option<Self>) -> uint;
}

pub struct Raw<T> {
    ptr: *mut T,
}

// FIXME: this T should be an associated type
pub trait Handle<T>: Allocated {
    fn raw(&self) -> *mut T;
    fn from_raw(t: *mut T) -> Self;

    unsafe fn alloc(_self: Option<Self>) -> *mut T {
        let uv_handle_type = Handle::uv_handle_type(None::<Self>);
        let size = uvll::uv_handle_size(uv_handle_type);
        heap::allocate(size as uint, 8) as *mut T
    }

    unsafe fn free(_self: Option<Self>, ptr: *mut T) {
        let uv_handle_type = Handle::uv_handle_type(None::<Self>);
        let size = uvll::uv_handle_size(uv_handle_type);
        heap::deallocate(ptr as *mut u8, size as uint, 8)
    }

    fn uv_loop(&self) -> Loop {
        unsafe {
            Loop::wrap(uvll::rust_uv_get_loop_for_uv_handle(self.raw() as *mut _))
        }
    }

    fn get_data(&self) -> *mut libc::c_void {
        unsafe { uvll::rust_uv_get_data_for_uv_handle(self.raw() as *mut _) }
    }
    fn set_data(&mut self, data: *mut libc::c_void) {
        unsafe {
            uvll::rust_uv_set_data_for_uv_handle(self.raw() as *mut _, data)
        }
    }
    // // FIXME(#8888) dummy self
    // fn alloc(_: Option<Self>, ty: uvll::uv_handle_type) -> *mut T {
    //     unsafe {
    //         let handle = uvll::malloc_handle(ty);
    //         assert!(!handle.is_null());
    //         handle as *mut T
    //     }
    // }

    // unsafe fn from_uv_handle<'a>(h: &'a *mut T) -> &'a mut Self {
    //     mem::transmute(uvll::get_data_for_uv_handle(*h))
    // }
    //
    // fn install(self: Box<Self>) -> Box<Self> {
    //     unsafe {
    //         let myptr = mem::transmute::<&Box<Self>, &*mut u8>(&self);
    //         uvll::set_data_for_uv_handle(self.uv_handle(), *myptr);
    //     }
    //     self
    // }
    //
    // fn close_async_(&mut self) {
    //     // we used malloc to allocate all handles, so we must always have at
    //     // least a callback to free all the handles we allocated.
    //     extern fn close_cb(handle: *mut uvll::uv_handle_t) {
    //         unsafe { uvll::free_handle(handle) }
    //     }
    //
    //     unsafe {
    //         uvll::set_data_for_uv_handle(self.uv_handle(), ptr::mut_null::<()>());
    //         uvll::uv_close(self.uv_handle() as *mut uvll::uv_handle_t, close_cb)
    //     }
    // }
    //
    // fn close(&mut self) {
    //     let mut slot = None;
    //
    //     unsafe {
    //         uvll::uv_close(self.uv_handle() as *mut uvll::uv_handle_t, close_cb);
    //         uvll::set_data_for_uv_handle(self.uv_handle(),
    //                                      ptr::mut_null::<()>());
    //
    //         wait_until_woken_after(&mut slot, &self.uv_loop(), || {
    //             uvll::set_data_for_uv_handle(self.uv_handle(), &mut slot);
    //         })
    //     }
    //
    //     extern fn close_cb(handle: *mut uvll::uv_handle_t) {
    //         unsafe {
    //             let data = uvll::get_data_for_uv_handle(handle);
    //             uvll::free_handle(handle);
    //             if data == ptr::mut_null() { return }
    //             let slot: &mut Option<BlockedTask> = mem::transmute(data);
    //             wakeup(slot);
    //         }
    //     }
    // }
}


impl<T: Allocated> Raw<T> {
}
