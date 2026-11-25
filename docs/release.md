# Assemble a release offline

GitHub-hosted runners test the packaging tools with temporary keys and build the unsigned DHO image.
The draft release carries the image, its build ID and model list, the image contents manifest,
`SHA256SUMS.unsigned` and the CI attestations. The maintainer signs the release on the offline machine.

## Version bump

- [ ] From the repository root, run `cargo xtask bump X.Y.Z` with the next release version. It updates
  the workspace crate manifests, both lockfiles, the version-bearing golden pictures and RGB hashes,
  and the changelog. It promotes an existing Unreleased heading and keeps its notes. Add the release
  notes below the new heading if it was empty.
- [ ] Review the bump diff and run `cargo xtask bump --check`. The fast gate runs the same check.
  If Cargo rejects a stale workspace lockfile before xtask starts, run the previously built
  `target/debug/xtask bump --check` for the full mismatch list. Use `--allow-same` to refresh the
  current version. Downgrades are refused.

## Inputs

- [ ] Take the unsigned assets and CI attestations from the draft for the tag being released. Verify the
  attestations against the repository and tag, and verify the downloads with
  `sha256sum -c SHA256SUMS.unsigned`. Record the attested image commit.
- [ ] On the offline machine, use a clean checkout of that commit and the pinned toolchain with cached
  dependencies. Build a host updater with `cargo build --offline --locked --release -p oos-fpgad --bin oos-update`.
- [ ] Record `python3 --version` and Python's `zlib.ZLIB_VERSION` and `zlib.ZLIB_RUNTIME_VERSION`.
  Reproducing the GEL bytes requires matching the Python and zlib versions used for level-9 gzip compression.
- [ ] Supply the project's signing key from outside the checkout. For a bridge, supply
  `keys/community-signing.key` from a separate upstream checkout. Keep both key paths local.

## Build

- [ ] Set `tag` to the draft's tag, `commit` to the attested commit, `key` to the offline key path, and
  `updater` to the absolute path of `target/release/oos-update`. Set `unsigned` to the directory outside the
  checkout that holds the unsigned assets. Run from the release checkout with a fresh output path.

  ```sh
  python3 -I tools/build-release.py --image "$unsigned/oos-$tag.img" \
    --tag "$tag" --commit "$commit" --updater "$updater" --key "$key" \
    --out "target/release-$tag"
  ```

  `OOS_SIGNING_KEY` can supply the key instead of `--key`. Add `--community-key "$community_key"`
  to the same command to include the bridge. The builder reads the build ID, models and stock installer
  from the image. It verifies the bridge against the upstream community public key and reserves 64 KiB of the
  upstream 70 MiB upload body limit for multipart framing.

## Verify

- [ ] From the output directory, run `sha256sum -c SHA256SUMS`. Compare the image digest with the
  attested unsigned image. Compare the manifest's commit, build ID, models, sizes and key IDs with
  the CI metadata and the verifier output.
- [ ] Run `"$updater" verify "oos-$tag.oosupd"`. Dry-run the package with
  `"$updater" install "oos-$tag.oosupd" --card "$stock_card" --state "$state" --model "$model"`.
  Use a regular stock-layout card fixture and set `state` to a scratch path such as
  `"$scratch/oos/update/state"`. Keep `--execute` absent.
- [ ] Extract the GEL into an empty scratch directory. Compare `oos/oos.oosupd` with the verified
  package using `cmp`, verify it with `"$updater" verify`, and compare `oos/oos-update` with
  the updater in the image contents manifest. Dry-run the extracted package with the host
  `"$updater" gel-install "$scratch/oos/oos.oosupd" --card "$stock_card" --vendor "$stock_vendor" --save "$scratch/rescue"`.
  Supply a fixture vendor file for a verified model. The dry run writes the rescue set to scratch.
- [ ] If a bridge is present, use an upstream host `oos-update` to run `verify "$bridge"` and
  `install "$bridge" --card "$stock_card" --state "$state"`. Use an upstream test state whose
  current build stamp is no newer than the bridge. Verify that the card fixture and state stay
  unchanged during the dry runs. Use the authenticated release `SHA256SUMS` as the trust anchor
  for the bridge.
- [ ] Record the owner-approved unit round before publication. The [install](install.md) and
  [switching](switching.md) drafts list the flows to verify.

## Attach and publish

- [ ] Keep the attested CI image, contents manifest, build metadata, `SHA256SUMS.unsigned` and
  attestations in the draft. The image from offline packaging is byte-identical to the CI asset.
- [ ] Attach the project-signed `.oosupd`, the GEL, `release-manifest.json`, `SHA256SUMS` and the
  labelled bridge, if included. Deliver `SHA256SUMS` through the authenticated project release
  channel. Include the source materials listed by `tools/release-sources.sh list`.
- [ ] Confirm the uploaded artifact inventory and checksums, then publish the draft after owner approval.
