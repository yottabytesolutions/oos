# Behaviour-preserving changes prove equality against baselines

- Number: 0008
- Category: policy
- State: implemented on host; new layout references require owner review
- Date: 2026-11-18

## Context

Moves can preserve an interface while changing pixels, register writes or computed values. Committed demo/fake-board
pictures, the 65-plan host matrix and bit-for-bit value comparisons catch those differences before a unit round. Current
CPU goldens use deterministic synthetic inputs and bundled fonts so pixel equality can be required.

## Decision

A behaviour-preserving change proves equality against the applicable committed baselines: golden pictures in
`ui/tests/golden/`, register plans in `xtask/data/plan-matrix.txt`, and bit-for-bit comparisons for moved computations.
Baseline changes have a reason in the commit message, a pull request label identifying the baseline change and
maintainer approval. CPU goldens require byte-for-byte equality of decoded RGB data, with no tolerance or masked regions.
`ui/tests/golden/README.md` lists the layout coverage and the limits of deterministic host inputs. CPU/GPU comparison
uses the tolerance in `gpu/test-host.sh`. Host equality proves nothing about timing; timing claims require unit evidence.

Check: `tests-debug`, `tests-release`, `plan-matrix`, CODEOWNERS on the baseline paths; review checks the label, reason
and value comparisons.

## Alternatives

Rely on review or unit timing alone: rejected because neither proves pixel, register or value equality. Replace
layout-check immediately: rejected until golden coverage includes every script and all three text sizes.

## Consequences

The temporary layout-check procedure stays in docs/development.md until its coverage replacement is accepted.
