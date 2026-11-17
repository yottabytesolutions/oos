// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
fn scan(text: &str, test: bool) -> Ratchet {
    findings(&sources::Source {
        rel: "p/src/a.rs".into(),
        module: String::new(),
        test,
        text: text.into(),
        file: syn::parse_file(text).unwrap(),
    })
}
#[test]
fn finds_inline_external_and_impl_test_context_without_flagging_production() {
    let body = "fn f() { loop { if start.elapsed() > limit { break; } std::thread::sleep(gap); } }";
    assert!(scan(body, false).0.is_empty());
    assert_eq!(scan(body, true).0.values().sum::<u64>(), 2);
    assert_eq!(scan(&format!("#[cfg(test)] mod tests {{ {body} }}"), false).0.values().sum::<u64>(), 2);
    assert_eq!(scan("#[test] fn f() { while Instant::now() < deadline {} }", false).0.values().sum::<u64>(), 1);
    assert_eq!(scan("#[cfg(test)] impl S { fn f() { sleep(gap); } }", false).0.values().sum::<u64>(), 1);
}
#[test]
fn manual_time_and_completion_events_are_clean() {
    assert!(scan(
        "#[test] fn f() { let now = Instant::now(); for n in 0..5 { step(now + n * gap); } rx.recv().unwrap(); }",
        false
    )
    .0
    .is_empty());
}
#[test]
fn comments_and_string_fixtures_are_not_executed_test_code() {
    assert!(scan(
        r##"#[test] fn f() { let source = "loop { thread::sleep(d); start.elapsed(); }"; /* thread::sleep(d); */ }"##,
        false
    )
    .0
    .is_empty());
}

#[test]
fn cfg_all_and_renamed_sleep_and_for_deadlines_are_checked() {
    let found = scan("#[cfg(all(test, unix))] mod tests { use std::thread::sleep as pause; fn f() { pause(gap); for n in 0..10 { assert!(start.elapsed() < limit); } } }", false);
    assert_eq!(found.0.values().sum::<u64>(), 2);
}
#[test]
fn the_test_time_ratchet_cannot_grow_or_gain_an_entry() {
    let baseline = Ratchet::parse("2 p/src/a.rs test sleep\n").unwrap();
    let reduced = Ratchet::parse("1 p/src/a.rs test sleep\n").unwrap();
    assert!(baseline.compare(&reduced).failures().is_empty());
    assert!(!crate::base::widened(&reduced, &baseline).is_empty());
    assert!(!baseline.compare(&Ratchet::parse("1 p/src/new.rs test sleep\n").unwrap()).failures().is_empty());
}

#[test]
fn clock_comparisons_are_found_outside_loops_and_through_bindings() {
    for body in [
        "assert!(start.elapsed() < limit);",
        "assert!(start.elapsed().is_zero());",
        "assert!(ready || start.elapsed().is_zero());",
        "let elapsed = start.elapsed(); assert!(elapsed.is_zero());",
        "assert_eq!(start.elapsed(), Duration::ZERO);",
        "assert_ne!(Instant::now(), deadline);",
        "let elapsed = start.elapsed(); assert_eq!(elapsed, limit);",
        "if Instant::now() >= deadline { done(); }",
        "let now = Instant::now(); if now < deadline { done(); }",
        "let elapsed = start.elapsed(); assert!(elapsed.as_millis() < 100);",
    ] {
        assert_eq!(scan(&format!("#[test] fn f() {{ {body} }}"), false).0.values().sum::<u64>(), 1, "{body}");
    }
}

#[test]
fn timeout_calls_require_a_long_hang_guard_including_bound_durations() {
    for body in [
        "rx.recv_timeout(Duration::from_secs(1));",
        "const HANG_GUARD: Duration = Duration::from_secs(30); rx.recv_timeout(HANG_GUARD);",
        "cv.wait_timeout(guard, Duration::from_millis(500));",
        "thread::park_timeout(Duration::from_secs(59));",
        "thread::park_timeout(Duration::from_secs(119));",
        "let wait = Duration::from_secs(2); rx.recv_timeout(wait);",
        "socket.set_read_timeout(Some(Duration::from_secs(2)));",
    ] {
        assert_eq!(scan(&format!("#[test] fn f() {{ {body} }}"), false).0.values().sum::<u64>(), 1, "{body}");
    }
    for body in ["rx.recv_timeout(HANG_GUARD);", "rx.recv_timeout(Duration::from_secs(120));"] {
        assert!(scan(&format!("#[test] fn f() {{ {body} }}"), false).0.is_empty());
    }
}

#[test]
fn macro_bodies_and_cfg_statements_do_not_hide_test_sleeps() {
    for body in [
        "#[cfg(test)] { thread::sleep(gap); }",
        "#[cfg(test)] thread::sleep(gap);",
        "#[cfg(test)] unsafe { thread::sleep(gap); }",
        "#[cfg(test)] async { thread::sleep(gap); };",
        "#[cfg(test)] let value = { thread::sleep(gap); 1 };",
        "#[cfg(test)] assert!({ thread::sleep(gap); true });",
        "#[cfg(test)] custom! { thread::sleep(gap); }",
        "#[cfg(test)] macro_rules! delay { () => { thread::sleep(gap); }; }",
    ] {
        assert_eq!(scan(&format!("fn f() {{ {body} }}"), false).0.values().sum::<u64>(), 1, "{body}");
    }
}

#[test]
fn only_named_wait_helpers_are_exempt() {
    let source = "fn new() { thread::sleep(gap); } fn unrelated() { thread::sleep(gap); }";
    let found = findings(&sources::Source {
        rel: "ui/src/ui/lifecycle/test_wait.rs".into(),
        module: String::new(),
        test: true,
        text: source.into(),
        file: syn::parse_file(source).unwrap(),
    });
    assert_eq!(found.0.len(), 1);
    assert!(found.0.keys().next().unwrap().contains("unrelated sleep"));
}

#[test]
fn a_virtual_clock_fork_in_a_production_function_is_rejected() {
    let found = scan("fn settle() { #[cfg(test)] { let now = Cell::new(Instant::now()); settle_with(|| now.get(), wait_worker); } #[cfg(not(test))] settle_with(Instant::now, sleep_one_ms); }", false);
    assert_eq!(found.0.values().sum::<u64>(), 1);
    assert!(found.0.keys().next().unwrap().ends_with("test-clock-fork"));
}

#[test]
fn timestamp_provenance_does_not_exempt_sleeps_in_the_same_test() {
    let text = "mod tests { #[test] fn partial_merges_and_picture_age_are_counted() { let before = Instant::now(); assert!(timestamp >= before); thread::sleep(gap); } }";
    let found = findings(&sources::Source {
        rel: "ui/src/drawpipe.rs".into(),
        module: String::new(),
        test: false,
        text: text.into(),
        file: syn::parse_file(text).unwrap(),
    });
    assert_eq!(found.0.len(), 1);
    assert!(found.0.keys().next().unwrap().ends_with("sleep"));
}
