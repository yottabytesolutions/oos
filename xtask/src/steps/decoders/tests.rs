// SPDX-License-Identifier: GPL-2.0-or-later
use std::{fs, path::PathBuf};

struct Fixture {
    root: PathBuf,
    ctx: crate::run::Context,
}

impl Fixture {
    fn new(source: &str) -> Self {
        let name = std::thread::current().name().unwrap().replace(':', "_");
        let root = std::env::temp_dir().join(format!("oos-decoders-{}-{name}", std::process::id()));
        fs::create_dir_all(root.join("ui/src")).unwrap();
        fs::create_dir_all(root.join("ui/tools")).unwrap();
        fs::create_dir_all(root.join("tools")).unwrap();
        for (path, text) in [
            (
                "ui/Cargo.toml",
                "[package]\nname = \"decoder-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\
                 [features]\ndefault = [\"decoders\"]\n\
                 decoders = [\"dec-first\", \"dec-second\"]\ndec-first = []\ndec-second = []\n",
            ),
            ("ui/Cargo.lock", "version = 4\n\n[[package]]\nname = \"decoder-fixture\"\nversion = \"0.1.0\"\n"),
            ("ui/src/lib.rs", source),
            ("ui/tools/decoder-features.sh", include_str!("../../../../ui/tools/decoder-features.sh")),
            ("tools/output.py", "from pathlib import Path\nprint(Path(__file__).resolve().parent.parent / 'target')\n"),
        ] {
            fs::write(root.join(path), text).unwrap();
        }
        let (ctx, _) = crate::run::start_logs(&root).unwrap();
        Self { root, ctx }
    }

    fn check(&self) -> super::Outcome {
        let step = crate::inventory::CHECKS.iter().find(|step| step.id == "decoders").unwrap();
        (step.run)(&self.ctx, &self.root)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn independent_decoders_are_each_checked_without_defaults() {
    let fixture = Fixture::new("");
    assert_eq!(
        fixture.check().unwrap(),
        [
            "ui: builds with the core decoders only",
            "ui: builds with dec-first alone",
            "ui: builds with dec-second alone",
        ]
    );
}

#[test]
fn a_decoder_cannot_depend_on_another_optional_decoder() {
    let fixture = Fixture::new(
        "#[cfg(all(feature = \"dec-second\", not(feature = \"dec-first\")))]\n\
         compile_error!(\"second depends on first\");\n",
    );
    let errors = fixture.check().unwrap_err().join("\n");
    assert!(errors.contains("dec-second"), "{errors}");
    assert!(errors.contains("second depends on first"), "{errors}");
}

#[test]
fn standalone_decoder_checks_include_test_targets() {
    let fixture =
        Fixture::new("#[cfg(all(test, feature = \"dec-first\"))]\ncompile_error!(\"decoder tests do not compile\");\n");
    let errors = fixture.check().unwrap_err().join("\n");
    assert!(errors.contains("dec-first"), "{errors}");
    assert!(errors.contains("decoder tests do not compile"), "{errors}");
}

#[test]
fn failure_to_list_decoders_fails_the_step() {
    let fixture = Fixture::new("");
    fs::write(fixture.root.join("ui/tools/decoder-features.sh"), "echo 'no decoder list' >&2\nexit 7\n").unwrap();
    assert!(fixture.check().unwrap_err().join("\n").contains("no decoder list"));
}
