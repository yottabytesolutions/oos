//! Crate dependency direction, from `cargo metadata`.

use super::rules::Rules;
use crate::run;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct Metadata {
    packages: Vec<Package>,
}

#[derive(Debug, Deserialize)]
struct Package {
    name: String,
    dependencies: Vec<Dependency>,
}

#[derive(Debug, Deserialize)]
struct Dependency {
    name: String,
}

/// The package name and its direct dependencies, from the metadata JSON of one crate.
fn direct(json: &str) -> Result<Vec<(String, Vec<String>)>, String> {
    let m: Metadata = serde_json::from_str(json).map_err(|e| format!("cargo metadata: {}", e))?;
    Ok(m.packages.into_iter().map(|p| (p.name, p.dependencies.into_iter().map(|d| d.name).collect())).collect())
}

/// Violations of the crate rules by `packages`: a package without a rule, or a dependency its rule does not allow.
fn violations(packages: &[(String, Vec<String>)], rules: &Rules) -> Vec<String> {
    let mut out = Vec::new();
    for (name, deps) in packages {
        let Some(allowed) = rules.crates.get(name) else {
            out.push(format!("{}: no crate rule in xtask/data/architecture.txt", name));
            continue;
        };
        out.extend(deps.iter().filter(|d| !allowed.contains(*d)).map(|d| format!("{} may not depend on {}", name, d)));
    }
    out
}

/// Checks the crate in `root/krate`.
pub fn check(root: &Path, krate: &str, rules: &Rules) -> Result<Vec<String>, String> {
    let out = run::capture(&root.join(krate), "cargo", &["metadata", "--format-version", "1", "--no-deps"])?;
    if !out.ok {
        return Err(run::tail(&out.stderr, 10));
    }
    Ok(violations(&direct(&out.stdout)?, rules))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dependency_outside_the_rule_fails() {
        let rules = super::super::rules::parse("crate oos-dsp ->\ncrate oos-ui -> oos-dsp\n").unwrap();
        let json = r#"{"packages":[{"name":"oos-dsp","dependencies":[{"name":"oos-hal"}]},
            {"name":"oos-ui","dependencies":[{"name":"oos-dsp"}]},{"name":"x","dependencies":[]}]}"#;
        let v = violations(&direct(json).unwrap(), &rules);
        assert_eq!(v, ["oos-dsp may not depend on oos-hal", "x: no crate rule in xtask/data/architecture.txt"]);
    }
}
