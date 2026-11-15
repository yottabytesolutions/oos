#[test]
fn cli_accepts_only_doctor_profiles() {
    let args = ["doctor", "--gpu", "--sim", "--image"].map(str::to_string);
    assert!(crate::parse(&args).is_ok());
    assert!(crate::parse(&["doctor".into(), "--install".into()]).is_err());
}

#[test]
fn full_preflight_is_separate_from_fast_syntax_checks() {
    assert!(crate::check::plan(crate::check::Options { fast: true, coverage: true })
        .iter()
        .all(|(name, _)| *name != "doctor"));
}

use super::{pins, probes};
use crate::run::Output;

fn good(program: &str, args: &[&str]) -> Result<Output, String> {
    let stdout = match (program, args) {
        ("rustc", ["--version"]) => "rustc 1.99.1",
        ("cargo", ["--version"]) => "cargo 1.99.1",
        ("cargo", ["llvm-cov", "--version"]) => "cargo-llvm-cov 0.9.1",
        ("python3", ["--version"]) => "Python 3.14.10",
        ("rustup", ["show", "active-toolchain"]) => "1.99.1-aarch64-apple-darwin (overridden)",
        ("rustup", ["component", "list", "--installed"]) => {
            "rustfmt-aarch64-apple-darwin\nclippy-aarch64-apple-darwin\nllvm-tools-aarch64-apple-darwin"
        }
        ("rustup", ["target", "list", "--installed"]) => "aarch64-unknown-linux-musl",
        ("rustc", ["--print", "sysroot"]) => "/path/to/sysroot",
        ("rustc", ["-vV"]) => "host: aarch64-apple-darwin\nrelease: 1.99.1",
        ("git", ["config", "core.hooksPath"]) => ".githooks",
        ("rustfmt", _) | ("cargo", ["clippy", "--version"]) => "component version",
        (path, ["--version"]) if path.ends_with("rustfmt") || path.ends_with("clippy-driver") => "component version",
        (path, ["-flavor", "gnu", "--version"]) if path.ends_with("rust-lld") => "LLD 23.1.1",
        (path, ["--version"]) if path.ends_with("rust-lld") => return Err("requires flavor".into()),
        _ => "available",
    };
    Ok(Output { ok: true, stdout: stdout.into(), stderr: String::new() })
}

#[test]
fn correct_toolchain_passes_without_path_lld() {
    let pins = pins::load(&crate::repo::root()).expect("pins");
    let calls = std::cell::RefCell::new(Vec::new());
    let exec = |program: &str, args: &[&str]| {
        calls.borrow_mut().push(format!("{} {}", program, args.join(" ")));
        good(program, args)
    };
    let mut inventory = probes::Inventory::new(&exec);
    inventory.base(&pins);
    assert!(!inventory.bad, "{:?}", inventory.lines);
    for forbidden in ["install", "pull", "binfmt", "sign", "scope", "ssh", "docker", "adb"] {
        assert!(calls.borrow().iter().all(|call| !call.split_whitespace().any(|word| word == forbidden)));
    }
    assert!(calls.borrow().iter().all(|call| !call.starts_with("rust-lld ")));
}

#[test]
fn python_minimum_accepts_newer_versions_and_reports_actual() {
    let pins = pins::load(&crate::repo::root()).expect("pins");
    for (actual, accepted) in
        [("3.11.9", false), ("3.12.0", true), ("3.12.9", true), ("3.14.10", true), ("4.0.0", true), ("3.bad.0", false)]
    {
        let exec = |program: &str, args: &[&str]| {
            let mut out = good(program, args)?;
            if program == "python3" && args == ["--version"] {
                out.stdout = format!("Python {}", actual);
            }
            Ok(out)
        };
        let mut inventory = probes::Inventory::new(&exec);
        inventory.base(&pins);
        assert_eq!(!inventory.bad, accepted, "{}", actual);
        let line = inventory.lines.iter().find(|l| l.contains("python3:")).expect("Python report");
        assert!(line.contains("expected >=3.12"));
        if accepted {
            assert!(line.contains(&format!("actual {}", actual)));
        }
    }
}

#[test]
fn independent_failures_and_unsuccessful_version_output_are_reported_without_private_data() {
    let pins = pins::load(&crate::repo::root()).expect("pins");
    let exec = |program: &str, args: &[&str]| {
        let mut out = good(program, args)?;
        out.ok = false;
        out.stderr = "/path/to/private credentials must not enter diagnostics".into();
        Ok(out)
    };
    let mut inventory = probes::Inventory::new(&exec);
    inventory.base(&pins);
    assert!(inventory.bad);
    for name in [
        "rustc:",
        "cargo:",
        "cargo-llvm-cov:",
        "python3:",
        "active toolchain:",
        "musl std:",
        "rust-lld:",
        "llvm-cov:",
        "llvm-profdata:",
        "writable isolated output:",
    ] {
        assert!(inventory.lines.iter().any(|l| l.starts_with(&format!("FAIL {}", name))), "{}", name);
    }
    assert!(!inventory.lines.join("\n").contains("private"));
}

#[test]
fn wrong_versions_components_and_target_fail_independently() {
    let pins = pins::load(&crate::repo::root()).expect("pins");
    let exec = |program: &str, args: &[&str]| {
        let mut out = good(program, args)?;
        if matches!(program, "rustc" | "cargo" | "python3" | "rustup" | "git") {
            out.stdout = "wrong 0.0.1".into();
        }
        Ok(out)
    };
    let mut inventory = probes::Inventory::new(&exec);
    inventory.base(&pins);
    assert!(inventory.bad);
    assert!(inventory.lines.iter().filter(|l| l.starts_with("FAIL")).count() >= 10);
}

#[test]
fn hooks_mismatch_warns_without_failing_or_disclosing_value() {
    let pins = pins::load(&crate::repo::root()).expect("pins");
    for hooks in [Some("/path/to/private/hooks"), None] {
        let exec = |program: &str, args: &[&str]| {
            if program == "git" && args == ["config", "core.hooksPath"] {
                return hooks
                    .map(|s| Output { ok: true, stdout: s.into(), stderr: String::new() })
                    .ok_or_else(|| "unset".into());
            }
            good(program, args)
        };
        let mut inventory = probes::Inventory::new(&exec);
        inventory.base(&pins);
        assert!(!inventory.bad, "{:?}", inventory.lines);
        assert!(inventory.lines.iter().any(|l| l.starts_with("WARN hooks:")));
        assert!(!inventory.lines.join("\n").contains("private"));
    }
}

#[test]
fn override_identity_must_match_the_pin_even_when_rustc_version_matches() {
    let pins = pins::load(&crate::repo::root()).expect("pins");
    for active in [
        "stable-aarch64-apple-darwin (directory override for '/path/to/private')",
        "1.98.0-aarch64-apple-darwin (directory override for '/path/to/private')",
        "1.99.1-custom (directory override for '/path/to/private')",
        "1.99.1-aarch64-apple-darwin-custom (directory override for '/path/to/private')",
    ] {
        let exec = |program: &str, args: &[&str]| {
            let mut out = good(program, args)?;
            if program == "rustup" && args == ["show", "active-toolchain"] {
                out.stdout = active.into();
            }
            Ok(out)
        };
        let mut inventory = probes::Inventory::new(&exec);
        inventory.base(&pins);
        assert!(inventory.bad, "accepted {active}");
        assert!(inventory.lines.iter().any(|l| l.starts_with("FAIL active toolchain:")));
        assert!(!inventory.lines.join("\n").contains("private"));
    }
}

#[test]
fn floating_duplicate_and_malformed_rust_pins_fail() {
    assert!(pins::table("[toolchain]\nchannel = \"stable\"\nchannel = \"1.99.1\"", "toolchain").is_err());
    assert!(pins::table("[toolchain]\nchannel = broken", "toolchain").is_err());
}

#[test]
fn missing_repository_prerequisites_are_all_visible() {
    let root = crate::testrepo::repo("doctor-missing", &[]);
    std::fs::create_dir(root.join("tools")).expect("tools");
    for name in ["rust-toolchain.toml", ".python-version", "tools/build-tools.json"] {
        std::fs::copy(crate::repo::root().join(name), root.join(name)).expect("pin fixture");
    }
    let lines = super::inventory(&root, &[]).expect_err("missing prerequisites");
    for name in ["Cargo.lock", "protected base", "musl configuration", "writable isolated output"] {
        assert!(lines.iter().any(|l| l.starts_with(&format!("FAIL {}:", name))), "{}", name);
    }
    assert!(!lines.join("\n").contains(root.to_str().expect("path")));
    std::fs::remove_dir_all(root).expect("remove private fixture");
}

#[test]
fn floating_toolchain_and_mismatched_python_pin_are_rejected() {
    let root = crate::testrepo::repo("doctor-pins", &[]);
    std::fs::create_dir(root.join("tools")).expect("tools");
    for name in ["rust-toolchain.toml", ".python-version", "tools/build-tools.json"] {
        std::fs::copy(crate::repo::root().join(name), root.join(name)).expect("pin fixture");
    }
    let path = root.join("rust-toolchain.toml");
    let original = std::fs::read_to_string(&path).expect("pin");
    std::fs::write(&path, original.replace("1.99.1", "stable")).expect("floating pin");
    assert!(pins::load(&root).is_err());
    std::fs::write(&path, original).expect("restore pin");
    std::fs::write(root.join(".python-version"), "3.14.9").expect("mismatched pin");
    assert!(pins::load(&root).is_err());
    std::fs::remove_dir_all(root).expect("remove private fixture");
}

#[test]
fn selected_profile_reports_each_failure_and_refuses_successful_output_with_failure_status() {
    let exec = |_: &str, _: &[&str]| {
        Ok(Output {
            ok: false,
            stdout: r#"{"failed":["docker","gpu-test"]}"#.into(),
            stderr: "/path/to/private".into(),
        })
    };
    let mut report = probes::Inventory::new(&exec);
    super::profiles::check(&mut report, "gpu", &["gpu-test".into()]);
    assert!(report.bad);
    assert_eq!(report.lines.len(), 2);
    assert!(report.lines.iter().all(|l| l.starts_with("FAIL")));
    assert!(!report.lines.join("\n").contains("private"));
    let exec = |_: &str, _: &[&str]| Ok(Output { ok: false, stdout: r#"{"failed":[]}"#.into(), stderr: String::new() });
    let mut report = probes::Inventory::new(&exec);
    super::profiles::check(&mut report, "gpu", &["gpu-test".into()]);
    assert!(report.bad);
}
