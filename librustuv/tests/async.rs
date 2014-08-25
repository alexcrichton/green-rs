use green::{Callback, RemoteCallback};
use rustuv::Async;

// Make sure that we can fire watchers in remote threads and that they
// actually trigger what they say they will.
test!(fn smoke_test() {
    struct MyCallback(Option<Sender<int>>);
    impl Callback for MyCallback {
        fn call(&mut self) {
            // this can get called more than once, but we only want to send
            // once
            let MyCallback(ref mut s) = *self;
            match s.take() {
                Some(s) => s.send(1),
                None => {}
            }
        }
    }

    let (tx, rx) = channel();
    let cb = box MyCallback(Some(tx));
    let watcher = Async::new(cb).unwrap();

    spawn(proc() {
        let mut watcher = watcher;
        watcher.fire();
    });
    assert_eq!(rx.recv(), 1);
})
