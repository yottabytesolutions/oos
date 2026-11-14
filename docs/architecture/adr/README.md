# Architecture decision records

One file per decision, `NNNN-short-name.md`, from `0000-template.md`. A decision is changed by a new record that
supersedes the old one. The target architecture is `../README.md`.

| ADR | Decision |
|---|---|
| [0001](0001-single-settings-owner.md) | One owner for settings |
| [0002](0002-commands-per-area.md) | Settings change only through commands, one enum per area |
| [0003](0003-one-change-point.md) | One change-detection point with requests |
| [0004](0004-explicit-stages.md) | The step is a fixed pipeline of stages with typed I/O |
| [0005](0005-single-picture-owner.md) | One owner of the picture |
| [0006](0006-batch-as-unit.md) | A batch is the unit of drawing |
| [0007](0007-threads-typed-channels.md) | Threads talk through typed channels |
| [0008](0008-ports-where-they-pay.md) | Ports and adapters where they pay |
| [0009](0009-formatting-and-lint.md) | Formatting and lint are enforced |
| [0010](0010-size-limits.md) | Size limits with a ratchet |
| [0011](0011-golden-pictures.md) | Golden pictures pin the drawing |
| [0012](0012-error-policy.md) | Error policy |
| [0013](0013-one-batch-path.md) | Frames mode becomes the batch path |
| [0014](0014-domain-crate.md) | The domain is its own crate, oos-core |
| [0015](0015-safe-start.md) | Safe start breaks restart loops |
