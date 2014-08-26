#![feature(macro_rules)]

extern crate libc;
extern crate green;
extern crate rustuv;

use std::sync::deque::BufferPool;
use std::rt::local::Local;
use std::rt::task::Task;
use std::io::stdio;

use green::sched::{Scheduler, Shutdown, SchedHandle};
use green::task::GreenTask;
use green::sleeper_list::SleeperList;
use green::TaskState;

use rustuv::EventLoop;

macro_rules! test( (fn $name:ident() $b:block) => (
    #[test]
    fn $name() { ::runtest(proc() $b) }
) )

mod addrinfo;
mod async;
mod event_loop;
mod fs;
mod idle;
mod pipe;
mod signal;
mod tcp;
mod timer;

struct SchedulerExiter { handle: SchedHandle }
impl Drop for SchedulerExiter {
    fn drop(&mut self) { self.handle.send(Shutdown) }
}

fn runtest(p: proc(): Send) {
    let stdout = stdio::set_stdout(box stdio::stdout());
    let stderr = stdio::set_stderr(box stdio::stderr());

    // Create a scheduler to run locally
    let pool = BufferPool::new();
    let (worker, stealer) = pool.deque();
    let (rx, state) = TaskState::new();
    let event_loop = box EventLoop::new().unwrap();
    let sched = Scheduler::new(100,
                               event_loop,
                               worker,
                               vec![stealer],
                               SleeperList::new(),
                               state);
    let mut sched = box sched;

    // Schedule the shutdown message to the scheduler, but only send it after
    // the scheduler has exited.
    let exit = SchedulerExiter { handle: sched.make_handle() };

    // Enqueue a fresh geen task for the given test
    let (tx1, rx1) = channel();
    let task = GreenTask::new(&mut sched.stack_pool, None, proc() {
        stdout.map(stdio::set_stdout);
        stderr.map(stdio::set_stderr);
        p();
        tx1.send(());
        drop(exit);
    });
    sched.enqueue_task(task);

    // Steal away the actual native test task and then run the scheduler
    {
        let _native_test_task = Local::borrow(None::<Task>);
        sched.bootstrap();
    }

    // Ensure the scheduler exited with all tasks having completed.
    rx.recv();

    // This will fail if the task did not exit cleanly.
    rx1.recv();
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
