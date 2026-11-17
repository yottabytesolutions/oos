# Governance

## Maintainer

OOS has one maintainer, GitHub user [@ichnograph](https://github.com/ichnograph). The maintainer decides what is
merged, sets the rules of the tree and cuts the releases.

## Decisions

Decisions others build on are written down as architecture decision records in [docs/adr](docs/adr/README.md). A
proposal is an ADR in a pull request; the maintainer accepts or rejects it. An accepted decision is changed only by
a new or amended ADR, not by a pull request that works around it.

## Review

Every pull request passes the same gate ([CONTRIBUTING.md](CONTRIBUTING.md#review-gate)): the required CI checks and
a Codex review of its head commit with no open findings. The pipeline is described in
[docs/process/ci.md](docs/process/ci.md).

Pull requests are sorted into [risk tiers](CONTRIBUTING.md#risk-tiers). The high-risk paths are listed in
[CODEOWNERS](.github/CODEOWNERS): hardware register writes, calibration, update packages and signing, boot, kexec,
the kernel and the pipeline itself. They also need the maintainer's approval and a run on a real unit. Other pull
requests merge on green checks.

## Becoming a reviewer

Reviewers are added when the project needs them. A contributor who has had several pull requests merged in an area,
and has reviewed others' pull requests there with useful findings, can be asked to become a reviewer for that area.
The maintainer records it in CODEOWNERS. Approval on high-risk paths stays with people who have a unit to test on.

## Forks

Forks are welcome, for experiments and for other models. Keep the licence and the authorship of the files you take.
A change from a fork comes back as a pull request against `main` under the rules above.

## Upstream

This tree is a hard fork of [OOS](https://github.com/dekajoule/oos)
([ADR 0030](docs/adr/process/0030-this-tree-is-a-hard-fork-of-oos.md)). Upstream is not merged, and it gets no pull
requests unless the maintainer asks for one. A single upstream change enters by cherry-pick, keeps its author,
carries `Cherry-picked-from: upstream <hash>` and passes the gate like any change.

## Conduct

Everyone who takes part follows the [Code of Conduct](CODE_OF_CONDUCT.md). The maintainer enforces it.
