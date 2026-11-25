---
id: 97
title: Lint and review idiomatic Rust
kind: chore
status: blocked
release: none
owner: coordinator
depends: []
---

## Goal

Deliver the owner-approved issue #198 brief without changing firmware behavior.
The brief supplies the claim on branch `ci/198-rust-style`.

## Plan

Reserved paths: workspace member Cargo.toml files, clippy.toml, xtask/src/ naming and ratchet integration,
xtask/data/ lint and naming baselines, docs/architecture/glossary.md, docs/process/rust-style.md,
docs/development.md, docs/adr/0006-format-lints-and-ratchets.md and AGENTS.md review guidelines.

1. Measure all pedantic and selected idiom lints before choosing enforce, ratchet or allow.
2. Inherit workspace lints, initialize only newly enabled rules, preserve existing ratchet limits.
3. Reuse the source walk for a glossary-backed naming ratchet with deterministic regression tests.
4. Document reviewed rules, run the fast gate and crate tests, hand off for independent review and full gate.

## Status

- 2026-11-25: PR #210 Clippy required-check investigation reserves this task only.
  The prepared checkout passes `cargo xtask step clippy`, exit 0:
  `partial gate passed (not done): 1 steps in 55 s`, including host and ARM targets.
  `cargo xtask check --fast` exits 0: `partial gate passed (not done): 13 steps in 9 s`.
  No source or ratchet changes; production LOC delta 0. No crates touched, so crate tests were not rerun.
  Host only. Build-only network access prevents retrieving the linked CI log.
  Coordinator must provide the failing Clippy diagnostic or rerun CI on the prepared checkout;
  the reported CI failure remains unverified.

- 2026-11-25: PR #210 required-check fix reserves `xtask/src/steps/python/tests.rs`,
  `xtask/data/xtask/clippy.txt` and this task. Reproduced Clippy exiting 1:
  `gate FAILED in 55 s: clippy`, with `default_trait_access` growing from one to two findings
  in the Python runner test. Named `Context::default()` explicitly in both calls and regenerated
  the ratchet with `cargo xtask ratchet`; only the affected entry changed.
  Clippy now exits 0: `partial gate passed (not done): 1 steps in 4 s`.
  Fast gate exits 0: `partial gate passed (not done): 13 steps in 9 s`.
  All xtask tests pass in debug and test-release: 329 passed, one ignored in each profile.
  Production LOC delta 0; host only, no behavior changes. Build-only network access prevents reading
  the linked CI logs, and the brief supplies no Codex findings or thread ids. Coordinator must supply
  that evidence or rerun review; the Codex review failure remains unverified.

- 2026-11-24: PR #210 required-check investigation reproduced the debug test job locally with
  `cargo xtask step tests-debug`: exit 0, 3836 passed, 0 failed, 97 ignored across all six crates;
  `partial gate passed (not done): 1 steps in 112 s`. The fast gate exits 0:
  `partial gate passed (not done): 13 steps in 20 s`. No source or ratchet changes; production LOC delta 0.
  Host only. The brief supplies no review findings or thread ids, and build-only network access prevents
  retrieving the linked CI logs. Coordinator must supply the failing test excerpt and review findings
  before a targeted fix can be identified. Task remains blocked pending that evidence.

- 2026-11-24: PR #210 required-check fix reserves the Clippy baselines, initial style inventory and this task.
  Reproduced stale entries for deleted files, then further baseline drift after the prepared rebase.
  Regenerated the initial style counts under ADR 0006 and tightened them with `cargo xtask ratchet`.
  Added the already-enabled pedantic kinds `duration_suboptimal_units`, `maybe_infinite_iter` and `match_bool`
  to the initial inventory after measuring their findings in existing source. Applied the recorded State Debug
  allowance move with `cargo xtask ratchet --move`; existing lint allowances remain protected.
  No Rust source changes; production LOC delta 0. Tooling tests pass in debug and test-release:
  329 passed, one ignored in each profile. Fast gate exits 0:
  `partial gate passed (not done): 13 steps in 9 s`. Clippy exits 0:
  `partial gate passed (not done): 1 steps in 4 s`; explicit CI base selection also exits 0:
  `partial gate passed (not done): 1 steps in 3 s`. Initial attempts exited 1 with `gate FAILED` on Clippy
  baseline drift; all reproduced findings are resolved. Host only. Coordinator owns final CI and review.

- 2026-11-18: PR #210 test-context fix covers `xtask/src/naming/`, `xtask/src/size/visit.rs`,
  `xtask/src/sources.rs` and this task. The regression failed before the fix on a `#[cfg(test)]` statement block.
  Both gates now share the traversal of cfg-bearing containers. The regression confirms that declarations in
  test-only code are skipped and their production equivalents still trigger findings. All 257 xtask tests pass
  in debug and test-release. The fast gate exits 0 with all 13 steps passed. Clippy counts in the changed Rust
  files match the ratchet. Firmware production LOC delta 0, Rust tooling -28. Host only.

- 2026-11-18: PR #210 required-check fix covers `xtask/src/naming/`, `xtask/data/ui/clippy.txt` and this task.
  The regression test rejected `batches` and `queries` before glossary lookup learned their plural forms.
  Regenerated the initial Clippy baseline for 11 UI descriptor files already at the protected base with
  `cargo xtask ratchet --init-style` and `cargo xtask ratchet`. No new lint findings in the naming code.
  macOS fast gate exits 0 with all 13 steps passed. Clippy and all 65 register
  plans exit 0: `partial gate passed (not done): 2 steps in 7 s`. UI and xtask pass debug (1987 passed,
  59 ignored) and test-release (1986 passed, 59 ignored). Firmware production LOC delta 0, Rust tooling +9.
  Linux x86-64 passes all 65 register plans and compiles all Clippy targets. Its gate exits 1:
  `gate FAILED in 391 s: clippy`. The unused counts are in `hal/src/system.rs`,
  `hal/src/update/card/writer.rs`, `ui/src/main.rs` and `fpgad/src/bin/oos-update.rs`.
  Those files differ from the older local `main` and match the cached PR base `origin/main`.
  Host only. Fetch was blocked by read-only Git metadata. The cached branch matches the supplied checkout.
  Coordinator owns the CI rerun and resolution of the review thread.

- 2026-11-18: PR #210 merge-compatibility fix reserves `xtask/data/hal/clippy.txt`,
  `xtask/data/ui/clippy.txt`, `xtask/data/ui/naming.txt` and this task. The supplied checkout already has
  cached `origin/main` as its direct parent, with no unresolved conflicts. Fetch was blocked by read-only Git
  metadata; the coordinator must verify fresh remote history. Initial fast and Clippy checks failed on stale
  baselines after the mDNS and descriptor changes on main. Regenerated with `cargo xtask ratchet` and
  `cargo xtask ratchet --init-style`, using the initial-baseline transition authorized by ADR 0006.
  Final fast exits 0: `partial gate passed (not done): 13 steps in 20 s`.
  Final Clippy exits 0: `partial gate passed (not done): 1 steps in 10 s`.
  All six crates pass debug (2885 passed, 73 ignored) and test-release (2884 passed, 73 ignored):
  `partial gate passed (not done): 2 steps in 163 s`. Production LOC delta 0; no Rust source changes.
  Host only. Coordinator owns final remote verification, review and integration.

- 2026-11-18: PR #210 trait-member traversal fix reserves `xtask/src/naming/mod.rs`,
  `xtask/src/naming/tests.rs` and this task. Reproduced missed associated-constant declarations with a failing test,
  then traversed all implementation members while exempting only mandated identifiers. Regression tests cover
  constants, associated types, method signatures, nested inherent members and test-only exclusions.
  All 255 xtask tests pass in debug and test-release. Focused Clippy exits 0 with no diagnostics in changed Rust
  files. Fast gate exits 0: `partial gate passed (not done): 13 steps in 15 s`; no lowered ratchet counts.
  Firmware production LOC delta 0; Rust tooling +9. Host only. Coordinator owns final CI, review and integration.

- 2026-11-18: PR #210 review fixes reserve `hal/src/lib.rs`, `dsp/src/lib.rs`,
  `docs/architecture/glossary.md` and this task on `ci/198-rust-style`.
  Gave each retained Debug warning a crate-specific reason and removed the duplicate FFT abbreviation.
  The checkout already contains xtask's original deny attributes in `xtask/src/main.rs` and its
  `xtask/data/xtask/clippy.txt` baseline. Compiler probes using those attributes reject missing Debug,
  unused code and a Clippy `all` violation; no further lint-policy change is needed for P1.
  Fast gate exits 0: `partial gate passed (not done): 13 steps in 17 s`.
  HAL, DSP and xtask tests pass in debug and test-release: 1086 passed, 14 ignored in each profile,
  including all 252 xtask tests. Production LOC delta 0. Host only; no firmware behavior changes.
  Clippy gate exits 0: `partial gate passed (not done): 1 steps in 141 s`; no lowered ratchet counts.
  The recorded full-gate failures are known host-test blockers outside this PR's changed source:
  the process-group coverage failure is also tracked in task 0096, and the unchanged intake/change
  performance guard exceeded its budget. The task remains blocked; coordinator owns resolving those
  failures and obtaining a passing final gate before merge. The documented policy and baselines describe
  the implementation, not completed acceptance.

- 2026-11-18: PR #210 Python-check fix reserves `tools/ci/changes.py` and `tools/tests/test_changes.py`.
  Reproduced the embedded-input regression: naming tests include the glossary, but documentation-only changes
  skipped Rust jobs. Added the glossary to the existing input exceptions and covered glossary-only selection.
  Focused path-filter tests pass (14). CI-mode `script-syntax` and `python-tests` exit 0:
  `partial gate passed (not done): 2 steps in 76 s`; discovery reports 288/70/9 tests, with two documented
  Rust-build skips. Fast gate exits 0: `partial gate passed (not done): 13 steps in 15 s`.
  Firmware production LOC delta 0; Python tooling +1. Host only. Coordinator owns review and CI rerun.

- 2026-11-18: Implementation handed off; coordinator owns independent review and resolution of the unchanged
  host-test blockers before acceptance. Final full gate exited 1:
  `gate FAILED in 708 s: perf, coverage`. Clippy, naming, all other static checks, all 65 register plans,
  decoder configurations, Python tests, debug (2833 passed, 73 ignored) and release (2832 passed, 73 ignored)
  passed. Both profiles include all 252 xtask tests.
  Coverage's UI run failed only `support::group_kill_reaches_grandchildren_and_nothing_else` after 120 seconds;
  the same helper failure is recorded in task 0096. The final performance run passed 24/25 UI guards; unchanged
  `perf_guard_intake_change_shadow_budget` measured intake/change p99 799585 ns against 500000 ns.
  Neither failing source file was edited. No additional retries, runtime fixes or relaxed limits were made.
  Final fast gate exited 0: `partial gate passed (not done): 13 steps in 18 s`; the shared naming step finished
  at 5.6 s versus 5.3 s for the other shared-walk checks. LOC: production 109374 (+2 lint attributes),
  Rust tooling 5627 (+324). No hardware access. All remaining working-tree changes belong to issue #198.
  Evidence: `target/style-evidence/full-gate-final.log`, `fast-handoff.log`, `loc-handoff.log` and the complete
  command logs named by each gate. The first full attempt is preserved as `full-gate.log`:
  `gate FAILED in 1310 s: naming, clippy, plan-matrix, perf`; its first-baseline/receipt failures were fixed,
  and its performance run overlapped tooling compilation. The final full run had no concurrent builds.

- 2026-11-18: Implemented inherited workspace lints, an explicitly limited first style baseline, the shared-walk
  naming ratchet, glossary, short Rust style guide and P2 reviewer guidelines. All new naming/baseline source files
  meet the policy without allowances. Generated baselines with `cargo xtask ratchet --init-style` and tightened
  with `cargo xtask ratchet`; existing lint counts were not raised. Firmware changes are only two retained Debug
  lint attributes: production LOC +2; tooling LOC +323 at the first LOC measurement.
  Stable fast gate exited 0: `partial gate passed (not done): 13 steps in 10 s` while the full run was active.
  The first full run passed debug (2833 tests), release (2832 tests), coverage in all six crates, Python tests,
  decoder configurations and performance builds. Its naming and Clippy results preceded the completed baselines;
  plan-matrix refused a source receipt because lint configuration changed during its build. A stable full rerun
  is pending. Evidence: `target/style-evidence/`, including before-policy host/ARM JSON, per-lint counts,
  baseline generation, focused tests and complete gate logs. No unit use or firmware behavior changes.

- 2026-11-18: Accepted owner brief; read repository rules and measured host Clippy with pedantic and the selected
  restriction/nursery lints. No firmware changes or hardware access planned. Coordinator owns Git writes,
  publication, independent cross-vendor review and integration.

## Lint decisions

Before-policy census: pinned toolchain, all targets on the host and ARM for the five firmware crates;
locations deduplicated by manifest, file, line, column and lint exactly as the gate does.
`module_name_repetitions` was measured explicitly after verifying that the pinned compiler now places it in
restriction rather than pedantic; its 29 findings are in existing declarations.
The census includes test code with the existing unwrap/expect test exceptions. Counts measure the candidate
rules before fixes or policy exceptions, rather than a later run with numeric warnings suppressed.
Zero-count rules in `all` and `pedantic` are enforced immediately. Nonzero rules are ratcheted unless listed
as allowed with a reason in Cargo.toml. Even small counts remain ratcheted when removing them would require
unrelated API or firmware changes; this job changes enforcement, not application behavior.

| Lint | Initial findings | Decision |
|---|---:|---|
| `allow_attributes_without_reason` | 44 | ratchet |
| `approx_constant` | 2 | ratchet |
| `assert_is_empty` | 207 | ratchet |
| `assigning_clones` | 19 | ratchet |
| `bool_assert_comparison` | 1 | ratchet |
| `bool_to_int_with_if` | 22 | ratchet |
| `borrow_as_ptr` | 45 | ratchet |
| `borrow_deref_ref` | 1 | ratchet |
| `case_sensitive_file_extension_comparisons` | 21 | ratchet |
| `cast_lossless` | 2061 | allow (reason in Cargo.toml) |
| `cast_possible_truncation` | 2040 | allow (reason in Cargo.toml) |
| `cast_possible_wrap` | 523 | allow (reason in Cargo.toml) |
| `cast_precision_loss` | 1626 | allow (reason in Cargo.toml) |
| `cast_ptr_alignment` | 9 | ratchet |
| `cast_sign_loss` | 845 | allow (reason in Cargo.toml) |
| `chunks_exact_to_as_chunks` | 24 | ratchet |
| `clone_on_copy` | 56 | ratchet |
| `cloned_instead_of_copied` | 3 | ratchet |
| `cloned_ref_to_slice_refs` | 1 | ratchet |
| `cognitive_complexity` | 34 | ratchet |
| `collapsible_if` | 1 | ratchet |
| `comparison_chain` | 2 | ratchet |
| `decimal_bitwise_operands` | 17 | ratchet |
| `default_trait_access` | 349 | ratchet |
| `derivable_impls` | 1 | ratchet |
| `doc_markdown` | 420 | ratchet |
| `double_ended_iterator_last` | 5 | ratchet |
| `elidable_lifetime_names` | 2 | ratchet |
| `empty_line_after_doc_comments` | 2 | ratchet |
| `enum_glob_use` | 54 | ratchet |
| `enum_variant_names` | 0 | enforce |
| `expect_used` | 196 | ratchet |
| `explicit_counter_loop` | 1 | ratchet |
| `explicit_iter_loop` | 27 | ratchet |
| `field_reassign_with_default` | 42 | ratchet |
| `filter_map_next` | 1 | ratchet |
| `float_cmp` | 105 | allow (reason in Cargo.toml) |
| `fn_params_excessive_bools` | 2 | ratchet |
| `format_collect` | 13 | ratchet |
| `format_in_format_args` | 1 | ratchet |
| `format_push_string` | 93 | ratchet |
| `identity_op` | 2 | ratchet |
| `if_not_else` | 17 | ratchet |
| `if_same_then_else` | 2 | ratchet |
| `ignored_unit_patterns` | 18 | ratchet |
| `implicit_clone` | 2 | ratchet |
| `inconsistent_struct_constructor` | 1 | ratchet |
| `inherent_to_string` | 1 | ratchet |
| `inline_always` | 2 | ratchet |
| `items_after_statements` | 54 | ratchet |
| `items_after_test_module` | 3 | ratchet |
| `large_futures` | 5 | ratchet |
| `large_stack_arrays` | 5 | ratchet |
| `large_types_passed_by_value` | 1 | ratchet |
| `manual_assert` | 2 | ratchet |
| `manual_assert_eq` | 12 | ratchet |
| `manual_checked_ops` | 1 | ratchet |
| `manual_clamp` | 1 | ratchet |
| `manual_contains` | 3 | ratchet |
| `manual_div_ceil` | 5 | ratchet |
| `manual_is_multiple_of` | 27 | ratchet |
| `manual_is_power_of_two` | 1 | ratchet |
| `manual_is_variant_and` | 1 | ratchet |
| `manual_let_else` | 1 | ratchet |
| `manual_midpoint` | 89 | ratchet |
| `manual_range_contains` | 8 | ratchet |
| `manual_repeat_n` | 2 | ratchet |
| `manual_string_new` | 25 | ratchet |
| `many_single_char_names` | 390 | ratchet |
| `map_unwrap_or` | 92 | ratchet |
| `match_same_arms` | 34 | ratchet |
| `match_wildcard_for_single_variants` | 15 | ratchet |
| `missing_const_for_fn` | 521 | allow (reason in Cargo.toml) |
| `missing_errors_doc` | 200 | ratchet |
| `missing_fields_in_debug` | 1 | ratchet |
| `missing_panics_doc` | 33 | ratchet |
| `module_name_repetitions` | 29 | ratchet |
| `must_use_candidate` | 929 | allow (reason in Cargo.toml) |
| `needless_bitwise_bool` | 1 | ratchet |
| `needless_borrow` | 3 | ratchet |
| `needless_borrows_for_generic_args` | 1 | ratchet |
| `needless_continue` | 4 | ratchet |
| `needless_for_each` | 23 | ratchet |
| `needless_late_init` | 1 | ratchet |
| `needless_lifetimes` | 1 | ratchet |
| `needless_option_as_deref` | 1 | ratchet |
| `needless_pass_by_value` | 30 | ratchet |
| `needless_range_loop` | 20 | ratchet |
| `needless_raw_string_hashes` | 1 | ratchet |
| `neg_cmp_op_on_partial_ord` | 87 | ratchet |
| `no_effect_underscore_binding` | 1 | ratchet |
| `nonminimal_bool` | 5 | ratchet |
| `option_if_let_else` | 83 | ratchet |
| `option_option` | 46 | ratchet |
| `precedence` | 3 | ratchet |
| `ptr_arg` | 1 | ratchet |
| `ptr_as_ptr` | 39 | ratchet |
| `question_mark` | 9 | ratchet |
| `range_plus_one` | 5 | ratchet |
| `redundant_closure` | 4 | ratchet |
| `redundant_closure_for_method_calls` | 157 | ratchet |
| `redundant_else` | 3 | ratchet |
| `ref_as_ptr` | 5 | ratchet |
| `ref_option` | 2 | ratchet |
| `return_self_not_must_use` | 11 | allow (reason in Cargo.toml) |
| `semicolon_if_nothing_returned` | 81 | ratchet |
| `similar_names` | 32 | ratchet |
| `single_char_pattern` | 2 | ratchet |
| `single_match` | 1 | ratchet |
| `single_match_else` | 66 | ratchet |
| `stable_sort_primitive` | 7 | ratchet |
| `struct_excessive_bools` | 22 | ratchet |
| `struct_field_names` | 8 | ratchet |
| `too_many_arguments` | 20 | ratchet |
| `too_many_lines` | 219 | ratchet |
| `trivially_copy_pass_by_ref` | 10 | ratchet |
| `type_complexity` | 13 | ratchet |
| `unchecked_time_subtraction` | 9 | ratchet |
| `undocumented_unsafe_blocks` | 62 | ratchet |
| `uninlined_format_args` | 1728 | ratchet |
| `unnecessary_box_returns` | 1 | ratchet |
| `unnecessary_cast` | 8 | ratchet |
| `unnecessary_lazy_evaluations` | 3 | ratchet |
| `unnecessary_map_or` | 14 | ratchet |
| `unnecessary_semicolon` | 2 | ratchet |
| `unnecessary_unwrap` | 3 | ratchet |
| `unnecessary_wraps` | 37 | ratchet |
| `unnested_or_patterns` | 16 | ratchet |
| `unreadable_literal` | 345 | allow (reason in Cargo.toml) |
| `unused_self` | 26 | ratchet |
| `unwrap_used` | 161 | ratchet |
| `use_self` | 964 | ratchet |
| `used_underscore_binding` | 3 | ratchet |
| `verbose_bit_mask` | 4 | ratchet |
| `wildcard_imports` | 254 | ratchet |
| `missing_debug_implementations` | 38 | ratchet |
