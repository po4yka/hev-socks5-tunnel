# Loop 5: DNS Cache — Planner Notes

## 2026-03-09 — Initial Planning

### C Reference Analysis (src/hev-mapped-dns.c, 332 LOC)

Key behaviors to replicate:
- `hev_mapped_dns_find(name)`: looks up name in RB tree; if found, moves to LRU tail (most-recently-used), returns idx. If not found, allocates node, assigns idx = self->use (pre-fill) or oldest LRU slot (when full). `ips[ipn] = self->net | idx`.
- `hev_mapped_dns_handle()`: copies request to response buffer, parses DNS questions, for each TYPE=A (QTYPE=1) + CLASS=IN (QCLASS=1) question calls find(), stores ip in response. Mutates rb[poff] = '.' for each label length byte. Response flags: `fl |= 0x8000 | ((fl & 0x100) >> 1)` = sets QR=1, sets RA=RD.
- `hev_mapped_dns_lookup(ip)`: idx = ip & ~mask. Returns records[idx]->name if exists. Also touches LRU.
- Eviction: when use==max, evict hev_list_first (oldest/LRU-front), reuse its idx.
- Index allocation: `self->use` increments up to max. When full, evict oldest idx.

### Submodule Situation
hev-task-system, lwip, yaml submodules NOT initialized — cannot link C code directly.
Strategy: Write standalone dns_shim.c that reimplements the same algorithm without submodule deps.
The shim only needs: stdint.h, stdlib.h, string.h, arpa/inet.h — all standard.
The shim will NOT use HevRBTree/HevList — use stdlib qsort-based binary search or simpler array approach that has identical observable behavior for the test cases.

Actually: the shim can use a simple array + linear search (O(n) but correct for test purposes) instead of RB tree. For 500 test cases with small max (e.g., 16 entries), performance is irrelevant.

### Rust Design
- `DnsCache { lru: LruCache<String, u32>, rev: HashMap<u32, String>, net: u32, max: usize, next_idx: usize }`
- Wait — LruCache doesn't directly expose eviction callbacks. Need to handle eviction manually.
- Better: use `LruCache::push()` which returns evicted entry. Then remove from rev map.
- Or: use `LruCache::get_or_insert()` pattern.
- IP = net | idx. idx is position in 0..max-1.
- Problem: LruCache doesn't guarantee idx assignment matches C semantics (C uses slot reuse).
- Need custom index tracking: maintain a free-list or reuse evicted slot idx.
- Approach: Keep a separate `records: Vec<Option<String>>` (size=max). LruCache maps name→idx. On eviction, records[idx] = None, assign to new entry.

### Revised Rust Design
```rust
pub struct DnsCache {
    lru: LruCache<String, usize>,  // name -> idx
    rev: HashMap<u32, String>,      // ip -> name
    records: Vec<Option<String>>,   // idx -> name (for reverse)
    net: u32,
    max: usize,
    next_free: usize,               // monotonically assigned up to max
}
```
On `find(name)`:
1. If name in lru: lru.get() touches LRU, return (net | idx) as u32
2. Else: new entry. Get idx:
   - If next_free < max: idx = next_free; next_free += 1
   - Else: lru.push(name, 0) will evict oldest; capture evicted (old_name, old_idx); remove rev[net|old_idx]; records[old_idx] = None; use old_idx as new idx
3. lru.put(name, idx); records[idx] = Some(name); rev.insert(net|idx, name); return net|idx

### DNS Wire Format Parser (handle)
Input: req buffer (DNS query). Output: filled response buffer.
1. Check len >= 12 (header size)
2. Copy request to response (caller provides response buf)
3. Parse qcount from bytes [4..6] big-endian. Check qcount <= 32.
4. Walk questions starting at offset 12:
   - Record question start offset (for name pointer in answer)
   - Parse labels: while buf[off] != 0 { replace buf[off] with '.'; off += 1 + old_len }
   - off++ (skip null terminator)
   - Check bounds: off+4 <= len
   - Read QTYPE (2B BE) and QCLASS (2B BE)
   - If QTYPE==1 and QCLASS==1: call find(), store ip
   - off += 4
5. Build answer records at current off:
   - For each ip: write 16 bytes: [0xc0, question_offset, type=1(2B), class=1(2B), ttl=1(4B), rdlen=4(2B), ip(4B)]
6. Set response flags: fl |= 0x8000 | ((fl & 0x0100) >> 1)  [QR=1, AA is not set by C, RA=RD]
7. Set ancount = ipn (2B BE)
8. Return total response length

### Task Queue Summary (8 tasks created)
1. task-1773065342-a71c: Write RED tests (P1, ready now)
2. task-1773065358-b8af: Implement find()+lookup() GREEN (blocked by 1)
3. task-1773065374-80c9: Implement handle() GREEN (blocked by 2)
4. task-1773065386-4a77: Proptest (blocked by 3)
5. task-1773065397-c1f3: C shim (blocked by 1)
6. task-1773065412-0f9b: Differential test 500 cases (blocked by 3,5)
7. task-1773065425-333f: Fuzz target (blocked by 3)
8. task-1773065450-914e: Final (blocked by 4,6,7)

## 2026-03-09 — Post-Implementation Coordination (commit.complete event)

### Status
- commit `191bd94`: feat(hs5t-dns-cache): implement DnsCache with LRU eviction and DNS wire parser
- 9/9 unit tests pass: sequential IP assignment, eviction, LRU touch, handle() valid/invalid, lookup, flags
- Closed tasks: b8af (find/lookup GREEN), 80c9 (handle GREEN)

### Ready tasks now (3):
- task-1773065386-4a77: proptest (LRU invariants, reverse lookup consistency)
- task-1773065397-c1f3: C shim for differential test (standalone, no submodule deps)
- task-1773065425-333f: fuzz target (arbitrary bytes → only Err, no panic)

### Once C shim done (c1f3), task-1773065412-0f9b (500-case diff test) will unblock
### Once all 3 complete, task-1773065450-914e (Final: ASAN + clippy + commit + LOOP_COMPLETE) unblocks

### Next emit: tasks.ready → Builder picks up proptest + C shim + fuzz in parallel

## 2026-03-09 — C shim (task-1773065397-c1f3) DONE

### Delivered
- `rust/tests/differential/src/dns_shim.c`: standalone C reimplementation of hev-mapped-dns
  - Uses flat array + doubly-linked LRU list by index; no submodule deps
  - Exposes: `dns_shim_new`, `dns_shim_free`, `dns_shim_handle`, `dns_shim_lookup`
  - `dns_shim_handle` mirrors C reference byte-for-byte (label mutation, flag patching, ipo[] trick)
- `rust/tests/differential/src/dns_shim_ffi.rs`: Rust FFI bindings + CDnsShimWrapper RAII wrapper
- `rust/tests/differential/src/dns_shim_smoke.rs`: 4 smoke tests (sequential IPs, byte-exact match vs Rust, error paths)
- `rust/tests/differential/build.rs`: updated to compile dns_shim.c as separate static lib

### Key behaviour note
C shim does NOT check qlen < 12 (matches C reference). Rust DnsCache checks `req_len < 12 → Err(Truncated)`.
Differential test must use valid (qlen ≥ 12, qd ≥ 1) queries only.

### Verified: 7/7 tests pass, clippy -D warnings clean

### Next ready: task-0f9b (500-case differential test), task-4a77 (proptest), task-333f (fuzz)
