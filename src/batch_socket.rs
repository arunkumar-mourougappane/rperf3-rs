//! Batch socket operations for improved performance.
//!
//! This module provides high-performance batch socket operations using
//! platform-specific APIs like `sendmmsg` and `recvmmsg` on Linux.
//! on platforms without native batch support, it falls back to standard
//! socket operations.
//!
//! # Performance
//!
//! Batch operations can improve UDP throughput by 30-50% at high packet
//! rates by reducing system call overhead.

use std::io;
use std::net::SocketAddr;

/// Maximum number of messages to batch in a single operation.
///
/// This value balances throughput gains against latency. Too high
/// and we introduce unnecessary latency, too low and we don't get
/// the full benefit of batching.
pub const MAX_BATCH_SIZE: usize = 64;

/// A batch of UDP packets ready to send.
///
/// This structure holds multiple packets that can be sent in a single
/// `sendmmsg` system call on Linux, or sent individually on other platforms.
#[derive(Debug)]
pub struct UdpSendBatch {
    /// The packets to send
    packets: Vec<Vec<u8>>,
    /// Target addresses for each packet
    addresses: Vec<SocketAddr>,
}

impl UdpSendBatch {
    /// Creates a new empty batch.
    pub fn new() -> Self {
        Self {
            packets: Vec::with_capacity(MAX_BATCH_SIZE),
            addresses: Vec::with_capacity(MAX_BATCH_SIZE),
        }
    }

    /// Creates a new batch with the specified capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            packets: Vec::with_capacity(capacity),
            addresses: Vec::with_capacity(capacity),
        }
    }

    /// Adds a packet to the batch.
    ///
    /// Returns `true` if the packet was added, `false` if the batch is full.
    pub fn add(&mut self, packet: Vec<u8>, addr: SocketAddr) -> bool {
        if self.packets.len() >= MAX_BATCH_SIZE {
            return false;
        }
        self.packets.push(packet);
        self.addresses.push(addr);
        true
    }

    /// Returns the number of packets in the batch.
    pub fn len(&self) -> usize {
        self.packets.len()
    }

    /// Returns `true` if the batch is empty.
    pub fn is_empty(&self) -> bool {
        self.packets.is_empty()
    }

    /// Returns `true` if the batch is full.
    pub fn is_full(&self) -> bool {
        self.packets.len() >= MAX_BATCH_SIZE
    }

    /// Clears the batch, removing all packets.
    pub fn clear(&mut self) {
        self.packets.clear();
        self.addresses.clear();
    }

    /// Sends all packets in the batch using the most efficient method available.
    ///
    /// On Linux, uses `sendmmsg` for batched sending. On other platforms,
    /// falls back to individual `send_to` calls.
    ///
    /// Returns the number of bytes sent and the number of packets successfully sent.
    #[cfg(target_os = "linux")]
    pub async fn send(&mut self, socket: &tokio::net::UdpSocket) -> io::Result<(usize, usize)> {
        if self.is_empty() {
            return Ok((0, 0));
        }

        // Use sendmmsg for batch sending on Linux
        self.send_mmsg(socket).await
    }

    /// Sends all packets in the batch using individual send_to calls.
    #[cfg(not(target_os = "linux"))]
    pub async fn send(&mut self, socket: &tokio::net::UdpSocket) -> io::Result<(usize, usize)> {
        if self.is_empty() {
            return Ok((0, 0));
        }

        let mut total_bytes = 0;
        let mut packets_sent = 0;

        for (packet, addr) in self.packets.iter().zip(self.addresses.iter()) {
            match socket.send_to(packet, addr).await {
                Ok(n) => {
                    total_bytes += n;
                    packets_sent += 1;
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // Socket buffer full, stop sending
                    break;
                }
                Err(e) => return Err(e),
            }
        }

        // Remove sent packets from the batch
        self.packets.drain(..packets_sent);
        self.addresses.drain(..packets_sent);

        Ok((total_bytes, packets_sent))
    }

    /// Linux-specific implementation using sendmmsg.
    #[cfg(target_os = "linux")]
    async fn send_mmsg(&mut self, socket: &tokio::net::UdpSocket) -> io::Result<(usize, usize)> {
        use std::os::unix::io::AsRawFd;

        if self.is_empty() {
            return Ok((0, 0));
        }

        let fd = socket.as_raw_fd();
        let packets = &self.packets;
        let addresses = &self.addresses;
        
        // Call the synchronous helper that does all the unsafe work
        let result = send_mmsg_sync(fd, packets, addresses)?;
        
        // Remove sent packets from the batch
        if result.1 > 0 {
            self.packets.drain(..result.1);
            self.addresses.drain(..result.1);
        }
        
        Ok(result)
    }
}

/// Synchronous helper for sendmmsg (Linux only)
#[cfg(target_os = "linux")]
fn send_mmsg_sync(
    fd: std::os::unix::io::RawFd,
    packets: &[Vec<u8>],
    addresses: &[SocketAddr],
) -> io::Result<(usize, usize)> {
    use libc::{iovec, mmsghdr, sendmmsg, sockaddr_in, sockaddr_in6, sockaddr_storage, AF_INET, AF_INET6, MSG_DONTWAIT};
    use std::mem;

    let count = packets.len();
    
    // Prepare mmsghdr structures
    let mut msgvec: Vec<mmsghdr> = Vec::with_capacity(count);
    let mut iovecs: Vec<iovec> = Vec::with_capacity(count);
    let mut addrs: Vec<sockaddr_storage> = Vec::with_capacity(count);

    for (packet, addr) in packets.iter().zip(addresses.iter()) {
        // Prepare iovec for this packet
        let iov = iovec {
            iov_base: packet.as_ptr() as *mut _,
            iov_len: packet.len(),
        };
        iovecs.push(iov);

        // Convert SocketAddr to sockaddr_storage
        let mut storage: sockaddr_storage = unsafe { mem::zeroed() };
        let addr_len = match addr {
            SocketAddr::V4(v4) => {
                let sin = sockaddr_in {
                    sin_family: AF_INET as u16,
                    sin_port: v4.port().to_be(),
                    sin_addr: libc::in_addr {
                        s_addr: u32::from_ne_bytes(v4.ip().octets()),
                    },
                    sin_zero: [0; 8],
                };
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        &sin as *const _ as *const u8,
                        &mut storage as *mut _ as *mut u8,
                        mem::size_of::<sockaddr_in>(),
                    );
                }
                mem::size_of::<sockaddr_in>() as u32
            }
            SocketAddr::V6(v6) => {
                let sin6 = sockaddr_in6 {
                    sin6_family: AF_INET6 as u16,
                    sin6_port: v6.port().to_be(),
                    sin6_flowinfo: 0,
                    sin6_addr: libc::in6_addr {
                        s6_addr: v6.ip().octets(),
                    },
                    sin6_scope_id: 0,
                };
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        &sin6 as *const _ as *const u8,
                        &mut storage as *mut _ as *mut u8,
                        mem::size_of::<sockaddr_in6>(),
                    );
                }
                mem::size_of::<sockaddr_in6>() as u32
            }
        };
        addrs.push(storage);

        // Prepare mmsghdr
        let mut hdr: mmsghdr = unsafe { mem::zeroed() };
        hdr.msg_hdr.msg_name = addrs.last_mut().unwrap() as *mut _ as *mut _;
        hdr.msg_hdr.msg_namelen = addr_len;
        hdr.msg_hdr.msg_iov = iovecs.last_mut().unwrap() as *mut _;
        hdr.msg_hdr.msg_iovlen = 1;
        msgvec.push(hdr);
    }

    // Perform the sendmmsg operation - this is non-blocking (MSG_DONTWAIT)
    let ret = unsafe {
        sendmmsg(
            fd,
            msgvec.as_mut_ptr(),
            count as u32,
            MSG_DONTWAIT,
        )
    };

    if ret < 0 {
        let err = io::Error::last_os_error();
        // If the socket would block, return what we can send (0)
        if err.kind() == io::ErrorKind::WouldBlock {
            return Ok((0, 0));
        }
        return Err(err);
    }

    // Calculate total bytes sent
    let packets_sent = ret as usize;
    let mut total_bytes = 0;
    for i in 0..packets_sent {
        total_bytes += msgvec[i].msg_len as usize;
    }

    Ok((total_bytes, packets_sent))
}

impl Default for UdpSendBatch {
    fn default() -> Self {
        Self::new()
    }
}

/// A batch of UDP packets received.
///
/// This structure holds multiple received packets from a single
/// `recvmmsg` system call on Linux.
#[derive(Debug)]
pub struct UdpRecvBatch {
    /// The received packets
    packets: Vec<Vec<u8>>,
    /// Source addresses for each packet
    addresses: Vec<SocketAddr>,
    /// Number of valid packets in the batch
    count: usize,
}

impl UdpRecvBatch {
    /// Creates a new empty batch with pre-allocated buffers.
    pub fn new() -> Self {
        let mut packets = Vec::with_capacity(MAX_BATCH_SIZE);
        for _ in 0..MAX_BATCH_SIZE {
            packets.push(vec![0u8; 65536]); // Max UDP packet size
        }

        Self {
            packets,
            addresses: vec![SocketAddr::from(([0, 0, 0, 0], 0)); MAX_BATCH_SIZE],
            count: 0,
        }
    }

    /// Receives a batch of packets using the most efficient method available.
    ///
    /// On Linux, uses `recvmmsg` for batched receiving. On other platforms,
    /// receives packets individually up to MAX_BATCH_SIZE.
    ///
    /// Returns the number of packets received.
    #[cfg(target_os = "linux")]
    pub async fn recv(&mut self, socket: &tokio::net::UdpSocket) -> io::Result<usize> {
        self.recv_mmsg(socket).await
    }

    /// Receives packets using individual recv_from calls.
    #[cfg(not(target_os = "linux"))]
    pub async fn recv(&mut self, socket: &tokio::net::UdpSocket) -> io::Result<usize> {
        self.count = 0;

        // Try to receive multiple packets without blocking
        for i in 0..MAX_BATCH_SIZE {
            match socket.try_recv_from(&mut self.packets[i]) {
                Ok((n, addr)) => {
                    self.packets[i].truncate(n);
                    self.addresses[i] = addr;
                    self.count += 1;
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // No more packets available
                    break;
                }
                Err(e) => return Err(e),
            }
        }

        // If we got no packets with try_recv, do a blocking receive for at least one
        if self.count == 0 {
            match socket.recv_from(&mut self.packets[0]).await {
                Ok((n, addr)) => {
                    self.packets[0].truncate(n);
                    self.addresses[0] = addr;
                    self.count = 1;
                }
                Err(e) => return Err(e),
            }
        }

        Ok(self.count)
    }

    /// Linux-specific implementation using recvmmsg.
    #[cfg(target_os = "linux")]
    async fn recv_mmsg(&mut self, socket: &tokio::net::UdpSocket) -> io::Result<usize> {
        use std::os::unix::io::AsRawFd;

        let fd = socket.as_raw_fd();

        // Prepare buffers for receiving
        for packet in self.packets.iter_mut() {
            packet.resize(65536, 0);
        }

        // Try non-blocking receive first
        let count = match recv_mmsg_sync(fd, &mut self.packets, &mut self.addresses, false) {
            Ok(count) if count > 0 => count,
            Ok(0) => {
                // No packets available, wait for socket to be readable
                socket.readable().await?;
                // Try again after socket is readable
                recv_mmsg_sync(fd, &mut self.packets, &mut self.addresses, false)?
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                // Wait for socket to be readable
                socket.readable().await?;
                // Try again after socket is readable
                recv_mmsg_sync(fd, &mut self.packets, &mut self.addresses, false)?
            }
            Ok(count) => count,
            Err(e) => return Err(e),
        };

        self.count = count;
        Ok(count)
    }

    /// Returns the number of packets in the batch.
    pub fn len(&self) -> usize {
        self.count
    }

    /// Returns `true` if the batch is empty.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Gets a reference to a packet and its source address by index.
    pub fn get(&self, index: usize) -> Option<(&[u8], SocketAddr)> {
        if index < self.count {
            Some((&self.packets[index], self.addresses[index]))
        } else {
            None
        }
    }

    /// Returns an iterator over the packets and their source addresses.
    pub fn iter(&self) -> impl Iterator<Item = (&[u8], SocketAddr)> {
        self.packets[..self.count]
            .iter()
            .zip(self.addresses[..self.count].iter())
            .map(|(p, a)| (p.as_slice(), *a))
    }
}

impl Default for UdpRecvBatch {
    fn default() -> Self {
        Self::new()
    }
}

/// Converts a sockaddr_storage to a SocketAddr (Linux-specific).
#[cfg(target_os = "linux")]
fn sockaddr_to_socketaddr(storage: &libc::sockaddr_storage, _len: u32) -> io::Result<SocketAddr> {
    use libc::{AF_INET, AF_INET6};
    use std::net::{Ipv4Addr, Ipv6Addr};

    unsafe {
        match storage.ss_family as i32 {
            AF_INET => {
                let sin: *const libc::sockaddr_in = storage as *const _ as *const _;
                let addr = Ipv4Addr::from(u32::from_be((*sin).sin_addr.s_addr).to_ne_bytes());
                let port = u16::from_be((*sin).sin_port);
                Ok(SocketAddr::from((addr, port)))
            }
            AF_INET6 => {
                let sin6: *const libc::sockaddr_in6 = storage as *const _ as *const _;
                let addr = Ipv6Addr::from((*sin6).sin6_addr.s6_addr);
                let port = u16::from_be((*sin6).sin6_port);
                Ok(SocketAddr::from((addr, port)))
            }
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Unsupported address family",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_capacity() {
        let mut batch = UdpSendBatch::new();
        assert_eq!(batch.len(), 0);
        assert!(batch.is_empty());
        assert!(!batch.is_full());

        for i in 0..MAX_BATCH_SIZE {
            let packet = vec![i as u8; 100];
            let addr = SocketAddr::from(([127, 0, 0, 1], 5000));
            assert!(batch.add(packet, addr));
        }

        assert_eq!(batch.len(), MAX_BATCH_SIZE);
        assert!(!batch.is_empty());
        assert!(batch.is_full());

        // Should not be able to add more
        let packet = vec![0u8; 100];
        let addr = SocketAddr::from(([127, 0, 0, 1], 5000));
        assert!(!batch.add(packet, addr));
    }

    #[test]
    fn test_batch_clear() {
        let mut batch = UdpSendBatch::new();
        
        for i in 0..10 {
            let packet = vec![i as u8; 100];
            let addr = SocketAddr::from(([127, 0, 0, 1], 5000));
            batch.add(packet, addr);
        }

        assert_eq!(batch.len(), 10);
        batch.clear();
        assert_eq!(batch.len(), 0);
        assert!(batch.is_empty());
    }
}

/// Synchronous helper for recvmmsg (Linux only)
#[cfg(target_os = "linux")]
fn recv_mmsg_sync(
    fd: std::os::unix::io::RawFd,
    packets: &mut [Vec<u8>],
    addresses: &mut [SocketAddr],
    _blocking: bool,
) -> io::Result<usize> {
    use libc::{iovec, mmsghdr, recvmmsg, sockaddr_storage, MSG_DONTWAIT};
    use std::mem;

    let count = packets.len().min(MAX_BATCH_SIZE);

    // Prepare mmsghdr structures
    let mut msgvec: Vec<mmsghdr> = Vec::with_capacity(count);
    let mut iovecs: Vec<iovec> = Vec::with_capacity(count);
    let mut addrs: Vec<sockaddr_storage> = Vec::with_capacity(count);

    for i in 0..count {
        // Prepare iovec for this packet
        let iov = iovec {
            iov_base: packets[i].as_mut_ptr() as *mut _,
            iov_len: packets[i].len(),
        };
        iovecs.push(iov);

        // Prepare address storage
        let storage: sockaddr_storage = unsafe { mem::zeroed() };
        addrs.push(storage);

        // Prepare mmsghdr
        let mut hdr: mmsghdr = unsafe { mem::zeroed() };
        hdr.msg_hdr.msg_name = addrs.last_mut().unwrap() as *mut _ as *mut _;
        hdr.msg_hdr.msg_namelen = mem::size_of::<sockaddr_storage>() as u32;
        hdr.msg_hdr.msg_iov = iovecs.last_mut().unwrap() as *mut _;
        hdr.msg_hdr.msg_iovlen = 1;
        msgvec.push(hdr);
    }

    // Perform the recvmmsg operation
    let ret = unsafe {
        recvmmsg(
            fd,
            msgvec.as_mut_ptr(),
            count as u32,
            MSG_DONTWAIT,
            std::ptr::null_mut(),
        )
    };

    if ret < 0 {
        return Err(io::Error::last_os_error());
    }

    let received_count = ret as usize;

    // Truncate buffers and extract addresses
    for i in 0..received_count {
        let bytes_received = msgvec[i].msg_len as usize;
        packets[i].truncate(bytes_received);

        // Convert sockaddr_storage to SocketAddr
        addresses[i] = sockaddr_to_socketaddr(&addrs[i], msgvec[i].msg_hdr.msg_namelen)?;
    }

    Ok(received_count)
}
