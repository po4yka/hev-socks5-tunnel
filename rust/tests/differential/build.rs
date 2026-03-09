// build.rs for differential tests
// Compiles the C ring-buffer implementation and standalone DNS-cache shim
// for byte-exact comparison testing.

fn main() {
    let misc = std::path::Path::new("../../../src/misc");

    // --- ring-buffer differential library ---
    cc::Build::new()
        .file(misc.join("hev-ring-buffer.c"))
        .file("src/ring_buffer_wrapper.c")
        .include(misc)
        .warnings(false) // suppress C compiler warnings from upstream code
        .compile("hev_c_ring_buffer");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/ring_buffer_wrapper.c");
    println!(
        "cargo:rerun-if-changed={}",
        misc.join("hev-ring-buffer.c").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        misc.join("hev-ring-buffer.h").display()
    );

    // --- DNS-cache differential library ---
    // Standalone shim: no submodule dependencies (no HevRBTree / HevList).
    cc::Build::new()
        .file("src/dns_shim.c")
        .warnings(true)
        .flag_if_supported("-Wno-unused-parameter")
        .compile("dns_shim");

    println!("cargo:rerun-if-changed=src/dns_shim.c");
}
