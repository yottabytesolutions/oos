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

## Tests and CI

- `/bin/kill -KILL -<pgid>` from procps-ng 4.0.4 parses only the first digit of the group id. A group id starting
  with 1 became `kill(-1)` and killed the CI runner. Put `--` before a negative process id.
- The soak test compared process-wide live bytes while the test harness was still allocating. Measure the calling
  thread's own allocations only.
- A socket test assumed the peer's close was readable on the next poll. On a loaded macOS host it was not. Wait on
  the event (a blocking `peek` with a hang guard), never on timing.
- The nightly image build fetched a patch from a host that answered HTTP 418 to CI. Fetch build inputs from a
  mirror that CI can reach.
