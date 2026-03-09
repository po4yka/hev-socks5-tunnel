# Loop 7: Core Tunnel Event Loop (Spec-Driven)

Repository: /mnt/nvme/home/po4yka/hev-socks5-tunnel
Crate: hs5t-core
C reference: src/hev-socks5-tunnel.c (727 LOC)

## Architecture: three C coroutines → three tokio tasks

1. `io_loop_task`: reads TUN fd → smoltcp iface.poll() → accepts new sessions
2. `accept_loop_task`: polls smoltcp for new TCP/UDP sockets → spawns session tasks
3. `shutdown_task`: tokio::signal::ctrl_c() + CancellationToken broadcast

## smoltcp integration pattern:
```rust
loop {
    // 1. Read all available TUN packets into smoltcp device rx queue
    while let Ok(n) = tun.try_read(&mut buf) {
        device.rx_queue.push(buf[..n].to_vec());
    }
    // 2. poll smoltcp
    iface.poll(Instant::now(), &mut device, &mut socket_set);
    // 3. Detect new TCP connections → spawn TcpSession
    for (handle, socket) in socket_set.iter_mut() {
        if let Some(tcp) = AnySocket::downcast::<TcpSocket>(socket) {
            if tcp.is_open() && !active_sessions.contains(&handle) {
                spawn_tcp_session(handle, cancel.clone());
            }
        }
    }
    // 4. Write smoltcp tx packets back to TUN fd
    while let Some(pkt) = device.tx_queue.pop() {
        tun.write_all(&pkt).await?;
    }
    // 5. Wait for next event: TUN readable OR poll_delay
    tokio::select! {
        _ = tun_readable() => {},
        _ = tokio::time::sleep(iface.poll_delay(...)) => {},
        _ = cancel.cancelled() => break,
    }
}
```

## Step 1: Write specification first (tests/integration/spec.md)

Describe every observable behavior:
- Tunnel initializes TUN interface with configured IP/MTU
- TCP connection to tunneled destination → SOCKS5 relay session spawned
- UDP packet to non-DNS destination → SOCKS5 UDP relay session spawned
- UDP packet to mapdns.network address port 53 → DnsCache::handle() response returned via TUN
- SIGINT → all sessions cancelled via CancellationToken → TUN down → exit 0
- max_session_count=N → (N+1)th session evicts oldest session
- post-up-script executes after TUN interface is UP
- pre-down-script executes before TUN interface is taken DOWN

## TDD sequence (implement spec test by test)

### Iter 2-5: smoltcp device + iface initialization
- TunDevice struct implementing smoltcp::phy::Device trait
- Interface::new() with IPv4/IPv6 addresses from config
- Unit test: inject IP packet → iface.poll() → receive in socket (loopback)

### Iter 6-10: Accept loop
- Detect new smoltcp TcpSocket (is_open, not active) → spawn TcpSession
- Detect UDP packets to non-53 port → spawn UdpSession
- Detect UDP packets to DNS port (53) → route to DnsCache::handle()
- DnsCache response returned as UDP packet via TUN

### Iter 11-16: Shutdown and lifecycle
- SIGINT via tokio::signal::unix::signal(SignalKind::interrupt())
- CancellationToken broadcast to all active session JoinHandles
- Wait for all JoinHandles with 5-second timeout
- post-up-script: std::process::Command spawn after TUN up
- pre-down-script: spawn before tunnel down

### Iter 17-22: Full integration test (requires CAP_NET_ADMIN, #[ignore])
- Real TUN interface (198.18.0.1/15 default)
- Real microsocks SOCKS5 proxy on 127.0.0.1:1080
- TCP: curl --interface tun0 http://1.1.1.1 returns HTTP response
- UDP: dig @198.18.0.1 google.com via TUN returns DNS answer
- SIGINT: all sessions clean up, TUN goes down

### Iter 23-25: Performance baseline + sanitizers
- Benchmark: iperf3 TCP throughput through tunnel; record baseline MBps
- Benchmark: 1KB TCP round-trip latency; record baseline μs
- TSAN: 10 concurrent sessions, no races
- ASAN: full integration test run, no leaks

## Exit criteria
- All spec tests from spec.md pass
- Full integration test: TCP + UDP end-to-end working
- SIGINT shuts down cleanly (no resource leaks via valgrind/LSAN)
- TSAN clean under concurrent 10-session load
- Performance within 2x of C baseline
- Write LOOP_COMPLETE to signal completion

LOOP_COMPLETE
