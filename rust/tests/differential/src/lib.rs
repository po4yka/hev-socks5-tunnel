// Differential test harness: C (dns_shim.c) vs Rust (hs5t-dns-cache).
// Note: ring-buffer differential tests removed in Loop 9 (C files deleted).

mod dns_diff;
mod dns_fuzz;
pub mod dns_shim_ffi;
mod dns_shim_smoke;
