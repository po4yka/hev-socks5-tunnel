// Differential test harness: C (hev-ring-buffer.c) vs Rust (hs5t-ring-buffer),
// and C (dns_shim.c) vs Rust (hs5t-dns-cache).

pub mod dns_shim_ffi;
mod dns_shim_smoke;


//
// Generates random sequences of write / read operations and asserts that both
// implementations produce bit-identical output at every step.

#[cfg(test)]
mod ring_buffer_diff {
    use hs5t_ring_buffer::{read_available, write_bytes, RingBuffer};
    use proptest::prelude::*;

    // ---------------------------------------------------------------------------
    // FFI declarations for the thin C wrapper compiled by build.rs
    // ---------------------------------------------------------------------------

    #[repr(C)]
    struct CHevRingBuffer {
        _opaque: [u8; 0],
    }

    extern "C" {
        fn rb_new(capacity: usize) -> *mut CHevRingBuffer;
        fn rb_free(rb: *mut CHevRingBuffer);
        fn rb_write_bytes(
            rb: *mut CHevRingBuffer,
            data: *const u8,
            len: usize,
        ) -> usize;
        fn rb_read_available(
            rb: *mut CHevRingBuffer,
            out: *mut u8,
            max_out: usize,
        ) -> usize;
        fn rb_get_max_size(rb: *mut CHevRingBuffer) -> usize;
        fn rb_get_use_size(rb: *mut CHevRingBuffer) -> usize;
        fn rb_get_rda_size(rb: *mut CHevRingBuffer) -> usize;
    }

    // ---------------------------------------------------------------------------
    // Safe wrapper over the raw C ring buffer
    // ---------------------------------------------------------------------------

    struct CRingBuffer {
        ptr: *mut CHevRingBuffer,
    }

    impl CRingBuffer {
        fn new(capacity: usize) -> Self {
            let ptr = unsafe { rb_new(capacity) };
            assert!(!ptr.is_null(), "rb_new returned NULL");
            Self { ptr }
        }

        fn write_bytes(&mut self, data: &[u8]) -> usize {
            unsafe { rb_write_bytes(self.ptr, data.as_ptr(), data.len()) }
        }

        fn read_available(&mut self) -> Vec<u8> {
            let rda = unsafe { rb_get_rda_size(self.ptr) };
            if rda == 0 {
                return vec![];
            }
            let mut out = vec![0u8; rda];
            let n = unsafe { rb_read_available(self.ptr, out.as_mut_ptr(), out.len()) };
            out.truncate(n);
            out
        }

        fn use_size(&self) -> usize {
            unsafe { rb_get_use_size(self.ptr) }
        }

        fn rda_size(&self) -> usize {
            unsafe { rb_get_rda_size(self.ptr) }
        }

        fn max_size(&self) -> usize {
            unsafe { rb_get_max_size(self.ptr) }
        }
    }

    impl Drop for CRingBuffer {
        fn drop(&mut self) {
            unsafe { rb_free(self.ptr) };
        }
    }

    // ---------------------------------------------------------------------------
    // Operation type for proptest
    // ---------------------------------------------------------------------------

    #[derive(Debug, Clone)]
    enum Op {
        Write(Vec<u8>),
        Read,
    }

    fn arb_op() -> impl Strategy<Value = Op> {
        prop_oneof![
            // Write 1-256 random bytes
            prop::collection::vec(any::<u8>(), 1usize..=256).prop_map(Op::Write),
            // Read all available
            Just(Op::Read),
        ]
    }

    fn arb_ops(len: usize) -> impl Strategy<Value = Vec<Op>> {
        prop::collection::vec(arb_op(), 1..=len)
    }

    // ---------------------------------------------------------------------------
    // Core differential runner — called by both test variants
    // ---------------------------------------------------------------------------

    fn run_differential(ops: Vec<Op>, capacity: usize) -> Result<(), TestCaseError> {
        let mut rust = RingBuffer::new(capacity);
        let mut c = CRingBuffer::new(capacity);

        prop_assert_eq!(c.max_size(), capacity);
        prop_assert_eq!(rust.max_size(), capacity);

        for op in ops {
            match op {
                Op::Write(data) => {
                    let space = capacity - rust.use_size();
                    let to_write = data.len().min(space);

                    let rust_written = if to_write > 0 {
                        write_bytes(&mut rust, &data[..to_write])
                    } else {
                        0
                    };
                    let c_written = c.write_bytes(&data[..to_write]);

                    prop_assert_eq!(
                        rust_written, c_written,
                        "write: Rust wrote {} bytes, C wrote {} bytes",
                        rust_written, c_written
                    );
                    prop_assert_eq!(
                        rust.use_size(),
                        c.use_size(),
                        "use_size diverged after write"
                    );
                    prop_assert_eq!(
                        rust.rda_size(),
                        c.rda_size(),
                        "rda_size diverged after write"
                    );
                }
                Op::Read => {
                    if rust.rda_size() == 0 {
                        continue;
                    }
                    let rust_data = read_available(&mut rust);
                    let c_data = c.read_available();

                    prop_assert_eq!(
                        rust_data.len(),
                        c_data.len(),
                        "read length mismatch: Rust {} vs C {}",
                        rust_data.len(),
                        c_data.len()
                    );
                    for (i, (&rb, &cb)) in rust_data.iter().zip(c_data.iter()).enumerate() {
                        prop_assert_eq!(
                            rb, cb,
                            "byte {} mismatch after read: Rust=0x{:02X} C=0x{:02X}",
                            i, rb, cb
                        );
                    }
                    prop_assert_eq!(
                        rust.use_size(),
                        c.use_size(),
                        "use_size diverged after read"
                    );
                    prop_assert_eq!(
                        rust.rda_size(),
                        c.rda_size(),
                        "rda_size diverged after read"
                    );
                }
            }
        }
        Ok(())
    }

    // ---------------------------------------------------------------------------
    // 10 000-case proptest (required by Loop 2 exit criteria)
    // ---------------------------------------------------------------------------

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(10_000))]

        #[test]
        fn prop_ring_buffer_differential_256(ops in arb_ops(60)) {
            run_differential(ops, 256)?;
        }
    }

    // Additional capacity variants for broader coverage
    proptest! {
        #[test]
        fn prop_ring_buffer_differential_small(
            ops in arb_ops(40),
            capacity in prop::sample::select(vec![4usize, 8, 16, 32]),
        ) {
            run_differential(ops, capacity)?;
        }
    }

    proptest! {
        #[test]
        fn prop_ring_buffer_differential_large(
            ops in arb_ops(40),
        ) {
            run_differential(ops, 4096)?;
        }
    }
}
