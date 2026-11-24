# Releases

See [Assemble a release offline](release.md) for packaging, verification and publication.

Verify a downloaded release asset with `gh attestation verify <file> -R yottabytesolutions/oos`.
Its `<file>.sigstore.json` bundle and, when available, `<file>.intoto.jsonl` statement are attached to the release.

## Versioning

- Releases are annotated SemVer tags `vX.Y.Z` on `main`, starting at `v0.2.0` for the hard fork.
- The version lives in the crate manifests of core, dsp, fpgad, hal, ui and xtask. A release is one
  `chore(release): vX.Y.Z` pull request that sets all six, then the tag. `release.yml` refuses a tag that differs.
- `oos-ui` reports it through `CARGO_PKG_VERSION` in `*IDN?`, the web UI and mDNS.
- The update package build id stays `YYYYMMDD-HHMMSS kernel <rel>`; the downgrade check compares it as text. A
  signed `version` field in the package is proposed, not implemented (risk path, needs its own task and a unit
  test).

## 0.2.0

Goal: ship a fast, responsive picture at the stable stop point after M12.
Keep the guardrails and golden pictures in place.

Exit criteria:

- M1 through M12 meet the [migration specification](architecture/migration.md#steps).
- Guardrails and golden comparisons pass. The full gate passes on the integrated result.
- Unit rounds through round 5 and the full stress round pass with the required hardware evidence.
- Publish the accepted build from this fork. No pull request goes upstream (ADR 0001).

State: active.

## 0.3.0

Goal: finish M13 through M18. Settings and commands have explicit owners and `Ui` is removed.

Exit criteria:

- M13 through M18 meet the [migration specification](architecture/migration.md#m13-settings-groups-replace-model).
- The target layout and boundary rules enforce. Legacy size entries for `ui.rs`, `hw.rs` and `model.rs` are gone.
- The full gate and golden comparisons pass on the integrated result.
- Unit round 6 passes against the M12 acceptance numbers.

State: planned.
