# ADR 0012: Updates are signed, started on trial and rolled back when they fail

- Status: accepted, amended by ADR 0041
- Date: 2026-11-07 (first public version); trial start by kexec with a new device tree from 0.1.7
- Category: system

Trust-policy amendment, 2026-11-17: [ADR 0041](0041-trust-only-the-project-release-key.md) supersedes the
community-key requirement and its security consequence below. The signed-package, trial-start and rollback
decisions remain accepted.

## Context

An update replaces the whole image in `recovery`. The scope has no loader of OOS's own that could count failed starts
(ADR 0011), many owners have no second card, and a damaged or wrong package must not leave a scope that needs a
computer to recover.

## Decision

- An update is one package (`.oosupd`) with hashes and an Ed25519 signature. It is checked completely, also against
  what the stock loader accepts, before anything is written.
- The new image is first started by kexec with `recovery` untouched. Only an image that came up is written, and its
  first start from the card is a trial: the watcher confirms it when the scope screen runs stable, or writes the
  previous image back from the rollback slot.
- While `recovery` is written the boot magic is `KRNL`, so a power loss is followed by the stock system.
- Every build trusts a community key that is in the repository on purpose, so anyone can build an update that installs
  on any OOS scope. An owner who wants only their own builds replaces it with their own key.

Use vetted crates for cryptography: `ed25519-dalek` for Ed25519 and `sha2` for SHA-2, under ADR 0037's dependency
policy. Replace the hand-written implementations; use a maintained inflate crate too if one fits. This is the
decision for the update implementation, not a claim that those replacements have landed.

## Consequences

- The signature with the community key stops damaged and mistaken packages, not an attacker; the documentation says so.
- Not yet verified on hardware: the automatic rollback of a bad update (installs, the kexec trial start and its
  confirmation ran on a DHO924S, 2026-11-07/08).
- A hardware fault during the cold boot counts as a good start; System > Roll back update is the way back then.
- A private signing key that is lost means installing an image by hand on scopes that trust only that key.

## References

[update.md](../../update.md), `hal/src/update/`, `fpgad/src/bin/oos-update.rs`, `keys/update.pub`.
