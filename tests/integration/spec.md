# Specification: hs5t-core — Core Tunnel Event Loop

**Status:** REVISED (v2 — addresses spec.rejected ambiguities)
**Crate:** `hs5t-core`
**C reference:** `src/hev-socks5-tunnel.c` (727 LOC)
**Loops:** 7 (this spec), 7+

---

## Summary

`hs5t-core` implements the main tunnel event loop that bridges a TUN network
interface with a SOCKS5 proxy.  It reads raw IP packets from the TUN fd, feeds
TCP packets into a smoltcp TCP/IP stack, dispatches accepted TCP sockets to
per-session tasks, routes UDP packets to UdpSession or DnsCache (pre-parsed at
the device layer before smoltcp ever sees them), and shuts down cleanly on
SIGINT.

---

## Architectural Decisions (binding for implementation)

### Decision A: smoltcp ↔ TcpSession bridge — tokio::io::duplex()

**Chosen:** `tokio::io::duplex(buf_size)` from the tokio standard library.

`TcpSession::run` requires `L: AsyncRead + AsyncWrite + Unpin`. smoltcp's
`TcpSocket` does not implement those traits.  The bridge is:

```rust
let (mut smoltcp_side, session_side) = tokio::io::duplex(DUPLEX_BUF: usize = 65536);
```

- `session_side` is passed to `TcpSession::run` as the `local` argument.
- `smoltcp_side` is retained by `io_loop_task`.
- Each io_loop iteration, for every active session:
  1. Read available bytes from `smoltcp_socket.recv_slice()` → write into `smoltcp_side` (using `AsyncWriteExt::write`).
  2. Read available bytes from `smoltcp_side` (non-blocking poll) → write into `smoltcp_socket.send_slice()`.
- When smoltcp TcpSocket reaches `STATE_CLOSE_WAIT` or `STATE_CLOSED`, `smoltcp_side.shutdown()` is called to send EOF to `session_side`.
- When `session_side` is dropped (TcpSession task exits), writes to `smoltcp_side` will return `BrokenPipe`; io_loop treats this as session close and calls `smoltcp_socket.close()`.

**No new crate dependencies required.** `tokio::io::duplex` is part of `tokio::io`.

---

### Decision B: UDP detection — pre-parse at device layer (before smoltcp)

**Chosen:** Parse raw IP packets before feeding them to `device.rx_queue`.

smoltcp has no global UDP recv callback equivalent to lwip's `udp_recv`.
Pre-parsing at the device boundary is the cleanest, self-contained approach.

```
Raw IP packet arrives from TUN fd
    │
    ├─ parse IP header
    │       │
    │       ├─ protocol = UDP
    │       │       │
    │       │       ├─ dst_addr ∈ mapdns_network AND dst_port = 53
    │       │       │       └─→ DnsCache::handle(udp_payload) → write UDP response to TUN fd
    │       │       │
    │       │       └─ anything else
    │       │               └─→ spawn UdpSession (carries raw UDP payload + src/dst)
    │       │
    │       └─ protocol = TCP (or other)
    │               └─→ device.rx_queue.push_back(packet)  ← smoltcp sees only TCP
    │
    └─ smoltcp never receives UDP packets
```

**Consequence:** smoltcp's socket set holds **only** TCP sockets.  UDP session
management is entirely outside smoltcp.

---

### Decision C: TCP session spawn — full extraction sequence

The precise steps when `io_loop_task` detects a new TCP socket:

```rust
// Step 1: extract remote endpoint from smoltcp TcpSocket
let remote: IpEndpoint = tcp_socket.remote_endpoint()
    .ok_or("no remote endpoint")?;

// Step 2: convert to TargetAddr (hs5t-session type)
let target = TargetAddr::Ip(SocketAddr::from((remote.addr, remote.port)));

// Step 3: create duplex bridge (Decision A)
let (smoltcp_side, session_side) = tokio::io::duplex(65536_usize);

// Step 4: build TcpSession with proxy config from Config
let session = TcpSession::new(
    config.socks5.proxy_addr(),   // SocketAddr
    config.socks5.auth(),         // Auth
    target,                        // TargetAddr
);

// Step 5: create child cancellation token
let child_cancel = cancel.child_token();

// Step 6: spawn session task
let handle = tokio::spawn(async move {
    session.run(session_side, child_cancel).await
});

// Step 7: register in active sessions
active_sessions.insert(socket_handle, smoltcp_side, child_cancel_clone, handle);
```

The `active_sessions` table maps `SocketHandle → (DuplexStream, CancellationToken, JoinHandle)`.

---

## Definitions

| Term | Meaning |
|---|---|
| TUN fd | File descriptor for a Linux TUN interface (layer-3, IP packets) |
| smoltcp device | `TunDevice` — a `smoltcp::phy::Device` implementation backed by the TUN fd |
| smoltcp iface | `smoltcp::iface::Interface` managing the TCP state machine |
| socket set | `smoltcp::iface::SocketSet` holding active **TCP-only** sockets |
| TCP session | `hs5t-session::TcpSession` task relaying one TCP flow to SOCKS5 |
| UDP session | `hs5t-session::UdpSession` task relaying one UDP flow to SOCKS5 |
| duplex bridge | `tokio::io::duplex()` pair connecting smoltcp socket to TcpSession |
| DNS cache | `hs5t-dns-cache::DnsCache::handle()` |
| CancellationToken | `tokio_util::sync::CancellationToken` — broadcast shutdown signal |
| mapdns address | IPv4 address from `config.mapdns.network` (fake DNS server address in tunnel) |
| device layer | The point where raw IP bytes are classified before reaching smoltcp |

---

## Architecture

Single-task design (avoids Arc<Mutex<>> contention on smoltcp state):

```
io_loop_task (single tokio task):
  ┌─ read raw packets from TUN fd
  ├─ classify at device layer (UDP vs TCP)
  │     UDP:53→mapdns → DnsCache → write UDP response to TUN fd
  │     UDP:other     → spawn UdpSession
  │     TCP           → device.rx_queue.push_back
  ├─ iface.poll() — advance smoltcp TCP state machines
  ├─ detect new TCP sockets → spawn TcpSession (Decision C)
  ├─ drive duplex bridges: smoltcp_socket ↔ smoltcp_side (Decision A)
  └─ flush tx_queue → TUN fd

shutdown_task:
  └─ SIGINT → CancellationToken::cancel()
```

`io_loop_task` and `shutdown_task` are the only two spawned tasks.
TcpSession and UdpSession tasks are spawned from within `io_loop_task`.

---

## Acceptance Criteria

### AC-1: TUN Interface Initialization

**Given** a `Config` with `tunnel.name = "tun0"`, `tunnel.mtu = 1500`,
`tunnel.ipv4 = "198.18.0.1"`, `tunnel.ipv6 = None`
**When** `Tunnel::init(&config)` is called (extern tun_fd = -1)
**Then**
- A TUN interface named `tun0` is created (via ioctl TUNSETIFF)
- MTU is set to 1500
- IPv4 address `198.18.0.1/32` is configured on the interface
- Interface state is brought UP
- `post-up-script` (if configured) is spawned with args `(name, index, 0)`
- The function returns `Ok(tun_fd)` where `tun_fd >= 0`

**Given** `extern_tun_fd >= 0` is supplied
**When** `Tunnel::init_with_fd(extern_tun_fd)` is called
**Then**
- The supplied fd is set to non-blocking mode via `FIONBIO`
- No new TUN interface is created
- No scripts are executed

---

### AC-2: TUN Interface Teardown

**Given** the tunnel was initialized with `tun_fd_local = true`
**When** shutdown completes
**Then**
- `pre-down-script` (if configured) is spawned with args `(name, index, 1)`
- The TUN fd is closed
- The interface is removed

**Given** `tun_fd_local = false` (external fd was supplied)
**When** shutdown completes
**Then**
- No script is executed
- The fd is NOT closed (caller owns it)

---

### AC-3: smoltcp Device and Interface Initialization

**Given** a TUN fd
**When** `TunDevice::new(tun_fd)` is called
**Then**
- A `TunDevice` implementing `smoltcp::phy::Device` is created
- `capabilities()` reports MTU from config, `medium = Medium::Ip`
- Injecting a raw IPv4/TCP packet into `rx_queue` then calling `iface.poll()`
  causes smoltcp to process the packet (unit-testable without real TUN)

**When** `Interface::new(&config, &mut device)` is called
**Then**
- An `smoltcp::iface::Interface` is created with loopback IPv4 `127.0.0.1`
  and loopback IPv6 `::1` as the smoltcp-internal addresses
- The interface processes incoming TCP for any destination address (wildcard
  accept, matching C's `NETIF_FLAG_PRETEND_TCP` / any-IP relay behavior)

---

### AC-4: TCP Session Spawning

**Given** the tunnel is running
**When** a raw IPv4 TCP SYN packet arrives from the TUN fd
**Then**
1. The packet is pushed to `device.rx_queue` (it is TCP — see Decision B)
2. `iface.poll()` advances smoltcp state; smoltcp completes the three-way
   handshake and the TcpSocket reaches ESTABLISHED state
3. `io_loop_task` iterates `socket_set` and finds the socket is open and not
   yet in `active_sessions`
4. The remote endpoint is extracted: `tcp_socket.remote_endpoint()` returns
   `IpEndpoint { addr, port }` — this is the original connection destination
5. A `tokio::io::duplex(65536)` pair is created: `(smoltcp_side, session_side)`
6. `TcpSession::new(proxy_addr, auth, TargetAddr::Ip(remote_socketaddr))` is
   constructed
7. A child CancellationToken is created: `child_cancel = cancel.child_token()`
8. The session task is spawned:
   `tokio::spawn(session.run(session_side, child_cancel.clone()))`
9. `(socket_handle, smoltcp_side, child_cancel, join_handle)` is inserted into
   `active_sessions`
10. Session count is incremented atomically

**Given** `config.misc.max_session_count = N` and N sessions are active
**When** an (N+1)th TCP connection arrives
**Then**
- The oldest entry's `child_cancel.cancel()` is called (eviction)
- Its `smoltcp_side` is dropped (causes session_side to see EOF)
- The new session is spawned normally

**Given** `config.misc.max_session_count = 0`
**Then** no eviction occurs (unlimited sessions)

---

### AC-5: UDP Session Spawning (non-DNS)

**Given** the tunnel is running and `config.mapdns.cache_size > 0`
**When** a UDP packet arrives from the TUN fd with destination != mapdns address
**Then**
- The packet is classified at the device layer as UDP-non-DNS (Decision B)
- The raw UDP payload bytes and (src, dst) endpoints are extracted
- A `UdpSession` tokio task is spawned with this data
- **The packet is NOT pushed to `device.rx_queue`** (smoltcp never sees it)
- Session count is incremented
- Max session count eviction applies identically to TCP (AC-4)

**Given** the tunnel is running and `config.mapdns.cache_size = 0`
**When** any UDP packet arrives
**Then** a `UdpSession` is spawned (DNS intercept path is inactive; all UDP
goes to UdpSession)

---

### AC-6: DNS Cache Intercept (UDP to mapdns address)

**Given** `config.mapdns.cache_size > 0`, mapdns address = `198.18.0.0`,
mapdns port = `53`
**When** a UDP packet arrives with destination `198.18.0.0:53`
**Then**
- Classified at the device layer: UDP, dst_addr ∈ mapdns_network, dst_port=53
- The raw UDP payload (DNS wire bytes) is extracted from the IP packet
- `DnsCache::handle(query_bytes)` is called
- If it returns `Ok(response_bytes)`: a complete IPv4/UDP packet is constructed
  with `src=198.18.0.0:53`, `dst=original_src`, `payload=response_bytes`
  and written to the TUN fd directly (not via smoltcp)
- If it returns `Err(_)`: the packet is silently dropped, no session spawned
- **No `UdpSession` task is spawned**
- **No packet pushed to `device.rx_queue`** (smoltcp never sees it)

---

### AC-7: io_loop duplex bridge pumping (Decision A in detail)

**Given** an active TcpSession with `(socket_handle, smoltcp_side)` in `active_sessions`
**Each io_loop iteration:**
1. Attempt to read from smoltcp socket:
   ```rust
   let mut tmp = [0u8; 4096];
   if let Ok(n) = tcp_socket.recv_slice(&mut tmp) {
       smoltcp_side.write_all(&tmp[..n]).await?;
   }
   ```
2. Attempt to write to smoltcp socket (data produced by TcpSession):
   ```rust
   let mut tmp = [0u8; 4096];
   match smoltcp_side.try_read(&mut tmp) {
       Ok(0) => { tcp_socket.close(); }  // session_side EOF
       Ok(n) => { tcp_socket.send_slice(&tmp[..n]).ok(); }
       Err(WouldBlock) => {}
   }
   ```
3. When smoltcp TcpSocket state becomes `CLOSE_WAIT` or `CLOSED`:
   - Call `smoltcp_side.shutdown().await.ok()` → sends EOF to `session_side`
   - Remove socket from `socket_set`
   - Remove entry from `active_sessions`

---

### AC-8: Shutdown via SIGINT

**Given** the tunnel is running with active sessions
**When** SIGINT is received (or `ctrl_c` future fires)
**Then**
- `shutdown_task` calls `CancellationToken::cancel()`
- All child CancellationTokens (from `child_token()`) are simultaneously
  cancelled via the parent-child relationship
- All active `TcpSession` and `UdpSession` tasks observe cancellation and
  terminate within 5 seconds
- `io_loop_task` exits its read loop on seeing `cancel.cancelled()`
- All `JoinHandle`s are awaited with a 5-second timeout; tasks not finished
  within the timeout are dropped (not aborted — they hold no kernel resources)
- `tunnel_fini()` executes: pre-down-script (if any), TUN fd closed
- Process exits with status 0

---

### AC-9: Statistics Tracking

**Given** the tunnel is running
**When** packets flow through the tunnel
**Then**
- `tx_packets`: count of IP packets read from TUN fd (inbound from OS)
- `tx_bytes`: total bytes of those packets
- `rx_packets`: count of IP packets written to TUN fd (outbound to OS)
- `rx_bytes`: total bytes of those packets
- All four counters are `AtomicU64` with `Ordering::Relaxed`
- `Tunnel::stats()` returns `(tx_packets, tx_bytes, rx_packets, rx_bytes)` as `u64` snapshot

---

### AC-10: post-up-script / pre-down-script

**Given** `config.tunnel.post_up_script = "/etc/tun-up.sh"`
**When** TUN interface is brought UP
**Then**
- `/etc/tun-up.sh <iface_name> <iface_index> 0` is spawned (non-blocking,
  matching C `hev_exec_run(..., 0)` — fire and forget)
- The tunnel continues initialization without waiting for script exit

**Given** `config.tunnel.pre_down_script = "/etc/tun-down.sh"`
**When** tunnel teardown begins
**Then**
- `/etc/tun-down.sh <iface_name> <iface_index> 1` is spawned
- Tunnel waits for the script to exit before closing the TUN fd (matching C
  `hev_exec_run(..., 1)` — blocking wait)

---

### AC-11: SIGPIPE Ignored

**When** the tunnel process starts
**Then** SIGPIPE is set to SIG_IGN so broken proxy connections do not kill
the process.

---

## Input / Output Examples

### Example 1: TCP relay flow (happy path)

```
Input:  IPv4 TCP SYN packet → TUN fd
         src=10.0.0.1:12345 dst=1.1.1.1:80
Step 1: Device layer: protocol=TCP → device.rx_queue.push_back(pkt)
Step 2: iface.poll() → smoltcp TcpSocket opened, remote_endpoint={1.1.1.1, 80}
Step 3: io_loop detects new socket (not in active_sessions)
Step 4: target = TargetAddr::Ip("1.1.1.1:80".parse())
Step 5: (smoltcp_side, session_side) = tokio::io::duplex(65536)
Step 6: TcpSession::new(proxy_addr, auth, target).run(session_side, child_cancel)
Step 7: Session connects SOCKS5 proxy → CONNECT 1.1.1.1:80
Step 8: io_loop bridges smoltcp_socket ↔ smoltcp_side each iteration
Output: Bytes flow: TUN → smoltcp → smoltcp_side → session_side → SOCKS5 proxy → internet
        On FIN: smoltcp_side.shutdown() → session_side EOF → TcpSession returns Ok(())
```

### Example 2: UDP DNS intercept

```
Input:  IPv4 UDP packet → TUN fd
         src=10.0.0.1:54321 dst=198.18.0.0:53
         payload: DNS query for "google.com" type A
Step 1: Device layer: UDP, dst=198.18.0.0:53, mapdns match
Step 2: DnsCache::handle(dns_wire_bytes) → Ok(response_bytes)
Step 3: Build IPv4/UDP response packet: src=198.18.0.0:53 dst=10.0.0.1:54321
Output: Response written to TUN fd (smoltcp never involved)
```

### Example 3: UDP non-DNS session

```
Input:  IPv4 UDP packet → TUN fd
         src=10.0.0.1:12345 dst=8.8.8.8:53
Step 1: Device layer: UDP, dst=8.8.8.8:53 (not in mapdns_network)
Step 2: Extract src, dst, payload
Step 3: UdpSession spawned with (src, dst, payload, proxy_addr, auth, child_cancel)
Output: UdpSession connects SOCKS5 UDP ASSOCIATE, relays datagram
```

### Example 4: Session eviction

```
Config: max_session_count = 100
State:  100 active sessions (oldest = entry[0] with socket_handle_0, child_cancel_0)
Event:  101st TCP SYN arrives
Action: child_cancel_0.cancel()  → TcpSession-0 terminates
        smoltcp_side_0 dropped   → session_side_0 sees EOF (belt-and-suspenders)
        101st session spawned normally
```

---

## Edge Cases and Error Conditions

| Condition | Expected behavior |
|---|---|
| TUN open fails (EPERM, no CAP_NET_ADMIN) | `init()` returns `Err`, process exits with non-zero status |
| TUN MTU set fails | `init()` returns `Err` |
| TUN read returns `WouldBlock` | io_loop waits on `tun.readable()` future (no busy-loop) |
| TUN write returns `WouldBlock` | tx_queue packet is retried on next poll cycle |
| SOCKS5 proxy unreachable | `TcpSession`/`UdpSession` return `Err`, removed from active_sessions |
| smoltcp socket `recv_slice` returns 0 | Skip (no data available); do not interpret as EOF |
| smoltcp socket `send_slice` returns 0 | smoltcp send buffer full; retry next iteration |
| `smoltcp_side.try_read` returns 0 | session_side closed; call `tcp_socket.close()` |
| `DnsCache::handle()` returns error | Packet silently dropped; no panic |
| post-up-script not executable | Logged as warning; tunnel continues |
| pre-down-script exits non-zero | Logged as warning; tunnel continues shutdown |
| Duplicate SIGINT (Ctrl-C twice) | Second signal is no-op (token already cancelled) |
| smoltcp `remote_endpoint()` returns None | Should not happen post-ESTABLISHED; log error, skip socket |
| TcpSession task panics | JoinHandle returns Err; session removed from active_sessions |

---

## Non-Functional Requirements

### Performance
- Throughput target: within 2x of the C implementation on iperf3 TCP benchmark
- Latency target: within 2x of C on 1 KB round-trip benchmark
- No busy-wait loops; io_loop must sleep on `tun.readable()` when idle
- Duplex buffer size: 65536 bytes (matches `config.misc.tcp_buffer_size` default)

### Correctness
- Zero unsafe code in `hs5t-core` (unsafe is permitted only in `hs5t-tunnel` for ioctl calls)
- No `unwrap()` on Result/Option in production paths; all errors propagate or are logged
- ASAN + LSAN: no heap leaks after clean shutdown
- TSAN: no data races under 10 concurrent sessions

### Observability
- All significant state transitions logged via `tracing` at appropriate levels:
  - INFO: tunnel up, tunnel down, session spawned (with remote addr), session count
  - DEBUG: packet counts, DNS intercept hits
  - WARN: script failures, session evictions
  - ERROR: TUN open failures, task spawn failures

---

## Out of Scope

- IPv6 smoltcp configuration (addresses parsed from config but IPv6 smoltcp
  socket listening is a future iteration)
- UDP-in-TCP encapsulation (Loop 6 UdpSession responsibility)
- Android JNI entry point (`hs5t-jni` crate)
- Windows / macOS TUN drivers (`hs5t-tunnel` platform stubs)
- Benchmark automation (manual iperf3 run, not a CI gate)

---

## Implementation Notes

### TunDevice (smoltcp phy::Device)

```rust
pub struct TunDevice {
    // Only used in unit tests; io_loop drives it directly via mutable ref
    rx_queue: VecDeque<Vec<u8>>,  // packets waiting to be consumed by smoltcp
    tx_queue: VecDeque<Vec<u8>>,  // packets produced by smoltcp, pending write to TUN
}

impl smoltcp::phy::Device for TunDevice {
    // receive() pops from rx_queue
    // transmit() pushes to tx_queue
}
```

The TUN fd is owned by io_loop_task, not by TunDevice. Device classification
and TUN reads/writes happen in io_loop_task code, not inside the Device impl.

### io_loop_task pseudo-code (incorporating all 3 decisions)

```rust
loop {
    // ── Phase 1: drain TUN fd ──────────────────────────────────────
    while let Ok(n) = tun.try_read(&mut buf) {
        stats.tx_packets.fetch_add(1, Relaxed);
        stats.tx_bytes.fetch_add(n as u64, Relaxed);

        // Decision B: classify BEFORE smoltcp
        match classify_ip_packet(&buf[..n]) {
            IpClass::TcpOrOther => {
                device.rx_queue.push_back(buf[..n].to_vec());
            }
            IpClass::UdpDns { src, payload } if mapdns_active => {
                if let Ok(resp) = dns_cache.handle(&payload) {
                    let pkt = build_udp_response(mapdns_addr, src, resp);
                    tun.write_all(&pkt).await?;
                }
                // drop on Err
            }
            IpClass::Udp { src, dst, payload } => {
                spawn_udp_session(src, dst, payload, &config, &cancel, &mut sessions);
            }
        }
    }

    // ── Phase 2: advance smoltcp ───────────────────────────────────
    iface.poll(Instant::now(), &mut device, &mut socket_set);

    // ── Phase 3: detect new TCP sockets + spawn (Decision C) ──────
    for (handle, socket) in socket_set.iter_mut() {
        if let Some(tcp) = socket.downcast::<TcpSocket>() {
            if tcp.is_open() && !sessions.contains_key(&handle) {
                let remote = tcp.remote_endpoint().unwrap();
                let target = TargetAddr::Ip(into_socketaddr(remote));
                let (smoltcp_side, session_side) = tokio::io::duplex(65536);
                let child_cancel = cancel.child_token();
                let session = TcpSession::new(config.proxy_addr(), config.auth(), target);
                let jh = tokio::spawn(session.run(session_side, child_cancel.clone()));
                sessions.insert(handle, (smoltcp_side, child_cancel, jh));
                evict_oldest_if_needed(&mut sessions, config.max_sessions);
            }
        }
    }

    // ── Phase 4: pump duplex bridges (Decision A) ─────────────────
    for (handle, (smoltcp_side, _, _)) in sessions.iter_mut() {
        let tcp = socket_set.get_mut::<TcpSocket>(*handle);
        // smoltcp → session
        let mut tmp = [0u8; 4096];
        if let Ok(n) = tcp.recv_slice(&mut tmp) {
            smoltcp_side.write_all(&tmp[..n]).await.ok();
        }
        // session → smoltcp
        match smoltcp_side.try_read(&mut tmp) {
            Ok(0) => { tcp.close(); sessions_to_remove.push(*handle); }
            Ok(n) => { tcp.send_slice(&tmp[..n]).ok(); }
            Err(_) => {}
        }
        // smoltcp closed
        if !tcp.is_open() {
            smoltcp_side.shutdown().await.ok();
            sessions_to_remove.push(*handle);
        }
    }
    for h in sessions_to_remove.drain(..) { sessions.remove(&h); }

    // ── Phase 5: flush smoltcp tx → TUN fd ────────────────────────
    while let Some(pkt) = device.tx_queue.pop_front() {
        tun.write_all(&pkt).await?;
        stats.rx_packets.fetch_add(1, Relaxed);
        stats.rx_bytes.fetch_add(pkt.len() as u64, Relaxed);
    }

    // ── Phase 6: wait for next event ──────────────────────────────
    let delay = iface.poll_delay(Instant::now(), &socket_set)
        .unwrap_or(Duration::from_millis(50));
    tokio::select! {
        _ = tun.readable() => {},
        _ = tokio::time::sleep(delay) => {},
        _ = cancel.cancelled() => break,
    }
}
```

### ActiveSessions structure

```rust
struct SessionEntry {
    smoltcp_side: tokio::io::DuplexStream,
    cancel: CancellationToken,          // child token
    handle: tokio::task::JoinHandle<io::Result<()>>,
}

struct ActiveSessions {
    map: IndexMap<SocketHandle, SessionEntry>,  // ordered: oldest-first by insertion
    max: usize,  // 0 = unlimited
}

impl ActiveSessions {
    fn insert(&mut self, handle: SocketHandle, entry: SessionEntry) {
        if self.max > 0 && self.map.len() >= self.max {
            // evict oldest
            if let Some((_, oldest)) = self.map.shift_remove_index(0) {
                oldest.cancel.cancel();
                // smoltcp_side drop sends EOF to session_side as belt-and-suspenders
            }
        }
        self.map.insert(handle, entry);
    }
}
```

---

## Test Plan

### Unit tests (no CAP_NET_ADMIN)

| ID | Description |
|---|---|
| U-01 | `TunDevice`: inject IPv4/TCP packet into rx_queue, call iface.poll(), verify TcpSocket opens |
| U-02 | `TunDevice`: smoltcp transmits data, verify it appears in tx_queue |
| U-03 | `classify_ip_packet`: UDP dst=mapdns:53 → IpClass::UdpDns; UDP dst=8.8.8.8:53 → IpClass::Udp; TCP → IpClass::TcpOrOther |
| U-04 | `ActiveSessions::insert` with max=3: 4th insert evicts oldest, oldest.cancel is called |
| U-05 | `ActiveSessions::insert` with max=0: no eviction, 100 inserts all present |
| U-06 | Duplex bridge: write bytes to smoltcp_side, read from session_side — data arrives intact |
| U-07 | Duplex bridge: drop session_side → smoltcp_side try_read returns 0 bytes (EOF) |
| U-08 | Statistics counters increment correctly on rx/tx |
| U-09 | Cancellation: parent cancel → child token cancelled → spawned task observes within 100 ms |

### Integration tests (#[ignore] — require CAP_NET_ADMIN)

| ID | Description |
|---|---|
| I-01 | Tunnel init creates TUN interface; post-up-script runs asynchronously |
| I-02 | TCP: HTTP GET through tunnel reaches mock SOCKS5 (microsocks), returns 200 |
| I-03 | UDP DNS: `dig` through tunnel hits DnsCache intercept, returns mapped A record |
| I-04 | SIGINT: clean shutdown, all sessions cancelled, TUN removed, exit 0 |
| I-05 | max_session_count=2: 3rd connection evicts 1st; both new sessions work |

### Sanitizer runs (CI nightly)

- ASAN + LSAN: run integration tests I-01..I-04 under address sanitizer
- TSAN: run I-02 with 10 concurrent curl requests simultaneously

---

*End of specification — v2.*
