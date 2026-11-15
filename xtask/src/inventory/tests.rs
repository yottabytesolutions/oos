use super::*;
use crate::check::Options;

fn ids(fast: bool, coverage: bool, image: bool) -> Vec<&'static str> {
    CHECKS.iter().filter(|c| selected(c, Options { fast, coverage }, image)).map(|c| c.id).collect()
}

#[test]
fn default_gate_never_selects_docker_or_image_prerequisites() {
    assert_eq!(
        ids(false, true, false),
        vec![
            "fmt",
            "size",
            "architecture",
            "mutable-statics",
            "unsafe",
            "method-exceptions",
            "known-failures",
            "agent-rules",
            "doctor-base",
            "clippy",
            "tests-debug",
            "tests-release",
            "perf",
            "coverage",
            "script-syntax",
            "python-tests"
        ]
    );
}

#[test]
fn default_doctor_uses_only_base_probes() {
    let check = CHECKS.iter().find(|c| c.id == "doctor-base").expect("required base doctor");
    assert_eq!(check.selection, Selection::Host);
    assert!(std::ptr::fn_addr_eq(check.run, crate::doctor::check as Runner));
    assert!(ids(false, false, false).contains(&check.id));
    assert!(!ids(true, true, false).contains(&check.id));
}

#[test]
fn image_tier_adds_every_image_check_to_the_default_gate() {
    let image = ids(false, true, true);
    for id in ids(false, true, false) {
        assert!(image.contains(&id), "image tier omitted default check {id}");
    }
    for id in ["doctor", "arm64-release-link", "gpu-host", "ssh-sim", "firewall-sim", "image-required", "image-suite"] {
        let check = CHECKS.iter().find(|c| c.id == id).expect("stable image ID");
        assert_eq!(check.selection, Selection::Image, "{id} must require --image");
        assert!(image.contains(&id));
    }
}

#[test]
fn host_failures_are_part_of_the_gate() {
    let selected = ids(false, true, false);
    for id in ["script-syntax", "python-tests"] {
        assert!(selected.contains(&id), "missing mandatory check {id}");
    }
}

#[test]
fn selections_preserve_the_fast_prefix_and_conditional_checks() {
    let fast = ids(true, true, false);
    assert_eq!(
        fast,
        vec![
            "fmt",
            "size",
            "architecture",
            "mutable-statics",
            "unsafe",
            "method-exceptions",
            "known-failures",
            "agent-rules"
        ]
    );
    let full = ids(false, true, false);
    assert_eq!(&full[..fast.len()], fast);
    assert!(full.contains(&"coverage") && !full.contains(&"doctor"));
    assert!(!ids(false, false, false).contains(&"coverage"));
    assert!(!full.contains(&"image-suite"));
    assert!(ids(false, true, true).contains(&"image-suite"));
    assert_eq!(ids(true, false, false), fast);
}

#[test]
fn registry_ids_are_unique_and_prerequisites_precede_consumers() {
    let mut seen = std::collections::BTreeSet::new();
    for c in CHECKS {
        for dependency in c.after {
            assert!(seen.contains(dependency));
        }
        assert!(seen.insert(c.id));
        assert!(!c.command.is_empty() && !c.prerequisite.is_empty());
    }
    let image = ids(false, true, true);
    assert!(image.iter().position(|id| *id == "arm64-release-link") < image.iter().position(|id| *id == "gpu-host"));
}

#[test]
fn subprocess_status_is_independent_of_pass_text() {
    let root = crate::repo::root();
    let fail = steps::host::command(&Default::default(), &root, "sh", &["-c", "echo PASS; exit 7"]);
    assert!(fail.is_err());
    assert!(fail.expect_err("failed subprocess")[0].contains("PASS"));
    assert!(steps::host::command(&Default::default(), &root, "oos-nonexistent-fixture-command", &[]).is_err());
    assert!(steps::host::command(&Default::default(), &root, "sh", &["-c", "echo FAIL; exit 0"]).is_ok());
}

#[test]
fn image_cannot_select_a_partial_gate() {
    for args in [vec!["check", "--image", "--fast"], vec!["check", "--image", "--no-coverage"]] {
        assert!(crate::parse(&args.into_iter().map(String::from).collect::<Vec<_>>()).is_err());
    }
    for args in [vec!["check", "--list"], vec!["check", "--image"], vec!["check", "--list", "--fast"]] {
        assert!(crate::parse(&args.into_iter().map(String::from).collect::<Vec<_>>()).is_ok());
    }
}
