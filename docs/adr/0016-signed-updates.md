# Updates trust only the project key and use trial rollback

- Number: 0016
- Category: system
- State: in force
- Date: 2026-11-18

## Context

An update replaces all of recovery, but OOS has no loader that can count failures before rcS starts and many owners have
no spare card. The former community signing seed was public, so anyone could authorize an accepted package. A
project-only trust set restricts authorship; verification and a retained image separately limit update damage.

## Decision

An update is one .oosupd package with SHA-256 hashes and an Ed25519 signature. Verify the entire package, stock-loader
image checks, board and model list before writing. Builds trust exactly the project release key in keys/update.pub; no
signing secret is in the tree. Signing requires --key or OOS_SIGNING_KEY. --any-model is explicit and logged; it
bypasses no other check.

Start the candidate by kexec while recovery is untouched. Write only an image that came up. Its first cold start is a
trial. The watcher confirms it after the stability interval in docs/update.md or restores the previous image from the
rollback slot. Keep KRNL while writing recovery. Cryptography and inflate use the maintained crates in hal/Cargo.toml,
not local implementations.

Check: hal update tests, including the pinned trust set; a unit update round before each release.

## Alternatives

Keep the community key beside the project key: rejected because its public seed still authorizes anyone. Trust the
upstream maintainer key: rejected because another project would authorize this fork's updates. Write recovery before
trying the candidate: rejected because failure before rcS cannot be counted by an OOS loader.

## Consequences

A lost signing secret requires hand installation on each unit. A panic before rcS in a confirmed image does not roll
back. Automatic rollback is not yet verified on hardware; the follow-up is listed in migration.md.
