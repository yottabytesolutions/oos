# Journal the shared data area

- Number: 0039
- Category: system
- State: in force
- Date: 2026-11-24

## Context

A power cut during file replacement left metadata inconsistent in the 16 MiB shared data area. The kernel
already uses the ext4 driver for this area, with JBD2 built in.

## Decision

Add only the ext3-compatible `has_journal` feature. Format an empty area with a journal. Convert existing ext2
in place on the checked scratch image if at least 1 MiB remains available after reserved blocks. The checked
writer publishes the primary journal flag last. Startup replays on scratch and persists the result before mount.
Retain the offline repair and verification path.

Builds before `a18a71b0` wrote a replayed primary superblock first. A cut can then leave the recovery
flag clear with a nonzero JBD2 start field. Startup forces journal-only replay on scratch in this state, then
checks the filesystem. It writes the replayed home blocks before clearing the journal head.
A read-write OOS data mount through the ext2 driver or with `noload` would let a later forced replay overwrite newer data. This fork never mounts the area that way.

First format writes and syncs `OOSDATA:first-format:v1` followed by zero padding in the first 512-byte sector
of the ext boot area. The writer can start that intent only after finding the entire data area zero. It then
writes backup superblocks and descriptors, ordinary sectors, and the journal inode. It clears and syncs the
intent before writing either primary superblock sector, with the primary flag sector last.
Under sector-prefix cuts, startup can repeat the format with an exact intent and no superblock copy. From the
first backup onward it repairs from the backup until the primary is committed. Startup clears a stale intent
beside an existing filesystem and preserves its files. Unrecognized nonzero data without an ext superblock is refused.

The storage image tests exercise first format, conversion and replay at each changed sector.

## Alternatives

Btrfs and F2FS need more space and kernel support. Full ext4 would enable features beyond the compatibility
contract with older builds and upstream.

## Consequences

With 1 KiB blocks, the journal uses 1024 blocks plus five indirect blocks. Existing files stay in place. A cut
may still lose changes that were outside a committed transaction. Conversion and replay have host evidence and
await unit verification.
