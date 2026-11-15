// SPDX-License-Identifier: GPL-2.0-or-later

//! Process-wide requested-live-byte characterisation, confined to this test executable.

use std::sync::atomic::Ordering::SeqCst;

#[global_allocator]
static HEAP: support::counting::Counting = support::counting::Counting::new();

fn limit(baseline: usize) -> usize {
    baseline + baseline.div_ceil(100)
}

#[test]
fn retained_growth_and_freed_transients_are_distinguished() {
    // Run alone in a fresh process so unrelated test-harness allocations cannot affect the measurement.
    if std::env::var_os("OOS_SOAK_PROBE").is_none() {
        let inputs = support::isolated_inputs();
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", "retained_growth_and_freed_transients_are_distinguished", "--nocapture"])
            .env("OOS_SOAK_PROBE", "1");
        assert!(support::run_command(&inputs, command, 120).status.success());
        return;
    }
    let baseline = HEAP.live.load(SeqCst);
    let buffer = vec![0u8; baseline / 50 + 4096];
    std::hint::black_box(&buffer);
    assert!(HEAP.live.load(SeqCst) > limit(baseline));
    drop(buffer);
    assert_eq!(HEAP.live.load(SeqCst), baseline);
    let mut buffer = vec![0u8; 4096];
    let allocated = HEAP.live.load(SeqCst);
    buffer.reserve_exact(8192);
    assert_eq!(HEAP.live.load(SeqCst), allocated + buffer.capacity() - 4096);
    let grown = HEAP.live.load(SeqCst);
    let capacity = buffer.capacity();
    buffer.truncate(2048);
    buffer.shrink_to_fit();
    assert_eq!(HEAP.live.load(SeqCst), grown - capacity + 2048);
    drop(buffer);
    assert_eq!(HEAP.live.load(SeqCst), baseline);
}

mod support;

use std::fmt::Write as _;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

fn fresh(name: &str, mode: &str, steps: usize, out: &std::path::Path) {
    let inputs = support::isolated_inputs();
    let mut command =
        std::process::Command::new(std::env::current_exe().expect("soak child paths and logs must be available"));
    command
        .env_clear()
        .env("LC_ALL", "C")
        .env("TZ", "UTC")
        .env("OOS_SOAK_MODE", mode)
        .env("OOS_SOAK_STEPS", steps.to_string())
        .env("OOS_SOAK_OUT", out)
        .current_dir(inputs.path(""))
        .args(["--exact", name, "--nocapture"]);
    let result = support::run_command(&inputs, command, 3600);
    std::fs::write(out.join("child.stdout.log"), &result.stdout).expect("soak child paths and logs must be available");
    std::fs::write(out.join("child.stderr.log"), &result.stderr).expect("soak child paths and logs must be available");
    assert!(
        result.status.success(),
        "{}: {}\n{}",
        mode,
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn scripted_modes_reach_a_plateau() {
    let long = std::env::var("OOS_LONG_SOAK").as_deref() == Ok("1");
    if cfg!(debug_assertions) && !long {
        println!("composed soak runs in release; use OOS_LONG_SOAK=1 for debug/image verification");
        return;
    }
    let inputs = support::isolated_inputs();
    // Resolve outputs through the same worktree policy as every build and host tool.
    let target = inputs.path("").parent().unwrap().parent().unwrap().to_path_buf();
    let profile = if cfg!(debug_assertions) { "debug" } else { "release" };
    let nonce = inputs.path("").file_name().unwrap().to_string_lossy().into_owned();
    let modes = ["raw", "codes", "pairs", "deep-pairs", "average"];
    let selected = std::env::var("OOS_SOAK_CASE").ok();
    assert!(selected.as_ref().is_none_or(|s| modes.contains(&s.as_str())), "unknown or empty soak selection");
    for mode in modes.into_iter().filter(|m| selected.as_ref().is_none_or(|s| s == m)) {
        let out = target.join("golden/soak").join(profile).join(mode).join(&nonce);
        std::fs::create_dir_all(&out).unwrap();
        fresh("plateau_child", mode, if long { 10_000 } else { 1000 }, &out);
        println!("soak {} artifacts: {}", mode, out.display());
    }
}

#[test]
fn plateau_child() {
    let Ok(mode) = std::env::var("OOS_SOAK_MODE") else { return };
    let out = PathBuf::from(std::env::var_os("OOS_SOAK_OUT").unwrap());
    let mut samples = std::fs::File::create(out.join("cycles.csv")).unwrap();
    writeln!(samples, "step,cycle_peak,cycle_end,allocator_high_water,allocations,frees,acquired,used,dropped,measured,planned_drops,zone_drops,elapsed_ms").unwrap();
    let requested: usize = std::env::var("OOS_SOAK_STEPS").unwrap().parse().unwrap();
    let warmup = 1000.min(requested / 2);
    let (mut peak, mut baseline, mut worst, mut failures) = (0, 0, 0, 0);
    let mut previous = [0; 6];
    let mut ends = [0; 6];
    let mut summary = String::with_capacity(1024);
    let mut steps = 0;
    let start = Instant::now();
    let result = oos_ui::Ui::scripted_soak(&mode, requested, |step, counters| {
        steps = step;
        let live = HEAP.live.load(SeqCst);
        peak = peak.max(live);
        if step % 100 == 0 {
            if step == warmup {
                baseline = peak;
            }
            if step > warmup {
                worst = worst.max(peak);
                failures += usize::from(peak > limit(baseline));
            }
            ends.rotate_left(1);
            ends[5] = live;
            cycle_row(&mut samples, step, peak, live, counters, &mut previous, start);
            peak = 0;
        }
    });
    writeln!(summary, "final_six_cycle_ends={:?} no_growth={}", ends, no_growth(&ends)).unwrap();
    write!(summary, "backend=counting-System; scope=Rust global-allocator requested live bytes including workers\nmode={} steps={} warmup={} cycle=100 baseline={} limit={} later_peak={} failed_cycles={} counters={:?} elapsed_ms={}\n",
        mode, steps, warmup, baseline, limit(baseline), worst, failures, previous, start.elapsed().as_millis()).unwrap();
    std::fs::write(out.join("summary.txt"), &summary).unwrap();
    print!("{}", summary);
    assert!(result.is_ok(), "{}: {:?}", mode, result);
    assert_eq!(steps, requested, "every requested step was sampled");
    assert_eq!(previous[4], (requested / 100 * 3) as u64, "three deliberate old-stamp drops per cycle");
    assert!(no_growth(&ends), "{}: final six cycle ends grew: {:?}", mode, ends);
    assert_eq!(failures, 0, "{}: post-warm-up live bytes grew more than 1 %", mode);
}

fn no_growth(ends: &[usize; 6]) -> bool {
    // Three cycles cover the 6-batch schedule against the 100-step storm, including STOP.
    ends[3..].iter().max() <= ends[..3].iter().max()
}

#[test]
fn cycle_end_trend_rejects_growth_below_one_percent() {
    assert!(no_growth(&[1000, 1007, 1000, 1000, 1007, 1000]));
    assert!(!no_growth(&[1000, 1000, 1000, 1001, 1001, 1001]));
}

fn cycle_row(
    samples: &mut std::fs::File,
    step: usize,
    peak: usize,
    live: usize,
    counters: [u64; 6],
    previous: &mut [u64; 6],
    start: Instant,
) {
    let delta = std::array::from_fn::<_, 6, _>(|i| counters[i] - previous[i]);
    *previous = counters;
    writeln!(
        samples,
        "{step},{peak},{live},{},{},{},{},{},{},{},{},{},{}",
        HEAP.peak.load(SeqCst),
        HEAP.allocations.load(SeqCst),
        HEAP.frees.load(SeqCst),
        delta[0],
        delta[1],
        delta[2],
        delta[3],
        delta[4],
        delta[5],
        start.elapsed().as_millis()
    )
    .expect("cycle output was opened before warm-up and must remain writable");
}
