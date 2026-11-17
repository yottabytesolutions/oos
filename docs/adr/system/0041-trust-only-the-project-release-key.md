# ADR 0041: Builds trust only the project's release key for updates

- Status: accepted
- Date: 2026-11-17
- Category: system

## Context

ADR 0012 requires every build to trust a community key whose signing seed is public in the repository. Anyone
can therefore sign a package those builds accept. That policy detects damaged or mistaken packages, but does
not restrict who can publish an accepted update. This hard fork has its own release key (ADR 0030, issue #102).

## Decision

Builds from this repository trust only the project's release key in `keys/update.pub`; neither the upstream
maintainer key nor the former public community key is a default trust anchor. The release signing secret stays
outside the repository, and signing requires an explicit key through `--key FILE` or `OOS_SIGNING_KEY`.

This supersedes only ADR 0012's community-key requirement and its security consequence. Its signed-package
checks, trial start and rollback decisions remain accepted. Owners may still build an image with their own
trust keys; a key used to sign a package does not change the installed build's trust policy.

## Alternatives

- **Keep the community key alongside the project key:** anyone with the public seed could still sign an
  accepted package, so the project key would not restrict update authorship.
- **Keep the upstream maintainer key:** it would let another project's releases authorize updates on this fork.

## Consequences

- Default builds reject packages not signed by the project release key, including community-signed packages.
  Public test signing fixtures are trusted by no build.
- Local builds need an explicitly supplied signing key. Installing their packages requires an image that trusts
  that key; the former `--community` signing mode is removed.
- Losing the sole trusted signing secret requires recovery with a hand-installed image. Keep the secret backed
  up outside the repository.
- The signature guards the update install path. The existing bench-LAN service policy and the owner's go for
  installation on a shared unit remain unchanged.
- Host regression tests pin the default key set and rejection of community signatures before card or state
  writes. This trust-policy change is not yet verified on hardware.

## References

[ADR 0012](0012-signed-updates-with-trial-start-and-rollback.md),
[ADR 0030](../process/0030-this-tree-is-a-hard-fork-of-oos.md), [update.md](../../update.md#keys),
`keys/update.pub`, `hal/src/update/tests/keys.rs`, `fpgad/src/bin/oos_update/sign.rs`, issue #102, PR #104.
