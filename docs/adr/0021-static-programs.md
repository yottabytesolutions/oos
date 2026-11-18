# Scope programs are static musl binaries and abort on panic

- Number: 0021
- Category: code
- State: in force
- Date: 2026-11-18

## Context

The scope programs must run independently of the stock userspace's libc and library versions. Mesa requires glibc, while
the Rust programs use static musl, so putting both in one process would couple their runtime requirements. Release panic
recovery belongs to the supervisor rather than unwinding through hardware-facing code.

## Decision

Build scope Rust programs for aarch64-unknown-linux-musl with static CRT and rust-lld. Each program is one static
binary. Release builds abort on panic. The C GPU helper alone uses glibc and Mesa in another process (ADR 0026). Host
tooling and test profiles are not constrained to this target or panic strategy.

Check: .cargo/config.toml and Cargo.toml profile review; `arm64-release-link` validates the static AArch64 ELF.

## Alternatives

Link Rust against stock shared libraries: rejected because deployment would depend on their ABI and availability. Link
Mesa into the Rust program: rejected because it needs glibc and would couple GPU failure to the scope process.

## Consequences

An unrecoverable Rust panic ends the process. The supervisor handles restart; unwinding cannot provide runtime recovery.
