//! The workspace from `cargo metadata`: its members, their dependencies and their cargo targets.

use crate::repo::CRATES;
use crate::run;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Metadata {
    pub packages: Vec<Package>,
    pub workspace_root: String,
}

#[derive(Debug, Deserialize)]
pub struct Package {
    pub name: String,
    pub manifest_path: String,
    pub dependencies: Vec<Dependency>,
    pub targets: Vec<Target>,
}

#[derive(Debug, Deserialize)]
pub struct Dependency {
    /// the package name
    pub name: String,
    /// the name in the manifest when it differs (`hal = { package = "oos-hal" }`)
    #[serde(default)]
    pub rename: Option<String>,
}

impl Dependency {
    /// The crate's name in code: the rename if any, `-` as `_` (`hal`, `oos_hal`).
    pub fn code_name(&self) -> String {
        self.rename.as_deref().unwrap_or(&self.name).replace('-', "_")
    }

    /// The canonical name rules use: the package name, `-` as `_` (`oos_hal`).
    pub fn canonical(&self) -> String {
        self.name.replace('-', "_")
    }
}

#[derive(Debug, Deserialize)]
pub struct Target {
    pub kind: Vec<String>,
    pub src_path: String,
}

impl Target {
    /// Test code as a whole: integration tests, benches and examples.
    pub fn is_test(&self) -> bool {
        self.kind.iter().any(|k| matches!(k.as_str(), "test" | "bench" | "example"))
    }
}

impl Metadata {
    /// The directory of `p` relative to the workspace root (`ui`).
    pub fn dir(&self, p: &Package) -> String {
        let manifest = Path::new(&p.manifest_path);
        let dir = manifest.parent().unwrap_or(manifest);
        dir.strip_prefix(&self.workspace_root).unwrap_or(dir).to_string_lossy().replace('\\', "/")
    }

    /// The member in directory `dir`.
    pub fn member(&self, dir: &str) -> Option<&Package> {
        self.packages.iter().find(|p| self.dir(p) == dir)
    }

    /// Members without a gate entry (`CRATES`) and entries without a member.
    pub fn unlisted(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .packages
            .iter()
            .filter(|p| !CRATES.iter().any(|c| c.dir == self.dir(p)))
            .map(|p| format!("{}: workspace member without an entry in CRATES (xtask/src/repo.rs)", p.name))
            .collect();
        out.extend(
            CRATES
                .iter()
                .filter(|c| self.member(c.dir).is_none())
                .map(|c| format!("{}: in CRATES but no workspace member", c.dir)),
        );
        out
    }
}

/// Parses `cargo metadata --format-version 1 --no-deps` output.
pub fn parse(json: &str) -> Result<Metadata, String> {
    serde_json::from_str(json).map_err(|e| format!("cargo metadata: {}", e))
}

/// The workspace at `root`.
pub fn load(ctx: &crate::run::Context, root: &Path) -> Result<Metadata, String> {
    let out = run::capture(ctx, root, "cargo", &["metadata", "--format-version", "1", "--no-deps", "--locked"])?;
    if !out.ok {
        return Err(run::tail(&out.stderr, 10));
    }
    parse(&out.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(members: &[&str]) -> Metadata {
        let p: Vec<String> = members
            .iter()
            .map(|d| {
                format!(
                    r#"{{"name":"x-{d}","manifest_path":"/r/{d}/Cargo.toml","dependencies":[],"targets":[{{"kind":["lib"],"src_path":"/r/{d}/src/lib.rs"}}]}}"#
                )
            })
            .collect();
        parse(&format!(r#"{{"packages":[{}],"workspace_root":"/r"}}"#, p.join(","))).expect("metadata")
    }

    #[test]
    fn a_member_without_a_gate_entry_fails() {
        let all = ["hal", "dsp", "ui", "fpgad", "xtask"];
        assert!(meta(&all).unlisted().is_empty());
        let mut more = all.to_vec();
        more.push("core");
        assert_eq!(meta(&more).unlisted(), ["x-core: workspace member without an entry in CRATES (xtask/src/repo.rs)"]);
        assert_eq!(meta(&all[..4]).unlisted(), ["xtask: in CRATES but no workspace member"]);
    }

    #[test]
    fn test_targets_are_test_code() {
        let t = |k: &str| Target { kind: vec![k.to_string()], src_path: String::new() };
        assert!(t("test").is_test() && t("bench").is_test() && !t("lib").is_test() && !t("bin").is_test());
    }

    // The hole: `hal = { package = "oos-hal" }` made code say `hal::dho`, and the rules looked for `oos_hal::dho`.
    #[test]
    fn a_renamed_dependency_has_its_code_name_and_its_package_name() {
        let m = parse(
            r#"{"workspace_root":"/r","packages":[{"name":"oos-ui","manifest_path":"/r/ui/Cargo.toml","targets":[],
                "dependencies":[{"name":"oos-hal","rename":"hal"},{"name":"oos-dsp","rename":null},{"name":"libc"}]}]}"#,
        )
        .expect("metadata");
        let d: Vec<(String, String)> =
            m.packages[0].dependencies.iter().map(|d| (d.code_name(), d.canonical())).collect();
        let s = |a: &str, b: &str| (a.to_string(), b.to_string());
        assert_eq!(d, [s("hal", "oos_hal"), s("oos_dsp", "oos_dsp"), s("libc", "libc")]);
    }
}
