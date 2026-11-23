// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use perf::Build;

fn fixture(disabled: bool, channels: usize, tdiv: f32) -> (Scope, Build) {
    let scope =
        Scope::with_counters(drawpipe::Counters { diagnostics_disabled: disabled.into(), ..Default::default() });
    let build = Build::new(false, channels, tdiv, 0);
    (scope, build)
}

#[test]
fn counters_preserve_picture_membership_pixels_and_pacing() {
    for channels in [1, 2, 4] {
        for tdiv in [1e-6, 10e-6] {
            let (reference_scope, mut reference) = fixture(true, channels, tdiv);
            let (candidate_scope, mut candidate) = fixture(false, channels, tdiv);
            for _ in 0..3 {
                reference_scope.select();
                reference.run();
                candidate_scope.select();
                candidate.run();
                compare(&picture(&reference.route.ui), &picture(&candidate.route.ui), "counters on/off");
                assert_eq!(reference.route.ui.wfm_total, candidate.route.ui.wfm_total);
            }
            assert!(drawpipe::counters().deposited.load(std::sync::atomic::Ordering::Relaxed) > 0);
            reference_scope.select();
            assert_eq!(drawpipe::counters().deposited.load(std::sync::atomic::Ordering::Relaxed), 0);
        }
    }
}

#[test]
#[ignore = "perf-guard latency: release, OOS_PERF_TESTS=1, one test thread"]
fn perf_guard_pipeline_counters() {
    assert_eq!(std::env::var("OOS_PERF_TESTS").as_deref(), Ok("1"));
    for channels in [1, 2, 4] {
        for tdiv in [1e-6, 10e-6] {
            let _scope = Scope::new();
            let counters = drawpipe::counters();
            // Both sides reuse the same workers, buffers and maps; allocation placement must not decide the staged guard.
            let build = std::cell::RefCell::new(Build::new(false, channels, tdiv, 0));
            let baseline = || {
                counters.diagnostics_disabled.store(true, std::sync::atomic::Ordering::Relaxed);
                build.borrow_mut().run();
            };
            let counted = || {
                counters.diagnostics_disabled.store(false, std::sync::atomic::Ordering::Relaxed);
                build.borrow_mut().run();
            };
            for _ in 0..8 {
                baseline();
                counted();
            }
            let before = build.borrow().route.ui.wfm_total;
            let pair = crate::system::perfguard::measure(crate::perfguard::ROUNDS, 16, baseline, counted);
            let build = build.borrow();
            println!("PIPELINE_COUNTERS channels={channels} tdiv={tdiv} frames={}", build.frames);
            pair.report("perf_guard_pipeline_counters");
            // ADR 0031: temporary F0 staging headroom; restore 1.01 at F3 frame cutover.
            let ratio = pair.best.1.as_secs_f64() / pair.best.0.as_secs_f64();
            println!(
                "COUNTERS_WALL ratio={ratio:.9} target=1.01 target_pass={} temporary_limit=1.02 temporary_pass={}",
                ratio <= 1.01,
                ratio <= 1.02
            );
            assert!(ratio <= 1.02, "{pair:?}");
            pair.check_tail(1.515);
            assert_eq!(build.route.ui.batch_picture.frames as usize, build.frames);
            assert_eq!(build.route.ui.wfm_total - before, (2 * crate::perfguard::ROUNDS * 16 * build.frames) as u64);
            let text = crate::claim_timing::summary();
            assert!(!text.contains("deposit.slice n=0"), "sampled claims must be exercised: {text}");
        }
    }
}
