// SPDX-License-Identifier: GPL-2.0-or-later
//! Crate dependency direction, from `cargo metadata`.

use super::rules::Rules;
use crate::workspace::Metadata;

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

/// Checks every member of the workspace.
pub fn check(meta: &Metadata, rules: &Rules) -> Vec<String> {
    let packages: Vec<(String, Vec<String>)> = meta
        .packages
        .iter()
        .map(|p| (p.name.clone(), p.dependencies.iter().map(|d| d.name.clone()).collect()))
        .collect();
    violations(&packages, rules)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dependency_outside_the_rule_fails() {
        let rules = super::super::rules::parse("crate oos-dsp ->\ncrate oos-ui -> oos-dsp\n").unwrap();
        let p = |n: &str, d: &[&str]| (n.to_string(), d.iter().map(|x| x.to_string()).collect::<Vec<_>>());
        let v = violations(&[p("oos-dsp", &["oos-hal"]), p("oos-ui", &["oos-dsp"]), p("x", &[])], &rules);
        assert_eq!(v, ["oos-dsp may not depend on oos-hal", "x: no crate rule in xtask/data/architecture.txt"]);
    }
}
