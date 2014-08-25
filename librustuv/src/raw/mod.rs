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

use {uvll, UvResult};

pub use self::async::Async;
pub use self::getaddrinfo::GetAddrInfo;
pub use self::idle::Idle;
pub use self::loop_::Loop;
pub use self::timer::Timer;

macro_rules! call( ($e:expr) => (
    match $e {
        n if n < 0 => Err(::UvError(n)),
        n => Ok(n),
    }
) )

mod async;
mod getaddrinfo;
mod idle;
mod loop_;
mod timer;

pub trait Allocated {
    fn size(_self: Option<Self>) -> uint;
}

pub struct Raw<T> {
    ptr: *mut T,
}

// FIXME: this T should be an associated type
pub trait Handle<T: Allocated> {
    fn raw(&self) -> *mut T;
    unsafe fn from_raw(t: *mut T) -> Self;

    fn uv_loop(&self) -> Loop {
        unsafe {
            let loop_ = uvll::rust_uv_get_loop_for_uv_handle(self.raw() as *mut _);
            Loop::from_raw(loop_)
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

    /// Invokes uv_close
    ///
    /// This is unsafe as there is no guarantee that this handle is not actively
    /// being used by other objects.
    unsafe fn close(&mut self, thunk: Option<uvll::uv_close_cb>) {
        uvll::uv_close(self.raw() as *mut _, thunk)
    }

    /// Deallocate this handle.
    ///
    /// This is unsafe as there is no guarantee that no one else is using this
    /// handle currently.
    unsafe fn free(&mut self) { drop(Raw::wrap(self.raw())) }

    /// Invoke uv_close, and then free the handle when the close operation is
    /// done.
    ///
    /// This is unsafe for the same reasons as `close` and `free`.
    unsafe fn close_and_free(&mut self) {
        extern fn done<T: Allocated>(t: *mut uvll::uv_handle_t) {
            unsafe { drop(Raw::wrap(t as *mut T)) }
        }
        self.close(Some(done::<T>))
    }

    fn uv_ref(&self) { unsafe { uvll::uv_ref(self.raw() as *mut _) } }
    fn uv_unref(&self) { unsafe { uvll::uv_unref(self.raw() as *mut _) } }
}

// FIXME: this T should be an associated type
pub trait Request<T: Allocated> {
    fn raw(&self) -> *mut T;
    unsafe fn from_raw(t: *mut T) -> Self;

    fn get_data(&self) -> *mut libc::c_void {
        unsafe { uvll::rust_uv_get_data_for_req(self.raw() as *mut _) }
    }

    fn set_data(&mut self, data: *mut libc::c_void) {
        unsafe {
            uvll::rust_uv_set_data_for_req(self.raw() as *mut _, data)
        }
    }

    /// Allocate a new uninitialized request.
    ///
    /// This function is unsafe as there is no scheduled destructor for the
    /// returned value.
    unsafe fn alloc() -> Self {
        Request::from_raw(Raw::<T>::new().unwrap())
    }

    /// Invokes uv_close
    ///
    /// This is unsafe as there is no guarantee that this handle is not actively
    /// being used by other objects.
    fn cancel(&mut self) -> UvResult<()> {
        unsafe { try!(call!(uvll::uv_cancel(self.raw() as *mut _))); }
        Ok(())
    }

    /// Deallocate this handle.
    ///
    /// This is unsafe as there is no guarantee that no one else is using this
    /// handle currently.
    unsafe fn free(&mut self) { drop(Raw::wrap(self.raw())) }
}

impl<T: Allocated> Raw<T> {
    /// Allocates a new instance of the underlying pointer.
    fn new() -> Raw<T> {
        let size = Allocated::size(None::<T>);
        unsafe {
            Raw { ptr: heap::allocate(size as uint, 8) as *mut T }
        }
    }

    /// Wrap a pointer, scheduling it for deallocation when the returned value
    /// goes out of scope.
    unsafe fn wrap(ptr: *mut T) -> Raw<T> { Raw { ptr: ptr } }

    fn get(&self) -> *mut T { self.ptr }

    /// Unwrap this raw pointer, cancelling its deallocation.
    ///
    /// This method is unsafe because it will leak the returned pointer.
    unsafe fn unwrap(mut self) -> *mut T {
        let ret = self.ptr;
        self.ptr = 0 as *mut T;
        return ret;
    }
}

#[unsafe_destructor]
impl<T: Allocated> Drop for Raw<T> {
    fn drop(&mut self) {
        if self.ptr.is_null() { return }

        let size = Allocated::size(None::<T>);
        unsafe {
            heap::deallocate(self.ptr as *mut u8, size as uint, 8)
        }
    }
}
