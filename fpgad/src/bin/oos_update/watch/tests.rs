// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use oos_hal::update::Entry;

const BUILD: &str = "test-build";
const SHA: [u8; 32] = [1; 32];

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Verdict,
    Confirm([u8; 32]),
    Arm,
    Rollback([u8; 32], String),
    Unhealthy(String),
    Marker(String),
}

struct Fake {
    verdict: Option<Outcome>,
    arm_ok: bool,
    calls: Vec<Call>,
    marker_path: Option<std::path::PathBuf>,
}

impl Ports for Fake {
    fn verdict(&mut self) -> Outcome {
        self.calls.push(Call::Verdict);
        self.verdict.take().unwrap()
    }

    fn confirm(&mut self, _: &State, sha: [u8; 32]) {
        self.calls.push(Call::Confirm(sha));
    }

    fn arm(&mut self) -> bool {
        self.calls.push(Call::Arm);
        self.arm_ok
    }

    fn rollback(&mut self, _: &State, sha: [u8; 32], why: &str) {
        self.calls.push(Call::Rollback(sha, why.into()));
    }

    fn unhealthy(&mut self, why: &str) {
        self.calls.push(Call::Unhealthy(why.into()));
    }

    fn publish(&mut self, marker: &str) {
        self.calls.push(Call::Marker(marker.into()));
        if let Some(path) = &self.marker_path {
            publish_to(path, marker).unwrap();
        }
    }
}

fn tracked_trial() -> State {
    let mut state = State::default();
    state.current = Some(Entry { build: BUILD.into(), sha256: SHA, len: 1 });
    state.previous = Some(Entry { build: "previous-build".into(), sha256: [2; 32], len: 1 });
    state.trial = Some(1);
    state.persist = true;
    state
}

fn calls(state: &State, running: &str, verdict: Option<Outcome>, arm_ok: bool) -> Vec<Call> {
    let kexec = verdict.is_none();
    let mut ports = Fake { verdict, arm_ok, calls: Vec::new(), marker_path: None };
    watch(&mut ports, state, kexec, Some(running));
    ports.calls
}

#[test]
fn confirms_the_trial_then_arms_then_publishes() {
    assert_eq!(
        calls(&tracked_trial(), BUILD, Some(Outcome::Good), true),
        [Call::Verdict, Call::Confirm(SHA), Call::Arm, Call::Marker("persist".into())]
    );
}

#[test]
fn already_confirmed_build_only_needs_an_arm() {
    let mut state = tracked_trial();
    state.trial = None;
    assert_eq!(
        calls(&state, BUILD, Some(Outcome::Good), true),
        [Call::Verdict, Call::Arm, Call::Marker("persist".into())]
    );
}

#[test]
fn an_untracked_start_only_needs_an_arm_in_persist_mode() {
    let mut state = State::default();
    state.persist = true;
    assert_eq!(
        calls(&state, BUILD, Some(Outcome::Good), true),
        [Call::Verdict, Call::Arm, Call::Marker("persist".into())]
    );
    assert_eq!(
        calls(&tracked_trial(), "other-build", Some(Outcome::Good), false),
        [Call::Verdict, Call::Arm, Call::Marker("stock".into())]
    );
}

#[test]
fn one_shot_confirms_but_skips_the_arm() {
    let mut state = tracked_trial();
    state.persist = false;
    assert_eq!(
        calls(&state, BUILD, Some(Outcome::Good), true),
        [Call::Verdict, Call::Confirm(SHA), Call::Marker("oneshot".into())]
    );
}

#[test]
fn kexec_skips_the_verdict_and_only_arms_persist_without_a_trial() {
    let mut state = tracked_trial();
    assert_eq!(calls(&state, BUILD, None, true), [Call::Marker("kexec".into())]);
    state.trial = None;
    assert_eq!(calls(&state, BUILD, None, true), [Call::Arm, Call::Marker("persist".into())]);
    assert_eq!(calls(&state, BUILD, None, false), [Call::Arm, Call::Marker("kexec".into())]);
    state.persist = false;
    assert_eq!(calls(&state, BUILD, None, true), [Call::Marker("kexec".into())]);
}

#[test]
fn rollback_finishes_before_publication() {
    let state = tracked_trial();
    let bad = || Some(Outcome::Bad("UI crashed".into()));
    assert_eq!(
        calls(&state, BUILD, bad(), true),
        [Call::Verdict, Call::Rollback(SHA, "UI crashed".into()), Call::Marker("stock".into())]
    );
    assert_eq!(
        calls(&state, "other-build", bad(), true),
        [Call::Verdict, Call::Unhealthy("UI crashed".into()), Call::Marker("stock".into())]
    );
}

#[test]
fn deliberate_ui_stop_leaves_the_trial_open_and_only_publishes() {
    assert_eq!(
        calls(&tracked_trial(), BUILD, Some(Outcome::UiOff), true),
        [Call::Verdict, Call::Marker("ui-off".into())]
    );
}

#[test]
fn failed_arm_publishes_with_boot_selection_unverified() {
    assert_eq!(
        calls(&tracked_trial(), BUILD, Some(Outcome::Good), false),
        [Call::Verdict, Call::Confirm(SHA), Call::Arm, Call::Marker("stock".into())]
    );
}

#[test]
fn judgment_replaces_the_marker_without_leaving_the_staging_file() {
    let directory = crate::oos_update::test_support::TempDir::new();
    let path = directory.path().join("judged");
    std::fs::write(&path, "old verdict").unwrap();
    let mut ports =
        Fake { verdict: Some(Outcome::UiOff), arm_ok: false, calls: Vec::new(), marker_path: Some(path.clone()) };
    watch(&mut ports, &tracked_trial(), false, Some(BUILD));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "ui-off");
    assert!(!path.with_extension("new").exists());
}
