use std::io::test::next_test_unix;
use std::io::fs::PathExtensions;
use std::time::Duration;
use green::task::spawn;

use rustuv::{Pipe, PipeListener};
use rustuv::uvll;

pub fn smalltest(server: proc(Pipe):Send, client: proc(Pipe):Send) {
    let path1 = next_test_unix();
    let path2 = path1.clone();

    let acceptor = PipeListener::bind(&path1).unwrap().listen();

    spawn(proc() {
        match Pipe::connect(&path2) {
            Ok(c) => client(c),
            Err(e) => panic!("failed connect: {}", e),
        }
    });

    match acceptor.unwrap().accept() {
        Ok(c) => server(c),
        Err(e) => panic!("failed accept: {}", e),
    }
}

test!(fn bind_error() {
    let path = "path/to/nowhere";
    match PipeListener::bind(&path) {
        Ok(..) => panic!(),
        Err(e) => {
            assert!(e.code() == uvll::EPERM ||
                    e.code() == uvll::ENOENT ||
                    e.code() == uvll::EACCES ||
                    e.code() == uvll::EINVAL,
                    "bad error: {}", e);
        }
    }
})

test!(fn connect_error() {
    let path = if cfg!(windows) {
        r"\\.\pipe\this_should_not_exist_ever"
    } else {
        "path/to/nowhere"
    };
    match Pipe::connect(&path) {
        Ok(..) => panic!(),
        Err(e) => {
            assert!(e.code() == uvll::ENOENT ||
                    e.code() == uvll::EACCES ||
                    e.code() == uvll::EINVAL,
                    "bad error: {}", e);
        }
    }
})

test!(fn smoke() {
    smalltest(proc(mut server) {
        let mut buf = [0];
        server.read(&mut buf).unwrap();
        assert!(buf[0] == 99);
    }, proc(mut client) {
        client.write(&[99]).unwrap();
    })
})

test!(fn read_eof() {
    smalltest(proc(mut server) {
        let mut buf = [0];
        assert!(server.read(&mut buf).is_err());
        assert!(server.read(&mut buf).is_err());
    }, proc(_client) {
        // drop the client
    })
})

test!(fn write_begone() {
    smalltest(proc(mut server) {
        let buf = [0];
        loop {
            match server.uv_write(&buf) {
                Ok(..) => {}
                Err(e) => {
                    assert!(e.code() == uvll::EPIPE ||
                            e.code() == uvll::ENOTCONN ||
                            e.code() == uvll::ECONNRESET,
                            "unknown error {}", e);
                    break;
                }
            }
        }
    }, proc(_client) {
        // drop the client
    })
})

test!(fn accept_lots() {
    let times = 10;
    let path1 = next_test_unix();
    let path2 = path1.clone();

    let mut acceptor = match PipeListener::bind(&path1).unwrap().listen() {
        Ok(a) => a,
        Err(e) => panic!("failed listen: {}", e),
    };

    spawn(proc() {
        for _ in range(0u, times) {
            let mut stream = Pipe::connect(&path2).unwrap();
            match stream.write(&[100]) {
                Ok(..) => {}
                Err(e) => panic!("failed write: {}", e)
            }
        }
    });

    for _ in range(0, times) {
        let mut client = acceptor.accept().unwrap();
        let mut buf = [0];
        match client.read(&mut buf) {
            Ok(..) => {}
            Err(e) => panic!("failed read/accept: {}", e),
        }
        assert_eq!(buf[0], 100);
    }
})

#[cfg(unix)]
test!(fn path_exists() {
    let path = next_test_unix();
    let _acceptor = PipeListener::bind(&path).unwrap().listen();
    assert!(path.exists());
})

test!(fn unix_clone_smoke() {
    let addr = next_test_unix();
    let acceptor = PipeListener::bind(&addr).unwrap().listen();

    spawn(proc() {
        let mut s = Pipe::connect(&addr).unwrap();
        let mut buf = [0, 0];
        assert_eq!(s.read(&mut buf), Ok(1));
        assert_eq!(buf[0], 1);
        s.write(&[2]).unwrap();
    });

    let mut s1 = acceptor.unwrap().accept().unwrap();
    let s2 = s1.clone();

    let (tx1, rx1) = channel();
    let (tx2, rx2) = channel();
    spawn(proc() {
        let mut s2 = s2;
        rx1.recv();
        s2.write(&[1]).unwrap();
        tx2.send(());
    });
    tx1.send(());
    let mut buf = [0, 0];
    assert_eq!(s1.read(&mut buf), Ok(1));
    rx2.recv();
})

test!(fn unix_clone_two_read() {
    let addr = next_test_unix();
    let mut acceptor = PipeListener::bind(&addr).unwrap().listen().unwrap();
    let (tx1, rx) = channel();
    let tx2 = tx1.clone();

    spawn(proc() {
        let mut s = Pipe::connect(&addr).unwrap();
        s.write(&[1]).unwrap();
        rx.recv();
        s.write(&[2]).unwrap();
        rx.recv();
    });

    let mut s1 = acceptor.accept().unwrap();
    let s2 = s1.clone();

    let (done, rx) = channel();
    spawn(proc() {
        let mut s2 = s2;
        let mut buf = [0, 0];
        s2.read(&mut buf).unwrap();
        tx2.send(());
        done.send(());
    });
    let mut buf = [0, 0];
    s1.read(&mut buf).unwrap();
    tx1.send(());

    rx.recv();
})

test!(fn unix_clone_two_write() {
    let addr = next_test_unix();
    let mut acceptor = PipeListener::bind(&addr).unwrap().listen().unwrap();

    spawn(proc() {
        let mut s = Pipe::connect(&addr).unwrap();
        let mut buf = [0, 1];
        s.read(&mut buf).unwrap();
        s.read(&mut buf).unwrap();
    });

    let mut s1 = acceptor.accept().unwrap();
    let s2 = s1.clone();

    let (tx, rx) = channel();
    spawn(proc() {
        let mut s2 = s2;
        s2.write(&[1]).unwrap();
        tx.send(());
    });
    s1.write(&[2]).unwrap();

    rx.recv();
})

#[cfg(not(windows))]
test!(fn drop_removes_listener_path() {
    let path = next_test_unix();
    let l = PipeListener::bind(&path).unwrap();
    assert!(path.exists());
    drop(l);
    assert!(!path.exists());
})

#[cfg(not(windows))]
test!(fn drop_removes_acceptor_path() {
    let path = next_test_unix();
    let l = PipeListener::bind(&path).unwrap();
    assert!(path.exists());
    drop(l.listen().unwrap());
    assert!(!path.exists());
})

test!(fn accept_timeout() {
    let addr = next_test_unix();
    let mut a = PipeListener::bind(&addr).unwrap().listen().unwrap();

    a.set_timeout(Some(Duration::milliseconds(10)));

    // Make sure we time out once and future invocations also time out
    let err = a.accept().err().unwrap();
    assert_eq!(err.code(), uvll::ECANCELED);
    let err = a.accept().err().unwrap();
    assert_eq!(err.code(), uvll::ECANCELED);

    // Also make sure that even though the timeout is expired that we will
    // continue to receive any pending connections.
    let (tx, rx) = channel();
    let addr2 = addr.clone();
    spawn(proc() {
        tx.send(Pipe::connect(&addr2).unwrap());
    });
    let l = rx.recv();
    for i in range(0u, 1001) {
        match a.accept() {
            Ok(..) => break,
            Err(ref e) if e.code() == uvll::ECANCELED => {}
            Err(e) => panic!("error: {}", e),
        }
        ::std::task::deschedule();
        if i == 1000 { panic!("should have a pending connection") }
    }
    drop(l);

    // Unset the timeout and make sure that this always blocks.
    a.set_timeout(None);
    let addr2 = addr.clone();
    spawn(proc() {
        drop(Pipe::connect(&addr2).unwrap());
    });
    a.accept().unwrap();
})

test!(fn connect_timeout_error() {
    let addr = next_test_unix();
    assert!(Pipe::connect_timeout(&addr, Duration::milliseconds(100)).is_err());
})

test!(fn connect_timeout_success() {
    let addr = next_test_unix();
    let _a = PipeListener::bind(&addr).unwrap().listen().unwrap();
    assert!(Pipe::connect_timeout(&addr, Duration::milliseconds(100)).is_ok());
})

test!(fn connect_timeout_zero() {
    let addr = next_test_unix();
    let _a = PipeListener::bind(&addr).unwrap().listen().unwrap();
    assert!(Pipe::connect_timeout(&addr, Duration::milliseconds(0)).is_err());
})

test!(fn connect_timeout_negative() {
    let addr = next_test_unix();
    let _a = PipeListener::bind(&addr).unwrap().listen().unwrap();
    assert!(Pipe::connect_timeout(&addr, Duration::milliseconds(-1)).is_err());
})

test!(fn close_readwrite_smoke() {
    let addr = next_test_unix();
    let a = PipeListener::bind(&addr).unwrap().listen().unwrap();
    let (_tx, rx) = channel::<()>();
    spawn(proc() {
        let mut a = a;
        let _s = a.accept().unwrap();
        let _ = rx.recv_opt();
    });

    let mut b = [0];
    let mut s = Pipe::connect(&addr).unwrap();
    let mut s2 = s.clone();

    // closing should prevent reads/writes
    s.close_write().unwrap();
    assert!(s.write(&[0]).is_err());
    s.close_read().unwrap();
    assert!(s.read(&mut b).is_err());

    // closing should affect previous handles
    assert!(s2.write(&[0]).is_err());
    assert!(s2.read(&mut b).is_err());

    // closing should affect new handles
    let mut s3 = s.clone();
    assert!(s3.write(&[0]).is_err());
    assert!(s3.read(&mut b).is_err());

    // make sure these don't die
    let _ = s2.close_read();
    let _ = s2.close_write();
    let _ = s3.close_read();
    let _ = s3.close_write();
})

test!(fn close_read_wakes_up() {
    let addr = next_test_unix();
    let a = PipeListener::bind(&addr).unwrap().listen().unwrap();
    let (_tx, rx) = channel::<()>();
    spawn(proc() {
        let mut a = a;
        let _s = a.accept().unwrap();
        let _ = rx.recv_opt();
    });

    let mut s = Pipe::connect(&addr).unwrap();
    let s2 = s.clone();
    let (tx, rx) = channel();
    spawn(proc() {
        let mut s2 = s2;
        assert!(s2.read(&mut [0]).is_err());
        tx.send(());
    });
    // this should wake up the child task
    s.close_read().unwrap();

    // this test will never finish if the child doesn't wake up
    rx.recv();
})

test!(fn read_timeouts() {
    let addr = next_test_unix();
    let mut a = PipeListener::bind(&addr).unwrap().listen().unwrap();
    let (tx, rx) = channel::<()>();
    spawn(proc() {
        let mut s = Pipe::connect(&addr).unwrap();
        rx.recv();
        let mut amt = 0;
        while amt < 100 * 128 * 1024 {
            match s.read(&mut [0, ..128 * 1024]) {
                Ok(n) => { amt += n; }
                Err(e) => panic!("{}", e),
            }
        }
        let _ = rx.recv_opt();
    });

    let mut s = a.accept().unwrap();
    s.set_read_timeout(Some(Duration::milliseconds(20)));
    assert_eq!(s.uv_read(&mut [0]).err().unwrap().code(), uvll::ECANCELED);
    assert_eq!(s.uv_read(&mut [0]).err().unwrap().code(), uvll::ECANCELED);

    tx.send(());
    for _ in range(0u, 100) {
        assert!(s.write(&[0, ..128 * 1024]).is_ok());
    }
})

test!(fn timeout_concurrent_read() {
    let addr = next_test_unix();
    let mut a = PipeListener::bind(&addr).unwrap().listen().unwrap();
    let (tx, rx) = channel::<()>();
    spawn(proc() {
        let mut s = Pipe::connect(&addr).unwrap();
        rx.recv();
        assert!(s.write(&[0]).is_ok());
        let _ = rx.recv_opt();
    });

    let mut s = a.accept().unwrap();
    let s2 = s.clone();
    let (tx2, rx2) = channel();
    spawn(proc() {
        let mut s2 = s2;
        assert!(s2.read(&mut [0]).is_ok());
        tx2.send(());
    });

    s.set_read_timeout(Some(Duration::milliseconds(20)));
    assert_eq!(s.uv_read(&mut [0]).err().unwrap().code(), uvll::ECANCELED);
    tx.send(());

    rx2.recv();
})

test!(fn clone_accept_smoke() {
    let addr = next_test_unix();
    let l = PipeListener::bind(&addr).unwrap();
    let mut a = l.listen().unwrap();
    let mut a2 = a.clone();

    let addr2 = addr.clone();
    spawn(proc() {
        let _ = Pipe::connect(&addr2);
    });
    spawn(proc() {
        let _ = Pipe::connect(&addr);
    });

    assert!(a.accept().is_ok());
    drop(a);
    assert!(a2.accept().is_ok());
})

test!(fn clone_accept_concurrent() {
    let addr = next_test_unix();
    let l = PipeListener::bind(&addr).unwrap();
    let a = l.listen().unwrap();
    let a2 = a.clone();

    let (tx, rx) = channel();
    let tx2 = tx.clone();

    spawn(proc() { let mut a = a; tx.send(a.accept()) });
    spawn(proc() { let mut a = a2; tx2.send(a.accept()) });

    let addr2 = addr.clone();
    spawn(proc() {
        let _ = Pipe::connect(&addr2);
    });
    spawn(proc() {
        let _ = Pipe::connect(&addr);
    });

    assert!(rx.recv().is_ok());
    assert!(rx.recv().is_ok());
})

test!(fn close_accept_smoke() {
    let addr = next_test_unix();
    let l = PipeListener::bind(&addr).unwrap();
    let mut a = l.listen().unwrap();

    a.close_accept().unwrap();
    assert_eq!(a.accept().err().unwrap().code(), uvll::EOF);
})

test!(fn close_accept_concurrent() {
    let addr = next_test_unix();
    let l = PipeListener::bind(&addr).unwrap();
    let a = l.listen().unwrap();
    let mut a2 = a.clone();

    let (tx, rx) = channel();
    spawn(proc() {
        let mut a = a;
        tx.send(a.accept());
    });
    a2.close_accept().unwrap();

    assert_eq!(rx.recv().err().unwrap().code(), uvll::EOF);
})
