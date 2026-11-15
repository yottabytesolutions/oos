//! Build prerequisite inventory; never installs anything.
mod pins;
mod probes;
mod profiles;
#[cfg(test)]
mod tests;

use std::path::Path;

pub(crate) fn syntax(root: &Path) -> crate::steps::Outcome {
    pins::check(root)
}

pub(crate) fn check(root: &Path) -> crate::steps::Outcome {
    inventory(root, &[])
}

pub(crate) fn inventory(root: &Path, flags: &[&str]) -> crate::steps::Outcome {
    let pins = pins::load(root).map_err(|e| vec![e])?;
    let exec = |program: &str, args: &[&str]| crate::run::capture(root, program, args);
    let mut report = probes::Inventory::new(&exec);
    report.base(&pins);
    report.check(
        "Cargo.lock",
        root.join("Cargo.lock").is_file(),
        "committed lockfile",
        if root.join("Cargo.lock").is_file() { "present" } else { "missing" },
        "restore Cargo.lock; cargo fetch --locked",
    );
    report.check(
        "protected base",
        crate::base::protected(root).is_ok(),
        "G1 protected base",
        if crate::base::protected(root).is_ok() { "available" } else { "unavailable" },
        "git fetch origin overhaul",
    );
    let config = std::fs::read_to_string(root.join(".cargo/config.toml")).unwrap_or_default();
    let table = pins::table(&config, "target.aarch64-unknown-linux-musl").unwrap_or_default();
    let good = table.get("linker") == Some(&serde_json::json!("rust-lld"))
        && table.get("rustflags")
            == Some(&serde_json::json!(["-C", "target-feature=+crt-static", "-C", "target-cpu=cortex-a72"]));
    report.check(
        "musl configuration",
        good,
        "rust-lld, static, Cortex-A72",
        if good { "matches" } else { "mismatch" },
        "restore .cargo/config.toml",
    );
    for profile in ["gpu", "sim", "image"] {
        if flags.contains(&"--image") || flags.contains(&format!("--{}", profile).as_str()) {
            profiles::check(&mut report, profile, &pins.tools.profiles[profile]);
        } else {
            report.lines.push(format!("{}: not checked (select --{})", profile, profile));
        }
    }
    crate::steps::outcome(report.lines.clone(), if report.bad { report.lines } else { Vec::new() })
}

pub(crate) fn run(root: &Path, flags: &[&str]) -> bool {
    let step = crate::report::step("doctor", || inventory(root, flags));
    step.ok
}
