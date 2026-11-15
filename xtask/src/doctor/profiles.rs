//! Structured selected-profile results contain names only, never process output.
use super::probes::Inventory;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Findings {
    failed: Vec<String>,
}

pub(super) fn check(report: &mut Inventory<'_>, profile: &str, roles: &[String]) {
    let mut names = vec!["docker".to_string()];
    names.extend_from_slice(roles);
    if profile == "image" {
        names.extend(["kernel-source".into(), "qemu".into()]);
    }
    let output = (report.exec)("python3", &["-I", "tools/build_profiles.py", profile, "--json"]);
    let parsed = output.as_ref().ok().and_then(|o| serde_json::from_str::<Findings>(&o.stdout).ok());
    let valid = parsed.as_ref().is_some_and(|f| f.failed.iter().all(|n| names.contains(n)))
        && output.as_ref().is_ok_and(|o| o.ok == parsed.as_ref().is_some_and(|f| f.failed.is_empty()));
    if !valid {
        report.check(
            profile,
            false,
            "valid successful probes",
            "probe unavailable/invalid result",
            &format!("python3 -I tools/build_profiles.py {}", profile),
        );
        return;
    }
    for name in names {
        let good = parsed.as_ref().is_some_and(|f| !f.failed.contains(&name));
        let remedy = match name.as_str() {
            "docker" => "start Docker; see docs/development.md".into(),
            "kernel-source" => "supply kernel/src/linux-6.12.115-base.tar.xz; see kernel/README.md".into(),
            "qemu" => "install qemu-system-aarch64; see docs/development.md".into(),
            _ => format!("tools/build-env.sh prepare {}", name),
        };
        report.check(
            &name,
            good,
            "locked identity and successful probe",
            if good { "matches" } else { "missing/mismatch/probe failed" },
            &remedy,
        );
    }
}
