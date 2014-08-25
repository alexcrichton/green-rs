
#[cfg(unix)]
mod test_unix {
    use libc;
    use green::Callback;
    use rustuv::Signal;

    fn sender(tx: Sender<()>) -> Box<Callback + Send> {
        struct MySender { tx: Sender<()> }
        impl Callback for MySender {
            fn call(&mut self) { self.tx.send(()); }
        }
        box MySender { tx: tx } as Box<Callback + Send>
    }

    fn sigint() {
        unsafe {
            libc::funcs::posix88::signal::kill(libc::getpid(), libc::SIGINT);
        }
    }

    test!(fn test_io_signal_smoketest() {
        let mut signal = Signal::new().unwrap();
        let (tx, rx) = channel();
        signal.start(libc::SIGINT, sender(tx)).unwrap();
        sigint();
        rx.recv();
    })

    test!(fn test_io_signal_two_signal_one_signum() {
        let mut s1 = Signal::new().unwrap();
        let mut s2 = Signal::new().unwrap();
        let (tx1, rx1) = channel();
        let (tx2, rx2) = channel();
        s1.start(libc::SIGINT, sender(tx1)).unwrap();
        s2.start(libc::SIGINT, sender(tx2)).unwrap();
        sigint();
        rx1.recv();
        rx2.recv();
    })

    test!(fn test_io_signal_unregister() {
        let mut s1 = Signal::new().unwrap();
        let mut s2 = Signal::new().unwrap();
        let (tx1, rx1) = channel();
        let (tx2, rx2) = channel();
        s1.start(libc::SIGINT, sender(tx1)).unwrap();
        s2.start(libc::SIGINT, sender(tx2)).unwrap();
        s2.stop().unwrap();
        sigint();
        rx1.recv();
        assert!(rx2.recv_opt().is_err());
    })
}
