# Lessons

Mistakes made in this repository and the rule that prevents a repeat. Read it before you start. Add an entry in the
pull request that fixes a mistake: what happened, then the rule.

## Git and pushing

- A rebase and a push chained with `;` pushed a half-rebased branch that equalled `main`, and GitHub closed the pull
  request. Chain dependent git steps with `&&`, and check the result of a rebase before pushing.
- `git push --force-with-lease` without a refspec was refused on a worktree branch that tracked `origin/main`.
  Push feature branches with an explicit refspec: `git push --force-with-lease=<branch>:<old-sha> origin
  HEAD:refs/heads/<branch>`.
- Git hooks export `GIT_DIR`. Test fixtures that ran `git` inherited it and rewrote the real clone's config
  (`core.bare=true`). Clear the `GIT_*` variables in every test that runs git.
- The global `fetch.pruneTags` setting deleted local release tags. The clone sets `fetch.pruneTags=false`.
- An agent ran `git checkout --detach` in the shared clone instead of its worktree. Never check out in the shared
  clone; use `git -C <worktree>` or `git show`.
- The Codex sandbox could not write the shared `.git` (`index.lock`) and refused a worktree path through a symlink.
  Let the runner commit, and use real worktree paths.
- After a rebase stopped on conflicts and the resolver failed, the rebase continued with conflict markers. Abort
  the rebase when conflicts are not resolved, and check for markers before continuing.

## Merging

- A squash merge made the bot the author and added a `Co-authored-by` line. `main` accepts rebase merges only.
- Auto-merge enabled with the squash method stayed stuck once the queue merged by rebase. Disable it and enable it
  again with `--rebase`.
- GitHub refuses auto-merge and `enqueuePullRequest` for stacked pull requests. Enqueue the top of the stack with
  `PUT /repos/{owner}/{repo}/pulls/{n}/merge-async`, without merge parameters: the queue sets the method.
- An admin bypass merge of a stacked pull request merges it into its base branch, not into `main`. Merge a stack
  bottom up.
- The asynchronous merge API defaults to a merge commit. With the ruleset bypass, pass `merge_method=rebase`.

## Commits and pull requests

- A follow-up commit subject was 73 characters and `pr-meta` rejected it. Keep subjects at 72 or fewer, and fold
  follow-ups into their topic commit before review.
- Two agents wrote pull request bodies into one shared scratch file and mixed them up. Each agent writes to its own
  directory.
- A rule change that contradicted an accepted ADR was flagged in review. Add a superseding or amending ADR in the
  same pull request, or leave the rule alone.
- `AGENTS.md` changed while a pull request was open (ratchet ownership, ADR 0006), and an agent followed the old
  rule. Re-read `AGENTS.md` on `origin/main` after every rebase.

## Tests and CI

- The disarm simulator preserved DONE through the stop pulse, hiding a hardware assumption in ring replay. Exercise
  alternate register behavior in host tests and keep the default conservative until the unit confirms the behavior.
- Disarm trace assertions counted the new samples as poll iterations and assumed the preceding batch's record count
  was the successor's arm size. Check the poll and samples separately and use the actual programmed slots.
- Concurrent golden runs in one build profile removed each other's scratch directories. Run the full golden
  runner once at a time for each profile and target directory.
- Even vblank sequences ran the UI at 48.7 Hz on the DHO900 timing. Derive the flip stride from the mode timing
  and test it with the actual totals as well as a 60 Hz mode.
- The test-time guard flagged a simulated rate window compared to a clock-derived instant. Use the injected
  vblank sequence to delimit the window.
- Sealing a picture could stage a sixth deposit even though compose tracks five inputs. Check queue capacity
  before taking the pending slot.
- A panel timeout left a stale DRM cookie, which cut short the next flip's EBUSY grace period. Read pending
  events before checking the timeout and discard the cookie before retrying an expired flip.
- Removing the pacing timestamp helper left a Linux-only DRM re-export unused. Run the target Clippy check after
  removing a platform-specific consumer.
- Extracting the update watcher missed the kexec shutdown caller of its PID helper. Search all callers and check
  all targets for the scope before accepting a host-only pass.

- A cancellation test counted reads on its source thread's counters and losses on the pipe. Use the pipe's
  injected instance for both sides of the conservation check, even when dispatch runs on another thread.
- Rejecting a serial STOP read after Clear also skipped the final acknowledgement. Check completion at the
  end of the stale DMA handoff, before the next held poll can acknowledge it.

- A zero-allocation check assumed eight warm-up passes set up the reply channel's waiter storage. With replies
  already queued, `recv` skipped that setup, and the first waiting receive allocated inside the measured pass.
  Count allocations inside `recv` separately from the pass under test.

- A phosphor clear test seeded visible counts through the raw map and skipped touched-row tracking. Use
  `edit_channel` or `edit_wide_channel` to seed counts that will be cleared.
- A reset fixture filled an old intake capacity while deliberately holding its consumer. When the hand-off shrinks,
  queue only its supported capacity before triggering reset.
- The intake's wake message could lose a retry when a batch occupied the channel. Keep the wake pending when the
  channel is full and check it before the coordinator blocks again.

- `/bin/kill -KILL -<pgid>` from procps-ng 4.0.4 parses only the first digit of the group id. A group id starting
  with 1 became `kill(-1)` and killed the CI runner. Put `--` before a negative process id.
- The soak test compared process-wide live bytes while the test harness was still allocating. Measure the calling
  thread's own allocations for synchronous work. Even a child running one test shares its heap with the harness.
- A socket test assumed the peer's close was readable on the next poll. On a loaded macOS host it was not. Wait on
  the event (a blocking `peek` with a hang guard), never on timing.
- The nightly image build fetched a patch from a host that answered HTTP 418 to CI. Fetch build inputs from a
  mirror that CI can reach.
- A wait loop watched only the CI checks, leaving a Codex finding unseen for an hour. Watch the `codex-review`
  status together with the checks.
- Runners started from a login bash got the system `python3` (3.9) and no `cargo`, so the fast gate failed. Scripts
  that run the gate set `PATH` explicitly.
- Runners were restarted while busy because a shell check compared a newline-separated list. Check each worker's
  own job file, never a joined list.
