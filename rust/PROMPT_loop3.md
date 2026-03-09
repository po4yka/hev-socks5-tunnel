# Loop 3: Config Crate (TDD + Differential)

Repository: /mnt/nvme/home/po4yka/hev-socks5-tunnel
Crate: hs5t-config
C reference: src/hev-config.c (682 LOC) + conf/main.yml

## Required validation rules (from C source):
- socks5.address and socks5.port are REQUIRED; absence → Err
- username and password must be both present or both absent
- mapdns section is entirely optional
- misc section is entirely optional (all fields have defaults)
- Defaults: task_stack_size=86016, tcp_buffer_size=65536

## TDD sequence (tests BEFORE implementation)

### Write these tests first:
1. Parse conf/main.yml → all fields match expected values
2. Missing socks5.port → Err with descriptive message
3. Missing socks5.address → Err with descriptive message
4. username present, password absent → Err
5. password present, username absent → Err
6. All defaults applied when optional sections absent
7. mapdns.network, mapdns.port defaults correct
8. misc.task_stack_size default = 86016
9. misc.tcp_buffer_size default = 65536
10. misc.udp_buffer_size default (from C source)

### Then implement with serde + serde_yaml

### DIFFERENTIAL TEST (tests/differential/config_diff.rs):
- Compile hev-config.c as shared lib via cc crate
- Expose test_config_from_str() returning flat C struct
- Test 12 YAML variants: all fields, defaults only, missing required, auth combos
- Assert every field matches between C and Rust

### PROPERTY-BASED TEST (proptest):
- Generate random valid YAML configs; verify parse() succeeds and roundtrips
- Generate configs with missing required fields; verify parse() fails

### FUZZ TARGET (tests/fuzz/fuzz_config.rs):
- Arbitrary bytes as YAML; verify no panic (Err is fine, panic is not)

## Config struct requirements
- Must be Send + Sync (required for Arc<Config> in core)
- All string fields: use String (owned)
- Optional fields: use Option<T>
- Implement Debug, Clone

## Exit criteria
- All tests pass; serde_yaml parses conf/main.yml correctly
- Differential test: 12 YAML variants × all fields match C
- Config struct is Send + Sync
- Fuzz target runs 1M iterations with no panics
- Write LOOP_COMPLETE to signal completion

LOOP_COMPLETE
