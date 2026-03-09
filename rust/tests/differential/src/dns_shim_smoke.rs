// Smoke tests for the standalone C DNS-cache shim (dns_shim.c).
//
// These verify that the shim produces the same answers as the Rust DnsCache
// for basic operations before running the full 500-case differential test.

#[cfg(test)]
mod dns_shim_smoke {
    use crate::dns_shim_ffi::CDnsShimWrapper;
    use hs5t_dns_cache::DnsCache;

    const NET: u32  = 0x0a_00_00_00; // 10.0.0.0
    const MASK: u32 = 0xff_ff_ff_00; // /24

    /// Build a minimal DNS A-query packet for `name` with RD=1.
    fn make_a_query(name: &str) -> Vec<u8> {
        let mut pkt = Vec::with_capacity(64);
        pkt.extend_from_slice(&1u16.to_be_bytes());       // ID = 1
        pkt.extend_from_slice(&0x0100u16.to_be_bytes());  // flags: RD=1
        pkt.extend_from_slice(&1u16.to_be_bytes());       // QDCOUNT = 1
        pkt.extend_from_slice(&0u16.to_be_bytes());       // ANCOUNT = 0
        pkt.extend_from_slice(&0u16.to_be_bytes());       // NSCOUNT = 0
        pkt.extend_from_slice(&0u16.to_be_bytes());       // ARCOUNT = 0
        // QNAME
        for label in name.split('.') {
            pkt.push(label.len() as u8);
            pkt.extend_from_slice(label.as_bytes());
        }
        pkt.push(0u8);                                    // end-of-name
        pkt.extend_from_slice(&1u16.to_be_bytes());       // QTYPE = A
        pkt.extend_from_slice(&1u16.to_be_bytes());       // QCLASS = IN
        pkt
    }

    /// Smoke: C shim assigns the first IP as NET|0.
    #[test]
    fn shim_smoke_sequential_ips() {
        let mut shim = CDnsShimWrapper::new(NET, MASK, 4);
        let names = ["a.com", "b.com", "c.com", "d.com"];

        for (i, name) in names.iter().enumerate() {
            let mut req = make_a_query(name);
            let mut res = vec![0u8; 512];
            let rlen = shim.handle(&mut req, &mut res);
            assert!(rlen > 0, "handle must succeed for {name}");

            // Extract the IP from the answer RDATA (last 4 bytes of the answer record).
            let rlen = rlen as usize;
            let ip = u32::from_be_bytes([
                res[rlen - 4],
                res[rlen - 3],
                res[rlen - 2],
                res[rlen - 1],
            ]);
            assert_eq!(ip, NET | i as u32, "{name} must get NET|{i}");
        }
    }

    /// Smoke: response bytes from C shim and Rust DnsCache must be identical
    /// for a single A-query.
    #[test]
    fn shim_vs_rust_single_a_query() {
        let mut c_shim   = CDnsShimWrapper::new(NET, MASK, 8);
        let mut r_cache  = DnsCache::new(NET, MASK, 8);

        let name = "example.com";
        let original_req = make_a_query(name);

        let mut c_req = original_req.clone();
        let mut r_req = original_req.clone();
        let mut c_res = vec![0u8; 512];
        let mut r_res = vec![0u8; 512];

        let c_len = c_shim.handle(&mut c_req, &mut c_res);
        let r_len = r_cache.handle(&mut r_req, &mut r_res)
            .expect("Rust handle must succeed");

        assert!(c_len > 0, "C shim must succeed");
        let c_len = c_len as usize;

        assert_eq!(
            c_len, r_len,
            "response lengths must match: C={c_len} Rust={r_len}"
        );
        assert_eq!(
            &c_res[..c_len], &r_res[..r_len],
            "response bytes must be identical"
        );
    }

    /// Smoke: C shim returns -1 when slen < qlen.
    #[test]
    fn shim_smoke_buf_too_small_rejects() {
        let mut shim = CDnsShimWrapper::new(NET, MASK, 4);
        let mut req = make_a_query("test.com");
        let mut res = vec![0u8; 4]; // smaller than req
        let rc = shim.handle(&mut req, &mut res);
        assert_eq!(rc, -1, "C shim must return -1 when slen < qlen");
    }

    /// Smoke: C shim returns -1 when QDCOUNT > 32.
    #[test]
    fn shim_smoke_too_many_questions() {
        let mut shim = CDnsShimWrapper::new(NET, MASK, 4);
        let mut req = vec![0u8; 12];
        req[5] = 33; // QDCOUNT = 33
        let mut res = vec![0u8; 512];
        let rc = shim.handle(&mut req, &mut res);
        assert_eq!(rc, -1, "C shim must return -1 for QDCOUNT > 32");
    }
}
