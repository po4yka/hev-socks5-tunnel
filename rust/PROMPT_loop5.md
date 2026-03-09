# Loop 5: DNS Cache (TDD + Differential)

Repository: /mnt/nvme/home/po4yka/hev-socks5-tunnel
Crate: hs5t-dns-cache
C reference: src/hev-mapped-dns.c (332 LOC)

## Critical C behaviour to replicate exactly:
- `handle()` mutates request buffer in-place (overwrites length prefixes with `.`)
- IP allocation: `net | index` where index is position in records array
- LRU uses red-black tree (lookup) + doubly-linked list (eviction order)
- Reverse lookup: ip → name mapping

## Rust design:
- Use `lru::LruCache<String, u32>` for name→ip mapping
- Use `HashMap<u32, String>` for ip→name reverse lookup
- DnsCache::handle() parses DNS wire format manually (no external DNS crate)
- Keep both maps in sync on eviction

## TDD sequence (tests FIRST, always)

### Write these tests first:
1. Insert up to max → IPs are net|0, net|1, ..., net|(max-1)
2. Insert max+1 → oldest evicted; lookup(oldest_ip) → None
3. Repeated access to same name keeps it (LRU touch semantics)
4. handle() with valid A-query → response has correct mapped IP
5. handle() with question_count > 32 → Err
6. handle() with truncated packet (< 12 bytes header) → Err
7. lookup(ip) after find(name) → Some(name)
8. find(name) after eviction → new IP assigned at that slot
9. DNS response format: header flags correct (QR=1, AA=1, RA=1)

### Then implement DnsCache

### DIFFERENTIAL TEST (tests/differential/dns_diff.rs):
- 500 randomly generated valid DNS A-query packets
- Run against C hev_mapped_dns_handle() and Rust handle()
- Assert: byte-exact match on response buffers
- Assert: IP mappings match after N insertions

### FUZZ TARGET (tests/fuzz/fuzz_dns_handle.rs):
- Arbitrary bytes as DNS request; assert no panic (only Err)
- Assert: if C returns -1, Rust returns Err; if C returns n>0, outputs match

### PROPERTY-BASED TEST (proptest):
- Random sequences of find()+handle()+lookup() operations
- Invariant: LRU eviction always removes least-recently-used name
- Invariant: reverse lookup always consistent with forward lookup

## DNS wire format reference (for manual parsing):
- Header: 12 bytes (ID, flags, QDCOUNT, ANCOUNT, NSCOUNT, ARCOUNT)
- Question: name labels, QTYPE, QCLASS
- Answer: name pointer, TYPE=A(1), CLASS=IN(1), TTL, RDLENGTH=4, RDATA(IPv4)

## Exit criteria
- 500-case differential test passes (byte-exact)
- Fuzz target: 1M iterations, no panics
- ASAN clean
- handle() never panics on any input (only returns Err or Ok)
- Write LOOP_COMPLETE to signal completion

LOOP_COMPLETE
