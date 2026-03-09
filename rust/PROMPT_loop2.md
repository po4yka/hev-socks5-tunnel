# Loop 2: Utility Crates (TDD)

Repository: /mnt/nvme/home/po4yka/hev-socks5-tunnel
Crates: hs5t-ring-buffer, hs5t-logger (includes exec logic)
C source references: src/misc/hev-ring-buffer.c (93 LOC), hev-logger.c (112 LOC), hev-exec.c (61 LOC)

## CRITICAL INVARIANT: Two-phase ring buffer read
- `reading()` → returns iovecs WITHOUT advancing rp
- `read_finish()` → advances rp (moves read pointer)
- `read_release()` → decrements `use_size` (separate from rp advance)
This two-phase protocol is used in TCP backward splice; must be preserved exactly.

## TDD sequence for each module

### hs5t-ring-buffer

WRITE TESTS FIRST (before any implementation):
1. write N bytes, read N bytes, verify round-trip
2. write to capacity, verify is_full(); try write → Err
3. wrap-around: write fills buffer, read half, write again (crosses boundary)
4. two-iovec path: write 3/4 full, advance wp past end, verify reading() returns 2 iovecs
5. read_finish() advances rp; use_size unchanged; read_release() decrements use_size
6. proptest: generate random sequences of writes (1-4096 bytes) and reads; verify data integrity

THEN IMPLEMENT minimum code to pass tests.

ADD DIFFERENTIAL TEST (rust/tests/differential/ring_buffer_diff.rs):
- Link src/misc/hev-ring-buffer.c via cc crate in build.rs
- Expose C functions via extern "C" declarations
- Generate 10,000 proptest sequences; run against both C and Rust
- Assert: every byte read matches; assert: state matches at every step

SANITIZER:
- `RUSTFLAGS="-Z sanitizer=address" cargo +nightly test -p hs5t-ring-buffer`

### hs5t-logger

WRITE TESTS FIRST:
1. Level filtering: debug messages suppressed when level=Info
2. Output to file matches format `[timestamp] [L] message\n`
3. Multiple log levels: Debug, Info, Warn, Error
4. hev-exec equivalent: Command::new(script).spawn() passes correct args; process exits 0

Crates to use: tracing, tracing-subscriber (for logger); std::process::Command (for exec)

## Exit criteria
- All unit + proptest tests pass
- 10,000-case differential test passes (byte-exact ring buffer match)
- ASAN clean on hs5t-ring-buffer
- cargo clippy -D warnings passes
- Write LOOP_COMPLETE to signal completion

LOOP_COMPLETE
