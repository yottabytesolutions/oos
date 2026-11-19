// SPDX-License-Identifier: GPL-2.0-or-later

//! Process isolation keeps unrelated test threads out of the allocation census.

use crate::{support, HEAP};

#[test]
fn hot_paths_allocate_nothing_after_warmup() {
    if std::env::var_os("OOS_ALLOC_CHILD").is_none() {
        let inputs = support::isolated_inputs();
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", "snapshots::hot_paths::hot_paths_allocate_nothing_after_warmup", "--nocapture"])
            .env("OOS_ALLOC_CHILD", "1");
        let output = support::run_command(&inputs, command, 120);
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        print!("{}", String::from_utf8_lossy(&output.stdout));
        return;
    }
    // Prove the observer sees freed transients, zeroed allocations and reallocations.
    // This thread's own count: the harness thread may still allocate its bookkeeping at this point (#24).
    let (count, ()) = HEAP.count_own(|| {
        let mut v = Vec::<u8>::with_capacity(16);
        v.push(1);
        v.reserve_exact(4096);
        std::hint::black_box(v);
        std::hint::black_box(vec![0u8; 4096]);
    });
    assert_eq!(count, 3, "alloc, realloc and alloc_zeroed are all counted after free");
    let mut paths = 0;
    oos_ui::Ui::hot_path_probe(|name, frame| {
        for _ in 0..8 {
            frame();
        }
        for index in 0..64 {
            let (allocations, ()) = HEAP.count(&mut *frame);
            assert_eq!(allocations, 0, "{name}: post-warmup frame {index}");
        }
        paths += 1;
        println!("ALLOC_BUDGET {} 64 0", name.replace(' ', "_"));
    });
    assert_eq!(paths, 6, "all currently guarded UI hot kernels were exercised");
    deep_census();
}

fn deep_census() {
    let mut cases = 0;
    oos_ui::Ui::deep_step_probe(|name, bytes, mut reference, step| {
        HEAP.set_large_min(bytes);
        let before = HEAP.large_allocations();
        std::hint::black_box(vec![0u8; bytes]);
        assert_eq!(HEAP.large_allocations() - before, 1, "observer sees freed {bytes}-byte copies");
        for _ in 0..8 {
            if let Some(reference) = reference.as_mut() {
                reference();
            }
            step();
        }
        for index in 0..64 {
            let before = HEAP.large_allocations();
            if let Some(reference) = reference.as_mut() {
                reference();
            }
            let scratch = HEAP.large_allocations() - before;
            let before = HEAP.large_allocations();
            step();
            let copies = HEAP.large_allocations() - before;
            assert_eq!(copies, scratch, "{name} step {index}: extra allocations >= {bytes} can copy samples");
        }
        cases += 1;
        println!("COPY_BUDGET {name} 64 no_extra_allocations_at_or_above_{bytes}_bytes");
    });
    assert_eq!(cases, 4, "deep pairs, shallower pairs, queued raw and synchronous raw readers ran");
}

#[test]
fn draw_timing_recording_allocates_nothing() {
    if std::env::var_os("OOS_DRAW_ALLOC_CHILD").is_none() {
        let inputs = support::isolated_inputs();
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", "snapshots::hot_paths::draw_timing_recording_allocates_nothing", "--nocapture"])
            .env("OOS_DRAW_ALLOC_CHILD", "1");
        let output = support::run_command(&inputs, command, 120);
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stdout));
        return;
    }
    let mut timing = oos_ui::StageTiming::default();
    let (allocations, ()) = HEAP.count(|| {
        for _ in 0..1024 {
            timing.draw_step([std::time::Duration::from_nanos(17); 5]);
            timing.draw_render(std::time::Duration::ZERO, false);
            timing.draw_render(std::time::Duration::ZERO, true);
        }
    });
    assert_eq!(allocations, 0, "step/render recording, including ring wrap, must allocate nothing");
    assert!(timing.diagnostics_text().contains("draw.grade n=256 total=1024 p50_ns=17 p90_ns=17 p99_ns=17"));
}
