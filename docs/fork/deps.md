# Downloaded sources of the image

Every source tarball the image builds from is listed once, in `rootfs/deps.lock`: name, owner, version, SHA-256, URL
(and, where the URL's directory has no listing, a page that lists the releases).

- The rootfs builds read it: `rootfs/build-dropbear.sh` (Dropbear), `rootfs/build-sftp-server.sh` (OpenSSH,
  `sftp-server` only), `rootfs/build-nft.sh` (libmnl, libnftnl, nftables), `rootfs/build-kexec.sh` (kexec-tools),
  `rootfs/build-fsck.sh` (e2fsprogs) and `rootfs/build-wlan-tools.sh` (wpa_supplicant, libnl). They source
  `rootfs/deps.sh`, whose `dep_fetch` is the one way a source is downloaded: once into `rootfs/out/sources/`
  (`DEPS_SOURCES` names another directory), from the line's URL and then the mirrors of its host (kernel.org: `cdn`,
  `www`; OpenBSD: `ftp`; netfilter.org: `ftp.netfilter.org/pub`; Dropbear: `dropbear.nl`). Each host gets a 20 s
  connect timeout and three retries with backoff. A file whose SHA-256 differs from the line stops the build and is
  deleted. The containers mount the directory read-only as `/src`.
- `deps.sh` also stamps each binary with the manifest name/version/checksum fields, builder hash, `deps.sh` hash and
  per-tool `rootfs-<tool>` build-environment identity (`<binary>.stamp`). Native binary receipts from
  `tools/output.py` hash this same input description. `boot/unit-hook.sh` rebuilds a missing binary or one whose
  stamp no longer matches, so a version bump, builder option or helper-only change takes effect at the next image
  build.
- `gpu/build.sh` keeps its own `ver=` and `sum=` and takes Mesa from Ubuntu's source package. Its line in the manifest
  (`upstream:gpu/build.sh`) documents it, and `tools/deps-check.sh` reports when the script and its line differ.
- `libnl` and `libnl-musl-patch` (owner `alpine:3.22`) are not built: `wpa_supplicant` links the `libnl3-static` of the
  Alpine image. The two lines are the files Alpine builds that package from. `rootfs/build-wlan-tools.sh` fetches them
  for the release and stops when Alpine installs another version.
- CI (`.github/workflows/image.yml`) caches `rootfs/out/sources/` keyed on the hash of `rootfs/deps.lock`. An older
  cache may be restored: the checksum check still applies to every file.
- What a release has to carry next to its image is listed by `tools/release-sources.sh list`, from this manifest and
  `kernel/base.sha256`; `tools/release-sources.sh check <tag>` compares a release on GitHub with it
  ([THIRD-PARTY-LICENSES.md](../../THIRD-PARTY-LICENSES.md)).
- Not in the manifest: the Docker base images (`alpine:3.22`, `ubuntu:26.04`, `rust:1-alpine`), busybox (Ubuntu's
  `busybox-static`, its version and the glibc it was built with recorded in `busybox.version` of each build), the
  kernel base archive
  (`kernel/src/SHA256SUMS`) and Rust crates (`Cargo.lock`).

## Rust crates

A crate from crates.io is added only where it removes code or blocking
([ADR 0037](../adr/code/0037-crates-where-they-remove-code-remote-io-on-tokio.md)). Each crate's allowed dependencies are its line in
`xtask/data/architecture.txt`; cargo-deny (`deny.toml`) vets licences and advisories. Crates outside `xtask` are
listed here. There are none: the Windows desktop window and its crates moved to
[oos-desktop](https://github.com/yottabytesolutions/oos-desktop) (ADR 0036).

## Checking for updates

`tools/deps-check.sh` reads each line's release listing and prints the pinned and the newest release (no release
candidates); exit status 1 when one is newer or an upstream script differs from its line. It only reads public
listing pages.

## Updating one

1. Read the release notes of the new version (security fixes, changed build options).
2. Set the version and the URL in its `rootfs/deps.lock` line.
3. Get the SHA-256 from a second source, not only from the downloaded file: OpenSSH's release notes (base64
   SHA256), Alpine's aports or Debian's `.dsc` for the same tarball, or a signature check. Write it into the line.
4. Build: `rootfs/build-<name>.sh` (or the next image build, which sees the stale stamp). A wrong checksum stops the
   build and deletes the download.
5. Run the sims that cover it: `tools/ssh-sim.sh` (Dropbear, sftp-server), `tools/fw-sim.sh` and `tools/ssh-sim.sh`
   (nft), then an image build and the unit checks of the feature (docs/network.md 6, 7).
6. Commit the manifest line (and a build script change, if the new version needs one) with the sources' release in
   the message.

For the `upstream:` line: change `gpu/build.sh` and the line together.

## Pending updates (2026-11-13)

`tools/deps-check.sh` found newer releases that were not taken in this round: libnftnl 1.2.9 -> 1.3.2, nftables
1.1.3 -> 1.1.7 (fork, `rootfs/build-nft.sh`); e2fsprogs 1.47.2 -> 1.47.4, wpa_supplicant 2.11 -> 2.12, Mesa 26.0.8 ->
26.3.0. Dropbear, OpenSSH, libmnl and kexec-tools are current.
