# Loop 6: Session Handlers (TDD + tokio)

Repository: /mnt/nvme/home/po4yka/hev-socks5-tunnel
Crate: hs5t-session
C references:
  - src/hev-socks5-session-tcp.c (382 LOC)
  - src/hev-socks5-session-udp.c (448 LOC)
  - src/hev-socks5-session.c (88 LOC)

## C→Rust translation map:
- `HEV_TASK_YIELD` → `tokio::task::yield_now().await`
- `HEV_TASK_WAITIO` → natural `await` on AsyncRead/AsyncWrite
- lwip pcb callbacks → `mpsc::channel` from smoltcp socket events
- `HevTaskMutex` → `tokio::sync::Mutex`
- Session cancellation → `tokio_util::sync::CancellationToken`

## Session lifecycle to replicate exactly

### TCP session:
1. TcpStream::connect(socks5_proxy)
2. SOCKS5 handshake (optional user/pass auth)
3. SOCKS5 CONNECT with destination addr (IPv4, IPv6, or domain)
4. Bidirectional splice loop:
   - FORWARD: smoltcp TcpSocket → ring_buffer → proxy TcpStream
   - BACKWARD: proxy TcpStream → ring_buffer → smoltcp TcpSocket
5. EOF handling: half-close propagation (shutdown(Write) on one side)

### UDP session:
1. Same connect + auth as TCP
2. SOCKS5 UDP ASSOCIATE request
3. Bidirectional UDP relay with SOCKS5 UDP encapsulation
4. Two modes: udp-in-udp (standard), udp-in-tcp (for firewalled networks)

## TDD sequence

### Iter 1-4: TCP session with mocked proxy
Use tokio_test::io::Builder to mock proxy side (or use MockBuilder).
WRITE TESTS FIRST:
- SOCKS5 NoAuth handshake: verify bytes `[0x05, 0x01, 0x00]` → `[0x05, 0x00]`
- SOCKS5 UserPass auth: verify correct auth bytes exchanged
- CONNECT IPv4: verify `[0x05, 0x01, 0x00, 0x01, ...]` format
- CONNECT IPv6: verify `[0x05, 0x01, 0x00, 0x04, ...]` format
- CONNECT domain: verify `[0x05, 0x01, 0x00, 0x03, len, ...]` format
- Forward splice: 1MB random data passes through intact
- Backward splice: 1MB random data passes through intact
- EOF from proxy propagates to smoltcp socket close
- EOF from smoltcp propagates to proxy socket shutdown(Write)

### Iter 5-8: UDP session
WRITE TESTS FIRST:
- UDP ASSOCIATE request format correct
- UDP frame encapsulation: `[0x00, 0x00, 0x00, atyp, addr, port, data]`
- UDP frame decapsulation: strip SOCKS5 header, deliver payload
- udp-in-tcp: control TcpStream EOF → session terminates

### Iter 9-12: Session lifecycle
- CancellationToken cancel → session terminates within 100ms
- SessionSet: max_session_count=3, 4th session evicts oldest (via oldest cancel)
- Stats: tx_bytes, rx_bytes, tx_packets, rx_packets counters (atomic u64) increment correctly

### Iter 13-16: Integration tests (real SOCKS5 proxy)
- Spawn microsocks as subprocess (apt install microsocks or compile from source)
- TCP session: HTTP GET "http://httpbin.org/get" round-trips correctly
- UDP session: DNS query for "google.com" round-trips correctly
- Session terminates cleanly when proxy closes

### Iter 17-20: Sanitizer runs
- ASAN: `RUSTFLAGS="-Z sanitizer=address" cargo +nightly test -p hs5t-session`
- TSAN: `RUSTFLAGS="-Z sanitizer=thread" cargo +nightly test -p hs5t-session`
- Fix all races and memory errors found

## Exit criteria
- TCP + UDP session unit tests pass with mock proxy
- Integration tests pass with real microsocks
- CancellationToken cancel: session terminates < 100ms
- ASAN + TSAN clean
- Write LOOP_COMPLETE to signal completion

LOOP_COMPLETE
