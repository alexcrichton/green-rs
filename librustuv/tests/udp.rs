use std::io::net::ip::{SocketAddr, Ipv4Addr};
use std::io::test::{next_test_ip4, next_test_ip6};
use std::time::Duration;
use green::task::spawn;

use rustuv::{uvll, Udp};

test!(fn bind_error() {
    let addr = SocketAddr { ip: Ipv4Addr(0, 0, 0, 0), port: 1 };
    match Udp::bind(addr) {
        Ok(..) => panic!(),
        Err(e) => assert_eq!(e.code(), uvll::EACCES),
    }
})

test!(fn socket_smoke_test_ip4() {
    let server_ip = next_test_ip4();
    let client_ip = next_test_ip4();
    let (tx1, rx1) = channel();
    let (tx2, rx2) = channel();

    spawn(proc() {
        match Udp::bind(client_ip) {
            Ok(ref mut client) => {
                rx1.recv();
                client.send_to(&mut [99], server_ip).unwrap()
            }
            Err(..) => panic!()
        }
        tx2.send(());
    });

    match Udp::bind(server_ip) {
        Ok(ref mut server) => {
            tx1.send(());
            let mut buf = [0];
            match server.recv_from(&mut buf) {
                Ok((nread, src)) => {
                    assert_eq!(nread, 1);
                    assert_eq!(buf[0], 99);
                    assert_eq!(src, client_ip);
                }
                Err(..) => panic!()
            }
        }
        Err(..) => panic!()
    }
    rx2.recv();
})

test!(fn socket_smoke_test_ip6() {
    let server_ip = next_test_ip6();
    let client_ip = next_test_ip6();
    let (tx, rx) = channel::<()>();

    spawn(proc() {
        match Udp::bind(client_ip) {
            Ok(ref mut client) => {
                rx.recv();
                client.send_to(&mut [99], server_ip).unwrap()
            }
            Err(..) => panic!()
        }
    });

    match Udp::bind(server_ip) {
        Ok(ref mut server) => {
            tx.send(());
            let mut buf = [0];
            match server.recv_from(&mut buf) {
                Ok((nread, src)) => {
                    assert_eq!(nread, 1);
                    assert_eq!(buf[0], 99);
                    assert_eq!(src, client_ip);
                }
                Err(..) => panic!()
            }
        }
        Err(..) => panic!()
    }
})

test!(fn stream_smoke_test_ip4() {
    let server_ip = next_test_ip4();
    let client_ip = next_test_ip4();
    let (tx1, rx1) = channel();
    let (tx2, rx2) = channel();

    spawn(proc() {
        match Udp::bind(client_ip) {
            Ok(mut client) => {
                rx1.recv();
                client.send_to(&mut [99], server_ip).unwrap();
            }
            Err(..) => panic!()
        }
        tx2.send(());
    });

    match Udp::bind(server_ip) {
        Ok(mut server) => {
            tx1.send(());
            let mut buf = [0];
            match server.recv_from(&mut buf) {
                Ok((nread, _)) => {
                    assert_eq!(nread, 1);
                    assert_eq!(buf[0], 99);
                }
                Err(..) => panic!()
            }
        }
        Err(..) => panic!()
    }
    rx2.recv();
})

pub fn socket_name(addr: SocketAddr) {
    let server = Udp::bind(addr);

    assert!(server.is_ok());
    let mut server = server.unwrap();

    // Make sure socket_name gives
    // us the socket we binded to.
    let so_name = server.socket_name();
    assert!(so_name.is_ok());
    assert_eq!(addr, so_name.unwrap());
}

test!(fn socket_name_ip4() {
    socket_name(next_test_ip4());
})

test!(fn socket_name_ip6() {
    socket_name(next_test_ip6());
})

test!(fn udp_clone_smoke() {
    let addr1 = next_test_ip4();
    let addr2 = next_test_ip4();
    let mut sock1 = Udp::bind(addr1).unwrap();
    let sock2 = Udp::bind(addr2).unwrap();

    spawn(proc() {
        let mut sock2 = sock2;
        let mut buf = [0, 0];
        assert_eq!(sock2.recv_from(&mut buf), Ok((1, addr1)));
        assert_eq!(buf[0], 1);
        sock2.send_to(&mut [2], addr1).unwrap();
    });

    let sock3 = sock1.clone();

    let (tx1, rx1) = channel();
    let (tx2, rx2) = channel();
    spawn(proc() {
        let mut sock3 = sock3;
        rx1.recv();
        sock3.send_to(&mut [1], addr2).unwrap();
        tx2.send(());
    });
    tx1.send(());
    let mut buf = [0, 0];
    assert_eq!(sock1.recv_from(&mut buf), Ok((1, addr2)));
    rx2.recv();
})

test!(fn udp_clone_two_read() {
    let addr1 = next_test_ip4();
    let addr2 = next_test_ip4();
    let mut sock1 = Udp::bind(addr1).unwrap();
    let sock2 = Udp::bind(addr2).unwrap();
    let (tx1, rx) = channel();
    let tx2 = tx1.clone();

    spawn(proc() {
        let mut sock2 = sock2;
        sock2.send_to(&mut [1], addr1).unwrap();
        rx.recv();
        sock2.send_to(&mut [2], addr1).unwrap();
        rx.recv();
    });

    let sock3 = sock1.clone();

    let (done, rx) = channel();
    spawn(proc() {
        let mut sock3 = sock3;
        let mut buf = [0, 0];
        sock3.recv_from(&mut buf).unwrap();
        tx2.send(());
        done.send(());
    });
    let mut buf = [0, 0];
    sock1.recv_from(&mut buf).unwrap();
    tx1.send(());

    rx.recv();
})

test!(fn udp_clone_two_write() {
    let addr1 = next_test_ip4();
    let addr2 = next_test_ip4();
    let mut sock1 = Udp::bind(addr1).unwrap();
    let sock2 = Udp::bind(addr2).unwrap();

    let (tx, rx) = channel();
    let (serv_tx, serv_rx) = channel();

    spawn(proc() {
        let mut sock2 = sock2;
        let mut buf = [0, 1];

        rx.recv();
        match sock2.recv_from(&mut buf) {
            Ok(..) => {}
            Err(e) => panic!("failed receive: {}", e),
        }
        serv_tx.send(());
    });

    let sock3 = sock1.clone();

    let (done, rx) = channel();
    let tx2 = tx.clone();
    spawn(proc() {
        let mut sock3 = sock3;
        match sock3.send_to(&mut [1], addr2) {
            Ok(..) => { let _ = tx2.send_opt(()); }
            Err(..) => {}
        }
        done.send(());
    });
    match sock1.send_to(&mut [2], addr2) {
        Ok(..) => { let _ = tx.send_opt(()); }
        Err(..) => {}
    }
    drop(tx);

    rx.recv();
    serv_rx.recv();
})

test!(fn recv_from_timeout() {
    let addr1 = next_test_ip4();
    let addr2 = next_test_ip4();
    let mut a = Udp::bind(addr1).unwrap();

    let (tx, rx) = channel();
    let (tx2, rx2) = channel();
    spawn(proc() {
        let mut a = Udp::bind(addr2).unwrap();
        assert_eq!(a.recv_from(&mut [0]), Ok((1, addr1)));
        assert_eq!(a.send_to(&mut [0], addr1), Ok(()));
        rx.recv();
        assert_eq!(a.send_to(&mut [0], addr1), Ok(()));

        tx2.send(());
    });

    // Make sure that reads time out, but writes can continue
    a.set_read_timeout(Some(Duration::milliseconds(20)));
    assert_eq!(a.recv_from(&mut [0]).err().unwrap().code(), uvll::ECANCELED);
    assert_eq!(a.recv_from(&mut [0]).err().unwrap().code(), uvll::ECANCELED);
    assert_eq!(a.send_to(&mut [0], addr2), Ok(()));

    // Cloned handles should be able to block
    let mut a2 = a.clone();
    assert_eq!(a2.recv_from(&mut [0]), Ok((1, addr2)));

    // Clearing the timeout should allow for receiving
    a.set_read_timeout(None);
    tx.send(());
    assert_eq!(a2.recv_from(&mut [0]), Ok((1, addr2)));

    // Make sure the child didn't die
    rx2.recv();
})
