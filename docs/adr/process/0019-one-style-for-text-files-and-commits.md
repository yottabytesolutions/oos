# ADR 0019: Style rules that need review

- Status: accepted
- Date: 2026-11-17
- Category: process

## Context

Tools enforce mechanical style. Review covers meaning and clarity.

## Decision

- Use English, plain words and short sentences. Names describe their purpose; comments explain why or what a
  hardware value means. Hardware statements say how they are known (ADR 0015).
- Name tests for the behaviour they hold, so a failure reads as the rule that broke.
- Wrap prose at 120 columns; tables, links and code blocks may be longer. Write units with a space and SI prefixes
  (`1.25 GHz`, `100 ns/div`, `4 MiB`), menu paths as `Display > Bottom bar`, and code and commands in backticks.
- Update a document with the code it describes. A change visible to a scope user gets a `CHANGELOG.md` entry
  describing its effect.
- Commit summaries use the imperative mood. The body explains why and gives test evidence when it is not obvious.
  Keep one topic per commit; formatting and moves have their own commits.

## Consequences

File rules live in [.editorconfig](../../../.editorconfig) and the xtask `text` step
(`xtask/src/text.rs`). Commit and pull request checks live in
[tools/ci/pr_meta.py](../../../tools/ci/pr_meta.py). Those files are the source for mechanically checked rules;
this record keeps the rules that need judgment.
