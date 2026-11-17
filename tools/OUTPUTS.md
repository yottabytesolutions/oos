# Worktree outputs and host runs

Use one worktree and one target directory per contributor. `CARGO_TARGET_DIR` selects
that directory; the default is `target` under the worktree root. Shell consumers
source `tools/output.sh`, which resolves relative overrides from the root, exports
an absolute path and rejects paths (including symlinks) outside the worktree or over tracked source directories.
For direct Cargo calls from crate directories, export an absolute value first:

```sh
export CARGO_TARGET_DIR="$PWD/target"
python3 tools/output.py build -p oos-ui
python3 tools/device-test.py --sim --only identity
python3 tools/output.py build --release --target aarch64-unknown-linux-musl -p oos-ui -p oos-fpgad
```

Cargo host and ARM artifacts use their normal layout under the selected directory.
Coverage uses its `xtask-cov/<crate>/` children. GPU builds use `gpu/` (bundle `gpu/gpu/`);
GPU comparisons use unique `gpu-check.*` directories. Mesa's installed build cache
and checked source tarball stay in worktree-local `rootfs/out/mesa/<environment>/` and
`rootfs/out/mesa_*.orig.tar.xz`, outside the target, so `cargo clean` preserves them.
The ARM hardware helper uses `hw/`. Image assembly and signing use `image/`, replacing the old `boot/out` default.
GEL packaging verifies the signed package and ARM updater receipts and writes its
own receipt for simulation. The build-id sidecar is verified before signing too.
The image workflow reads and uploads from the same selected directory.
Consumers with explicit image arguments must use the selected directory. Kernel and rootfs prerequisite caches keep their
existing worktree-local locations; their builders record the selected environment and installed package inventory.

Rootfs tool receipts use the specific builder script and its `rootfs-<tool>` environment identity.
For builders using `deps.sh`, they hash the exact `dep_stamp_text` description consumed by build-dev-image:
builder hash, helper hash, per-tool environment identity and selected dependency name/version/checksum
fields from `deps.lock`. URL-only edits leave both receipts and stamps current. Environment queries use
`tools/build-env.sh` so callers can supply a fixture resolver. Inline source versions/checksums are
covered by the builder script itself.
A Rust edit leaves e2fsck, kexec, WLAN, nft, Dropbear and SFTP receipts valid; changing their actual inputs
or replacing their payloads invalidates them. Builders capture `tools/output.py identity ARTIFACT` before
building and pass it to `stamp`, which checks for input changes during the build. Rust compiler probes run
from the repository root so the selected toolchain is independent of the caller's working directory.

Build consumable Rust binaries through `tools/output.py build`. It runs locked,
offline Cargo and writes adjacent `.oos-build.json` receipts only on success and
when build inputs did not change during the build. The source hash covers the sorted,
deduplicated paths from `git ls-files -z --cached --others --exclude-standard`, excluding:

- the selected output directory and everything under it;
- root `docs/**` (including `docs/tasks/**`) and `.github/**`;
- files with the `.md` suffix at any depth;
- draft files whose basename starts with `COMMIT_MSG`.

The tracked fonts `ui/assets/font*.bin` are inputs: oos-ui embeds them with `include_bytes!`.

Each remaining existing entry contributes its relative path, filesystem mode and contents
(a symlink contributes its link target, including dangling links). Missing tracked paths
are absent inputs, so committing a deletion also keeps the same hash. HEAD and the
tracked/untracked distinction are absent, so committing unchanged
inputs or editing a task document keeps receipts valid. Editing a `.rs` file invalidates
them. Receipts also cover artifact contents and permissions. Missing receipts and replaced
payloads require rebuilding. Ignored inputs, including `kernel/out/base` and `rootfs/out`,
Cargo features and `RUSTFLAGS` are out of scope: changing
them requires an explicit rebuild; this receipt does not verify their provenance or
build configuration. GPU bundle receipts include the libraries. Image assembly,
signing, GPU comparisons, simulated device tests and owner-only deploy verify
their inputs before consumption. Plain Cargo builds are useful for development
but do not produce these receipts. Do not override output flags on the build helper.

`--serve` is a host-only loopback server with all listeners allocated using port 0;
its old fixed port options no longer select host ports. HTTP and MCP share one
port. VXI-11 has separate dynamic TCP/UDP mapper ports and a dynamic core port;
GETPORT advertises that core port. Device service ports are unchanged.
`host_harness.Host` verifies the executable, creates a private temporary storage
directory, launches the child, and waits up to 15 seconds for an atomically
published readiness record with its PID, `build` and all five ports. `build` echoes
the parent's non-empty hexadecimal `OOS_HARNESS_BUILD` value; the child does not hash
itself. Identity is checked by `host_harness`'s `output.verify` calls before launch
and after readiness, together with the child PID. Failed startup exits non-zero;
the harness includes the child's stderr tail in its exception before cleanup.
Context exit terminates and reaps the
child before removing its private directory. Test reports remain in unique
`host-results-*` directories. SSH/firewall/USB/WLAN/GEL and DTB simulations also use private temporary
directories. No hardware access is needed for these host runs.

Verification commands (the socket test needs an unrestricted host):

```sh
python3 -I -m unittest discover -s tools/tests -p test_output_harness.py
python3 -I -m unittest discover -s tools/tests -p test_image_outputs.py
cargo test -p oos-ui --test host_sockets
```


Rust receipts also hash the actual `rustc -vV` and Cargo identities. A changed compiler invalidates shipped Rust
binary receipts even if source, pin files and executable bytes match. The build helper checks compiler identity
before and after building. The digests contain no host paths or raw tool output. Existing Rust receipts without
this identity require rebuilding. Container base/recipe/package-request changes are source inputs, so they invalidate
receipts. The lock's environment digest selects independent Mesa caches and kernel build volumes instead of
removing old caches. Rootfs dependency stamps include the locked environment digest too. A completed kernel build records its
environment under `kernel/out/base/build-environment`; image assembly rejects a missing or different identity.
This stamp covers the base/recipe/request contract; task 0033 retains responsibility for complete kernel input provenance.

Prepare selected environments explicitly with `tools/bootstrap.sh --gpu`, `--sim` or `--image`; simulation scripts
only resolve existing base/recipe-identified images. Doctor never pulls or installs them. Builders record sorted
`name=version` package inventories next to their output; these files are informational, without version locks.
The base/recipe/request identity does not freeze apt/apk resolution or detect package changes after a rebuild.
Task 0033 owns complete package locks and the binfmt installer identity. See `docs/development.md` for output
inventory names and setup commands. Linux x86-64 runner validation remains untested, without a hold here.
