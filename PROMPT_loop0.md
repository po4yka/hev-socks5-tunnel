# Loop 0: C Test Coverage Foundation

Repository: /mnt/nvme/home/po4yka/hev-socks5-tunnel
Task: Write comprehensive C tests BEFORE any Rust migration begins.
No existing tests. No Rust code yet.

## Per-iteration targets

### Iter 1-2: Unit test framework + utilities
- Add Unity (single-header) or cmocka to tests/c/
- Makefile: build with -fsanitize=address,undefined
- Unit tests: hev-ring-buffer (wrap-around, full/empty, two-iovec scatter-gather, two-phase read/release)
- Unit tests: hev-list (add_tail, del, iteration)
- Unit tests: hev-logger (level filtering, file vs stderr, format verification)
- Unit tests: hev-exec (fork/exec of a test script, argument passing)

### Iter 3-4: Config + DNS unit tests
- Unit tests: hev-config (all fields, defaults, missing required fields → -1, user without pass → -1)
- Unit tests: hev-mapped-dns (LRU eviction at capacity, handle() valid query, handle() malformed → -1, lookup() after eviction → NULL, reverse lookup)

### Iter 5-8: Sanitizer runs
- Run all unit tests under ASAN: zero errors required
- Run all unit tests under TSAN: zero races required
- Run all unit tests under UBSAN: zero UB required
- Document any bugs found and fix them

### Iter 9-14: Fuzz targets (libFuzzer)
- Fuzz target: hev_config_init_from_str() (YAML parser)
- Fuzz target: hev_mapped_dns_handle() (DNS wire-format parser)
- Fuzz target: ring buffer op sequences (random read/write/finish/release sequences)
- Run each for 5 minutes minimum; fix all crashes; add crashes as regression corpus

### Iter 15-18: Integration test harness
- Script: launch microsocks or dante as SOCKS5 proxy
- Script: create TUN interface, configure routing, start tunnel binary
- Script: inject test HTTP/TCP traffic via TUN, verify egress at proxy
- Script: UDP DNS query through tunnel
- Document in tests/integration/README.md

### Iter 19-20: Regression tests from any production bugs
- Search git log and GitHub issues for bug fixes: `git log --oneline | head -50`
- For each bug fix commit found: write a regression test that would catch it
- Ensure all tests pass cleanly before marking Loop 0 complete

## Key architectural invariants to test

### Ring buffer two-phase read protocol (CRITICAL):
- `reading()` → returns iovecs WITHOUT advancing rp
- `read_finish()` → advances rp (moves read pointer)
- `read_release()` → decrements `use_size` (separate from rp advance)
These must be tested independently: reading→finish→release, reading→release→finish.

### DNS cache behaviour:
- `handle()` mutates request buffer in-place (overwrites length prefixes with `.`)
- IP allocation: `net | index` where index is position in records array
- LRU eviction: oldest evicted when capacity reached
- Reverse lookup: lookup(ip) after find(name) returns Some(name)

## Exit criteria
- `make -C tests/c/ test SANITIZERS=1` passes with zero errors
- All fuzz targets have run 5+ minutes with no crashes
- Integration test harness runs end-to-end successfully
- Write LOOP_COMPLETE to signal completion

LOOP_COMPLETE
