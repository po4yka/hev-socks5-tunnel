// build.rs for differential tests
// Compiles the C ring-buffer implementation for byte-exact comparison testing.

fn main() {
    let misc = std::path::Path::new("../../../src/misc");

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
}
