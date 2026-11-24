# Hard fork; no upstream merges or pull requests

- Number: 0001
- Category: policy
- State: in force
- Date: 2026-11-18

## Context

Regular upstream intake required a special ratchet rebaseline path and still failed the gate during integration 30,
which was aborted. Sending changes back required porting them to upstream's different layout and rules. A hard
fork allows the tree to enforce one set of rules without intake exceptions.

## Decision

This tree is a hard fork of upstream OOS at tag `hard-fork-20261117`. `yottabytesolutions/oos` is the source of
truth. `main` is the only long-lived branch. Upstream is neither merged nor tracked. No pull requests go upstream.

A single upstream commit can enter by cherry-pick. It keeps its author, carries `Cherry-picked-from: upstream <full
hash>` and passes the gate without exemptions. The licence remains GPL-2.0-or-later. Copyright lines, `LICENSE`, history
authorship and README attribution stay.

Check: the pr-meta CI check and the linear-history ruleset reject merge commits; review checks cherry-pick trailers and
attribution.

## Alternatives

Keep upstream merges: rejected because each intake widened ratchets and required special rebaselining. Rebase onto
upstream and send all changes back: rejected because the restructuring and upstream's different rules made porting cost
more than it returned.

## Consequences

Upstream fixes enter individually. Reversing the fork requires a new owner-approved decision.
