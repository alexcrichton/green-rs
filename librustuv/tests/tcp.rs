use std::io::Acceptor;
use std::io::net::ip::SocketAddr;
use std::io::test::{next_test_ip4, next_test_ip6};
use std::time::Duration;
use green::task::spawn;

use rustuv::{uvll, TcpListener, UvResult, Tcp};

fn to_sockaddr(s: &str, port: u16) -> SocketAddr {
    if s.contains(":") {
        from_str(format!("[{}]:{}", s, port).as_slice()).unwrap()
    } else {
        from_str(format!("{}:{}", s, port).as_slice()).unwrap()
    }
}

fn bind(s: &str, port: u16) -> UvResult<TcpListener> {
    TcpListener::bind(to_sockaddr(s, port))
}

fn connect(s: &str, port: u16) -> UvResult<Tcp> {
    Tcp::connect(to_sockaddr(s, port))
}

test!(fn bind_error() {
    match bind("0.0.0.0", 1) {
        Ok(..) => panic!(),
        Err(e) => assert_eq!(e.code(), uvll::EACCES),
    }
})

test!(fn connect_error() {
    match connect("0.0.0.0", 1) {
        Ok(..) => panic!(),
        Err(e) => assert_eq!(e.code(), uvll::ECONNREFUSED),
    }
})

test!(fn listen_ip4_localhost() {
    let socket_addr = next_test_ip4();
    let ip_str = socket_addr.ip.to_string();
    let port = socket_addr.port;
    let listener = bind(ip_str.as_slice(), port).unwrap();
    let mut acceptor = listener.listen().unwrap();

    spawn(proc() {
        let mut stream = connect("127.0.0.1", port).unwrap();
        stream.write(&[144]).unwrap();
    });

    let mut stream = acceptor.accept().unwrap();
    let mut buf = [0];
    stream.read(&mut buf).unwrap();
    assert!(buf[0] == 144);
})

test!(fn connect_localhost() {
    let addr = next_test_ip4();
    let ip_str = addr.ip.to_string();
    let port = addr.port;
    let mut acceptor = bind(ip_str.as_slice(), port).unwrap().listen().unwrap();

    spawn(proc() {
        let mut stream = connect("127.0.0.1", addr.port).unwrap();
        stream.write(&[64]).unwrap();
    });

    let mut stream = acceptor.accept().unwrap();
    let mut buf = [0];
    stream.read(&mut buf).unwrap();
    assert!(buf[0] == 64);
})

test!(fn connect_ip4_loopback() {
    let addr = next_test_ip4();
    let ip_str = addr.ip.to_string();
    let port = addr.port;
    let mut acceptor = bind(ip_str.as_slice(), port).unwrap().listen().unwrap();

    spawn(proc() {
        let mut stream = connect("127.0.0.1", addr.port).unwrap();
        stream.write(&[44]).unwrap();
    });

    let mut stream = acceptor.accept().unwrap();
    let mut buf = [0];
    stream.read(&mut buf).unwrap();
    assert!(buf[0] == 44);
})

test!(fn connect_ip6_loopback() {
    let addr = next_test_ip6();
    let ip_str = addr.ip.to_string();
    let port = addr.port;
    let mut acceptor = bind(ip_str.as_slice(), port).unwrap().listen().unwrap();

    spawn(proc() {
        let mut stream = connect("::1", addr.port).unwrap();
        stream.write(&[66]).unwrap();
    });

    let mut stream = acceptor.accept().unwrap();
    let mut buf = [0];
    stream.read(&mut buf).unwrap();
    assert!(buf[0] == 66);
})

test!(fn smoke_test_ip4() {
    let addr = next_test_ip4();
    let ip_str = addr.ip.to_string();
    let port = addr.port;
    let mut acceptor = bind(ip_str.as_slice(), port).unwrap().listen().unwrap();

    spawn(proc() {
        let mut stream = connect(ip_str.as_slice(), port).unwrap();
        stream.write(&[99]).unwrap();
    });

    let mut stream = acceptor.accept().unwrap();
    let mut buf = [0];
    stream.read(&mut buf).unwrap();
    assert!(buf[0] == 99);
})

test!(fn smoke_test_ip6() {
    let addr = next_test_ip6();
    let ip_str = addr.ip.to_string();
    let port = addr.port;
    let mut acceptor = bind(ip_str.as_slice(), port).unwrap().listen().unwrap();

    spawn(proc() {
        let mut stream = connect(ip_str.as_slice(), port).unwrap();
        stream.write(&[99]).unwrap();
    });

    let mut stream = acceptor.accept().unwrap();
    let mut buf = [0];
    stream.read(&mut buf).unwrap();
    assert!(buf[0] == 99);
})

test!(fn read_eof_ip4() {
    let addr = next_test_ip4();
    let ip_str = addr.ip.to_string();
    let port = addr.port;
    let mut acceptor = bind(ip_str.as_slice(), port).unwrap().listen().unwrap();

    spawn(proc() {
        let _stream = connect(ip_str.as_slice(), port).unwrap();
        // Close
    });

    let mut stream = acceptor.accept().unwrap();
    let mut buf = [0];
    let nread = stream.read(&mut buf);
    assert!(nread.is_err());
})

test!(fn read_eof_ip6() {
    let addr = next_test_ip6();
    let ip_str = addr.ip.to_string();
    let port = addr.port;
    let mut acceptor = bind(ip_str.as_slice(), port).unwrap().listen().unwrap();

    spawn(proc() {
        let _stream = connect(ip_str.as_slice(), port).unwrap();
        // Close
    });

    let mut stream = acceptor.accept().unwrap();
    let mut buf = [0];
    let nread = stream.read(&mut buf);
    assert!(nread.is_err());
})

test!(fn read_eof_twice_ip4() {
    let addr = next_test_ip4();
    let ip_str = addr.ip.to_string();
    let port = addr.port;
    let mut acceptor = bind(ip_str.as_slice(), port).unwrap().listen().unwrap();

    spawn(proc() {
        let _stream = connect(ip_str.as_slice(), port).unwrap();
        // Close
    });

    let mut stream = acceptor.accept().unwrap();
    let mut buf = [0];
    let nread = stream.read(&mut buf);
    assert!(nread.is_err());

    match stream.uv_read(&mut buf) {
        Ok(..) => panic!(),
        Err(ref e) => {
            assert!(e.code() == uvll::ENOTCONN || e.code() == uvll::EOF,
                    "unknown kind: {}", e);
        }
    }
})

test!(fn read_eof_twice_ip6() {
    let addr = next_test_ip6();
    let ip_str = addr.ip.to_string();
    let port = addr.port;
    let mut acceptor = bind(ip_str.as_slice(), port).unwrap().listen().unwrap();

    spawn(proc() {
        let _stream = connect(ip_str.as_slice(), port).unwrap();
        // Close
    });

    let mut stream = acceptor.accept().unwrap();
    let mut buf = [0];
    let nread = stream.read(&mut buf);
    assert!(nread.is_err());

    match stream.uv_read(&mut buf) {
        Ok(..) => panic!(),
        Err(ref e) => {
            assert!(e.code() == uvll::ENOTCONN || e.code() == uvll::EOF,
                    "unknown kind: {}", e);
        }
    }
})

test!(fn write_close_ip4() {
    let addr = next_test_ip4();
    let ip_str = addr.ip.to_string();
    let port = addr.port;
    let mut acceptor = bind(ip_str.as_slice(), port).unwrap().listen().unwrap();

    spawn(proc() {
        let _stream = connect(ip_str.as_slice(), port).unwrap();
        // Close
    });

    let mut stream = acceptor.accept().unwrap();
    let buf = [0];
    loop {
        match stream.uv_write(&buf) {
            Ok(..) => {}
            Err(e) => {
                assert!(e.code() == uvll::ECONNRESET ||
                        e.code() == uvll::EPIPE ||
                        e.code() == uvll::ECONNABORTED,
                        "unknown error: {}", e);
                break;
            }
        }
    }
})

test!(fn write_close_ip6() {
    let addr = next_test_ip6();
    let ip_str = addr.ip.to_string();
    let port = addr.port;
    let mut acceptor = bind(ip_str.as_slice(), port).unwrap().listen().unwrap();

    spawn(proc() {
        let _stream = connect(ip_str.as_slice(), port).unwrap();
        // Close
    });

    let mut stream = acceptor.accept().unwrap();
    let buf = [0];
    loop {
        match stream.uv_write(&buf) {
            Ok(..) => {}
            Err(e) => {
                assert!(e.code() == uvll::ECONNRESET ||
                        e.code() == uvll::EPIPE ||
                        e.code() == uvll::ECONNABORTED,
                        "unknown error: {}", e);
                break;
            }
        }
    }
})

test!(fn multiple_connect_serial_ip4() {
    let addr = next_test_ip4();
    let ip_str = addr.ip.to_string();
    let port = addr.port;
    let max = 10u;
    let mut acceptor = bind(ip_str.as_slice(), port).unwrap().listen().unwrap();

    spawn(proc() {
        for _ in range(0, max) {
            let mut stream = connect(ip_str.as_slice(), port).unwrap();
            stream.write(&[99]).unwrap();
        }
    });

    for stream in acceptor.incoming().take(max) {
        let mut buf = [0];
        stream.unwrap().read(&mut buf).unwrap();
        assert_eq!(buf[0], 99);
    }
})

test!(fn multiple_connect_serial_ip6() {
    let addr = next_test_ip6();
    let ip_str = addr.ip.to_string();
    let port = addr.port;
    let max = 10u;
    let mut acceptor = bind(ip_str.as_slice(), port).unwrap().listen().unwrap();

    spawn(proc() {
        for _ in range(0, max) {
            let mut stream = connect(ip_str.as_slice(), port).unwrap();
            stream.write(&[99]).unwrap();
        }
    });

    for stream in acceptor.incoming().take(max) {
        let mut buf = [0];
        stream.unwrap().read(&mut buf).unwrap();
        assert_eq!(buf[0], 99);
    }
})

test!(fn multiple_connect_interleaved_greedy_schedule_ip4() {
    let addr = next_test_ip4();
    let ip_str = addr.ip.to_string();
    let port = addr.port;
    static MAX: int = 10;
    let acceptor = bind(ip_str.as_slice(), port).unwrap().listen().unwrap();

    spawn(proc() {
        let mut acceptor = acceptor;
        for (i, stream) in acceptor.incoming().enumerate().take(MAX as uint) {
            // Start another task to handle the connection
            spawn(proc() {
                let mut buf = [0];
                stream.unwrap().read(&mut buf).unwrap();
                assert!(buf[0] == i as u8);
            });
        }
    });

    myconnect(0, addr);

    fn myconnect(i: int, addr: SocketAddr) {
        let ip_str = addr.ip.to_string();
        let port = addr.port;
        if i == MAX { return }

        spawn(proc() {
            let mut stream = connect(ip_str.as_slice(), port).unwrap();
            // Connect again before writing
            myconnect(i + 1, addr);
            stream.write(&[i as u8]).unwrap();
        });
    }
})

test!(fn multiple_connect_interleaved_greedy_schedule_ip6() {
    let addr = next_test_ip6();
    let ip_str = addr.ip.to_string();
    let port = addr.port;
    static MAX: int = 10;
    let acceptor = bind(ip_str.as_slice(), port).unwrap().listen().unwrap();

    spawn(proc() {
        let mut acceptor = acceptor;
        for (i, stream) in acceptor.incoming().enumerate().take(MAX as uint) {
            // Start another task to handle the connection
            spawn(proc() {
                let mut buf = [0];
                stream.unwrap().read(&mut buf).unwrap();
                assert!(buf[0] == i as u8);
            });
        }
    });

    myconnect(0, addr);

    fn myconnect(i: int, addr: SocketAddr) {
        let ip_str = addr.ip.to_string();
        let port = addr.port;
        if i == MAX { return }

        spawn(proc() {
            let mut stream = connect(ip_str.as_slice(), port).unwrap();
            // Connect again before writing
            myconnect(i + 1, addr);
            stream.write(&[i as u8]).unwrap();
        });
    }
})

test!(fn multiple_connect_interleaved_lazy_schedule_ip4() {
    static MAX: int = 10;
    let addr = next_test_ip4();
    let ip_str = addr.ip.to_string();
    let port = addr.port;
    let acceptor = bind(ip_str.as_slice(), port).unwrap().listen().unwrap();

    spawn(proc() {
        let mut acceptor = acceptor;
        for stream in acceptor.incoming().take(MAX as uint) {
            // Start another task to handle the connection
            spawn(proc() {
                let mut buf = [0];
                stream.unwrap().read(&mut buf).unwrap();
                assert!(buf[0] == 99);
            });
        }
    });

    myconnect(0, addr);

    fn myconnect(i: int, addr: SocketAddr) {
        let ip_str = addr.ip.to_string();
        let port = addr.port;
        if i == MAX { return }

        spawn(proc() {
            let mut stream = connect(ip_str.as_slice(), port).unwrap();
            // Connect again before writing
            myconnect(i + 1, addr);
            stream.write(&[99]).unwrap();
        });
    }
})

test!(fn multiple_connect_interleaved_lazy_schedule_ip6() {
    static MAX: int = 10;
    let addr = next_test_ip6();
    let ip_str = addr.ip.to_string();
    let port = addr.port;
    let acceptor = bind(ip_str.as_slice(), port).unwrap().listen().unwrap();

    spawn(proc() {
        let mut acceptor = acceptor;
        for stream in acceptor.incoming().take(MAX as uint) {
            // Start another task to handle the connection
            spawn(proc() {
                let mut buf = [0];
                stream.unwrap().read(&mut buf).unwrap();
                assert!(buf[0] == 99);
            });
        }
    });

    myconnect(0, addr);

    fn myconnect(i: int, addr: SocketAddr) {
        let ip_str = addr.ip.to_string();
        let port = addr.port;
        if i == MAX { return }

        spawn(proc() {
            let mut stream = connect(ip_str.as_slice(), port).unwrap();
            // Connect again before writing
            myconnect(i + 1, addr);
            stream.write(&[99]).unwrap();
        });
    }
})

pub fn socket_name(addr: SocketAddr) {
    let ip_str = addr.ip.to_string();
    let port = addr.port;
    let mut listener = bind(ip_str.as_slice(), port).unwrap();

    // Make sure socket_name gives
    // us the socket we binded to.
    let so_name = listener.socket_name();
    assert!(so_name.is_ok());
    assert_eq!(addr, so_name.unwrap());
}

pub fn peer_name(addr: SocketAddr) {
    let ip_str = addr.ip.to_string();
    let port = addr.port;
    let acceptor = bind(ip_str.as_slice(), port).unwrap().listen().unwrap();
    spawn(proc() {
        let mut acceptor = acceptor;
        acceptor.accept().unwrap();
    });

    let stream = connect(ip_str.as_slice(), port);

    assert!(stream.is_ok());
    let mut stream = stream.unwrap();

    // Make sure peer_name gives us the
    // address/port of the peer we've
    // connected to.
    let peer_name = stream.peer_name();
    assert!(peer_name.is_ok());
    assert_eq!(addr, peer_name.unwrap());
}

test!(fn socket_and_peer_name_ip4() {
    peer_name(next_test_ip4());
    socket_name(next_test_ip4());
})

test!(fn socket_and_peer_name_ip6() {
    // FIXME: peer name is not consistent
    //peer_name(next_test_ip6());
    socket_name(next_test_ip6());
})

test!(fn partial_read() {
    let addr = next_test_ip4();
    let port = addr.port;
    let (tx, rx) = channel();
    spawn(proc() {
        let ip_str = addr.ip.to_string();
        let mut srv = bind(ip_str.as_slice(), port).unwrap().listen().unwrap();
        tx.send(());
        let mut cl = srv.accept().unwrap();
        cl.write(&[10]).unwrap();
        let mut b = [0];
        cl.read(&mut b).unwrap();
        tx.send(());
    });

    rx.recv();
    let ip_str = addr.ip.to_string();
    let mut c = connect(ip_str.as_slice(), port).unwrap();
    let mut b = [0, ..10];
    assert_eq!(c.read(&mut b), Ok(1));
    c.write(&[1]).unwrap();
    rx.recv();
})

test!(fn double_bind() {
    let addr = next_test_ip4();
    let ip_str = addr.ip.to_string();
    let port = addr.port;
    let _listener = bind(ip_str.as_slice(), port).unwrap().listen().unwrap();
    match bind(ip_str.as_slice(), port).unwrap().listen() {
        Ok(..) => panic!(),
        Err(e) => {
            assert!(e.code() == uvll::ECONNREFUSED ||
                    e.code() == uvll::EADDRINUSE, "unknown error: {}", e);
        }
    }
})

test!(fn fast_rebind() {
    let addr = next_test_ip4();
    let port = addr.port;
    let (tx, rx) = channel();

    spawn(proc() {
        let ip_str = addr.ip.to_string();
        rx.recv();
        let _stream = connect(ip_str.as_slice(), port).unwrap();
        // Close
        rx.recv();
    });

    {
        let ip_str = addr.ip.to_string();
        let acceptor = bind(ip_str.as_slice(), port).unwrap().listen();
        tx.send(());
        {
            let _stream = acceptor.unwrap().accept().unwrap();
            // Close client
            tx.send(());
        }
        // Close listener
    }
    let _listener = bind(addr.ip.to_string().as_slice(), port).unwrap();
})

test!(fn tcp_clone_smoke() {
    let addr = next_test_ip4();
    let ip_str = addr.ip.to_string();
    let port = addr.port;
    let mut acceptor = bind(ip_str.as_slice(), port).unwrap().listen().unwrap();

    spawn(proc() {
        let mut s = connect(ip_str.as_slice(), port).unwrap();
        let mut buf = [0, 0];
        assert_eq!(s.read(&mut buf), Ok(1));
        assert_eq!(buf[0], 1);
        s.write(&[2]).unwrap();
    });

    let mut s1 = acceptor.accept().unwrap();
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

test!(fn tcp_clone_two_read() {
    let addr = next_test_ip6();
    let ip_str = addr.ip.to_string();
    let port = addr.port;
    let mut acceptor = bind(ip_str.as_slice(), port).unwrap().listen().unwrap();
    let (tx1, rx) = channel();
    let tx2 = tx1.clone();

    spawn(proc() {
        let mut s = connect(ip_str.as_slice(), port).unwrap();
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

test!(fn tcp_clone_two_write() {
    let addr = next_test_ip4();
    let ip_str = addr.ip.to_string();
    let port = addr.port;
    let mut acceptor = bind(ip_str.as_slice(), port).unwrap().listen().unwrap();

    spawn(proc() {
        let mut s = connect(ip_str.as_slice(), port).unwrap();
        let mut buf = [0, 1];
        s.read(&mut buf).unwrap();
        s.read(&mut buf).unwrap();
    });

    let mut s1 = acceptor.accept().unwrap();
    let s2 = s1.clone();

    let (done, rx) = channel();
    spawn(proc() {
        let mut s2 = s2;
        s2.write(&[1]).unwrap();
        done.send(());
    });
    s1.write(&[2]).unwrap();

    rx.recv();
})

test!(fn shutdown_smoke() {
    let addr = next_test_ip4();
    let ip_str = addr.ip.to_string();
    let port = addr.port;
    let a = bind(ip_str.as_slice(), port).unwrap().listen().unwrap();
    spawn(proc() {
        let mut a = a;
        let mut c = a.accept().unwrap();
        assert_eq!(c.read_to_end(), Ok(vec!()));
        c.write(&[1]).unwrap();
    });

    let mut s = connect(ip_str.as_slice(), port).unwrap();
    assert!(s.close_write().is_ok());
    assert!(s.write(&[1]).is_err());
    assert_eq!(s.read_to_end(), Ok(vec!(1)));
})

test!(fn accept_timeout() {
    let addr = next_test_ip4();
    let ip_str = addr.ip.to_string();
    let port = addr.port;
    let mut a = bind(ip_str.as_slice(), port).unwrap().listen().unwrap();

    a.set_timeout(Some(Duration::milliseconds(10)));

    // Make sure we time out once and future invocations also time out
    let err = a.accept().err().unwrap();
    assert_eq!(err.code(), uvll::ECANCELED);
    let err = a.accept().err().unwrap();
    assert_eq!(err.code(), uvll::ECANCELED);

    // Also make sure that even though the timeout is expired that we will
    // continue to receive any pending connections.
    //
    // FIXME: freebsd apparently never sees the pending connection, but
    //        testing manually always works. Need to investigate this
    //        flakiness.
    if !cfg!(target_os = "freebsd") {
        let (tx, rx) = channel();
        spawn(proc() {
            tx.send(connect(addr.ip.to_string().as_slice(),
                            port).unwrap());
        });
        let _l = rx.recv();
        for i in range(0i, 1001) {
            match a.accept() {
                Ok(..) => break,
                Err(ref e) if e.code() == uvll::ECANCELED => {}
                Err(e) => panic!("error: {}", e),
            }
            ::std::task::deschedule();
            if i == 1000 { panic!("should have a pending connection") }
        }
    }

    // Unset the timeout and make sure that this always blocks.
    a.set_timeout(None);
    spawn(proc() {
        drop(connect(addr.ip.to_string().as_slice(), port).unwrap());
    });
    a.accept().unwrap();
})

test!(fn close_readwrite_smoke() {
    let addr = next_test_ip4();
    let ip_str = addr.ip.to_string();
    let port = addr.port;
    let a = bind(ip_str.as_slice(), port).unwrap().listen().unwrap();
    let (_tx, rx) = channel::<()>();
    spawn(proc() {
        let mut a = a;
        let _s = a.accept().unwrap();
        let _ = rx.recv_opt();
    });

    let mut b = [0];
    let mut s = connect(ip_str.as_slice(), port).unwrap();
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
    let addr = next_test_ip4();
    let ip_str = addr.ip.to_string();
    let port = addr.port;
    let a = bind(ip_str.as_slice(), port).unwrap().listen().unwrap();
    let (_tx, rx) = channel::<()>();
    spawn(proc() {
        let mut a = a;
        let _s = a.accept().unwrap();
        let _ = rx.recv_opt();
    });

    let mut s = connect(ip_str.as_slice(), port).unwrap();
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
    let addr = next_test_ip6();
    let ip_str = addr.ip.to_string();
    let port = addr.port;
    let mut a = bind(ip_str.as_slice(), port).unwrap().listen().unwrap();
    let (tx, rx) = channel::<()>();
    spawn(proc() {
        let mut s = connect(ip_str.as_slice(), port).unwrap();
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
    for _ in range(0i, 100) {
        assert!(s.write(&[0, ..128 * 1024]).is_ok());
    }
})

test!(fn timeout_concurrent_read() {
    let addr = next_test_ip6();
    let ip_str = addr.ip.to_string();
    let port = addr.port;
    let mut a = bind(ip_str.as_slice(), port).unwrap().listen().unwrap();
    let (tx, rx) = channel::<()>();
    spawn(proc() {
        let mut s = connect(ip_str.as_slice(), port).unwrap();
        rx.recv();
        assert_eq!(s.write(&[0]), Ok(()));
        let _ = rx.recv_opt();
    });

    let mut s = a.accept().unwrap();
    let s2 = s.clone();
    let (tx2, rx2) = channel();
    spawn(proc() {
        let mut s2 = s2;
        assert_eq!(s2.read(&mut [0]), Ok(1));
        tx2.send(());
    });

    s.set_read_timeout(Some(Duration::milliseconds(20)));
    assert_eq!(s.uv_read(&mut [0]).err().unwrap().code(), uvll::ECANCELED);
    tx.send(());

    rx2.recv();
})

test!(fn clone_while_reading() {
    let addr = next_test_ip6();
    let listen = bind(addr.ip.to_string().as_slice(), addr.port);
    let mut accept = listen.unwrap().listen().unwrap();

    // Enqueue a task to write to a socket
    let (tx, rx) = channel();
    let (txdone, rxdone) = channel();
    let txdone2 = txdone.clone();
    spawn(proc() {
        let mut tcp = connect(addr.ip.to_string().as_slice(),
                              addr.port).unwrap();
        rx.recv();
        tcp.write_u8(0).unwrap();
        txdone2.send(());
    });

    // Spawn off a reading clone
    let tcp = accept.accept().unwrap();
    let tcp2 = tcp.clone();
    let txdone3 = txdone.clone();
    spawn(proc() {
        let mut tcp2 = tcp2;
        tcp2.read_u8().unwrap();
        txdone3.send(());
    });

    // Try to ensure that the reading clone is indeed reading
    for _ in range(0i, 50) {
        ::std::task::deschedule();
    }

    // clone the handle again while it's reading, then let it finish the
    // read.
    let _ = tcp.clone();
    tx.send(());
    rxdone.recv();
    rxdone.recv();
})

test!(fn clone_accept_smoke() {
    let addr = next_test_ip4();
    let l = bind(addr.ip.to_string().as_slice(), addr.port).unwrap();
    let mut a = l.listen().unwrap();
    let mut a2 = a.clone();

    spawn(proc() {
        let _ = connect(addr.ip.to_string().as_slice(), addr.port).unwrap();
    });
    spawn(proc() {
        let _ = connect(addr.ip.to_string().as_slice(), addr.port).unwrap();
    });

    assert!(a.accept().is_ok());
    assert!(a2.accept().is_ok());
})

test!(fn clone_accept_concurrent() {
    let addr = next_test_ip4();
    let l = bind(addr.ip.to_string().as_slice(), addr.port).unwrap();
    let a = l.listen().unwrap();
    let a2 = a.clone();

    let (tx, rx) = channel();
    let tx2 = tx.clone();

    spawn(proc() { let mut a = a; tx.send(a.accept()) });
    spawn(proc() { let mut a = a2; tx2.send(a.accept()) });

    spawn(proc() {
        let _ = connect(addr.ip.to_string().as_slice(), addr.port).unwrap();
    });
    spawn(proc() {
        let _ = connect(addr.ip.to_string().as_slice(), addr.port).unwrap();
    });

    assert!(rx.recv().is_ok());
    assert!(rx.recv().is_ok());
})

test!(fn close_accept_smoke() {
    let addr = next_test_ip4();
    let l = bind(addr.ip.to_string().as_slice(), addr.port).unwrap();
    let mut a = l.listen().unwrap();

    a.close_accept().unwrap();
    assert_eq!(a.accept().err().unwrap().code(), uvll::EOF);
})

test!(fn close_accept_concurrent() {
    let addr = next_test_ip4();
    let l = bind(addr.ip.to_string().as_slice(), addr.port).unwrap();
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
