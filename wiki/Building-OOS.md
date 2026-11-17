How to build the kernel, choose a signing key and build the image, on macOS or Linux. You need Rust with the `aarch64-unknown-linux-musl` target
and Docker: see [You will need](Install-on-a-second-card#you-will-need).

## Build the kernel

Download `linux-6.12.115-base.tar.xz` from the [releases](https://github.com/yottabytesolutions/oos/releases) into `kernel/src/`
([kernel/README.md](https://github.com/yottabytesolutions/oos/blob/main/kernel/README.md)), then:

```sh
kernel/build-base.sh        # in Docker, 5-30 minutes depending on the machine, result in kernel/out/base
```

You only have to repeat this when the kernel changes.

## Signing key

Update packages must be signed. To let everyone build updates for every OOS scope, the repository includes a **public
community key**, `keys/community-signing.key`, and builds trust it by default. Signing requires explicit selection.
The image build never signs. Select a private key with `--key` or `OOS_SIGNING_KEY`, or the public development fixture
with `--community`.

The community key only protects against damaged packages. It says nothing about who built one, so **only install
packages from people you trust.**

If you want a scope to accept only your own builds, create your own key before you build:

```sh
cargo run --offline --release --manifest-path fpgad/Cargo.toml --bin oos-update -- keygen --out /path/to/signing.key
```

Replace all keys in `keys/update.pub` with the line it prints. Keep the secret key outside the repository.
**Back it up and never publish it.** Details:
[docs/update.md](https://github.com/yottabytesolutions/oos/blob/main/docs/update.md).

## Build the image

These paths use the default `target` directory. Set `CARGO_TARGET_DIR` to select
another directory inside this worktree, and use its `image/` directory below.
See [the output contract](https://github.com/yottabytesolutions/oos/blob/main/tools/OUTPUTS.md).

```sh
boot/build-dev-image.sh
boot/sign-dev-image.sh --key /path/to/signing.key # or --community for development
boot/build-gel.py
```

These separate steps produce `target/image/oos-dev.img` for the card, `target/image/oos-dev.oosupd`, the signed
update package, and `target/image/DHO800_DHO900_OOS_Install.GEL` for the USB stick. Signing and packaging verify
build receipts before consuming these local artifacts.

## Building on Linux

The kernel, the key and the image on Linux, here with Debian or Ubuntu, on an ordinary x86-64 PC as well as on arm64. On x86-64:

- the kernel and `oos-hw` are cross-compiled with Ubuntu's `gcc-aarch64-linux-gnu` inside the build container; you
  install no cross compiler yourself,
- the Rust programs are cross-compiled by rustup's `aarch64-unknown-linux-musl` target and `rust-lld`,
- busybox, wpa_supplicant and e2fsck come from arm64 containers, which run through QEMU (the `binfmt` line below).
  `docker run --rm --platform linux/arm64 alpine:3.22 uname -m` prints `aarch64` when that works.

Run everything as yourself, not with `sudo`.

```sh
sudo apt install docker.io git python3 curl
sudo usermod -aG docker $USER      # then log out and in again, so that docker runs without sudo
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh      # Rust from rustup, not from apt
rustup target add aarch64-unknown-linux-musl
# x86-64 only: arm64 containers through QEMU, for busybox, wpa_supplicant and e2fsck (again after a reboot)
docker run --privileged --rm tonistiigi/binfmt --install arm64

git clone https://github.com/yottabytesolutions/oos.git && cd oos
mkdir -p kernel/src
curl -fL -o kernel/src/linux-6.12.115-base.tar.xz \
  https://github.com/yottabytesolutions/oos/releases/latest/download/linux-6.12.115-base.tar.xz
kernel/build-base.sh
boot/build-dev-image.sh
```

The scripts say what is missing before they start: Docker not reachable, no rustup or no target, no arm64 containers.
On x86-64 the first build takes longer than on arm64, mostly for wpa_supplicant and e2fsck under QEMU; they are built
once. A finished build leaves the same files as on macOS. The workflow `.github/workflows/image.yml` builds on arm64
and on x86-64 for every release and compares the kernel, the DTB and the files inside the image of the two. To install them on a second card from Linux, see
[[Install on a second card]], with `/dev/sdX` for the card.
