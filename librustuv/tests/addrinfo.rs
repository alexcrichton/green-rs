use rustuv::get_host_addresses;
use std::io::net::ip::Ipv4Addr;

test!(fn dns_smoke_test() {
    let ipaddrs = get_host_addresses("localhost").unwrap();
    let mut found_local = false;
    let local_addr = &Ipv4Addr(127, 0, 0, 1);
    for addr in ipaddrs.iter() {
        found_local = found_local || addr == local_addr;
    }
    assert!(found_local);
})

test!(fn issue_10663() {
    // Something should happen here, but this certainly shouldn't cause
    // everything to die. The actual outcome we don't care too much about.
    get_host_addresses("example.com").unwrap();
})
