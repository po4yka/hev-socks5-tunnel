//! FreeBSD TUN driver stub.
//!
//! All operations return TunnelError::NotSupported until implemented.

use std::net::{Ipv4Addr, Ipv6Addr};
use std::os::unix::io::RawFd;

use crate::{TunnelDriver, TunnelError};

/// FreeBSD TUN device stub.
pub struct FreeBsdTunnel;

impl TunnelDriver for FreeBsdTunnel {
    fn open(_name: Option<&str>, _multi_queue: bool) -> Result<Self, TunnelError> {
        Err(TunnelError::NotSupported)
    }

    fn fd(&self) -> RawFd {
        -1
    }

    fn name(&self) -> &str {
        ""
    }

    fn index(&self) -> u32 {
        0
    }

    fn set_mtu(&self, _mtu: u32) -> Result<(), TunnelError> {
        Err(TunnelError::NotSupported)
    }

    fn set_ipv4(&self, _addr: Ipv4Addr, _prefix: u8) -> Result<(), TunnelError> {
        Err(TunnelError::NotSupported)
    }

    fn set_ipv6(&self, _addr: Ipv6Addr, _prefix: u8) -> Result<(), TunnelError> {
        Err(TunnelError::NotSupported)
    }

    fn set_up(&self) -> Result<(), TunnelError> {
        Err(TunnelError::NotSupported)
    }

    fn set_down(&self) -> Result<(), TunnelError> {
        Err(TunnelError::NotSupported)
    }
}
