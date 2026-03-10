// build.rs for differential tests
// Compiles the standalone DNS-cache shim for byte-exact comparison testing.
// Note: ring-buffer C files removed in Loop 9 (replaced by hs5t-ring-buffer).

fn main() {
    // --- DNS-cache differential library ---
    // Standalone shim: no submodule dependencies (no HevRBTree / HevList).
    cc::Build::new()
        .file("src/dns_shim.c")
        .warnings(true)
        .flag_if_supported("-Wno-unused-parameter")
        .compile("dns_shim");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/dns_shim.c");
}
