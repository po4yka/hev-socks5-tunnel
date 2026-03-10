# Integration Specification

**Status:** Current
**Runtime:** Rust-only
**Primary crates:** `hs5t-bin`, `hs5t-core`, `hs5t-session`, `hs5t-tunnel`

## Summary

The project provides a Rust SOCKS5 tunnel runtime with:

- a Linux CLI that can open and configure a TUN device itself
- an integration mode where the host process passes an already-open TUN file descriptor
- TCP relay through a SOCKS5 proxy
- UDP relay through a SOCKS5 proxy
- optional DNS interception through `hs5t-dns-cache`
- clean shutdown through cancellation and signal handling

## Supported runtime modes

### Linux native CLI

`hs5t` reads a YAML config file, opens/configures the TUN interface, starts the
runtime, and exits cleanly on `SIGINT` or `SIGTERM`.

### Host-managed TUN

When `HEV_SOCKS5_TUNNEL_FD` is set, the runtime uses that file descriptor
instead of opening a new TUN device. This is the expected integration path for
Android and other host-managed environments.

## Runtime behavior

### Initialization

Given a valid config file:

- the CLI loads and validates YAML configuration
- a TUN fd is obtained either from `HEV_SOCKS5_TUNNEL_FD` or from the Linux
  tunnel driver
- `hs5t_core::run_tunnel` starts the async runtime loop
- tracing is initialized to stderr

### TCP forwarding

When a TCP packet arrives from the TUN device:

- the packet is classified and fed into the smoltcp TCP stack
- newly active TCP sockets spawn `TcpSession` tasks
- each `TcpSession` establishes a SOCKS5 `CONNECT` session to the configured proxy
- data is bridged between smoltcp and the SOCKS5 stream with a duplex channel

### UDP forwarding

When a non-DNS UDP packet arrives from the TUN device:

- the packet is classified outside smoltcp
- a `UdpSession` task relays the datagram with SOCKS5 UDP associate semantics

### DNS interception

When DNS interception is enabled and a UDP packet targets the configured mapdns
address and port:

- `hs5t-dns-cache` handles the DNS request
- the runtime writes the synthesized UDP response directly back to the TUN device
- the packet does not enter smoltcp and no UDP relay task is spawned

### Shutdown

When the process receives `SIGINT` or `SIGTERM`:

- the root `CancellationToken` is cancelled
- child session tasks observe cancellation and terminate
- the IO loop exits cleanly
- the CLI exits with status `0`

## Verification targets

The current verification baseline is:

- `cargo test --workspace` passes in `rust/`
- CLI integration tests verify:
  - `--version`
  - `--help`
  - invalid config handling
  - clean shutdown after `SIGINT`

## Out of scope

These platforms are not implemented yet in the Rust runtime:

- macOS
- iOS
- FreeBSD
- NetBSD
- Windows
