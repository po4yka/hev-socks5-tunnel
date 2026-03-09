// 500-case differential test: C dns_shim vs Rust DnsCache.
//
// Generates random valid DNS A-query packets, feeds them to both
// implementations (with identical fresh state), and asserts byte-exact
// match on the response buffers.

#[cfg(test)]
mod dns_diff {
    use crate::dns_shim_ffi::CDnsShimWrapper;
    use hs5t_dns_cache::DnsCache;
    use proptest::prelude::*;

    const NET: u32 = 0x0a_00_00_00; // 10.0.0.0
    const MASK: u32 = 0xff_ff_ff_00; // /24
    const MAX: usize = 8;

    // -----------------------------------------------------------------------
    // DNS packet builder helpers
    // -----------------------------------------------------------------------

    /// Encode a label string in DNS wire format: `[len]<bytes>`.
    fn encode_label(label: &str) -> Vec<u8> {
        let mut out = Vec::with_capacity(1 + label.len());
        out.push(label.len() as u8);
        out.extend_from_slice(label.as_bytes());
        out
    }

    /// Build a valid DNS A-query for `name` with given ID and RD flag.
    fn build_a_query(id: u16, rd: bool, name: &str) -> Vec<u8> {
        let mut pkt = Vec::with_capacity(64);
        pkt.extend_from_slice(&id.to_be_bytes());
        let flags: u16 = if rd { 0x0100 } else { 0x0000 };
        pkt.extend_from_slice(&flags.to_be_bytes());
        pkt.extend_from_slice(&1u16.to_be_bytes()); // QDCOUNT = 1
        pkt.extend_from_slice(&0u16.to_be_bytes()); // ANCOUNT
        pkt.extend_from_slice(&0u16.to_be_bytes()); // NSCOUNT
        pkt.extend_from_slice(&0u16.to_be_bytes()); // ARCOUNT
        for label in name.split('.') {
            pkt.extend(encode_label(label));
        }
        pkt.push(0u8); // end-of-name
        pkt.extend_from_slice(&1u16.to_be_bytes()); // QTYPE = A
        pkt.extend_from_slice(&1u16.to_be_bytes()); // QCLASS = IN
        pkt
    }

    // -----------------------------------------------------------------------
    // Arbitrary strategies
    // -----------------------------------------------------------------------

    /// 1-8 lowercase alphanumeric characters.
    fn arb_label_str() -> impl Strategy<Value = String> {
        prop::string::string_regex("[a-z][a-z0-9]{0,7}").unwrap()
    }

    /// Valid FQDN: 1-3 dot-separated labels, each 1-8 chars.
    fn arb_name_str() -> impl Strategy<Value = String> {
        prop::collection::vec(arb_label_str(), 1usize..=3)
            .prop_map(|labels| labels.join("."))
    }

    /// A single DNS query descriptor.
    #[derive(Debug, Clone)]
    struct QueryDesc {
        id: u16,
        rd: bool,
        name: String,
    }

    fn arb_query() -> impl Strategy<Value = QueryDesc> {
        (any::<u16>(), any::<bool>(), arb_name_str())
            .prop_map(|(id, rd, name)| QueryDesc { id, rd, name })
    }

    // -----------------------------------------------------------------------
    // Differential runner
    // -----------------------------------------------------------------------

    /// Feed `queries` in sequence to both implementations (fresh state for
    /// each proptest case) and assert byte-exact response equality.
    fn run_dns_diff(queries: Vec<QueryDesc>) -> Result<(), TestCaseError> {
        let mut c_shim = CDnsShimWrapper::new(NET, MASK, MAX);
        let mut r_cache = DnsCache::new(NET, MASK, MAX);

        for q in &queries {
            let original = build_a_query(q.id, q.rd, &q.name);

            let mut c_req = original.clone();
            let mut c_res = vec![0u8; 512];
            let c_rlen = c_shim.handle(&mut c_req, &mut c_res);

            let mut r_req = original.clone();
            let mut r_res = vec![0u8; 512];
            let r_result = r_cache.handle(&mut r_req, &mut r_res);

            // Both should succeed for valid queries.
            prop_assert!(c_rlen > 0, "C shim failed for query {:?}", q);
            let c_rlen = c_rlen as usize;

            let r_rlen = r_result
                .map_err(|e| TestCaseError::fail(format!("Rust handle failed: {e:?} query={q:?}")))?;

            prop_assert_eq!(
                c_rlen,
                r_rlen,
                "response length mismatch for query {:?}: C={} Rust={}",
                q,
                c_rlen,
                r_rlen,
            );

            prop_assert_eq!(
                &c_res[..c_rlen],
                &r_res[..r_rlen],
                "response bytes differ for query {:?}",
                q,
            );
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // 500-case proptest — single query per test case (exercises state accumulation)
    // -----------------------------------------------------------------------

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(500))]

        #[test]
        fn prop_dns_diff_500_single_query(q in arb_query()) {
            run_dns_diff(vec![q])?;
        }
    }

    // -----------------------------------------------------------------------
    // Additional: sequence of 1-16 queries to exercise LRU-state interaction.
    // -----------------------------------------------------------------------

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(200))]

        #[test]
        fn prop_dns_diff_sequence(queries in prop::collection::vec(arb_query(), 1usize..=16)) {
            run_dns_diff(queries)?;
        }
    }
}
