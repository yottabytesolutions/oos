// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;

const A53: &str = "devices/system/cpu/cpufreq/policy0";

pub(super) fn policies(tree: &Tree) {
    tree.set(A53, "scaling_available_frequencies", "1200000 408000 1008000 600000");
    tree.set(A53, "scaling_min_freq", "408000");
    tree.set(A53, "scaling_max_freq", "1200000");
    tree.set(CPU, "scaling_available_frequencies", "1800000 408000 1200000 816000 1416000");
    tree.set(CPU, "scaling_max_freq", "1500000");
}

fn a53_min(tree: &Tree) -> String {
    std::fs::read_to_string(tree.0.join(A53).join("scaling_min_freq")).unwrap()
}

#[test]
fn run_selects_each_policys_highest_opp_within_its_maximum() {
    let tree = Tree::new();
    policies(&tree);
    let mut c = tree.controller("");
    for (maximum, expected) in [("1200000", "1200000"), ("1100000", "1008000")] {
        tree.set(A53, "scaling_max_freq", maximum);
        c.update(true);
        assert_eq!(a53_min(&tree), expected);
        assert_eq!(tree.minima(), ("1416000".into(), "600000000".into()));
        let text = c.status().to_string();
        assert!(text.contains(&format!("cpu_floor a53 engaged reason none min_khz={expected}")), "{text}");
        assert!(text.contains("a72 engaged reason none min_khz=1416000"), "{text}");
        c.update(false);
        assert_eq!(a53_min(&tree), "408000");
        assert_eq!(tree.minima(), ("408000".into(), "400000000".into()));
    }
}

#[test]
fn valid_lower_cpu_readback_is_engaged_and_restorable() {
    for (policy, actual, label) in [(CPU, 816000, "a72"), (A53, 1008000, "a53")] {
        let tree = Tree::new();
        policies(&tree);
        let port = Clamping(linux::Sysfs(tree.0.clone()), policy, actual);
        let mut c = Controller::new(Box::new(port), Config::parse(""), Some(GPU), tree.0.join("saved"));
        c.update(true);
        let text = c.status().to_string();
        assert!(text.contains(&format!("{label} engaged reason none min_khz={actual}")), "{text}");
        assert!(!text.contains("not-engaged"), "{text}");
        c.update(false);
        assert_eq!(a53_min(&tree), "408000");
        assert_eq!(tree.minima(), ("408000".into(), "400000000".into()));
    }
}

#[test]
fn cpu_policies_recover_independently_even_when_switched_off() {
    let tree = Tree::new();
    policies(&tree);
    tree.set(A53, "scaling_min_freq", "1200000");
    tree.set(CPU, "scaling_min_freq", "1416000");
    std::fs::write(tree.0.join("saved"), "cpu-a53 408000\ncpu-a72 816000\n").unwrap();
    let c = tree.controller("perf.cpu_floor = off");
    assert_eq!(a53_min(&tree), "408000");
    assert_eq!(tree.minima().0, "816000");
    assert!(!tree.0.join("saved").exists());
    drop(c);
    let mut c = tree.controller("");
    c.update(true);
    let receipt = std::fs::read_to_string(tree.0.join("saved")).unwrap();
    assert!(receipt.contains("cpu-a53 408000\n"), "{receipt}");
    assert!(receipt.contains("cpu-a72 816000\n"), "{receipt}");
    c.restore();
    assert_eq!(a53_min(&tree), "408000");
    assert_eq!(tree.minima().0, "816000");
}

#[test]
fn invalid_or_higher_readback_disables_only_the_affected_policy() {
    for actual in [0, 123000, 1800000] {
        let tree = Tree::new();
        policies(&tree);
        let port = Clamping(linux::Sysfs(tree.0.clone()), CPU, actual);
        let mut c = Controller::new(Box::new(port), Config::parse(""), Some(GPU), tree.0.join("saved"));
        c.update(true);
        let text = c.status().to_string();
        assert!(text.contains("a72 disabled reason not-engaged"), "{text}");
        assert!(text.contains("a53 engaged reason none min_khz=1200000"), "{text}");
        assert!(text.contains("gpu_floor engaged reason none"), "{text}");
        c.restore();
        assert_eq!(a53_min(&tree), "408000");
        assert_eq!(tree.minima(), ("408000".into(), "400000000".into()));
    }
}

#[test]
fn both_policies_yield_to_thermal_guard_and_restore_on_drop() {
    let tree = Tree::new();
    policies(&tree);
    let clock = ManualClock::new();
    let mut c = tree.controller_with_clock(&clock);
    c.update(true);
    tree.set("class/thermal/thermal_zone7", "temp", "65000");
    clock.advance(Duration::from_secs(1));
    c.update(true);
    assert_eq!(a53_min(&tree), "408000");
    assert_eq!(tree.minima().0, "408000");
    let text = c.status().to_string();
    for policy in ["a53", "a72"] {
        assert!(text.contains(&format!("{policy} disabled reason thermal-limit")), "{text}");
    }
    tree.set("class/thermal/thermal_zone7", "temp", "59999");
    clock.advance(Duration::from_secs(1));
    c.update(true);
    assert_eq!(a53_min(&tree), "1200000");
    assert_eq!(tree.minima().0, "1416000");
    drop(c);
    assert_eq!(a53_min(&tree), "408000");
    assert_eq!(tree.minima().0, "408000");
    assert!(!tree.0.join("saved").exists());
}

#[test]
fn cpu_optout_and_missing_policy_do_not_affect_other_devices() {
    let tree = Tree::new();
    policies(&tree);
    let mut c = tree.controller("perf.cpu_floor = off");
    c.update(true);
    assert_eq!(a53_min(&tree), "408000");
    assert_eq!(tree.minima(), ("408000".into(), "600000000".into()));
    drop(c);
    std::fs::remove_file(tree.0.join(A53).join("scaling_available_frequencies")).unwrap();
    let mut c = tree.controller("");
    c.update(true);
    assert_eq!(a53_min(&tree), "408000");
    assert_eq!(tree.minima(), ("1416000".into(), "600000000".into()));
    assert!(c.status().to_string().contains("a53 disabled reason device-unavailable"));
}
