# DMA fixture replay and stage benchmark

A batch dump stores the DMA bytes as read and a versioned `fixture.txt` sidecar. `frames` is the completed
count and `avail` is the count dispatched from that buffer. The separately read, decoded replacement waveform
is present as `newest.bin` only when it was supplied by acquisition. The ordinary newest waveform comes from
`dma.bin`. Digital samples are stored in `la.bin` when present. `dump_io_ns` covers the payload writes and
sidecar body write. The timing field itself and the final atomic rename follow that measurement.

The `oos-batch.v2` loader ignores metadata from the retired FPGA interpolation experiment.
Replay still requires the DMA layout to match the recorded acquisition geometry. Captures whose
interpolated payload changed the frame size fail this check.

Set `acq.dump = on` and optionally `acq.dump.count = <n>` in system.conf before restarting oos-ui.
The switches default to off and four batches per change, with the count bounded to 1–32.
The root defaults to `/tmp/oos-batches`. The `OOS_BATCH_DUMP`, `OOS_BATCH_DUMP_COUNT` and
`OOS_BATCH_DUMP_DIR` environment overrides are read at startup. Set `acq.frame_header = on` for
headered captures or leave it off for the raw-code path. `OOS_FRAME_HEADER=1` enables full logging
and `OOS_FRAME_HEADER=off` selects off-trigger logging. `:SYSTem:STATe?` reports both startup switches.

Use saved fixture directories or a directory of fixtures as `OOS_FIXTURES`. The host replay asserts the full
`route.v1`, including the change in `wide_search` during placement. The benchmark verifies the same route on
one thread using the recorded worker count for column sharding and the recorded prior hits for preflight.
The live dump samples the process route counter, so concurrent trigger searches outside the pool can affect it.

Build the ignored benchmark as a release test. Cargo prints the exact executable, named
`target/aarch64-unknown-linux-musl/release/deps/oos_ui-<hash>`.

```sh
CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
  cargo +1.99.1 test -p oos-ui --bin oos-ui --release \
  --target aarch64-unknown-linux-musl --features private-picture-counts --no-run
```

Run the binary with `raw_batch_stage_benchmark --ignored --nocapture --test-threads=1`.
`OOS_BENCH_CPU` pins the benchmark thread where Linux affinity is available. `OOS_BENCH_STAGE` selects one
stage, such as `core_estimate`, `min_max` or `split_columns`.

- `OOS_BENCH_REPEATS=1` is the default. It walks submitted records in DMA order without a fixture warm-up.
  The production prepare job runs first, including any deferred newest-waveform decode. Its per-record
  estimate and decode are charged to the newest record. Route verification runs separately after timing.
- Repeats greater than one are for external instruction counters. Each selected kernel runs repeatedly at
  its actual call site. Choose one stage per PMU run. Fixture loading, dispatch and the untimed route check
  contribute fixed overhead to the process instruction count. The JSON wall-time fields are null in this mode.

Each line identifies the mode, fixture, absolute record index and physical channel. A stage reports one logical
call per repeat for each participating record. Channel-specific probes report their own time. Shared work
is divided evenly across the enabled channels and appears in each channel's attributed total. Plane reset time
is divided across the records whose tiles populated the planes. `split_columns` measures joining the column
shards and also includes dispatch bookkeeping in the cold pass. The shard kernels have their own stage timings.
`placement_geometry` measures the batch layout and view projection lookups, including first-use setup. `place_remainder` reports the wall time
outside the named placement stages, including probe overhead. Check it against 5% of `place` on target.
