# Loop 8: Entry Points

Repository: /mnt/nvme/home/po4yka/hev-socks5-tunnel
Crates: hs5t-bin, hs5t-jni

## CLI binary (hs5t-bin)

### Behavior to implement:
- argv[1] = config file path (required)
- Optional: read tun_fd from environment variable HEV_SOCKS5_TUNNEL_FD
- --version flag: print "VERSION_MAJOR.MINOR.MICRO COMMIT_ID" to stdout
- --help flag: usage message
- SIGINT → tokio::signal::ctrl_c() → CancellationToken cancel
- SIGTERM → same as SIGINT
- Exit code 0 on clean shutdown, 1 on error

### Tests:
1. `./hs5t --version` prints version string, exits 0
2. `./hs5t --help` prints usage, exits 0
3. `./hs5t /nonexistent` exits 1 with error message to stderr
4. Integration: start binary with valid config, send SIGINT, exits 0 within 3 seconds

## Android JNI (hs5t-jni)

### CRITICAL: JNI method names MUST match Java exactly:
- `Java_hev_htproxy_TProxyService_TProxyStartService`
  (env: JNIEnv, obj: JObject, config_path: JString, tun_fd: jint) -> jint
- `Java_hev_htproxy_TProxyService_TProxyStopService`
  (env: JNIEnv, obj: JObject) -> ()
- `Java_hev_htproxy_TProxyService_TProxyGetStats`
  (env: JNIEnv, obj: JObject) -> jlongArray [tx_pkt, rx_pkt, tx_bytes, rx_bytes]

### CRITICAL SAFETY rules:
- Every JNI function MUST use std::panic::catch_unwind() to prevent Rust panic crossing FFI
- If catch_unwind catches a panic: log the error, return error code (-1 for Start, 0-array for Stats)
- Every unsafe block MUST have `// SAFETY:` comment

### Runtime management:
```rust
static RUNTIME: OnceCell<Runtime> = OnceCell::new();
static CANCEL: OnceCell<Arc<CancellationToken>> = OnceCell::new();
static STATS: OnceCell<Arc<TunnelStats>> = OnceCell::new();
```

### Test:
- `cargo build --target aarch64-linux-android -p hs5t-jni` succeeds

## C FFI library (hs5t-core, feature = "c-api")

### Functions to expose:
```c
int hev_socks5_tunnel_main_from_file(const char *config_path, int tun_fd);
int hev_socks5_tunnel_main_from_str(const unsigned char *config_str, unsigned int len, int tun_fd);
void hev_socks5_tunnel_quit(void);
void hev_socks5_tunnel_stats(size_t *tx_pkt, size_t *rx_pkt, size_t *tx_bytes, size_t *rx_bytes);
```

### cbindgen setup:
- Add cbindgen.toml to hs5t-core
- build.rs generates include/hev-main-rust.h
- CI check: diff include/hev-main.h include/hev-main-rust.h → zero functional differences

## Exit criteria
- CLI binary: starts and responds to SIGINT cleanly within 3 seconds
- JNI: `cargo build --target aarch64-linux-android -p hs5t-jni` exits 0
- C FFI header diff vs original: zero functional differences
- Zero unsafe blocks without SAFETY comment (CI grep)
- Write LOOP_COMPLETE to signal completion

LOOP_COMPLETE
