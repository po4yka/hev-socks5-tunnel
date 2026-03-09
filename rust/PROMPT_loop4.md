# Loop 4: Platform TUN Drivers

Repository: /mnt/nvme/home/po4yka/hev-socks5-tunnel
Crate: hs5t-tunnel
C source references:
  - src/hev-tunnel-linux.c (213 LOC)
  - src/hev-tunnel-macos.c (259 LOC)
  - src/hev-tunnel-freebsd.c (242 LOC)
  - src/hev-tunnel-netbsd.c (225 LOC)
  - src/hev-tunnel-windows.c (214 LOC)

## Step 1: Define the TunnelDriver trait FIRST

```rust
use std::net::{Ipv4Addr, Ipv6Addr};
use std::os::unix::io::RawFd;

#[derive(Debug, thiserror::Error)]
pub enum TunnelError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Ioctl error: {0}")]
    Ioctl(String),
    #[error("Not supported on this platform")]
    NotSupported,
}

pub trait TunnelDriver: Send + Sync {
    fn open(name: Option<&str>, multi_queue: bool) -> Result<Self, TunnelError> where Self: Sized;
    fn fd(&self) -> RawFd;
    fn name(&self) -> &str;
    fn index(&self) -> u32;
    fn set_mtu(&self, mtu: u32) -> Result<(), TunnelError>;
    fn set_ipv4(&self, addr: Ipv4Addr, prefix: u8) -> Result<(), TunnelError>;
    fn set_ipv6(&self, addr: Ipv6Addr, prefix: u8) -> Result<(), TunnelError>;
    fn set_up(&self) -> Result<(), TunnelError>;
    fn set_down(&self) -> Result<(), TunnelError>;
}
```

## Step 2: Linux implementation

Use nix crate for ioctl calls. Use OwnedFd for ownership. Drop closes fd.

Key Linux TUN operations:
- open("/dev/net/tun", O_RDWR)
- ioctl(TUNSETIFF) with IFF_TUN | IFF_NO_PI flags
- For multi_queue: add IFF_MULTI_QUEUE flag
- ioctl(SIOCSIFMTU) via socket(AF_INET, SOCK_DGRAM)
- ioctl(SIOCSIFADDR) for IPv4
- ioctl(SIOCSIFNETMASK) for prefix
- ioctl(SIOCSIFFLAGS) IFF_UP to bring up

## Step 3: macOS implementation

Use SYSPROTO_CONTROL + UTUN_CONTROL_NAME socket:
- socket(PF_SYSTEM, SOCK_DGRAM, SYSPROTO_CONTROL)
- ioctl CTLIOCGINFO to get ctl_id
- connect() with sockaddr_ctl to get utunN interface

## Step 4: FreeBSD/NetBSD stubs

Compile-time stubs that return TunnelError::NotSupported

## Step 5: Windows stub

Behind #[cfg(target_os = "windows")], use wintun crate stub

## SAFETY REQUIREMENTS (enforced by CI grep):
Every unsafe block MUST have `// SAFETY:` comment explaining:
- The pointer is valid (what it points to and why)
- The ioctl number is correct for this kernel
- No aliasing occurs

## Tests (require CAP_NET_ADMIN, marked #[ignore]):
1. open TUN, fd is valid (>= 0)
2. set_mtu(1500) succeeds
3. set_ipv4(198.18.0.1, 32) succeeds; verify via ip addr show
4. set_up / set_down toggle; verify via ip link show
5. Drop closes fd: verify fd -1 after drop via fcntl

## Differential test:
Compare `ip addr show tun0` output after C setup vs Rust setup: mtu, IP, state must match.

## Exit criteria
- Linux tests pass: `sudo cargo test -p hs5t-tunnel -- --include-ignored`
- Zero unsafe blocks without SAFETY comment (CI grep check)
- macOS target compiles: `cargo check --target x86_64-apple-darwin -p hs5t-tunnel`
- ASAN clean on Linux TUN tests
- Write LOOP_COMPLETE to signal completion

LOOP_COMPLETE
