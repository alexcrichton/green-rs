use std::mem;
use std::cell::RefCell;
use std::rc::Rc;
use std::rt::task::{BlockedTask, Task};
use std::rt::local::Local;

use green::{Callback, PausableIdleCallback};
use rustuv::Idle;

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

fn mk(v: uint) -> (Idle, Chan) {
    let rc = Rc::new(RefCell::new((None, 0)));
    let cb = box MyCallback(rc.clone(), v);
    let cb = cb as Box<Callback>;
    let cb = unsafe { mem::transmute(cb) };
    (Idle::new(cb).unwrap(), rc)
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
    let (_idle, _chan) = mk(1);
})

test!(fn smoke_test() {
    let (mut idle, chan) = mk(1);
    idle.resume();
    assert_eq!(sleep(&chan), 1);
})

test!(fn smoke_drop() {
    let (mut idle, _chan) = mk(1);
    idle.resume();
    drop(idle);
})

test!(fn fun_combinations_of_methods() {
    let (mut idle, chan) = mk(1);
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
    let (mut idle1, chan1) = mk(1);
    let (mut idle2, chan2) = mk(2);
    idle2.resume();
    assert_eq!(sleep(&chan2), 2);
    idle2.pause();
    idle1.resume();
    assert_eq!(sleep(&chan1), 1);
})
