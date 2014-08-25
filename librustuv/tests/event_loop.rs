use green::EventLoop;
use rustuv;

test!(fn callback_run_once() {
    let mut event_loop = rustuv::EventLoop::new().unwrap();
    let mut count = 0;
    let count_ptr: *mut int = &mut count;
    event_loop.callback(proc() {
        unsafe { *count_ptr += 1 }
    });
    event_loop.run();
    assert_eq!(count, 1);
})

