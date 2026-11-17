// SPDX-License-Identifier: GPL-2.0-or-later

//! Process-wide requested-live-byte characterisation, confined to this test executable.

use std::alloc::{GlobalAlloc, Layout, System};
use support::counting::Counting;

#[global_allocator]
static HEAP: Counting = Counting::new();

// SAFETY: every allocation and layout is delegated unchanged to System. Atomic counters never allocate.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller supplies GlobalAlloc's valid layout; System owns the returned allocation.
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            self.allocated(layout.size());
        }
        p
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the valid layout is passed unchanged to System, preserving its zero-initialization contract.
        let p = unsafe { System.alloc_zeroed(layout) };
        if !p.is_null() {
            self.allocated(layout.size());
        }
        p
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the caller provides the System allocation and its original layout.
        unsafe { System.dealloc(ptr, layout) };
        self.freed(layout.size());
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        // SAFETY: the caller provides the live System allocation, original layout and valid new size.
        let p = unsafe { System.realloc(ptr, layout, size) };
        if !p.is_null() {
            self.resized(layout.size(), size);
        }
        p
    }
}

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
        // Hang guard only: the child does a fixed amount of work and exits in milliseconds.
        let out = support::run_command(&inputs, command, 120);
        assert!(
            out.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        return;
    }
    // The harness thread still allocates its bookkeeping while this thread runs (#24), so the exact accounting reads
    // this thread's own share. The process-wide baseline only sizes the retained buffer against the soak limit.
    let baseline = HEAP.live();
    let own = HEAP.own_live();
    let grown = |bytes: usize| own + isize::try_from(bytes).expect("probe buffers fit in isize");
    let buffer = vec![0u8; baseline / 50 + 4096];
    std::hint::black_box(&buffer);
    assert_eq!(HEAP.own_live(), grown(buffer.len()));
    assert!(baseline + buffer.len() > limit(baseline), "retained growth must exceed the soak limit");
    drop(buffer);
    assert_eq!(HEAP.own_live(), own);
    let mut buffer = vec![0u8; 4096];
    assert_eq!(HEAP.own_live(), grown(4096));
    buffer.reserve_exact(8192);
    assert_eq!(HEAP.own_live(), grown(buffer.capacity()));
    buffer.truncate(2048);
    buffer.shrink_to_fit();
    assert_eq!(HEAP.own_live(), grown(2048));
    drop(buffer);
    assert_eq!(HEAP.own_live(), own);
}

pub mod support;
mod snapshots {
    pub mod change_alloc;
    pub mod hot_paths;
}

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
    let jitter = channel_block();
    let start = Instant::now();
    let result = oos_ui::Ui::scripted_soak(&mode, requested, |step, counters| {
        steps = step;
        let live = HEAP.live();
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
    let flat = no_growth(&ends, jitter);
    writeln!(summary, "final_six_cycle_ends={:?} jitter={} no_growth={}", ends, jitter, flat).unwrap();
    write!(summary, "backend=counting-System; scope=Rust global-allocator requested live bytes including workers\nmode={} steps={} warmup={} cycle=100 baseline={} limit={} later_peak={} failed_cycles={} counters={:?} elapsed_ms={}\n",
        mode, steps, warmup, baseline, limit(baseline), worst, failures, previous, start.elapsed().as_millis()).unwrap();
    std::fs::write(out.join("summary.txt"), &summary).unwrap();
    print!("{}", summary);
    assert!(result.is_ok(), "{}: {:?}", mode, result);
    assert_eq!(steps, requested, "every requested step was sampled");
    assert_eq!(previous[4], (requested / 100 * 3) as u64, "three deliberate old-stamp drops per cycle");
    assert!(flat, "{}: final six cycle ends grew more than one channel block ({} bytes): {:?}", mode, jitter, ends);
    assert_eq!(failures, 0, "{}: post-warm-up live bytes grew more than 1 %", mode);
}

/// Whether the last three cycle ends stay within `jitter` of the highest of the three before them.
fn no_growth(ends: &[usize; 6], jitter: usize) -> bool {
    // Three cycles cover the 6-batch schedule against the 100-step storm, including STOP.
    let (earlier, later) = (ends[..3].iter().max(), ends[3..].iter().max());
    later.zip(earlier).is_some_and(|(later, earlier)| *later <= earlier + jitter)
}

/// The bytes of one queue block of an unbounded std channel that carries a boxed trait object, measured on this
/// thread. The draw pipe returns read-out buffers over such a channel. std allocates its queue in blocks and frees a
/// block once the receiver has passed it, from whichever thread is last to touch it. So a cycle end can hold one
/// block more or less than another, depending only on scheduling (#24). Retained growth beyond that is a leak.
fn channel_block() -> usize {
    let (tx, rx) = std::sync::mpsc::channel::<Box<dyn std::any::Any + Send>>();
    let before = HEAP.own_live();
    tx.send(Box::new(())).expect("the receiver is alive");
    let block = HEAP.own_live() - before;
    drop((tx, rx));
    let block = usize::try_from(block).expect("the first send allocates the queue block");
    assert!(block > 0, "the first send allocates the queue block");
    block
}

#[test]
fn cycle_end_trend_rejects_growth_beyond_one_channel_block() {
    assert!(no_growth(&[1000, 1007, 1000, 1000, 1007, 1000], 0));
    assert!(!no_growth(&[1000, 1000, 1000, 1001, 1001, 1001], 0));
    // CI: one cycle end held one extra block of the buffer return channel.
    let block = 752;
    assert!(no_growth(&[32083188, 32083188, 32083188, 32083188, 32083188, 32083188 + block], block));
    assert!(no_growth(&[32083188, 32083188, 32083188, 32083188 + block, 32083188, 32083188], block));
    assert!(!no_growth(&[32083188, 32083188, 32083188, 32083188 + block + 1, 32083188, 32083188], block));
}

#[test]
fn channel_block_matches_the_draw_pipe_return_channel() {
    // Box<dyn RawBatch> and Box<dyn Any + Send> are both two-word pointers, so the block layouts agree.
    assert_eq!(size_of::<Box<dyn std::any::Any + Send>>(), 2 * size_of::<usize>());
    assert!(channel_block() > 31 * size_of::<Box<dyn std::any::Any + Send>>());
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
        HEAP.peak(),
        HEAP.allocations(),
        HEAP.frees(),
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

#[test]
fn change_snapshots_reuse_allocations_after_warmup() {
    if std::env::var_os("OOS_CHANGE_ALLOC").is_none() {
        let inputs = support::isolated_inputs();
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", "change_snapshots_reuse_allocations_after_warmup", "--nocapture"])
            .env("OOS_CHANGE_ALLOC", "1");
        let out = support::run_command(&inputs, command, 120);
        print!("{}", String::from_utf8_lossy(&out.stdout));
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        return;
    }
    snapshots::change_alloc::check();
}

#[test]
fn warmed_ui_observer_allocation_census() {
    if std::env::var_os("OOS_UI_CHANGE_CENSUS").is_none() {
        let inputs = support::isolated_inputs();
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", "warmed_ui_observer_allocation_census", "--nocapture"])
            .env("OOS_UI_CHANGE_CENSUS", "1");
        let out = support::run_command(&inputs, command, 120);
        print!("{}", String::from_utf8_lossy(&out.stdout));
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        return;
    }
    let rows = oos_ui::Ui::scripted_change_census(256, || HEAP.allocations());
    for (name, [calls, allocations]) in
        ["prepare_keys", "compare", "finish_capture", "stage", "live_normalize", "consumers"].into_iter().zip(rows)
    {
        println!("UI observer census {name}: calls={calls} allocations={allocations}");
        assert_eq!(calls, 256);
    }
    assert_eq!(rows[5][1], 0, "warmed reset consumers must not allocate");
    assert_eq!(rows[0][1], 0, "warmed UI keys, normalize and classify must not allocate");
    assert_eq!(rows[1][1], 0, "warmed compare must not allocate");
    assert_eq!(rows[2][1], 0, "warmed finish capture and proposals_agree must not allocate");
    assert_eq!(rows[3][1], 0, "warmed stage capture and proposals_agree must not allocate");
    assert_eq!(rows[4][1], 0, "warmed live normalize and its legacy bridge must not allocate");
}
