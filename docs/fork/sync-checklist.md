# Upstream sync checklist

Every merge of an upstream release into `dho804`. Policy and history: private RE notes `docs/oos-fork.md`, "Upstream
sync". Nothing here touches the unit.

1. Branch `sync/<version>` from `dho804`; `git merge upstream/main` (no rebase).
2. Image builds are generic (ADR 0015). Keep shared defaults and dependency assembly in `boot/build-dev-image.sh`;
   device identity and overrides belong on the device. There are no per-unit build hooks to restore.
3. SSH instead of telnet (docs/network.md 7): keep the fork's side. rcS step 3 starts `oos-ssh`, never `telnetd`;
   `ui/src/remote.rs` handles firewall and SSH ACL reloads only. Drop any upstream telnet change: the fork has no
   telnet service, setting or menu item. Upstream's
   `remote::hostname` lives in the fork's `ui/src/hostname.rs` (`hostname::set`). The mDNS services in
   `Ui::mdns_config` (ui/remote.rs) announce `_ssh._tcp` while `ssh::listens()`, never telnet. The SSH page
   itself (`ui/src/ui/ssh.rs`, `ui/src/ssh.rs`, `oos-ssh`) is fork-only. Check: `grep -rn telnetd rootfs ui/src` finds nothing.
4. Conflicts in `hal/src/dho/seq.rs` Poll handling: upstream's Poll body lives in the fork's `plain_step`. Port any
   upstream change there; `execute_until` stays a thin wrapper.
5. The coordinator runs `cargo xtask check`
   ([default inventory](../development.md#executable-inventory-and-completion-evidence)) on the
   integrated batch, then `cargo xtask check --image` before unit rounds and releases. The default
   gate includes script syntax and Python tests; the image tier adds the ARM release link, GPU
   parity and all simulations. Repository CI is off until 0.2.0. Keep complete candidate logs.
   Downloaded sources: upstream's own build scripts (kexec, fsck, WLAN tools, Mesa) keep their pinned `ver=` and `sum=`
   untouched; `tools/deps-check.sh` reports where a script and `rootfs/deps.lock` differ after a sync, and the
   manifest line follows the script (docs/fork/deps.md).
6. The `target_os = "linux"` paths: the default gate's scope target clippy and the image tier's release link check the shipped crates for
   aarch64-unknown-linux-musl on the host, with the image build's flags. The earlier Docker check is no longer needed.
7. `tools/perf-guard.sh` (also a step of the gate): allocation counts and the early trigger end (deterministic), then the timing ratios of the
   fork's fast paths against reference copies of the paths they replaced. A failing ratio means the merge brought an
   upstream path back into the fork's loop, or slowed the fork's one: find which before fast-forwarding.
8. `gpu/test-host.sh` is the image tier's `gpu-host` step: expected CPU/GPU pictures for every
   script and all three monitor cost checks. The preceding `arm64-release-link` builds and verifies
   the selected output and receipts. Upstream's device frame-rate test still needs the unit.
9. `tools/plan-matrix.sh` with the old and the new `oos-fpgad`, `diff`: the register sequences must not change unless
   the merge meant to.
10. Record the sync in `docs/fork/unit-dho804.md` and private RE notes `docs/oos-fork.md`; fast-forward `dho804`.

## What the guards protect

| guard | where | breaks when |
|---|---|---|
| no allocation per read, one vector per channel per decode, none with kept vectors | `hal/tests/alloc_batch.rs` | `read_dma` or `deinterleave` + `volts` return to the batch loop; the decode allocates scratch space |
| early trigger end for new settings on the generic path | `hal/tests/early_trigger_end.rs` | an upstream Poll edit does not reach `plain_step` |
| compressed batch, then the newest frame raw | `hal/tests/compress_raw_again.rs` | `read_steps_at` stops resetting 0x1170 (pass-through) or the play index / length changes |
| pairs drawn like the raw fast path, glitches kept | `ui/src/compress.rs` tests | `draw_frame`, `fastdraw::deposit` or the trigger refinement change for envelope frames |
| fused f32 decode >= 2x upstream's two-pass decode; f32 <= 0.8x the f64 fused decode | `hal/tests/perf_ratio.rs` (ignored) | the decode stops vectorising, goes back to f64, or upstream's path returns |
| bit-level or bounded equivalence of the fast paths | `seq.rs`, `bus.rs` unit tests | a fast path computes something else than the path it replaced |
| oos-ui timing ratios | ignored tests named `perf_guard*` in `ui` | see the tests |
