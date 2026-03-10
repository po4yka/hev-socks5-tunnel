//! C FFI entry points (feature = "c-api").
//!
//! These functions mirror the original `hev-socks5-tunnel` C API for drop-in
//! compatibility with existing integrations.
//!
//! All `_main` / `_main_from_*` functions are **blocking**: they block the
//! calling thread until `hev_socks5_tunnel_quit` is called or an error occurs.
//! `_quit` and `_stats` are safe to call from any thread at any time.

use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_uint};
use std::sync::{Arc, Mutex};

use once_cell::sync::OnceCell;
use tokio::runtime::Runtime;
use tokio_util::sync::CancellationToken;

use hs5t_config::Config;

use crate::{tunnel_api::run_tunnel, Stats};

// ── Global state ──────────────────────────────────────────────────────────────

/// Tokio runtime — created once, reused across repeated start/stop cycles.
static C_RUNTIME: OnceCell<Runtime> = OnceCell::new();

/// Cancellation token for the currently-running tunnel.
/// Replaced on each `_main*` call so repeated start/stop works correctly.
static C_CANCEL: Mutex<Option<Arc<CancellationToken>>> = Mutex::new(None);

/// Traffic statistics for the currently-running tunnel.
static C_STATS: Mutex<Option<Arc<Stats>>> = Mutex::new(None);

fn get_runtime() -> Option<&'static Runtime> {
    C_RUNTIME
        .get_or_try_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
        })
        .ok()
}

// ── Public C API ──────────────────────────────────────────────────────────────

/// Start the tunnel using a configuration file.  Blocks until quit or error.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
///
/// `config_path` must be a valid, null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn hev_socks5_tunnel_main_from_file(
    config_path: *const c_char,
    tun_fd: c_int,
) -> c_int {
    if config_path.is_null() {
        return -1;
    }
    // SAFETY: caller guarantees `config_path` is valid and null-terminated.
    let path = match unsafe { CStr::from_ptr(config_path) }.to_str() {
        Ok(s) => s.to_owned(),
        Err(_) => return -1,
    };
    let config = match Config::from_file(&path) {
        Ok(c) => Arc::new(c),
        Err(_) => return -1,
    };
    run_with_config(config, tun_fd)
}

/// Legacy alias for `hev_socks5_tunnel_main_from_file`.
///
/// # Safety
///
/// Same as `hev_socks5_tunnel_main_from_file`.
#[no_mangle]
pub unsafe extern "C" fn hev_socks5_tunnel_main(
    config_path: *const c_char,
    tun_fd: c_int,
) -> c_int {
    // SAFETY: forwarding the same safety contract.
    unsafe { hev_socks5_tunnel_main_from_file(config_path, tun_fd) }
}

/// Start the tunnel using an in-memory YAML config string.  Blocks until quit or error.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
///
/// `config_str` must point to `config_len` valid, UTF-8 bytes.
#[no_mangle]
pub unsafe extern "C" fn hev_socks5_tunnel_main_from_str(
    config_str: *const u8,
    config_len: c_uint,
    tun_fd: c_int,
) -> c_int {
    if config_str.is_null() {
        return -1;
    }
    // SAFETY: caller guarantees `config_str` points to `config_len` valid bytes.
    let bytes = unsafe { std::slice::from_raw_parts(config_str, config_len as usize) };
    let yaml = match std::str::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => return -1,
    };
    let config = match yaml.parse::<Config>() {
        Ok(c) => Arc::new(c),
        Err(_) => return -1,
    };
    run_with_config(config, tun_fd)
}

/// Cancel the running tunnel.  Safe to call from any thread.
#[no_mangle]
pub extern "C" fn hev_socks5_tunnel_quit() {
    if let Ok(guard) = C_CANCEL.lock() {
        if let Some(ref token) = *guard {
            token.cancel();
        }
    }
}

/// Write traffic statistics to the provided out-parameters.
///
/// # Safety
///
/// All four pointer arguments must be valid, non-null, writable `size_t`
/// locations.  The function writes atomically-snapshotted counters.
#[no_mangle]
pub unsafe extern "C" fn hev_socks5_tunnel_stats(
    tx_packets: *mut usize,
    tx_bytes: *mut usize,
    rx_packets: *mut usize,
    rx_bytes: *mut usize,
) {
    if tx_packets.is_null() || tx_bytes.is_null() || rx_packets.is_null() || rx_bytes.is_null() {
        return;
    }
    if let Ok(guard) = C_STATS.lock() {
        if let Some(ref s) = *guard {
            let (tp, tb, rp, rb) = s.snapshot();
            // SAFETY: caller guarantees all pointers are valid and writable.
            unsafe {
                *tx_packets = tp as usize;
                *tx_bytes = tb as usize;
                *rx_packets = rp as usize;
                *rx_bytes = rb as usize;
            }
        }
    }
}

// ── Internal helper ───────────────────────────────────────────────────────────

fn run_with_config(config: Arc<Config>, tun_fd: c_int) -> c_int {
    let rt = match get_runtime() {
        Some(r) => r,
        None => return -1,
    };

    let cancel = Arc::new(CancellationToken::new());
    let stats = Arc::new(Stats::new());

    // Publish the new cancel/stats handles before entering block_on so that
    // `hev_socks5_tunnel_quit` and `hev_socks5_tunnel_stats` can observe them.
    if let Ok(mut guard) = C_CANCEL.lock() {
        *guard = Some(cancel.clone());
    }
    if let Ok(mut guard) = C_STATS.lock() {
        *guard = Some(stats.clone());
    }

    let result = rt.block_on(run_tunnel(config, tun_fd, (*cancel).clone(), stats));

    // Clear handles after the tunnel exits.
    if let Ok(mut guard) = C_CANCEL.lock() {
        *guard = None;
    }

    match result {
        Ok(()) => 0,
        Err(_) => -1,
    }
}
