// SPDX-License-Identifier: GPL-2.0-or-later
use super::Fixture;
use crate::repo::CRATES;
use crate::{ratchet::Ratchet, size::Measured};
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn concurrent_checks_share_results_without_sharing_mutations() {
    let fixture = Fixture::new();
    let ctx = fixture.start();
    let calls = AtomicUsize::new(0);
    let barrier = std::sync::Barrier::new(5);
    std::thread::scope(|scope| {
        for _ in 0..5 {
            scope.spawn(|| {
                barrier.wait();
                let mut result = ctx
                    .source_measurements(&fixture.0, || {
                        calls.fetch_add(1, Ordering::Relaxed);
                        Ok(vec![(
                            CRATES[0],
                            Measured {
                                sizes: Ratchet::parse("61 fn hal/src/example.rs check\n").unwrap(),
                                ..Default::default()
                            },
                        )])
                    })
                    .unwrap();
                assert_eq!(result[0].1.sizes.0.values().copied().collect::<Vec<_>>(), [61]);
                result[0].1.sizes.0.clear();
            });
        }
    });
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}

#[test]
fn a_source_failure_reaches_every_check_but_is_retried_in_the_next_run() {
    let fixture = Fixture::new();
    let ctx = fixture.start();
    assert_eq!(
        ctx.source_measurements(&fixture.0, || Err("invalid Rust source".into())).unwrap_err(),
        "invalid Rust source"
    );
    assert_eq!(
        ctx.source_measurements(&fixture.0, || panic!("must retain the source error")).unwrap_err(),
        "invalid Rust source"
    );
    assert!(fixture.start().source_measurements(&fixture.0, || Ok(Vec::new())).unwrap().is_empty());
}

#[test]
fn ad_hoc_contexts_and_other_roots_always_read_fresh_sources() {
    let fixture = Fixture::new();
    let ctx = fixture.start();
    ctx.source_measurements(&fixture.0, || Ok(Vec::new())).unwrap();
    let calls = AtomicUsize::new(0);
    let ad_hoc = crate::run::Context::default();
    for (context, root) in [(&ad_hoc, fixture.0.as_path()), (&ctx, std::path::Path::new("/path/to/other"))] {
        for _ in 0..2 {
            context
                .source_measurements(root, || {
                    calls.fetch_add(1, Ordering::Relaxed);
                    Err("fresh source error".into())
                })
                .unwrap_err();
        }
    }
    assert_eq!(calls.load(Ordering::Relaxed), 4);
}
