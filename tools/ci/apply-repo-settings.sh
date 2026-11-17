#!/bin/sh
# SPDX-License-Identifier: GPL-2.0-or-later
# Repository settings, Actions policy, labels and the main ruleset for the public repository, as files in Git.
# Usage: tools/ci/apply-repo-settings.sh [--apply] [OWNER/NAME]
# Without --apply it prints the calls. Needs `gh` logged in as a repository admin. Idempotent: run it again after
# any change to this script, .github/labels.yml or .github/rulesets/main.json.
set -eu
apply=false
if [ "${1:-}" = --apply ]; then apply=true; shift; fi
repo=${1:-yottabytesolutions/oos}
root=$(cd "$(dirname "$0")/../.." && pwd)

run() {
	printf '+ %s\n' "$*"
	if [ "$apply" = true ]; then "$@"; fi
}

# Merge: squash and rebase only (linear history), squash subject = PR title, body = commit messages (keeps the
# Signed-off-by trailers), branches deleted after merge, auto-merge allowed, DCO sign-off for web commits.
run gh api -X PATCH "repos/$repo" \
	-F allow_squash_merge=true -F allow_rebase_merge=true -F allow_merge_commit=false \
	-f squash_merge_commit_title=PR_TITLE -f squash_merge_commit_message=COMMIT_MESSAGES \
	-F delete_branch_on_merge=true -F allow_auto_merge=true -F allow_update_branch=true \
	-F web_commit_signoff_required=true

# Actions: GitHub-owned actions plus the pinned third-party ones in .github/workflows, full-SHA pins required.
# Tools (cargo-deny, git-cliff, cargo-llvm-cov) come through taiki-e/install-action, not an action of their own.
run gh api -X PUT "repos/$repo/actions/permissions" -F enabled=true -f allowed_actions=selected \
	-F sha_pinning_required=true
run gh api -X PUT "repos/$repo/actions/permissions/selected-actions" --input - <<'JSON'
{"github_owned_allowed": true, "verified_allowed": false,
 "patterns_allowed": ["Swatinem/rust-cache@*", "taiki-e/install-action@*", "ossf/scorecard-action@*"]}
JSON
# The workflow token is read-only unless a job asks for more, and it never approves PRs.
run gh api -X PUT "repos/$repo/actions/permissions/workflow" \
	-f default_workflow_permissions=read -F can_approve_pull_request_reviews=false
# Fork PR workflows wait for maintainer approval for every outside contributor.
run gh api -X PUT "repos/$repo/actions/permissions/fork-pr-contributor-approval" \
	-f approval_policy=all_external_contributors

# Security: Dependabot alerts and private vulnerability reporting.
run gh api -X PUT "repos/$repo/vulnerability-alerts"
run gh api -X PUT "repos/$repo/private-vulnerability-reporting"

# Labels.
if [ "$apply" = true ]; then
	python3 -I "$root/tools/ci/sync_labels.py" "$repo"
else
	python3 -I "$root/tools/ci/sync_labels.py" "$repo" --dry-run
fi

# The main ruleset: update by name, or create it.
id=$(gh api "repos/$repo/rulesets" --jq '.[] | select(.name == "main" and .target == "branch") | .id')
if [ -n "$id" ]; then
	run gh api -X PUT "repos/$repo/rulesets/$id" --input "$root/.github/rulesets/main.json"
else
	run gh api -X POST "repos/$repo/rulesets" --input "$root/.github/rulesets/main.json"
fi

# No API switches off Copilot's automatic code review for a repository; it is enabled only through a
# `copilot_code_review` ruleset rule or a personal setting. Codex is the review bot here.
copilot=$(gh api "repos/$repo/rules/branches/main" --jq '[.[] | select(.type == "copilot_code_review")] | length')
if [ "$copilot" != 0 ]; then
	echo "warning: a ruleset requests Copilot review on main; remove that rule (Settings > Rules)" >&2
fi
echo "Manual: chatgpt.com/codex settings > Code review: enable automatic reviews for $repo."
