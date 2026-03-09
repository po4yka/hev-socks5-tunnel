use std::net::{Ipv4Addr, Ipv6Addr};

#[cfg(unix)]
use std::os::unix::io::RawFd;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "linux")]
pub use linux::LinuxTunnel;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub use macos::MacosTunnel;

#[cfg(target_os = "freebsd")]
mod freebsd;

#[cfg(target_os = "freebsd")]
pub use freebsd::FreeBsdTunnel;

#[cfg(target_os = "netbsd")]
mod netbsd;

#[cfg(target_os = "netbsd")]
pub use netbsd::NetBsdTunnel;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "windows")]
pub use windows::WindowsTunnel;

#[derive(Debug, thiserror::Error)]
pub enum TunnelError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Ioctl error: {0}")]
    Ioctl(String),
    #[error("Not supported on this platform")]
    NotSupported,
}

/// Platform-independent TUN device driver.
///
/// Implementations are responsible for opening a TUN interface,
/// configuring its address/MTU/state, and exposing the raw fd for I/O.
#[cfg(unix)]
pub trait TunnelDriver: Send + Sync {
    /// Open (or create) a TUN interface.
    ///
    /// `name` – optional interface name (e.g. `"tun0"`); kernel picks one when `None`.
    /// `multi_queue` – enable IFF_MULTI_QUEUE for parallel I/O threads.
    fn open(name: Option<&str>, multi_queue: bool) -> Result<Self, TunnelError>
    where
        Self: Sized;

    /// Return the raw file descriptor for the TUN device.
    fn fd(&self) -> RawFd;

    /// Return the kernel-assigned interface name (e.g. `"tun0"`).
    fn name(&self) -> &str;

    /// Return the kernel interface index (matches `if_nametoindex`).
    fn index(&self) -> u32;

    /// Set the MTU for the interface.
    fn set_mtu(&self, mtu: u32) -> Result<(), TunnelError>;

    /// Assign an IPv4 address with the given prefix length.
    fn set_ipv4(&self, addr: Ipv4Addr, prefix: u8) -> Result<(), TunnelError>;

    /// Assign an IPv6 address with the given prefix length.
    fn set_ipv6(&self, addr: Ipv6Addr, prefix: u8) -> Result<(), TunnelError>;

    /// Bring the interface up (IFF_UP).
    fn set_up(&self) -> Result<(), TunnelError>;

    /// Bring the interface down (clear IFF_UP).
    fn set_down(&self) -> Result<(), TunnelError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    // Verify TunnelError implements std::error::Error (compile-time check).
    fn _assert_error<E: std::error::Error>() {}
    fn _check_tunnel_error() {
        _assert_error::<TunnelError>();
    }

    // Verify TunnelError variants are constructible.
    #[test]
    fn tunnel_error_io_from() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "no dev");
        let err: TunnelError = io_err.into();
        assert!(matches!(err, TunnelError::Io(_)));
    }

    #[test]
    fn tunnel_error_ioctl_display() {
        let err = TunnelError::Ioctl("TUNSETIFF failed".to_string());
        assert_eq!(err.to_string(), "Ioctl error: TUNSETIFF failed");
    }

    #[test]
    fn tunnel_error_not_supported_display() {
        let err = TunnelError::NotSupported;
        assert_eq!(err.to_string(), "Not supported on this platform");
    }

    // Verify TunnelDriver trait object is Send + Sync (compile-time check).
    #[cfg(unix)]
    fn _assert_send_sync<T: Send + Sync>() {}

    // A minimal mock that satisfies the trait bounds to confirm the API compiles.
    #[cfg(unix)]
    struct MockTunnel;

    #[cfg(unix)]
    impl TunnelDriver for MockTunnel {
        fn open(_name: Option<&str>, _multi_queue: bool) -> Result<Self, TunnelError> {
            Ok(MockTunnel)
        }
        fn fd(&self) -> RawFd {
            -1
        }
        fn name(&self) -> &str {
            "mock0"
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

    #[cfg(unix)]
    #[test]
    fn mock_tunnel_trait_compiles() {
        let t = MockTunnel::open(None, false).unwrap();
        assert_eq!(t.fd(), -1);
        assert_eq!(t.name(), "mock0");
        assert_eq!(t.index(), 0);
        assert!(matches!(t.set_mtu(1500), Err(TunnelError::NotSupported)));
        assert!(matches!(
            t.set_ipv4("198.18.0.1".parse().unwrap(), 32),
            Err(TunnelError::NotSupported)
        ));
        assert!(matches!(t.set_up(), Err(TunnelError::NotSupported)));
        assert!(matches!(t.set_down(), Err(TunnelError::NotSupported)));
    }

    #[cfg(unix)]
    #[test]
    fn mock_tunnel_is_send_sync() {
        _assert_send_sync::<MockTunnel>();
    }
}
