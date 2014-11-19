// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::mem;
use std::rt::task::BlockedTask;
use std::time::Duration;
use libc;

use homing::HomingMissile;
use {access, uvll, raw, UvError, UvResult, EventLoop};
use raw::{Handle, Request};

/// Management of a timeout when gaining access to a portion of a duplex stream.
pub struct AccessTimeout<T> {
    inner: Box<Inner<T>>, // stored in a box to get a stable address
}

struct Inner<T> {
    state: State,
    timer: Option<raw::Timer>,
    user_unblock: Option<fn(uint) -> Option<BlockedTask>>,
    user_payload: uint,
    access: access::Access<T>,
}

pub struct Guard<'a, T: 'a> {
    state: &'a mut State,
    pub access: access::Guard<'a, T>,
    pub can_timeout: bool,
}

#[deriving(PartialEq)]
enum State {
    NoTimeout,
    TimeoutPending(Client),
    TimedOut,
}

#[deriving(PartialEq)]
enum Client {
    NoWaiter,
    AccessPending,
    RequestPending,
}

impl<T: Send> AccessTimeout<T> {
    pub fn new(data: T) -> AccessTimeout<T> {
        AccessTimeout {
            inner: box Inner {
                state: State::NoTimeout,
                timer: None,
                user_unblock: None,
                user_payload: 0,
                access: access::Access::new(data),
            },
        }
    }

    /// Grants access to half of a duplex stream, timing out if necessary.
    ///
    /// On success, Ok(Guard) is returned and access has been granted to the
    /// stream. If a timeout occurs, then Err is returned with an appropriate
    /// error.
    pub fn grant<'a>(&'a mut self, m: HomingMissile) -> UvResult<Guard<'a, T>> {
        // First, flag that we're attempting to acquire access. This will allow
        // us to cancel the pending grant if we timeout out while waiting for a
        // grant.
        let inner = &mut *self.inner;
        match inner.state {
            State::NoTimeout => {},
            State::TimeoutPending(ref mut client) => {
                *client = Client::AccessPending;
            }
            State::TimedOut => return Err(UvError(uvll::ECANCELED))
        }
        let access = inner.access.grant(inner as *mut _ as uint, m);

        // After acquiring the grant, we need to flag ourselves as having a
        // pending request so the timeout knows to cancel the request.
        let can_timeout = match inner.state {
            State::NoTimeout => false,
            State::TimeoutPending(ref mut client) => {
                *client = Client::RequestPending; true
            }
            State::TimedOut => return Err(UvError(uvll::ECANCELED))
        };

        Ok(Guard {
            access: access,
            state: &mut inner.state,
            can_timeout: can_timeout
        })
    }

    pub fn timed_out(&self) -> bool {
        match self.inner.state {
            State::TimedOut => true,
            _ => false,
        }
    }

    pub fn access(&mut self) -> &mut access::Access<T> { &mut self.inner.access }

    /// Sets the pending timeout to the value specified.
    ///
    /// The home/loop variables are used to construct a timer if one has not
    /// been previously constructed.
    ///
    /// The callback will be invoked if the timeout elapses, and the data of
    /// the time will be set to `data`.
    pub fn set_timeout(&mut self, dur: Option<Duration>,
                       uv_loop: raw::Loop,
                       cb: fn(uint) -> Option<BlockedTask>,
                       data: uint) {
        self.inner.state = State::NoTimeout;
        let ms = match dur {
            Some(dur) if dur.num_milliseconds() < 0 => 0,
            Some(dur) => dur.num_milliseconds() as u64,
            None => return match self.inner.timer {
                Some(ref mut t) => t.stop().unwrap(),
                None => {}
            }
        };

        // If we have a timeout, lazily initialize the timer which will be used
        // to fire when the timeout runs out.
        if self.inner.timer.is_none() {
            let mut timer = unsafe { raw::Timer::new(&uv_loop).unwrap() };
            timer.set_data(&*self.inner as *const _ as *mut _);
            self.inner.timer = Some(timer);
        }

        // Update our local state and timer with the appropriate information for
        // the new timeout.
        self.inner.user_unblock = Some(cb);
        self.inner.user_payload = data;
        self.inner.state = State::TimeoutPending(Client::NoWaiter);
        let timer = self.inner.timer.as_mut().unwrap();
        timer.stop().unwrap();
        timer.start(ms, 0, timer_cb::<T>).unwrap();

        // When the timeout fires, we expect a TimeoutPending message and we
        // take an appropriate action depending on what state any waiter is in.
        extern fn timer_cb<T: Send>(timer: *mut uvll::uv_timer_t) {
            unsafe {
                let timer: raw::Timer = Handle::from_raw(timer);
                let inner: &mut Inner<T> = mem::transmute(timer.get_data());
                match mem::replace(&mut inner.state, State::TimedOut) {
                    State::TimedOut | State::NoTimeout => unreachable!(),
                    State::TimeoutPending(Client::NoWaiter) => {}
                    State::TimeoutPending(Client::AccessPending) => {
                        match inner.access.dequeue(inner as *mut _ as uint) {
                            Some(task) => task.reawaken(),
                            None => unreachable!(),
                        }
                    }
                    State::TimeoutPending(Client::RequestPending) => {
                        match (inner.user_unblock.unwrap())(inner.user_payload) {
                            Some(task) => task.reawaken(),
                            None => unreachable!(),
                        }
                    }
                }
            }
        }
    }
}

impl<T: Send> Clone for AccessTimeout<T> {
    fn clone(&self) -> AccessTimeout<T> {
        AccessTimeout {
            inner: box Inner {
                access: self.inner.access.clone(),
                state: State::NoTimeout,
                timer: None,
                user_unblock: None,
                user_payload: 0,
            },
        }
    }
}

#[unsafe_destructor]
impl<'a, T> Drop for Guard<'a, T> {
    fn drop(&mut self) {
        match *self.state {
            State::TimeoutPending(Client::NoWaiter) |
            State::TimeoutPending(Client::AccessPending) => unreachable!(),

            State::NoTimeout | State::TimedOut => {}
            State::TimeoutPending(Client::RequestPending) => {
                *self.state = State::TimeoutPending(Client::NoWaiter);
            }
        }
    }
}

#[unsafe_destructor]
impl<T> Drop for AccessTimeout<T> {
    fn drop(&mut self) {
        match self.inner.timer {
            Some(ref mut timer) => unsafe {
                timer.close_and_free();
            },
            None => {}
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// Connect timeouts
////////////////////////////////////////////////////////////////////////////////

pub struct ConnectCtx {
    status: libc::c_int,
    task: Option<BlockedTask>,
    timer: Option<raw::Timer>,
}

impl ConnectCtx {
    pub fn new() -> ConnectCtx {
        ConnectCtx { status: -1, task: None, timer: None }
    }

    pub fn connect<T>(mut self, obj: T, timeout: Option<Duration>,
                      io: &mut EventLoop,
                      f: |raw::Connect, &T, uvll::uv_connect_cb| -> UvResult<()>)
                      -> UvResult<T> {
        // Issue the connect request
        let mut req = unsafe { Request::alloc() };
        match f(req, &obj, connect_cb) {
            Ok(()) => {}
            Err(e) => unsafe { req.free(); return Err(e) },
        }
        req.set_data(&self as *const _ as *mut _);

        // Apply any timeout by scheduling a timer to fire when the timeout
        // expires which will wake up the task.
        match timeout {
            Some(t) => unsafe {
                let t = t.num_milliseconds();
                if t <= 0 { return Err(UvError(uvll::ECANCELED)) }

                let mut timer = raw::Timer::new(&io.uv_loop()).unwrap();
                timer.start(t as u64, 0, timer_cb).unwrap();
                timer.set_data(&self as *const _ as *mut _);
                self.timer = Some(timer);
            },
            None => {}
        }

        // Wait for some callback to fire.
        unsafe {
            ::block(io.uv_loop(), |task| {
                self.task = Some(task);
            });
        }

        // Make sure an erroneously fired callback doesn't have access
        // to the context any more.
        req.set_data(0 as *mut _);
        match self.timer {
            Some(ref mut t) => unsafe { t.close_and_free() },
            None => {}
        }

        // If we failed because of a timeout, drop the TcpWatcher as
        // soon as possible because it's data is now set to null and we
        // want to cancel the callback ASAP.
        return match self.status {
            0 => Ok(obj),
            n => { drop(obj); Err(UvError(n)) }
        };

        extern fn timer_cb(handle: *mut uvll::uv_timer_t) {
            // Don't close the corresponding request, just wake up the task
            // and let RAII take care of the pending watcher.
            unsafe {
                let raw: raw::Timer = Handle::from_raw(handle);
                let cx: &mut ConnectCtx = mem::transmute(raw.get_data());
                cx.status = uvll::ECANCELED;
                ::wakeup(&mut cx.task);
            }
        }

        extern fn connect_cb(req: *mut uvll::uv_connect_t, status: libc::c_int) {
            // This callback can be invoked with ECANCELED if the watcher is
            // closed by the timeout callback. In that case we just want to free
            // the request and be along our merry way.
            unsafe {
                let mut req: raw::Connect = Request::from_raw(req);
                if status == uvll::ECANCELED { req.free(); return }

                // Apparently on windows when the handle is closed this callback
                // may not be invoked with ECANCELED but rather another error
                // code.  Either ways, if the data is null, then our timeout has
                // expired and there's nothing we can do.
                let data = req.get_data();
                if data.is_null() { req.free(); return }

                let cx: &mut ConnectCtx = &mut *(data as *mut ConnectCtx);
                cx.status = status;
                match cx.timer {
                    Some(ref mut t) => t.stop().unwrap(),
                    None => {}
                }

                // Note that the timer callback doesn't cancel the connect
                // request (that's the job of uv_close()), so it's possible for
                // this callback to get triggered after the timeout callback
                // fires, but before the task wakes up. In that case, we did
                // indeed successfully connect, but we don't need to wake
                // someone up. We updated the status above (correctly so), and
                // the task will pick up on this when it wakes up.
                if cx.task.is_some() {
                    ::wakeup(&mut cx.task);
                }
                req.free();
            }
        }
    }
}

pub struct AcceptTimeout<T> {
    access: AccessTimeout<AcceptorState<T>>,
}

pub struct Pusher<T> {
    access: access::Access<AcceptorState<T>>,
}

struct AcceptorState<T> {
    blocked_acceptor: Option<BlockedTask>,
    pending: Vec<UvResult<T>>,
}

impl<T: Send> AcceptTimeout<T> {
    pub fn new() -> AcceptTimeout<T> {
        AcceptTimeout {
            access: AccessTimeout::new(AcceptorState {
                blocked_acceptor: None,
                pending: Vec::new(),
            })
        }
    }

    pub fn accept(&mut self,
                  missile: HomingMissile,
                  uv_loop: raw::Loop) -> UvResult<T> {
        // If we've timed out but we're not closed yet, poll the state of the
        // queue to see if we can peel off a connection.
        if self.access.timed_out() &&
           !self.access.inner.access.is_closed(&missile) {
            let tmp = self.access.inner.access.get_mut(&missile);
            return match tmp.pending.remove(0) {
                Some(msg) => msg,
                None => Err(UvError(uvll::ECANCELED))
            }
        }

        // Now that we're not polling, attempt to gain access and then peel off
        // a connection. If we have no pending connections, then we need to go
        // to sleep and wait for one.
        //
        // Note that if we're woken up for a pending connection then we're
        // guaranteed that the check above will not steal our connection due to
        // the single-threaded nature of the event loop.
        let mut guard = try!(self.access.grant(missile));
        if guard.access.is_closed() {
            return Err(UvError(uvll::EOF))
        }

        match guard.access.pending.remove(0) {
            Some(msg) => return msg,
            None => {}
        }

        ::block(uv_loop, |task| {
            guard.access.blocked_acceptor = Some(task);
        });

        match guard.access.pending.remove(0) {
            _ if guard.access.is_closed() => Err(UvError(uvll::EOF)),
            Some(msg) => msg,
            None => Err(UvError(uvll::ECANCELED))
        }
    }

    pub fn pusher(&self) -> Pusher<T> {
        Pusher { access: self.access.inner.access.clone() }
    }

    pub fn set_timeout(&mut self,
                       dur: Option<Duration>,
                       uv_loop: raw::Loop) {
        let data = self.access.inner.access.unsafe_get() as uint;
        self.access.set_timeout(dur, uv_loop, cancel_accept::<T>, data);

        fn cancel_accept<T: Send>(me: uint) -> Option<BlockedTask> {
            unsafe {
                let me: &mut AcceptorState<T> = mem::transmute(me);
                me.blocked_acceptor.take()
            }
        }
    }

    pub fn close(&mut self, m: HomingMissile) {
        self.access.inner.access.close(&m);
        let task = self.access.inner.access.get_mut(&m).blocked_acceptor.take();
        drop(m);
        let _ = task.map(|t| t.reawaken());
    }
}

impl<T: Send> Pusher<T> {
    pub unsafe fn push(&self, t: UvResult<T>) {
        let state = self.access.unsafe_get();
        (*state).pending.push(t);
        let _ = (*state).blocked_acceptor.take().map(|t| t.reawaken());
    }
}

impl<T: Send> Clone for AcceptTimeout<T> {
    fn clone(&self) -> AcceptTimeout<T> {
        AcceptTimeout { access: self.access.clone() }
    }
}
