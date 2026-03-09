// Fuzz-style test for DnsCache::handle().
//
// Feeds arbitrary byte slices as DNS request buffers and verifies:
//   1. Rust DnsCache::handle() never panics — it must always return Ok or a
//      well-typed DnsCacheError, never unwind or abort.
//   2. For well-formed inputs (len >= 12, QDCOUNT in 1..=32) the C shim and
//      Rust produce identical byte responses.  The C shim is not robust to
//      truly arbitrary bytes (no bounds-check on degenerate packets), so we
//      only compare when the packet is structurally sound.
//
// Uses proptest so it runs in the normal `cargo test` pipeline.
// Run with PROPTEST_CASES=100000 for deeper coverage.

#[cfg(test)]
mod dns_fuzz {
    use crate::dns_shim_ffi::CDnsShimWrapper;
    use hs5t_dns_cache::DnsCache;
    use proptest::prelude::*;

    const NET: u32 = 0x0a_00_00_00;
    const MASK: u32 = 0xff_ff_ff_00;
    const MAX: usize = 8;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(10_000))]

        /// Arbitrary bytes must never panic DnsCache::handle().
        #[test]
        fn prop_fuzz_handle_no_panic(
            mut req in prop::collection::vec(any::<u8>(), 0usize..=512),
        ) {
            let mut cache = DnsCache::new(NET, MASK, MAX);
            let mut res = vec![0u8; 1024];
            // The implementation must return Ok or a typed Err — never panic.
            let _ = cache.handle(&mut req, &mut res);
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(2_000))]

        /// For structurally valid packets (len >= 12, QDCOUNT in 1..=32),
        /// the C shim and Rust must produce identical responses.
        ///
        /// We use a hand-crafted strategy that always produces a packet with
        /// a valid 12-byte header and QDCOUNT <= 32, then appends arbitrary
        /// question bytes.  This ensures the C shim does not SEGFAULT on
        /// under-length inputs while still giving proptest a rich search space.
        #[test]
        fn prop_fuzz_valid_header_matches_c_shim(
            id in any::<u16>(),
            flags in any::<u16>(),
            qdcount in 1u16..=4u16,
            extra in prop::collection::vec(any::<u8>(), 0usize..=128),
        ) {
            // Build a packet with a valid header but potentially garbled questions.
            let mut pkt = Vec::with_capacity(12 + extra.len());
            pkt.extend_from_slice(&id.to_be_bytes());
            pkt.extend_from_slice(&flags.to_be_bytes());
            pkt.extend_from_slice(&qdcount.to_be_bytes()); // QDCOUNT
            pkt.extend_from_slice(&[0u8; 6]);               // ANCOUNT/NSCOUNT/ARCOUNT
            pkt.extend_from_slice(&extra);

            let mut c_shim = CDnsShimWrapper::new(NET, MASK, MAX);
            let mut r_cache = DnsCache::new(NET, MASK, MAX);

            let mut c_req = pkt.clone();
            let mut c_res = vec![0u8; 512];
            let c_rlen = c_shim.handle(&mut c_req, &mut c_res);

            let mut r_req = pkt.clone();
            let mut r_res = vec![0u8; 512];
            // Must not panic.
            let r_result = r_cache.handle(&mut r_req, &mut r_res);

            if c_rlen > 0 {
                let c_rlen = c_rlen as usize;
                let r_rlen = r_result.map_err(|e| {
                    TestCaseError::fail(format!(
                        "C succeeded (len={}) but Rust returned Err({:?})",
                        c_rlen, e
                    ))
                })?;
                prop_assert_eq!(
                    c_rlen, r_rlen,
                    "length mismatch: C={} Rust={}", c_rlen, r_rlen,
                );
                prop_assert_eq!(
                    &c_res[..c_rlen],
                    &r_res[..r_rlen],
                    "response bytes differ for packet with QDCOUNT={}", qdcount,
                );
            }
            // C returning -1 is fine; Rust may return Ok or Err for malformed questions.
        }
    }
}
