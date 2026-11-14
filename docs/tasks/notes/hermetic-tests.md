# Task 0030: hermetic test review corrections

Date: 2026-11-15. Branch: `t0030-hermetic-tests`. Base: `aae6c0e`.
This report supersedes the earlier verification of that commit.
All Cargo commands used `--offline --locked`. No network service or device was
contacted. Changes remain uncommitted. The owner retains integration and review.

## Review corrections

1. Restored all four original private assertions. The DDR taps, boot ADC
   parameter sequence, AFG codes and default LA calibration each have a separate
   `#[ignore = "private calibration: set OOS_CAL_DIR"]` test. Missing or empty
   `OOS_CAL_DIR` fails explicitly. These tests read only that directory. They
   cannot fall through to another private calibration source. No real private
   calibration validation was performed.
2. Removed every production `cfg!(test)` branch, including the older ones in
   time, WLAN, USB polling, Bode and HTTP. The clock and host source enter through
   `Ui::with_host`. Unit tests explicitly use `Ui::for_tests`. The system adapter
   remains compiled in tests. Host services are a runtime capability of the
   supplied source. UI metadata, disk space, temperatures, setup lookup, image
   defaults and timestamps use those inputs. The configuration adapters again
   use their normal paths in every build. Parser tests supply explicit values
   through the existing `*_from`, `fork_text_of` and configuration loaders.
   Deleted both tautological tests. Added parser and UI input propagation checks.
3. Documented the synthetic AFG code formulas and boot register packing at their
   assertions. The invented formulas remain independent of private unit data.
4. Added `*.hex binary`. Removed the 717724-byte vertical fixture from the tree.
   The Rust helper now generates it in memory. Its complete-container CRC is
   checked against the prior Python reference output. The Python model uses a
   temporary copy. All nine stored fixture/reference files regenerate unchanged.
5. Removed the leftover scope in `seq.rs` and the blank line in `calfile.rs`.
6. Restored all 13 `OOS_SHOT_DIR` picture dumps through five ignored manual tests.
   Mandatory tests share the scenario helpers without writing pictures.
7. Every bare ignore now has a reason and a required input. Timing checks require
   `OOS_PERF_TESTS=1`. `tools/perf-guard.sh` supplies it. Reports require
   `OOS_REPORTS=1`. Live multicast requires `OOS_MDNS_IF`. Simulated radio checks
   require `OOS_WLAN_SIM=1`. The demo server requires `OOS_WEB_DEMO_S`.
   Screenshot directories must be explicit and nonempty. No fallback to `.` remains.

Fixture details and private-suite commands are in the
[fixture README](../../../hal/testdata/synthetic/README.md).

## Full debug and release runs

Ran `cargo test --offline --locked --no-fail-fast` in each crate.
Added `--release` for its release run. The actual Cargo exit codes are below.
The table totals sum every printed target. Exact target summaries follow.
Zero-test and ignored-only targets are recorded, without counting as coverage.

| Crate | Profile | Exit | Passed | Failed | Ignored |
| --- | --- | ---: | ---: | ---: | ---: |
| hal | debug | 101 | 340 | 1 | 10 |
| hal | release | 101 | 340 | 1 | 10 |
| dsp | debug | 0 | 168 | 0 | 1 |
| dsp | release | 0 | 168 | 0 | 1 |
| ui | debug | 101 | 500 | 19 | 29 |
| ui | release | 101 | 500 | 19 | 29 |
| fpgad | debug | 0 | 12 | 0 | 0 |
| fpgad | release | 0 | 12 | 0 | 0 |

## Known sandbox socket failures

The same failures occur in debug and release. These are listed only.
Direct binds report `PermissionDenied: Operation not permitted`. Listener
wrappers report that the port is unavailable. No socket test was weakened.

- `hal: wlan::tests::join_commands`
- `ui: http::tests::a_body_without_content_length_gets_411`
- `ui: http::tests::a_request_the_frame_loop_does_not_take_gets_503`
- `ui: http::tests::cmd_takes_keys_knobs_and_touch_only`
- `ui: http::tests::every_limit_answers`
- `ui: http::tests::expect_continue_is_answered_and_refusals_come_first`
- `ui: http::tests::large_answers_go_through_a_thread`
- `ui: http::tests::live_picture_from_the_frame_loop_without_a_copy`
- `ui: http::tests::live_picture_once_per_screen_and_long_poll`
- `ui: http::tests::page_lets_the_viewer_pick_png_or_bmp`
- `ui: http::tests::live_picture_with_the_gpu_is_a_picture_read_back`
- `ui: http::tests::pages_and_downloads`
- `ui: http::tests::too_many_connections_get_503`
- `ui: http::tests::update_upload_limits`
- `ui: net6::tests::dual_stack_or_ipv4`
- `ui: net::tests::reads_without_waiting`
- `ui: scpi::vernier_tests::compound_query_is_one_line_on_the_socket`
- `ui: sntp::tests::no_server_means_an_error`
- `ui: sntp::tests::query_against_a_local_server`
- `ui: ui::tests::bode_sweep_drives_the_generator`

## Additional checks

The production UI also passes `cargo check --offline --locked`.
Eighteen clock, host and configuration input checks pass with conflicting process
settings, including `OOS_FIXED_TIME=0` and `TZ=JST-9`:

```text
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 530 filtered out; finished in 0.02s
```

The five private opt-in checks were selected with `OOS_CAL_DIR` unset.
They fail as required. Eight manual picture checks were selected without their
output variables. They fail before writing any output. The restored manual
suite passes with an explicit temporary output directory. It writes 13 valid
PNG files. The temporary pictures were removed after verification.

```text
Private opt-in without OOS_CAL_DIR, exit 101:
test result: FAILED. 0 passed; 5 failed; 0 ignored; 0 measured; 332 filtered out; finished in 0.00s
Manual picture checks without output variables, exit 101:
test result: FAILED. 0 passed; 8 failed; 0 ignored; 0 measured; 540 filtered out; finished in 0.00s
Restored manual pictures with OOS_SHOT_DIR, exit 0:
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 543 filtered out; finished in 0.31s
```

The missing-input failures above are deliberate negative checks. They are
separate from the eight full-suite runs. Static searches found no production
`cfg!(test)` branch or bare ignored attribute. `git diff --check` passes.

The full xtask gate is unavailable in this snapshot. There is no root Cargo
workspace or xtask implementation. No final commit was created. Task status
remains active for the coordinator's review and integration. Hooks, ratchets,
dependencies and lockfiles were not changed.

## Cargo summaries as printed

### hal debug, exit 101

```text
unittests src/lib.rs
test result: FAILED. 328 passed; 1 failed; 8 ignored; 0 measured; 0 filtered out; finished in 52.86s
tests/alloc_batch.rs
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
tests/compress_raw_again.rs
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
tests/early_trigger_end.rs
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.00s
tests/idle_during_ring_fill.rs
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
tests/overclock.rs
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
tests/overclock_1375.rs
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
tests/perf_ratio.rs
test result: ok. 0 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.00s
Doc-tests oos_hal
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### hal release, exit 101

```text
unittests src/lib.rs
test result: FAILED. 328 passed; 1 failed; 8 ignored; 0 measured; 0 filtered out; finished in 4.13s
tests/alloc_batch.rs
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
tests/compress_raw_again.rs
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
tests/early_trigger_end.rs
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.00s
tests/idle_during_ring_fill.rs
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
tests/overclock.rs
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
tests/overclock_1375.rs
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
tests/perf_ratio.rs
test result: ok. 0 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.00s
Doc-tests oos_hal
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### dsp debug, exit 0

```text
unittests src/lib.rs
test result: ok. 50 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.58s
tests/analysis.rs
test result: ok. 46 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 17.36s
tests/bus.rs
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
tests/bus2.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
tests/decode_ext.rs
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.51s
tests/decode_ext2.rs
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
tests/decode_ext3.rs
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.63s
tests/measure.rs
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
tests/triggers.rs
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
Doc-tests oos_dsp
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### dsp release, exit 0

```text
unittests src/lib.rs
test result: ok. 50 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s
tests/analysis.rs
test result: ok. 46 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 1.60s
tests/bus.rs
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
tests/bus2.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
tests/decode_ext.rs
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
tests/decode_ext2.rs
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
tests/decode_ext3.rs
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
tests/measure.rs
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
tests/triggers.rs
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Doc-tests oos_dsp
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### ui debug, exit 101

```text
unittests src/main.rs
test result: FAILED. 500 passed; 19 failed; 29 ignored; 0 measured; 0 filtered out; finished in 71.42s
```

### ui release, exit 101

```text
unittests src/main.rs
test result: FAILED. 500 passed; 19 failed; 29 ignored; 0 measured; 0 filtered out; finished in 10.11s
```

### fpgad debug, exit 0

```text
unittests src/main.rs
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
unittests src/bin/oos-update.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### fpgad release, exit 0

```text
unittests src/main.rs
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
unittests src/bin/oos-update.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Exact counts, ignore reasons and target classifications are also in
[the structured results](hermetic-tests-results.json).
