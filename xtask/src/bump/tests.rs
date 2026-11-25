// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use std::fs;

struct Fixture(PathBuf);

impl Fixture {
    fn copy(name: &str) -> Self {
        let source = crate::repo::root();
        let root = std::env::temp_dir().join(format!("xtask-bump-{name}-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let fixture = Self(root);
        for path in crate::repo::CRATES
            .iter()
            .map(|c| Path::new(c.dir).join("Cargo.toml"))
            .chain(["Cargo.lock", "fuzz/Cargo.lock", "CHANGELOG.md"].into_iter().map(PathBuf::from))
        {
            fs::create_dir_all(fixture.0.join(&path).parent().unwrap()).unwrap();
            fs::copy(source.join(&path), fixture.0.join(&path)).unwrap();
        }
        fixture
    }

    fn read(&self, path: &str) -> String {
        fs::read_to_string(self.0.join(path)).unwrap()
    }

    fn write(&self, path: &str, text: &str) {
        fs::write(self.0.join(path), text).unwrap();
    }

    fn version(&self) -> Version {
        Release::load(&self.0).unwrap().version
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn release_versions_follow_numeric_order_and_reject_invalid_forms() {
    assert!(Version::parse("1.2.10").unwrap() > Version::parse("1.2.9").unwrap());
    for invalid in ["v1.2.3", "1.2", "01.2.3", "1.2.3-beta", "1.2.+3", "1.2.3.4", "1..3"] {
        assert!(parse(&[invalid]).is_err(), "{invalid}");
    }
    assert!(matches!(parse(&["--check"]).unwrap(), Request::Check));
    assert!(parse(&["--check", "--allow-same"]).is_err());
}

#[test]
fn bump_only_changes_release_packages_and_renames_unreleased() {
    let fixture = Fixture::copy("update");
    let version = Version::parse("2026.11.25").unwrap();
    let old = fixture.version();
    fixture.write("CHANGELOG.md", &format!("# Changes\n\n## Unreleased\n\nPending notes.\n\n## {old}: Previous\n"));
    let third_party = format!(
        "\n[[package]]\nname = \"oos-ui\"\nversion = \"{old}\"\nsource = \"registry+https://example.invalid/index\"\n"
    );
    fixture.write("Cargo.lock", &(fixture.read("Cargo.lock") + &third_party));
    let count = apply(&fixture.0, version, false, |root| {
        assert_eq!(Release::load(root)?.version, version);
        assert!(fs::read_to_string(root.join("fuzz/Cargo.lock"))
            .unwrap()
            .contains(&format!("version = \"{version}\"")));
        Ok(Vec::new())
    })
    .unwrap();
    assert_eq!(count, 9);
    assert_eq!(fixture.version(), version);
    assert_eq!(
        fixture.read("CHANGELOG.md"),
        format!("# Changes\n\n## {version}\n\nPending notes.\n\n## {old}: Previous\n")
    );
    assert!(fixture.read("Cargo.lock").ends_with(&third_party));
    assert!(fixture.read("fuzz/Cargo.lock").contains("name = \"oos-fuzz\"\nversion = \"0.0.0\""));
    assert!(Release::load(&fixture.0).unwrap().checks(version).into_iter().all(|edit| !edit.unwrap().changed()));
}

#[test]
fn downgrade_and_failed_render_leave_release_files_intact() {
    let fixture = Fixture::copy("rollback");
    let before = fixture.read("Cargo.lock");
    let old = fixture.version();
    let next = Version::parse("2026.11.25").unwrap();
    assert!(matches!(apply(&fixture.0, old, false, |_| unreachable!()), Err(Error::Order(_))));
    assert!(matches!(apply(&fixture.0, Version([0, 0, 0]), true, |_| unreachable!()), Err(Error::Order(_))));
    assert!(matches!(
        apply(&fixture.0, next, false, |_| Err(Error::Command("render failed".into()))),
        Err(Error::Command(_))
    ));
    assert_eq!(fixture.version(), old);
    assert_eq!(fixture.read("Cargo.lock"), before);
    assert_eq!(apply(&fixture.0, old, true, |_| Ok(Vec::new())).unwrap(), 0);
}

#[test]
fn check_names_multiple_independent_disagreements() {
    let fixture = Fixture::copy("disagreements");
    let version = fixture.version();
    let old = version.to_string();
    let mismatch = "0.0.1";
    let entries = [
        ("dsp/Cargo.toml", "version = \""),
        ("Cargo.lock", "name = \"oos-core\"\nversion = \""),
        ("fuzz/Cargo.lock", "name = \"oos-core\"\nversion = \""),
        ("CHANGELOG.md", "## "),
    ];
    for (path, prefix) in entries {
        let original = fixture.read(path);
        let expected = format!("{prefix}{old}");
        assert!(original.contains(&expected), "{path}");
        fixture.write(path, &original.replacen(&expected, &format!("{prefix}{mismatch}"), 1));
    }
    let failures = Release::load(&fixture.0)
        .unwrap()
        .checks(version)
        .into_iter()
        .flat_map(|edit| edit.unwrap().mismatches)
        .collect::<Vec<_>>();
    assert_eq!(failures.len(), 4, "{failures:?}");
    for (path, _) in entries {
        assert!(failures.iter().any(|error| error.starts_with(path)), "{failures:?}");
    }
}
