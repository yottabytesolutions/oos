# ADR 0037: Crates where they remove code or blocking; remote IO runs async on tokio

- Status: accepted
- Date: 2026-11-17
- Category: code (touches process: the dependency rule)

## Context

ADR 0013 and ADR 0021 kept everything that runs on the scope on `libc` and the crates of the tree. The rule was
written for a small static binary that anyone can read. It did not weigh what the tree pays for it: hand-written
protocol servers, one accept loop copied per transport, and blocking socket code that reaches into the frame loop.
The SCPI raw socket writes its answer from the frame loop and can stall it for up to 2 s, with a 30 s limit (#47,
Wave D). Every transport polls its own sockets from `main.rs` (ADR 0022).

The licence and advisory risk of a crate is now checked by tooling: the `deny` step (`deny.toml`, cargo-deny) is a
required check and accepts only GPL-2.0-compatible licences and no known advisories.

Decided by the owner on 2026-11-17 (#47, #50).

## Decision

- **Dependencies.** Any crate of the workspace may depend on a well-maintained crate from crates.io when it removes
  code of our own or removes blocking. Prefer few, widely used crates with few dependencies of their own. A new
  dependency is a line in `xtask/data/architecture.txt` for its crate and a reason in the pull request; cargo-deny
  vets its licence and advisories. Host-only tooling (`xtask`) follows the same rule.
- **Remote IO is async.** SCPI raw, VXI-11, HiSLIP, HTTP and mDNS run on tokio, on one IO thread of their own, with
  a current-thread runtime. Tokio's features are limited to `rt`, `net`, `time`, `sync`, `io-util` and `macros`.
- **The frame loop stays synchronous.** The IO thread hands decoded requests to the frame loop over bounded channels
  and gets the answers back the same way. The frame loop drains its channel without waiting. It never awaits.
- **The domain and DSP stay synchronous.** `oos-core`, `oos-dsp` and `oos-hal` use no async runtime: their work is
  CPU-bound. `xtask/data/architecture.txt` confines tokio to the remote IO module of `oos-ui`.
- ADR 0022 holds: one SCPI interpreter, in the frame loop. Only the transports move to the IO thread.

## Alternatives

- **One `poll(2)` IO thread on `libc`:** the plan in #47 under the old rule. It removes the stall but keeps a
  hand-written event loop, timers and per-protocol state machines. Tokio already is that, tested by many users.
- **A thread per connection with blocking IO:** simple per transport, but every transport keeps its own accept
  loop, and threads grow with clients.
- **Async throughout, the frame loop included:** the frame loop is CPU-bound and owns the state without locks
  (ADR 0022). An executor there adds latency and nothing to gain.
- **Keep the libc-only rule:** the tree keeps paying for code a crate already holds.

## Consequences

- The scope binary pulls in crates. Its first build needs the network or a vendored registry, like `xtask` already
  does. `Cargo.lock` pins every version.
- A crate is code the project runs but did not write. The pull request that adds one says what it replaces and why
  this crate; cargo-deny catches licences and advisories, not quality.
- The frame loop no longer polls sockets. A blocking write cannot stall the screen.
- Answers that wait (`*OPC?`) stay continuations in the frame loop (ADR 0022); the IO thread only holds the
  connection open.
- The size of the image grows by what the crates add. Worth a second look if that becomes a problem on the card.
- ADR 0013's vendor rule moves to ADR 0040, unchanged.

## References

ADR 0013 and ADR 0021 (superseded), ADR 0022, ADR 0024, ADR 0025, `deny.toml`, `xtask/data/architecture.txt`,
[fork/deps.md](../../fork/deps.md), issues #47 and #50.
