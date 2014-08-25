use std::time::Duration;
use green::Callback;
use green::task::spawn;
use rustuv::Timer;

fn sender(tx: Sender<()>) -> Box<Callback + Send> {
    struct MySender { tx: Sender<()> }
    impl Callback for MySender {
        fn call(&mut self) { self.tx.send(()); }
    }
    box MySender { tx: tx } as Box<Callback + Send>
}

fn ms(n: i32) -> Duration { Duration::milliseconds(n) }

test!(fn test_io_timer_sleep_simple() {
    let mut timer = Timer::new().unwrap();
    timer.sleep(ms(1));
})

test!(fn test_io_timer_sleep_oneshot() {
    let mut timer = Timer::new().unwrap();
    let (tx, rx) = channel();
    timer.oneshot(ms(1), sender(tx));
    rx.recv();
})

test!(fn test_io_timer_sleep_oneshot_forget() {
    let mut timer = Timer::new().unwrap();
    let (tx, _rx) = channel();
    timer.oneshot(ms(100000000), sender(tx));
})

test!(fn oneshot_twice() {
    let mut timer = Timer::new().unwrap();
    let (tx1, rx1) = channel();
    let (tx2, rx2) = channel();
    timer.oneshot(ms(10000), sender(tx1));
    timer.oneshot(ms(1), sender(tx2));
    rx2.recv();
    assert_eq!(rx1.recv_opt(), Err(()));
})

test!(fn test_io_timer_oneshot_then_sleep() {
    let mut timer = Timer::new().unwrap();
    let (tx, rx) = channel();
    timer.oneshot(ms(100000000), sender(tx));
    timer.sleep(ms(1)); // this should invalidate rx

    assert_eq!(rx.recv_opt(), Err(()));
})

test!(fn test_io_timer_sleep_periodic() {
    let mut timer = Timer::new().unwrap();
    let (tx, rx) = channel();
    timer.periodic(ms(1), sender(tx));
    rx.recv();
    rx.recv();
    rx.recv();
})

test!(fn test_io_timer_sleep_periodic_forget() {
    let mut timer = Timer::new().unwrap();
    let (tx, _rx) = channel();
    timer.periodic(ms(100000000), sender(tx));
})

test!(fn oneshot() {
    let mut timer = Timer::new().unwrap();

    let (tx, rx) = channel();
    timer.oneshot(ms(1), sender(tx));
    rx.recv();
    assert!(rx.recv_opt().is_err());

    let (tx, rx) = channel();
    timer.oneshot(ms(1), sender(tx));
    rx.recv();
    assert!(rx.recv_opt().is_err());
})

test!(fn override() {
    let mut timer = Timer::new().unwrap();
    let (otx, orx) = channel();
    let (ptx, prx) = channel();
    timer.oneshot(ms(100), sender(otx));
    timer.periodic(ms(100), sender(ptx));
    timer.sleep(ms(1));
    assert_eq!(orx.recv_opt(), Err(()));
    assert_eq!(prx.recv_opt(), Err(()));
    let (tx, rx) = channel();
    timer.oneshot(ms(1), sender(tx));
    rx.recv();
})

test!(fn period() {
    let mut timer = Timer::new().unwrap();
    let (tx, rx) = channel();
    timer.periodic(ms(1), sender(tx));
    rx.recv();
    rx.recv();
    let (tx, rx) = channel();
    timer.periodic(ms(1), sender(tx));
    rx.recv();
    rx.recv();
})

test!(fn sleep() {
    let mut timer = Timer::new().unwrap();
    timer.sleep(ms(1));
    timer.sleep(ms(1));
})

test!(fn oneshot_fail() {
    let mut timer = Timer::new().unwrap();
    let (tx, _rx) = channel();
    timer.oneshot(ms(1), sender(tx));
    drop(timer);
})

test!(fn period_fail() {
    let mut timer = Timer::new().unwrap();
    let (tx, _rx) = channel();
    timer.periodic(ms(1), sender(tx));
    drop(timer);
})

test!(fn normal_fail() {
    let _timer = Timer::new().unwrap();
})

test!(fn closing_channel_during_drop_doesnt_kill_everything() {
    // see issue #10375
    let mut timer = Timer::new().unwrap();
    let (tx, rx) = channel();
    timer.periodic(ms(1000), sender(tx));

    spawn(proc() {
        let _ = rx.recv_opt();
    });

    // when we drop the Timer we're going to destroy the channel,
    // which must wake up the task on the other end
})

test!(fn reset_doesnt_switch_tasks() {
    // similar test to the one above.
    let mut timer = Timer::new().unwrap();
    let (tx, rx) = channel();
    timer.periodic(ms(1000), sender(tx));

    spawn(proc() {
        let _ = rx.recv_opt();
    });

    let (tx, _rx) = channel();
    timer.oneshot(ms(1), sender(tx));
})

test!(fn reset_doesnt_switch_tasks2() {
    // similar test to the one above.
    let mut timer = Timer::new().unwrap();
    let (tx, rx) = channel();
    timer.periodic(ms(1000), sender(tx));

    spawn(proc() {
        let _ = rx.recv_opt();
    });

    timer.sleep(ms(1));
})

test!(fn sender_goes_away_oneshot() {
    let rx = {
        let mut timer = Timer::new().unwrap();
        let (tx, rx) = channel();
        timer.oneshot(ms(1000), sender(tx));
        rx
    };
    assert_eq!(rx.recv_opt(), Err(()));
})

test!(fn sender_goes_away_period() {
    let rx = {
        let mut timer = Timer::new().unwrap();
        let (tx, rx) = channel();
        timer.periodic(ms(1000), sender(tx));
        rx
    };
    assert_eq!(rx.recv_opt(), Err(()));
})

test!(fn receiver_goes_away_oneshot() {
    let mut timer1 = Timer::new().unwrap();
    let (tx, _rx) = channel();
    timer1.oneshot(ms(1), sender(tx));
    let mut timer2 = Timer::new().unwrap();
    // while sleeping, the previous timer should fire and not have its
    // callback do something terrible.
    timer2.sleep(ms(2));
})

test!(fn receiver_goes_away_period() {
    let mut timer1 = Timer::new().unwrap();
    let (tx, _rx) = channel();
    timer1.periodic(ms(1), sender(tx));
    let mut timer2 = Timer::new().unwrap();
    // while sleeping, the previous timer should fire and not have its
    // callback do something terrible.
    timer2.sleep(ms(2));
})

test!(fn sleep_zero() {
    let mut timer = Timer::new().unwrap();
    timer.sleep(ms(0));
})

test!(fn sleep_negative() {
    let mut timer = Timer::new().unwrap();
    timer.sleep(ms(-1000000));
})

test!(fn oneshot_zero() {
    let mut timer = Timer::new().unwrap();
    let (tx, rx) = channel();
    timer.oneshot(ms(0), sender(tx));
    rx.recv();
})

test!(fn oneshot_negative() {
    let mut timer = Timer::new().unwrap();
    let (tx, rx) = channel();
    timer.oneshot(ms(-1000000), sender(tx));
    rx.recv();
})

test!(fn periodic_zero() {
    let mut timer = Timer::new().unwrap();
    let (tx, rx) = channel();
    timer.periodic(ms(0), sender(tx));
    rx.recv();
    rx.recv();
    rx.recv();
    rx.recv();
})

test!(fn periodic_negative() {
    let mut timer = Timer::new().unwrap();
    let (tx, rx) = channel();
    timer.periodic(ms(-1000000), sender(tx));
    rx.recv();
    rx.recv();
    rx.recv();
    rx.recv();
})
