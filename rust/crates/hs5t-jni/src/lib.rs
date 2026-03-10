//! Android JNI shim for hev-socks5-tunnel.
//!
//! Exposes three JNI entry points consumed by `hev.htproxy.TProxyService`:
//!
//! - [`Java_hev_htproxy_TProxyService_TProxyStartService`] — blocking, runs
//!   until stopped or an error occurs.
//! - [`Java_hev_htproxy_TProxyService_TProxyStopService`] — cancels the
//!   running tunnel from any thread.
//! - [`Java_hev_htproxy_TProxyService_TProxyGetStats`] — returns traffic
//!   counters as a `long[4]`: `[tx_pkt, rx_pkt, tx_bytes, rx_bytes]`.

use std::sync::{Arc, Mutex};

use jni::objects::{JObject, JString};
use jni::sys::{jint, jlongArray};
use jni::JNIEnv;
use once_cell::sync::OnceCell;
use tokio::runtime::Runtime;
use tokio_util::sync::CancellationToken;

use hs5t_core::Stats;

// ── Global state ──────────────────────────────────────────────────────────────

/// Single Tokio runtime for the lifetime of the process.
static RUNTIME: OnceCell<Runtime> = OnceCell::new();

/// Cancellation token for the currently-running tunnel.
/// Reset on each `TProxyStartService` call to support restart.
static CANCEL: Mutex<Option<Arc<CancellationToken>>> = Mutex::new(None);

/// Traffic statistics for the currently-running tunnel.
static STATS: Mutex<Option<Arc<Stats>>> = Mutex::new(None);

fn get_runtime() -> Option<&'static Runtime> {
    RUNTIME
        .get_or_try_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
        })
        .ok()
}

// ── JNI entry points ──────────────────────────────────────────────────────────

/// Start the tunnel.  Called from a dedicated Java background thread.
///
/// Blocks until `TProxyStopService` is called or an error occurs.
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "system" fn Java_hev_htproxy_TProxyService_TProxyStartService(
    mut env: JNIEnv,
    _obj: JObject,
    config_path: JString,
    tun_fd: jint,
) -> jint {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        start_service_impl(&mut env, config_path, tun_fd)
    }))
    .unwrap_or(-1)
}

/// Stop the running tunnel.  Safe to call from any thread.
#[no_mangle]
pub extern "system" fn Java_hev_htproxy_TProxyService_TProxyStopService(
    _env: JNIEnv,
    _obj: JObject,
) {
    let _ = std::panic::catch_unwind(|| {
        if let Ok(guard) = CANCEL.lock() {
            if let Some(ref token) = *guard {
                token.cancel();
            }
        }
    });
}

/// Return traffic statistics as a Java `long[4]` in the order
/// `[tx_pkt, rx_pkt, tx_bytes, rx_bytes]`, or `null` on error.
#[no_mangle]
pub extern "system" fn Java_hev_htproxy_TProxyService_TProxyGetStats(
    env: JNIEnv,
    _obj: JObject,
) -> jlongArray {
    // Fetch the snapshot inside catch_unwind to guard against unexpected panics.
    let snapshot = std::panic::catch_unwind(get_stats_snapshot).ok().flatten();

    let (tx_pkt, rx_pkt, tx_bytes, rx_bytes) = match snapshot {
        Some(v) => v,
        None => return std::ptr::null_mut(),
    };

    match env.new_long_array(4) {
        Ok(arr) => {
            let values: [i64; 4] = [
                tx_pkt as i64,
                rx_pkt as i64,
                tx_bytes as i64,
                rx_bytes as i64,
            ];
            // `arr` is a valid JLongArray of length 4; `values` has 4 elements.
            if env.set_long_array_region(&arr, 0, &values).is_ok() {
                arr.into_raw()
            } else {
                std::ptr::null_mut()
            }
        }
        Err(_) => std::ptr::null_mut(),
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn start_service_impl(env: &mut JNIEnv, config_path: JString, tun_fd: jint) -> jint {
    let path: String = match env.get_string(&config_path) {
        Ok(s) => s.into(),
        Err(_) => return -1,
    };

    let config = match hs5t_config::Config::from_file(&path) {
        Ok(c) => std::sync::Arc::new(c),
        Err(_) => return -1,
    };

    let rt = match get_runtime() {
        Some(r) => r,
        None => return -1,
    };

    let cancel = Arc::new(CancellationToken::new());
    let stats = Arc::new(Stats::new());

    // Publish handles before entering block_on so TProxyStopService /
    // TProxyGetStats can observe them from other threads.
    if let Ok(mut guard) = CANCEL.lock() {
        *guard = Some(cancel.clone());
    }
    if let Ok(mut guard) = STATS.lock() {
        *guard = Some(stats.clone());
    }

    let result = rt.block_on(hs5t_core::run_tunnel(
        config,
        tun_fd,
        (*cancel).clone(),
        stats,
    ));

    // Clear handles after the tunnel exits.
    if let Ok(mut guard) = CANCEL.lock() {
        *guard = None;
    }

    match result {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

/// Returns `(tx_pkt, rx_pkt, tx_bytes, rx_bytes)` from the active stats cell,
/// or `None` if the tunnel is not running.
fn get_stats_snapshot() -> Option<(u64, u64, u64, u64)> {
    let guard = STATS.lock().ok()?;
    let s = guard.as_ref()?;
    let (tx_pkt, tx_bytes, rx_pkt, rx_bytes) = s.snapshot();
    // Reorder: JNI array layout is [tx_pkt, rx_pkt, tx_bytes, rx_bytes].
    Some((tx_pkt, rx_pkt, tx_bytes, rx_bytes))
}
