# Releases

## 0.2.0

Goal: ship a fast, responsive picture at the stable stop point after M12.
Keep the guardrails and golden pictures in place.

Exit criteria:

- M1 through M12 meet the [migration specification](architecture/migration.md#steps).
- Guardrails and golden comparisons pass. The full gate passes on the integrated result.
- Unit rounds through round 5 and the full stress round pass with the required hardware evidence.
- Publish the accepted build and update the upstream PR.

State: active.

## 0.3.0

Goal: finish M13 through M18. Settings and commands have explicit owners and `Ui` is removed.

Exit criteria:

- M13 through M18 meet the [migration specification](architecture/migration.md#m13-settings-groups-replace-model).
- The target layout and boundary rules enforce. Legacy size entries for `ui.rs`, `hw.rs` and `model.rs` are gone.
- The full gate and golden comparisons pass on the integrated result.
- Unit round 6 passes against the M12 acceptance numbers.

State: planned.
