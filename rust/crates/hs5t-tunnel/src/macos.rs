//! macOS UTUN driver stub.
//!
//! Full implementation would use SYSPROTO_CONTROL + UTUN_CONTROL_NAME.
//! All operations return TunnelError::NotSupported until implemented.

use std::net::{Ipv4Addr, Ipv6Addr};
use std::os::unix::io::RawFd;

use crate::{TunnelDriver, TunnelError};

/// macOS UTUN device stub.
///
/// Provides the correct compile-time shape for cross-compilation targets.
/// All runtime operations return [`TunnelError::NotSupported`].
pub struct MacosTunnel;

impl TunnelDriver for MacosTunnel {
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
