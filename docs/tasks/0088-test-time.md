---
id: 88
title: Tests independent of machine speed
kind: fix
status: active
release: 0.2.0
owner: codex
depends: []
---

## Goal

Tests must assert behavior independently of host speed, including coverage instrumentation on loaded hosts.
No commits or hardware access in this task.

## Plan

1. Establish ADR 0019 and the test-time rule authorized by the owner's task instructions.
2. Convert failed SIM, measurement and pair waits to completion events. Audit all workspace test timing,
   replacing scheduling assumptions with events and elapsed-time behavior with explicit time inputs.
3. Add a source check with a nonincreasing ratchet and regression fixtures.
4. Run debug/release tests, exact Clippy, fast gate and UI stress tests with CPU load and coverage.
5. Record the converted tests, safe audit exceptions and complete validation evidence; prepare the commit message.

Reserved paths: `AGENTS.md`, `docs/tasks/0088-test-time.md`, `docs/architecture/adr/0019-test-time.md`,
`docs/architecture/adr/README.md`, `docs/architecture/README.md`, `docs/development.md`, `ui/src/` (including `ui/src/ui.rs` and `ui/src/board/dho/hw.rs`), `hal/src/`,
`dsp/src/`, `fpgad/src/`, `hal/tests/`, `ui/tests/`, `xtask/src/`, `xtask/data/ui/test-time.txt`, `xtask/data/moves.txt`, `tools/perf-guard.sh`.

### Round 2 review repair plan

Apply all review findings on the existing claim. Share the shipped query and receiver paths, restore deterministic
negative assertions, extend and store the timing ratchet, move duration budgets into perf-guard, and rerun the
requested validation. Keep the task active for independent review and unrestricted socket validation.

### Failing-log tests converted

All seven distinct tests named by `target/full1.log` and `target/full3.log` use event waits now:

- `ui::batch2_tests::eye_on_a_stopped_sim_ref_computes_instead_of_waiting_for_edges`
- `ui::analysis::channel_measurement_tests::large_screen_pan_rejects_old_gate_and_preserves_native_timebase`
- `ui::analysis::measurement_gate_tests::stale_large_results_cannot_replace_a_new_source_or_gate`
- `ui::analysis::own::tests::large_distinct_row_levels_share_one_worker_without_cache_thrash`
- `ui::analysis::own::tests::pending_row_rejects_changed_gate_levels_and_replaced_reference`
- `ui::measure_pairs::tests::pair_result_rejects_stale_setup_hold_settings_and_source_reload`
- `ui::measure_pairs::tests::setup_and_hold_take_the_data_from_a_and_the_clock_from_b`

### Workspace audit

The audit covers Cargo targets and test modules in ui, hal, dsp, core, fpgad and xtask, including integration
harnesses and platform-gated sources. No ordinary test source retains sleeps or clock-comparison deadline loops
outside the named reviewed helpers and ignored performance probes. Tests of elapsed behavior supply time; worker waits consume a completion
event or observe a worker-updated completion counter. Ordinary tests never assert real durations. Performance ratio
assertions remain in explicitly ignored `OOS_PERF_TESTS` benchmarks, outside ordinary debug/release/coverage tests.

| Area | Conversion or safe disposition |
|---|---|
| SIM, auto Eye, measurement, own rows, pairs, catalog, math, demo, decode/search, trace Bus | Await a submitted output receiver, retain its result, then run the ordinary poll. The shared helper names the missing worker completion. |
| Deposit, publication, loader, networking, generator | Wait for the actual completion counter, queued program, socket reply, received command or returned buffer. Socket reply helpers use a named 120-second hang guard. |
| HAL trigger/ring fill | Real/manual Clock at the acquisition execution boundary. The four timing-sensitive tests advance manual time. Production wrappers use the same real clock and waits. |
| Diagnostic intake/locks/frame costs | Explicit timestamps or clock samples. Exact synthetic duration accounting replaces real-duration inequalities. Real wiring tests retain ownership and conservation assertions. |
| Shutdown and GPU helper | Verify deadlines relative to one supplied origin; use an already-expired deadline with a deliberately held worker/helper for cancellation. Successful completion waits use generous guards. |
| Update flock and shell supervisors/ledger writers | Probe lock exclusion directly; wait for fixture-published markers/acknowledgements and process exit. Record requested backoff delays instead of measuring them. |
| HTTP deadline and long poll | Inject deadline samples and advance long-poll time after the pending-request event. Positive long polls cannot expire on the host clock. Fragmentation no longer sleeps between writes. |
| Existing manual clocks | HAL watchdog Port fake, pacing/guard tests, supplied UI date clock, startup persistence and other Duration/Instant inputs already advance explicit values. Safe and retained. |
| DSP and HAL/UI timing reports | Explicitly ignored benchmarks only sample/print durations or run performance ratio guards. They do not run in ordinary tests. |
| core | Pure domain code; no wall-clock waits or real-duration assertions found. |
| fpgad and xtask | Remaining clocks/sleeps are production command execution/reporting. Test strings containing forbidden code are parser fixtures, not executed waits. |
| Production IO/pacing | Hardware polling, service retry, frame pacing, GPU process handling and protocol timeout logic are operational code. They are not test waits. No unit access was performed. |

Three allowlisted helper modules own hang handling: `ui/src/ui/lifecycle/test_wait.rs`,
`hal/tests/support/mod.rs` and `ui/tests/support/mod.rs`. The last retains owned child/group kill-and-reap;
ordinary limits are at least 120 seconds. Its zero-limit self-test deliberately starts with an expired deadline
to exercise cancellation, without a duration assertion.

The initial nonincreasing ratchet contains exactly ten occurrences in four ignored manual tools:
`http::tests::web_demo` and `http::tests::scpi_validation_fixture` serve for a selected observation interval;
`mdns::tests::host_responder` observes an explicitly requested network; `ui::wlan::tests::live_radio` is an
explicit physical-radio integration exercise. These are excluded from host gates. They were not executed.
No existing ratchet file was changed. The guard is syntactic; arbitrary generated macros or clock aliases
through unrelated helpers still require review. Windows-only loader/network paths were source audited;
this host does not execute them.

### Changed test bodies and shared helpers

The following inventory includes timer removals, event conversions, generous shutdown/socket guards and fixture
adaptations to the buffered test receivers. It also names manual probes changed to report rather than assert time.
Additional tests listed above inherit the converted shared measurement helpers. New regressions include
`awaiting_completion_preserves_the_result_for_the_real_poll`, `shot_expiry_clock_is_sampled_under_state_lock`,
the five `xtask::test_time` fixtures and `test_context_follows_external_compound_cfg_modules`.

- `hal/src/update/tests.rs`: `mode_and_lock`.
- `hal/tests/early_trigger_end.rs`: `the_trigger_wait_ends_for_new_settings`, `without_new_settings_the_wait_runs_to_its_end`.
- `hal/tests/idle_during_ring_fill.rs`: `idle_runs_while_the_ring_fills_and_never_after`, `without_work_the_wait_is_unchanged`.
- `hal/tests/rootfs_selfheal.rs`: `the_watchdog_wrapper_restarts_an_exiting_kicker_with_growing_delays`.
- `hal/tests/rootfs_watchdog.rs`: `handoff_reaps_without_magic_close_and_resumes_after_each_failed_jump`, `usr1_during_backoff_acknowledges_before_the_sleep_finishes`, `kexec_failure_during_backoff_resumes_without_waiting_for_the_sleep`.
- `hal/tests/safe_start/lock.rs`: `live_matching_identity_is_protected_until_released`.
- `hal/tests/safe_start/round2.rs`: `concurrent_writers_wait_for_commit_and_reclaim_killed_writer_lock`.
- `ui/src/announce.rs`: `a_query_to_the_socket_is_answered`.
- `ui/src/board/dho/hw.rs`: `deep_record_gives_vavg_from_the_raw_window`.
- `ui/src/board/dho/publication.rs`: `publication_releases_shared_before_waiting_to_record_copy`.
- `ui/src/desktop/lan.rs`: `desktop_network_services_share_the_window_session`.
- `ui/src/desktop/picker.rs`: `background_reader_commits_only_the_latest_choice_in_the_same_workflow`.
- `ui/src/drawpipe.rs`: `shutdown_leaves_a_busy_deposit_workers_buffers_owned`, `partial_merges_and_picture_age_are_counted`.
- `ui/src/drawpipe/shutdown.rs`: `closing_idle_pipe_joins_workers_and_refuses_intake`.
- `ui/src/framecost.rs`: `parts_add_to_their_own_sums`.
- `ui/src/gpu/process.rs`: `uncooperative_helper_is_killed_within_bound`.
- `ui/src/hislip.rs`: `a_socket_round_trip_answers_idn`.
- `ui/src/http.rs`: `expect_continue_is_answered_and_refusals_come_first`, `a_body_without_content_length_gets_411`, `every_limit_answers`, `too_many_connections_get_503`, `web_demo`, `scpi_validation_fixture`, `waveform_upload_loads_the_chosen_reference_and_preserves_it_on_error`, `waveform_upload_has_its_own_limit_and_origin_protection`, `page_lets_the_viewer_pick_png_or_bmp`, `pages_and_downloads`, `large_answers_go_through_a_thread`, `live_picture_with_the_gpu_is_a_picture_read_back`, `live_picture_once_per_screen_and_long_poll`, `update_upload_limits`, `a_large_body_at_a_time_and_net_allow_before_the_body`, `net_allow_follows_a_change_without_a_restart`, `identification_names_the_listeners`, `cmd_takes_keys_knobs_and_touch_only`.
- `ui/src/http/shutdown.rs`: `shutdown_joins_workers_and_prevents_late_spawns`, `completed_worker_closes_its_connection_before_handle_is_reaped`.
- `ui/src/mdns/lifecycle.rs`: `shutdown_wakes_network_wait_and_leaves_time_for_http`.
- `ui/src/net.rs`: `reads_without_waiting`.
- `ui/src/perfguard/deep/tests.rs`: `guard_records_both_sides_and_releases_shared_before_recording`, `blocked_mutex_records_wait_longer_than_held`, `uncontended_mutex_records_held_longer_than_wait`.
- `ui/src/perfguard/intake/tests.rs`: `external_nesting_excludes_its_polls_and_their_waits_once`.
- `ui/src/scpi.rs`: `compound_query_is_one_line_on_the_socket`, `compatibility_raw_tcp_pending_opc_does_not_block_other_clients`, `compatibility_hislip_defers_entire_multiline_payload_with_original_message_id`, `net_allow_follows_a_change_without_a_restart`, `a_query_is_answered_after_the_client_closed_its_side`, `compatibility_disconnected_raw_client_cannot_resume_trailing_writes`, `compatibility_opc_waits_for_actual_large_sim_generation_but_not_future_captures`, `compatibility_hislip_sync_and_async_clear_discard_pending_tail_without_stale_reply`, `decoder_scpi_large_reference_reports_busy_then_fresh_events_without_acquisition`, `selected_catalog_rows_are_exported_and_setup_clear_accepts_an_empty_catalog`, `raw_tcp_compound_queries_share_one_response_message`, `raw_tcp_empty_query_is_a_line_and_compound_query_keeps_its_position`.
- `ui/src/system/shutdown.rs`: `total_shutdown_deadlines_share_one_time_origin`, `overrun_continues_without_freeing_live_buffers`, `completed_workers_are_joined`, `existing_workers_stop_in_order_even_when_persistence_fails`.
- `ui/src/ui.rs`: `clear_drops_the_record_waiting_for_the_worker`, `background_measurements_follow_a_running_record`, `bode_sweep_drives_the_generator`.
- `ui/src/ui/analysis.rs`: `actual_large_reference_full_record_cache_latency_and_memory`, `large_full_measurements_are_nonblocking_and_use_one_worker_without_fake_captures`, `a_large_screen_gate_runs_in_background_and_discards_stale_pan_results`.
- `ui/src/ui/analysis/channel_measurement_tests.rs`: `native_full_basic_values_are_inline_and_do_not_start_edge_analysis`, `native_full_edge_job_is_nonblocking_shared_and_reused_after_pan`.
- `ui/src/ui/analysis/own.rs`: `large_distinct_row_levels_share_one_worker_without_cache_thrash`.
- `ui/src/ui/analysis/window/tests.rs`: `background_completion_is_displayed_after_stop_without_main_dsp`, `busy_updates_coalesce_to_the_newest_record_and_preserve_record_identity`, `a_raw_record_of_an_old_acquisition_generation_is_not_submitted`.
- `ui/src/ui/analysis/window/tests/review.rs`: `settings_changes_mark_the_accepted_result_stale_immediately`.
- `ui/src/ui/autoset.rs`: `actual_gen7_20m_reference_fit_is_bounded_and_keeps_native_records`.
- `ui/src/ui/backend_followup_tests.rs`: `actual_gen7_reference_math_zoom_and_automatic_eye_backend`.
- `ui/src/ui/batch2_tests.rs`: `eye_on_a_stopped_sim_ref_computes_instead_of_waiting_for_edges`.
- `ui/src/ui/bus.rs`: `retained_native_bus_uses_a_bounded_worker_and_reuses_the_same_view_result`.
- `ui/src/ui/bus_candidate.rs`: `real_scpi_queries_keep_pending_candidate_and_force_uses_the_live_bus_path`.
- `ui/src/ui/capture_memory.rs`: `explicit_demo_memory_is_fully_filled_off_thread_and_survives_stop`, `full_demo_conditioning_is_native_once_and_shared_by_display_math_and_measurements`, `every_single_press_rearms_and_rejects_a_queued_full_demo_from_before_the_press`.
- `ui/src/ui/change/intake.rs`: `completed_step_separates_nested_poll_external_body_and_shared_wait`.
- `ui/src/ui/eyepage.rs`: `auto_eye_background_rejects_old_source_and_does_not_recount_pan`, `auto_eye_stored_reference_stays_background_when_another_slot_is_simulated`.
- `ui/src/ui/eyepage/automatic.rs`: `native_channel_eye_uses_full_frame_and_ignores_interpolated_display_pan`, `long_reference_analysis_ignores_display_geometry_and_keeps_all_native_edges`.
- `ui/src/ui/lint.rs`: `a_remote_knob_turn_has_a_limit`.
- `ui/src/ui/mathwave.rs`: `large_reference_math_is_background_cached_and_rejects_stale_settings`, `oversized_convolution_returns_bounded_error_and_does_not_block_next_math`.
- `ui/src/ui/meascatalog/backend.rs`: `families_queue_once_share_immutable_data_and_never_count_stopped_reads`, `clear_cancels_old_accounting_even_when_capture_number_is_reused`, `processed_channel_reused_buffer_and_capture_after_clear_cannot_reuse_old_samples`.
- `ui/src/ui/measure_pairs.rs`: `large_pair_is_background_cached_in_physical_time_and_full_pan_does_not_resubmit`.
- `ui/src/ui/reference_decode.rs`: `large_decode_is_nonblocking_single_worker_and_discards_changed_source_results`.
- `ui/src/ui/remote.rs`: `a_change_of_allowed_runs_the_firewall_and_ssh`.
- `ui/src/ui/sim.rs`: `main_loop_sim_changes_samples_measurements_and_eye_only_on_accepted_captures`, `loaded_sim_uses_run_stop_and_returns_to_live_explicitly`.
- `ui/src/ui/tests/epochs.rs`: `delayed_measurements_require_gen_epoch_and_stats_gen`.
- `ui/src/ui/tools/frozen41_helpers_tests.rs`: `force_after_completed_single_arms_a_fresh_receipt_in_live_and_stored_modes`, `statistics_reset_keeps_rows_and_pending_single_but_rejects_old_deferred_counts`.
- `ui/src/ui/tools/scpi_helpers_tests.rs`: `remote_clear_cancels_deferred_statistics_before_same_row_is_readded`.
- `ui/src/ui/zoom_tests.rs`: `explicit_zoom_close_survives_pending_sim_setup_and_fresh_running_or_stopped_captures`.
- `xtask/src/agent_rules/tests.rs`: `both_gate_modes_dispatch_the_check_without_losing_g1_steps`.
- `xtask/src/inventory/tests.rs`: `default_gate_never_selects_docker_or_image_prerequisites`, `selections_preserve_the_fast_prefix_and_conditional_checks`.
- `xtask/src/sources.rs`: `test_context_follows_external_compound_cfg_modules`.

Shared helper conversions:

- `hal/tests/rootfs_watchdog.rs`: `wait_for`.
- `hal/tests/safe_start/lock.rs`: `wait`.
- `ui/src/board/dho/hw/batch/tests.rs`: `finish`.
- `ui/src/board/dho/matching/tests.rs`: `finish`.
- `ui/src/drawpipe.rs`: `one_worker_held`, `settle`, `recycled`.
- `ui/src/drawpipe/host.rs`: `finish_batch`.
- `ui/src/hislip.rs`: `read_one`.
- `ui/src/scpi.rs`: `read_until`.
- `ui/src/ui.rs`: `live_raw_at`.
- `ui/src/ui/analysis.rs`: `wait_reference_measurement_event`, `pump`.
- `ui/src/ui/analysis/channel_measurement_tests.rs`: `wait`.
- `ui/src/ui/analysis/own.rs`: `measurement_settle`, `measurement_settle_with`, `wait`.
- `ui/src/ui/batch5_tests.rs`: `sim_eye`.
- `ui/src/ui/bus.rs`: `wait_bus_event`.
- `ui/src/ui/capture_memory.rs`: `wait_demo_event`.
- `ui/src/ui/eyepage/automatic.rs`: `wait_eye_event`.
- `ui/src/ui/mathwave.rs`: `wait_math_event`.
- `ui/src/ui/meascatalog/backend.rs`: `wait_catalog_event`, `wait`.
- `ui/src/ui/measure_pairs.rs`: `wait_pair_measurement_event`, `wait`.
- `ui/src/ui/reference_decode.rs`: `wait_reference_decode_event`, `wait`.
- `ui/src/ui/reference_search.rs`: `wait_reference_search_event`, `wait`.
- `ui/src/ui/sim.rs`: `wait_sim_generation`, `wait_sim_worker`, `wait_sim_event`, `drive_until`.

### Validation evidence

All commands use the locked workspace and the pinned toolchain. Logs are worktree-local under `target/`.
The base remains `b582fa277d9822b9dbcd2e174bcce9e3a2de9c67`; no commit, push, rebase or unit access occurred.
The Rust/build/guard input fingerprint is `a21621350768c957f893811e370de50e05d331949029a0f978c50f38188769ad`
(663 files; `target/test-time-candidate.sha256`). Working-tree documentation is excluded from that fingerprint.

| Check | Result and evidence |
|---|---|
| `cargo xtask check --fast` | Exit 0: `partial gate passed (not done): 9 steps in 46 s`. All nine steps passed, including size, architecture and test-time. `target/fast-final-code.log`. |
| Exact Clippy | Exit 0. Per crate: `cargo clippy --locked --all-targets --message-format=json --quiet`, plus `--target aarch64-unknown-linux-musl` for hal, dsp, core, ui and fpgad. Deduplicated and compared with the unchanged gate ratchets: hal 176, dsp 265, core 46, ui 668, fpgad 19, xtask 0; no new/grown finding or compile error. `target/clippy-exact-final.log`, `target/clippy-results.json`, full per-run JSON/stderr logs. |
| xtask tests | Exit 0: 146 passed, 0 failed. Includes five timing-check fixtures and compound cfg propagation. `target/xtask-debug.log`. |
| HAL trigger/ring-fill tests | Debug and release: 4 passed, 0 failed, using manual time. `target/hal-clock-tests.log`, `target/workspace-release.log`. |
| Complete workspace debug | `cargo test --locked --workspace --no-fail-fast`, exit 101, 568.50 s. Only hal library and UI binary failed; all other targets and golden comparison passed. This early build contained the constant query-clock defect described below; the later loaded UI run supersedes that UI failure. `target/workspace-debug.log`. |
| Complete workspace release | `cargo test --locked --workspace --no-fail-fast --release`, exit 101, 590.20 s. HAL library: 375 passed, 1 socket-permission failure, 8 ignored. UI binary: 1295 passed, 38 socket-permission failures, 47 ignored. All other targets and golden comparison passed. `target/workspace-release.log`. |
| Loaded debug UI | 16 owned CPU burners, `RUST_TEST_THREADS=64`: exit 101, 1297 passed, 38 socket-permission failures, 47 ignored; no timing failure. Test runtime 160.64 s; command 163.78 s; one-minute load 144.56 to 179.08. `target/ui-final-load-debug.stdout`, `.stderr`, `target/load-final-results.json`. |
| Loaded full coverage UI | Gate command/profile: in ui, `cargo llvm-cov --locked --json --summary-only`, `CARGO_TARGET_DIR=target/xtask-cov`; 16 burners and 64 test threads. Exit 101, 1296 passed, 38 socket-permission failures, 47 ignored; no timing failure. Test runtime 210.28 s; command 302.88 s; load 189.39 to 241.07. `target/ui-load-coverage.stderr`, `target/load-results.json`. |
| Loaded focused coverage | The 46 measurement/pair/SIM cases plus the retained diagnostics-label case: 47 passed, 0 failed, 1 ignored, exit 0. Same coverage profile, 16 burners and 64 test threads; runtime 11.48 s; load 179.08 to 163.78. `target/ui-final-load-coverage-focused.stderr`. |
| Final focused release and coverage | Added the pure snapshot-clock lock-order regression to that selection: each profile has 48 passed, 0 failed, 1 explicitly ignored external-waveform probe, exit 0. Release runtime 1.61 s; coverage runtime 13.04 s. `target/final-focused-release.stdout`, `target/final-focused-coverage.stderr`, `target/final-focused-results.json`. |
| Snapshot clock regression | Debug: 1 passed, 0 failed. The injected snapshot clock is sampled after locking state, preserving production ordering without sockets. `target/shot-clock-regression.log`. |
| Privacy/diff | `git diff --check` passed. Tree scan reports 139 pre-existing findings; none intersect additions. `target/privacy-tree.log`, `target/privacy-additions.log`. The commit-message scan is recorded in Status. |

The full test failures are caused by sandbox denial of TCP/UDP/Unix socket binds (`Operation not permitted`),
including `hal::wlan::tests::join_commands`; assertions that listeners exist then fail as a consequence. These
were not skipped or disabled. Successful focused coverage does not establish the full workspace coverage floor.
The complete default gate and independent cross-vendor review remain coordinator work before acceptance.
All CPU burners were terminated and reaped by their owning driver after both stress batches.

The first stress attempt additionally exposed a test-query settling loop whose constant virtual clock could
not expire while a catalog result awaited another UI poll. The test clock now advances one millisecond per
completion wait. The dedicated catalog regression passed; later full loaded debug and coverage runs have only
the socket failures. Initial failure logs remain preserved in `target/ui-load-debug.*` and
`target/workspace-debug.log`. The retained diagnostics-label test and new snapshot-clock regression were
checked after the earlier full-suite builds; the final focused runs cover both.

### Round 2 review changes

- P1.1: One settling body in both builds. The runtime receives clock/wait callbacks at construction;
  `Ui::for_tests` installs manual time and completion waits. The expiry regression holds a submitted job,
  advances exactly QUERY_WAIT and invokes `measurement_settle`, proving the value remains unavailable.
- P1.2: Socket tests are compiled in debug/release/coverage and audited from source. The coordinator rerun list
  below includes every sandbox failure, plus converted socket cases that happen to pass without binding here.
- P2.3-7: Buffered receivers and original channel capacities in both builds; bounded-channel backpressure test;
  helper polling sleeps 1 ms; metric-lock release has a try_lock hang guard; both timing samples assert lock
  ownership; nonblocking socket reads require WouldBlock.
- P2.8-9: Mode changes use a shared lock-acquisition seam; a nonblocking attempt on the held update lock must fail
  before changing mode. Watchdog negatives follow a positive marker from inside the handoff holding loop.
- P2.10-11, P3.12: Test-time is a store kind with tightening, protected-base checks and move support. Fixtures cover
  clock comparisons, bound clocks, duration assertions outside loops, short timeout APIs, macro bodies and cfg(test)
  statements/expressions. The original virtual-clock fork shape is rejected. Exemptions name exact functions.
- P3.13-18, P3.20: Removed dead talk arguments; the HTTP deadline clock permits a partial head then expires on the
  second read sample, requiring `request took too long`. Picture provenance is checked against an Instant captured
  before feed. Child timeout arguments are used exactly; every nonzero ordinary call site already supplies at least
  120 seconds. Restored the original 30-second production batch-return timeout: the review text reversed the
  round-1 direction (30 to 120). Reverted the unrelated power-analysis borrow edit and cleaned assertion gaps and
  the AGENTS list. The source guard exposed legacy optional Eye profiling; its explicit environment switch now
  enables the same diagnostic path in production and tests.
- P3.19: Retained the original dispatch/cached-read budgets in ignored release perf_guard cases for native,
  reference, custom-row, pair, catalog and decode measurements, plus socket reads, remote knobs and shutdown.
  Normal functional tests keep their behavior assertions and wait on completion events.
- P3.21: Removed Manual from the production HAL module altogether. Integration-only `hal/tests/support/clock.rs`
  implements the shipped Clock trait, so default integration tests need no feature or self dependency. A self dev
  dependency would violate the current crate-direction rule; no architecture allowance was widened.

Round 1 evidence above is historical. The round 2 results and logs below supersede it for the reviewed candidate.

### Socket tests for the coordinator

Run the full debug/release suites and coverage on an unrestricted host. These 38 UI cases fail here at socket
bind/listener setup (`Operation not permitted`), as does the HAL WLAN fixture below. None was skipped or disabled.
The added ignored socket-read latency guard needs the unrestricted release perf-guard lane as well.

```text
oos-ui: announce::tests::a_query_to_the_socket_is_answered
oos-ui: hislip::tests::a_socket_round_trip_answers_idn
oos-ui: http::tests::a_body_without_content_length_gets_411
oos-ui: http::tests::a_large_body_at_a_time_and_net_allow_before_the_body
oos-ui: http::tests::a_request_the_frame_loop_does_not_take_gets_503
oos-ui: http::tests::cmd_takes_keys_knobs_and_touch_only
oos-ui: http::tests::every_limit_answers
oos-ui: http::tests::expect_continue_is_answered_and_refusals_come_first
oos-ui: http::tests::identification_names_the_listeners
oos-ui: http::tests::large_answers_go_through_a_thread
oos-ui: http::tests::live_picture_from_the_frame_loop_without_a_copy
oos-ui: http::tests::live_picture_once_per_screen_and_long_poll
oos-ui: http::tests::live_picture_with_the_gpu_is_a_picture_read_back
oos-ui: http::tests::net_allow_follows_a_change_without_a_restart
oos-ui: http::tests::page_lets_the_viewer_pick_png_or_bmp
oos-ui: http::tests::pages_and_downloads
oos-ui: http::tests::too_many_connections_get_503
oos-ui: http::tests::update_upload_limits
oos-ui: http::tests::waveform_upload_has_its_own_limit_and_origin_protection
oos-ui: http::tests::waveform_upload_loads_the_chosen_reference_and_preserves_it_on_error
oos-ui: net6::tests::dual_stack_or_ipv4
oos-ui: net6::tests::host_listeners_are_private_and_independent
oos-ui: net::tests::reads_without_waiting
oos-ui: scpi::tests::a_query_is_answered_after_the_client_closed_its_side
oos-ui: scpi::tests::compatibility_disconnected_raw_client_cannot_resume_trailing_writes
oos-ui: scpi::tests::compatibility_hislip_defers_entire_multiline_payload_with_original_message_id
oos-ui: scpi::tests::compatibility_hislip_sync_and_async_clear_discard_pending_tail_without_stale_reply
oos-ui: scpi::tests::compatibility_raw_tcp_pending_opc_does_not_block_other_clients
oos-ui: scpi::tests::generic_model_queries_preserve_statistics_configuration_and_pending_requests
oos-ui: scpi::tests::net_allow_follows_a_change_without_a_restart
oos-ui: scpi::tests::raw_tcp_compound_queries_share_one_response_message
oos-ui: scpi::tests::raw_tcp_empty_query_is_a_line_and_compound_query_keeps_its_position
oos-ui: scpi::vernier_tests::compound_query_is_one_line_on_the_socket
oos-ui: sntp::tests::no_server_means_an_error
oos-ui: sntp::tests::query_against_a_local_server
oos-ui: ui::bus_candidate::tests::real_scpi_queries_keep_pending_candidate_and_force_uses_the_live_bus_path
oos-ui: ui::tests::bode_sweep_drives_the_generator
oos-ui: vxi11::tests::host_vxi_listeners_use_loopback_and_advertise_the_selected_core
oos-hal: wlan::tests::join_commands
```

The converted Windows-only `desktop::lan::tests::desktop_network_services_share_the_window_session` also needs
its platform run. Host and ARM builds compile the socket cases selected on those platforms. Source review checked
request bytes, reply completion predicates, pending-operation/clear acknowledgements, client EOF, listener policy,
and the explicit partial-head deadline. The screen long-poll case now also requires a witnessed pending request and
explicit manual expiry before accepting 304, preserving the removed duration assertion as a deterministic negative.

### Round 2 validation evidence

All requested commands used the locked workspace and pinned toolchain. Logs remain local in `target/`.
The Rust/build/ratchet input fingerprint is `2b6e6c69fa28dc20521a58b243b1ee80db7861c9cdc5f2dbe87e394400259048` (`target/r2-candidate.sha256`).
No existing lint, size, coverage, static or unsafe ratchet was edited. The new per-crate test-time store is empty.

| Check | Result and evidence |
|---|---|
| xtask | `cargo test --locked -p xtask`: exit 0, 153 passed, 0 failed. Includes each review guard fixture, exact function exemptions, and test-time ratchet tightening/moves. `target/r2-xtask-final.log`. |
| Exact Clippy | Exit 0. Each crate used `cargo clippy --locked --all-targets --message-format=json --quiet`; hal/dsp/core/ui/fpgad also used `--target aarch64-unknown-linux-musl`. Unchanged ratchets admit hal 174, dsp 265, core 46, ui 671, fpgad 19, xtask 0 findings; no new/grown finding or compile error. `target/r2-clippy-final.log`, `target/clippy-results.json`, per-target JSON/stderr logs. |
| Fast gate | Exit 0: `partial gate passed (not done): 9 steps in 44 s`. Every selected step passed, including the zero-finding test-time store. `target/r2-fast-final.log`. |
| Full UI/HAL debug | `cargo test --locked -p oos-ui -p oos-hal --no-fail-fast`: exit 101, 615.96 s. UI binary 1300 passed, 38 socket failures, 58 ignored; HAL library 375 passed, 1 socket failure, 8 ignored. All integration targets, including acquisition clocks, six watchdog handoffs and golden comparison, passed. `target/r2-debug-final.log`. |
| Full UI/HAL release | Same command with `--release`: exit 101, 450.12 s. UI binary 1299 passed, 38 socket failures, 58 ignored; HAL library 375 passed, 1 socket failure, 8 ignored. All integration targets and golden comparison passed. `target/r2-release-final.log`. |
| Loaded full UI debug | 16 CPU burners and `RUST_TEST_THREADS=64`: exit 101, 178.59 s command / 177.80 s test runtime. 1300 passed, the same 38 socket failures, 58 ignored, no timing failure. `target/r2-loaded-ui-debug.stdout`, `.stderr`. |
| Loaded full UI coverage | Gate command/profile in ui: `cargo llvm-cov --locked --json --summary-only`, coverage target `target/xtask-cov`; 16 burners / 64 threads. Exit 101, 248.87 s command / 177.30 s tests. 1300 passed, the same 38 socket failures, 58 ignored, no timing failure. `target/r2-loaded-ui-coverage.stdout`, `.stderr`. |
| Loaded full HAL coverage | Same gate coverage command/profile in hal, 16 burners / 64 threads: exit 101, 323.36 s command / 314.76 s tests. 375 passed, the single WLAN socket failure, 8 ignored. `target/r2-loaded-hal-coverage.stdout`, `.stderr`. |
| Focused UI coverage | Same instrumentation, selecting measurement/channel/custom-row/pair/catalog/SIM regressions, mutex metrics and snapshot-clock sampling: exit 0, 97 passed, 0 failed, 8 ignored. `target/r2-focused-coverage.stdout`, `.stderr`. |
| Acquisition/watchdog coverage | Same instrumentation, selecting early_trigger_end, idle_during_ring_fill and rootfs_watchdog: exit 0, 2 + 2 + 6 passed, 0 failed. `target/r2-hal-clock-watchdog-coverage.stdout`, `.stderr`. |
| Restored latency guards | Release, `OOS_PERF_TESTS=1`, one test thread, selecting the eleven added perf_guard cases: exit 101; 10 passed, only `net::tests::perf_guard_nonblocking_socket_reads` failed at the denied bind. No duration budget failed. `target/r2-latency-perf.stdout`, `.stderr`. |
| Privacy/diff | `git diff --check` passed. Commit-message privacy scan: exit 0, `privacy: 0 finding(s)`. Tree scan retains 139 pre-existing findings; zero intersect additions. `target/r2-privacy-message.log`, `target/r2-privacy-tree.log`, `target/r2-privacy-additions.log`. |

Earlier round 2 fast-gate attempts reported failed steps `size`, `architecture`, `known-failures` and `test-time`.
Those failures exposed an oversized lock module, a rejected self dev dependency/module-path experiment and newly
recognized timing patterns. The lock was extracted, the unsupported experiment removed, and findings fixed;
`target/round2-fast*.log` and `target/r2-fast-probe.log` preserve the failed runs. The final gate result above
supersedes them.

The complete command/result records are `target/r2-validation.json`, `target/r2-load-results.json` and
`target/r2-followup-results.json`. All 16 owned burners were terminated and reaped; one-minute load rose from
48.97 to 247.87 across the loaded runs. The last socket-only edits (HiSLIP hang-guard constants and the explicit
long-poll negative) were compiled by the final host/ARM Clippy and release run; their behavior still requires the
unrestricted socket rerun. Ordinary non-socket behavior is covered by the full and final focused runs.

Full coverage commands cannot establish the coverage floor because their socket fixtures fail. The task remains
active for the coordinator's unrestricted socket run, full default gate and renewed independent review. No commit,
push, rebase, remote change or unit access occurred.

## Status

- 2026-11-17: Coordinator validation on an unrestricted host. The rewritten half-closed SCPI client test hung in
  debug and coverage (one poll accepts, answers and drops the client) and passed in release; it now waits for the
  answer and EOF at the client. Then: alone 20/20; debug ui 1338 and hal 376 passed; release ui 1337 and hal 376
  passed; loaded per-crate coverage above every floor (ui 89.1%); fast gate passed. Committed.

- 2026-11-17: Final evidence and source fingerprint verified. `git diff --check` passed; commit-message privacy
  has zero findings, and no tree privacy finding intersects additions. Existing ratchets and HEAD are unchanged.
  Both validation drivers completed and all 16 burners were terminated/reaped. No commits or unit access.

- 2026-11-17: Round 2 review repairs and requested validation are recorded above. Tested: 153 xtask tests passed;
  exact host/ARM Clippy exit 0; fast gate exit 0, `partial gate passed (not done): 9 steps in 44 s`.
  Full debug/release and loaded UI/HAL coverage have only the listed 38 UI plus one HAL socket failures;
  focused coverage has 97 UI plus 10 acquisition/watchdog passes. Ten latency guards passed; the socket-read
  latency guard requires an unrestricted bind. Both golden comparisons passed. All burners were reaped.
  Prepared `target/COMMIT_MSG.txt` with a Tested line; no commits or unit access. Acceptance remains pending
  renewed review, unrestricted sockets and the full default gate.

- 2026-11-17: Round 2 repairs implemented under the existing claim, without commits or unit access. Added manual
  query-expiry, channel-backpressure, lock-contention and lock-order regressions; strengthened the source guard and
  integrated its ratchet store/move lifecycle. Validation is in progress; earlier failed fast-gate attempts are kept
  in `target/round2-fast*.log` and have not been reported as passing.

- 2026-11-17: Final code fingerprint verified unchanged; `git diff --check` passed. Prepared
  `target/COMMIT_MSG.txt`; its privacy scan exited 0 with `privacy: 0 finding(s)`. Existing ratchets and
  HEAD remain unchanged. Final focused release/coverage driver exited 0; fast gate and exact Clippy exited 0.
- 2026-11-17: Implementation and audit prepared for review, without commits or unit access. Final fast gate exit 0:
  `partial gate passed (not done): 9 steps in 46 s`; exact host/ARM Clippy exit 0; final focused release and
  coverage each 48 passed, 0 failed, 1 ignored. The loaded full UI runs have no timing failures and retain
  38 sandbox socket failures. All burners are stopped/reaped. Full acceptance remains pending an unrestricted
  socket-capable run, the coordinator's full gate and independent cross-vendor review.

- 2026-11-17: Completed event-wait conversions for SIM, shared channel/reference/custom-row measurements,
  pairs, catalog, math, decode/search, demo capture, trace Bus and automatic Eye. Production and tests share buffered
  receivers over std channels; fixtures retain completed results for the normal poll. Added explicit clocks for acquisition,
  diagnostic accounting, HTTP deadline/long-poll decisions and shutdown budget offsets. HAL trigger/ring tests:
  4 passed, 0 failed, using manual time. Audit and source guard validation continue.
- 2026-11-17: Initial UI debug feedback: 1295 passed, 39 failed; fixed one introduced diagnostic count assertion.
  Second run: 1296 passed, 38 failed, 47 ignored. Remaining failures require socket operations rejected by the
  sandbox (`Operation not permitted`). HAL initial library feedback: 375 passed, 1 failed, 8 ignored;
  `wlan::tests::join_commands` also cannot bind its fixture socket. Complete logs remain under `target/`.

- 2026-11-16: Author Codex; branch `t0088-test-time`, base `b582fa27`. Owner assigned this isolated worktree
  and task id. Plan written before code. No unit access. Initial logs identify SIM, shared reference/channel
  measurement, custom-row and pair completion deadlines; broader audit is in progress.
