fn main() {
    // Only generate the C header when the `c-api` feature is active.
    if std::env::var("CARGO_FEATURE_C_API").is_ok() {
        let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let include_dir = format!("{}/include", crate_dir);
        std::fs::create_dir_all(&include_dir).expect("failed to create include/ dir");

        let config = cbindgen::Config::from_file(format!("{}/cbindgen.toml", crate_dir))
            .expect("cbindgen.toml not found");

        cbindgen::generate_with_config(&crate_dir, config)
            .expect("cbindgen failed to generate header")
            .write_to_file(format!("{}/hev-main-rust.h", include_dir));

        println!("cargo:rerun-if-changed=src/c_api.rs");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
