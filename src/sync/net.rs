//! The broadcast addresses of this PC's networks (e.g. 192.168.1.255 for 192.168.1.x/24).
//!
//! A plain 255.255.255.255 broadcast leaves Windows through one network adapter only, which
//! may be a VPN or a virtual adapter rather than the Wi-Fi the other device is on. Sending to
//! each adapter's own broadcast address reaches every local network.

use std::net::Ipv4Addr;

use windows::Win32::NetworkManagement::IpHelper::{
    GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_MULTICAST, GetAdaptersAddresses,
    IP_ADAPTER_ADDRESSES_LH,
};
use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
use windows::Win32::Networking::WinSock::{AF_INET, SOCKADDR_IN};

pub fn broadcast_addresses() -> Vec<Ipv4Addr> {
    let mut addresses = vec![Ipv4Addr::BROADCAST];
    let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
    // Asked twice at most: the first answer may be "the buffer is too small, it takes N bytes"
    let mut size = 16 * 1024u32;
    for _ in 0..2 {
        // u64 elements keep the buffer aligned for the structures written into it
        let mut buffer = vec![0u64; size as usize / 8 + 1];
        let first = buffer.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH;
        let result = unsafe { GetAdaptersAddresses(AF_INET.0 as u32, flags, None, Some(first), &mut size) };
        if result != 0 {
            continue;
        }
        let mut adapter = first as *const IP_ADAPTER_ADDRESSES_LH;
        while let Some(a) = unsafe { adapter.as_ref() } {
            if a.OperStatus == IfOperStatusUp {
                let mut unicast = a.FirstUnicastAddress;
                while let Some(u) = unsafe { unicast.as_ref() } {
                    let sockaddr = u.Address.lpSockaddr as *const SOCKADDR_IN;
                    if let Some(sockaddr) = unsafe { sockaddr.as_ref() }
                        && sockaddr.sin_family == AF_INET
                    {
                        let ip = Ipv4Addr::from(u32::from_be(unsafe { sockaddr.sin_addr.S_un.S_addr }));
                        let prefix = u.OnLinkPrefixLength as u32;
                        if !ip.is_loopback() && !ip.is_link_local() && (1..31).contains(&prefix) {
                            let host_bits = u32::MAX >> prefix;
                            let broadcast = Ipv4Addr::from(u32::from(ip) | host_bits);
                            if !addresses.contains(&broadcast) {
                                addresses.push(broadcast);
                            }
                        }
                    }
                    unicast = u.Next;
                }
            }
            adapter = a.Next;
        }
        break;
    }
    addresses
}
