#![macro_escape]

use std::sync::deque::BufferPool;
use std::mem;
use std::rt::local::Local;
use std::rt::task::Task;

use green::sched::{Scheduler, Shutdown};
use green::task::GreenTask;
use green::sleeper_list::SleeperList;
use green::TaskState;

use uvio::UvEventLoop;

local_data_key!(local_loop_key: uint)

macro_rules! test( (fn $name:ident() $b:block) => (
    #[test]
    fn $name() { ::test::runtest(proc() $b) }
) )

pub fn local_loop() -> &'static mut UvEventLoop {
    unsafe { mem::transmute(*local_loop_key.get().unwrap()) }
}

pub fn runtest(p: proc(): Send) {
    let pool = BufferPool::new();
    let (worker, stealer) = pool.deque();
    let (rx, state) = TaskState::new();
    let event_loop = box UvEventLoop::new();
    let eloop_key = &*event_loop as *const _ as uint;
    let sched = Scheduler::new(100,
                               event_loop,
                               worker,
                               vec![stealer],
                               SleeperList::new(),
                               state);
    let mut sched = box sched;
    sched.make_handle().send(Shutdown);
    let task = GreenTask::new(&mut sched.stack_pool, None, proc() {
        let _ = local_loop_key.replace(Some(eloop_key));
        p();
    });
    sched.enqueue_task(task);
    let _native_test_task = Local::borrow(None::<Task>);
    sched.bootstrap();
    rx.recv();
}


// #[cfg(test)]
// fn next_test_ip4() -> std::rt::rtio::SocketAddr {
//     use std::io;
//     use std::rt::rtio;
//
//     let io::net::ip::SocketAddr { ip, port } = io::test::next_test_ip4();
//     let ip = match ip {
//         io::net::ip::Ipv4Addr(a, b, c, d) => rtio::Ipv4Addr(a, b, c, d),
//         _ => unreachable!(),
//     };
//     rtio::SocketAddr { ip: ip, port: port }
// }
//
// #[cfg(test)]
// fn next_test_ip6() -> std::rt::rtio::SocketAddr {
//     use std::io;
//     use std::rt::rtio;
//
//     let io::net::ip::SocketAddr { ip, port } = io::test::next_test_ip6();
//     let ip = match ip {
//         io::net::ip::Ipv6Addr(a, b, c, d, e, f, g, h) =>
//             rtio::Ipv6Addr(a, b, c, d, e, f, g, h),
//         _ => unreachable!(),
//     };
//     rtio::SocketAddr { ip: ip, port: port }
// }
