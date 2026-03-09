# Memories

## Patterns

## Decisions

### mem-1773060442-b375
> hev-config.c depends on lwip (tcp.h) and yaml.h (libyaml) — submodules NOT initialized. Differential test for config cannot link against C parser directly. Strategy: write 12-variant YAML regression test in Rust only, OR provide a thin C shim that avoids libyaml linkage.
<!-- tags: config, differential-testing, submodules | created: 2026-03-09 -->

## Fixes

## Context
