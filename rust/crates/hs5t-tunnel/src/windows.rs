//! Windows WinTun driver stub.
//!
//! TunnelDriver is unix-only; this module exposes a freestanding open()
//! that always returns NotSupported until a full wintun implementation lands.

use crate::TunnelError;

/// Windows TUN device stub (via wintun crate, not yet implemented).
pub struct WindowsTunnel;

impl WindowsTunnel {
    /// Always returns [`TunnelError::NotSupported`].
    pub fn open(_name: Option<&str>, _multi_queue: bool) -> Result<Self, TunnelError> {
        Err(TunnelError::NotSupported)
    }
}
