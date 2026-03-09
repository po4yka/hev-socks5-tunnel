# Loop 1: Rust Workspace Scaffold

Repository: /mnt/nvme/home/po4yka/hev-socks5-tunnel
Task: Create the Cargo workspace structure.

## Steps

1. Create rust/Cargo.toml as workspace with all 9 crate members:
   - hs5t-config
   - hs5t-logger
   - hs5t-ring-buffer
   - hs5t-dns-cache
   - hs5t-tunnel
   - hs5t-session
   - hs5t-core
   - hs5t-jni
   - hs5t-bin
   Plus: tests/differential, tests/fuzz (workspace members)

2. Create each crate: Cargo.toml + src/lib.rs (empty, with #[cfg(test)] mod tests {})
   - hs5t-bin gets src/main.rs not lib.rs

3. Crate dependency declarations in Cargo.toml:
   - hs5t-bin → hs5t-core
   - hs5t-core → hs5t-session, hs5t-tunnel, hs5t-dns-cache, hs5t-config, hs5t-logger
   - hs5t-session → hs5t-ring-buffer
   - hs5t-jni → hs5t-core
   - hs5t-config: serde = {features=["derive"]}, serde_yaml
   - hs5t-logger: tracing, tracing-subscriber
   - hs5t-session: tokio = {features=["full"]}, fast-socks5
   - hs5t-core: smoltcp, tokio = {features=["full"]}, tokio-util
   - hs5t-dns-cache: lru
   - hs5t-jni: jni (behind feature "android")

4. Create .github/workflows/rust-ci.yml:
   - cargo check --workspace
   - cargo clippy --workspace -- -D warnings
   - cargo test --workspace
   - cargo fmt --check
   - Nightly: RUSTFLAGS="-Z sanitizer=address" cargo +nightly test --workspace

5. Create tests/differential/build.rs stub (compiles C test lib via cc crate)
   - Add cc as build-dependency
   - Stub that will compile hev-ring-buffer.c

6. Create tests/fuzz/ directory with placeholder Cargo.toml for cargo-fuzz

## Workspace layout to create

```
rust/
  Cargo.toml                  # workspace root
  crates/
    hs5t-config/
      Cargo.toml
      src/lib.rs
    hs5t-logger/
      Cargo.toml
      src/lib.rs
    hs5t-ring-buffer/
      Cargo.toml
      src/lib.rs
    hs5t-dns-cache/
      Cargo.toml
      src/lib.rs
    hs5t-tunnel/
      Cargo.toml
      src/lib.rs
    hs5t-session/
      Cargo.toml
      src/lib.rs
    hs5t-core/
      Cargo.toml
      src/lib.rs
    hs5t-jni/
      Cargo.toml
      src/lib.rs
    hs5t-bin/
      Cargo.toml
      src/main.rs
  tests/
    differential/
      Cargo.toml
      build.rs
      src/lib.rs
    fuzz/
      Cargo.toml
```

## Exit criteria
- `cd rust && cargo check --workspace` exits 0
- `cd rust && cargo clippy --workspace -- -D warnings` exits 0
- Directory structure matches the plan exactly
- Write LOOP_COMPLETE to signal completion

LOOP_COMPLETE
