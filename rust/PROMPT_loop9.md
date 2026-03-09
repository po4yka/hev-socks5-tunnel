# Loop 9: C Code Removal (Leaf-First)

Repository: /mnt/nvme/home/po4yka/hev-socks5-tunnel

## Deletion order (leaf-first, each gated on prior tests passing):

Before deleting ANY file:
1. Verify `cd rust && cargo test --workspace` passes
2. Verify the corresponding Rust crate's tests pass
3. Delete C files
4. Remove from Makefile SRCFILES
5. Verify `cd rust && cargo test --workspace` still passes
6. Commit: `refactor: remove C hev-{module} (replaced by hs5t-{module})`

### Step 1: Ring buffer and list (Loop 2 passed)
Files to delete:
- src/misc/hev-ring-buffer.c
- src/misc/hev-ring-buffer.h
- src/misc/hev-list.c (if exists)
- src/misc/hev-list.h

### Step 2: Logger and exec (Loop 2 passed)
Files to delete:
- src/misc/hev-logger.c
- src/misc/hev-logger.h
- src/misc/hev-exec.c
- src/misc/hev-exec.h

### Step 3: Config (Loop 3 passed)
Files to delete:
- src/hev-config.c
- src/hev-config.h
- src/hev-config-const.h
- third-part/yaml/ (entire directory)
Update: remove yaml from build system

### Step 4: TUN drivers (Loop 4 passed)
Files to delete:
- src/hev-tunnel-linux.c / .h
- src/hev-tunnel-macos.c / .h
- src/hev-tunnel-freebsd.c / .h
- src/hev-tunnel-netbsd.c / .h
- src/hev-tunnel-windows.c / .h
- src/hev-tunnel.h

### Step 5: DNS cache (Loop 5 passed)
Files to delete:
- src/hev-mapped-dns.c
- src/hev-mapped-dns.h

### Step 6: Session handlers (Loop 6 passed)
Files to delete:
- src/hev-socks5-session-tcp.c / .h
- src/hev-socks5-session-udp.c / .h
- src/hev-socks5-session.c / .h

### Step 7: Core tunnel + lwip + hev-task-system (Loop 7 passed)
Files to delete:
- src/hev-socks5-tunnel.c
- src/hev-socks5-tunnel.h
- third-part/lwip/ (entire directory)
- third-part/hev-task-system/ (entire directory)
Update: remove lwip and hev-task-system from build system

### Step 8: Entry points (Loop 8 passed)
Files to delete:
- src/hev-main.c
- src/hev-main.h
- src/hev-jni.c

### Final verification:
- `cd rust && cargo build --release` produces working binary
- `cd rust && cargo test --workspace` all pass
- Original C Makefile build is expected to be broken (document this)
- Update README.md to note that the project is now pure Rust

## Exit criteria
- All C source files removed
- `cd rust && cargo test --workspace` passes
- `cd rust && cargo build --release` succeeds
- No references to deleted files in active code (grep check)
- Write LOOP_COMPLETE to signal completion

LOOP_COMPLETE
