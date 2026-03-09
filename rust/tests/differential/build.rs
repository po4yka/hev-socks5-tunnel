// build.rs for differential tests
// Compiles the C implementations as a static library for comparison testing

fn main() {
    // TODO: compile C ring buffer for differential testing
    // let src_root = std::path::Path::new("../../src");
    //
    // cc::Build::new()
    //     .file(src_root.join("misc/hev-ring-buffer.c"))
    //     .include(src_root.join("misc"))
    //     .compile("hev_c_ring_buffer");

    // Placeholder: no C sources compiled yet
    println!("cargo:rerun-if-changed=build.rs");
}
