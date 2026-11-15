//! M2.1 rules exercised on compiled, reachable modules, plus the actual domain tree.

use super::{modules, rules};
use crate::{repo, run, sources, workspace};
use std::path::Path;

const NAMES: [&str; 8] = ["change", "config", "model", "math", "decode", "zone", "la", "protocols"];

fn core_rules() -> rules::Rules {
    let rules = super::load(&repo::root()).expect("architecture rules");
    assert_eq!(rules.layers.iter().filter(|r| r.krate == "core").count(), NAMES.len());
    rules
}

fn fixture(dir: &Path, violation: Option<&str>) -> Vec<sources::Source> {
    let mut out = Vec::new();
    std::fs::create_dir_all(dir).expect("fixture directory");
    let mut root = "pub mod fixture { pub struct Stage; }\n".to_string();
    for name in NAMES {
        root.push_str(&format!("pub mod {name};\n"));
        let mut text = match name {
            "change" => "pub struct Stage;\npub fn inspect(_: crate::model::Model) {}\n".into(),
            "model" => "pub struct Model { pub channel: crate::math::Setting }\n\
                pub fn inspect(m: Model) { let Model { channel } = m;\n\
                let crate::math::Setting {} = channel; }\n"
                .to_string(),
            _ => "pub struct Setting {}\n".to_string(),
        };
        if violation == Some(name) {
            let to = if name == "model" {
                "change::Stage"
            } else if name == "change" {
                "fixture::Stage"
            } else {
                "model::Model"
            };
            text.push_str(&format!("pub fn forbidden(_: crate::{to}) {{}}\n"));
        }
        std::fs::write(dir.join(format!("{name}.rs")), &text).expect("module source");
        out.push(sources::Source {
            rel: format!("core/src/{name}.rs"),
            module: name.into(),
            test: false,
            file: syn::parse_file(&text).expect("syntax"),
            text,
        });
    }
    std::fs::write(dir.join("lib.rs"), &root).expect("crate root");
    let parsed = syn::parse_file(&root).expect("root syntax");
    for name in NAMES {
        assert!(parsed.items.iter().any(|i| matches!(i, syn::Item::Mod(m) if m.ident == name && m.content.is_none())));
        assert!(dir.join(format!("{name}.rs")).is_file());
    }
    let output = run::capture(
        &Default::default(),
        dir,
        "rustc",
        &["--edition=2021", "--crate-type=lib", "--emit=metadata", "lib.rs", "-o", "fixture.rmeta"],
    )
    .expect("compile boundary fixture");
    assert!(output.ok, "{}", output.stderr);
    out
}

#[test]
fn core_layers_reject_each_forbidden_import_in_a_compiled_module() {
    let dir = std::env::temp_dir().join(format!("oos-core-layers-{}", std::process::id()));
    let rules = core_rules();
    let layers = rules.layers.iter().filter(|l| l.krate == "core").collect();
    let rules = modules::CrateRules { layers, ..Default::default() };
    assert!(modules::check(&fixture(&dir, None), &rules).is_empty());
    for name in NAMES {
        let failures = modules::check(&fixture(&dir, Some(name)), &rules);
        let layer = rules.layers.iter().find(|l| l.module == name).expect("rule exists");
        let to = if name == "model" {
            "change::Stage"
        } else if name == "change" {
            "fixture::Stage"
        } else {
            "model::Model"
        };
        assert_eq!(
            failures,
            [format!("core/src/{name}.rs names crate::{to} (layer {name} allows {:?})", layer.allowed)]
        );
    }
    std::fs::remove_dir_all(dir).expect("remove own fixture");
}

#[test]
fn every_core_layer_names_an_actual_reachable_module() {
    let root = repo::root();
    let meta = workspace::load(&Default::default(), &root).expect("workspace");
    let sources = sources::of_dir(&meta, "core").expect("core sources");
    for layer in core_rules().layers.iter().filter(|l| l.krate == "core") {
        assert!(sources.iter().any(|s| s.module == layer.module && !s.test), "absent module {}", layer.module);
    }
    assert!(super::check(&root, &meta).expect("boundaries").is_empty());
}

#[test]
fn core_purity_lints_reject_outside_observations_and_unsafe() {
    let root = repo::root();
    let dir = std::env::temp_dir().join(format!("oos-core-purity-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("fixture directory");
    let probes = [
        ("pub fn pure(x: u32) -> u32 { x + 1 }", None),
        ("pub fn bad() { let _ = std::env::var(\"OOS_FIXTURE\"); }", Some("disallowed method")),
        ("pub fn bad() { let _ = std::fs::read(\"example.invalid\"); }", Some("disallowed method")),
        ("pub fn bad(_: Option<std::net::TcpStream>) {}", Some("disallowed type")),
        ("pub fn bad() { let _ = std::time::Instant::now(); }", Some("disallowed method")),
        ("pub fn bad() { let _ = std::time::SystemTime::now(); }", Some("disallowed method")),
        ("pub fn bad() { std::thread::sleep(std::time::Duration::ZERO); }", Some("disallowed method")),
        ("pub fn bad() { let _ = std::thread::spawn(|| ()); }", Some("disallowed method")),
        ("pub fn bad() { let _ = std::io::stdout(); }", Some("disallowed method")),
        ("pub fn bad() { std::println!(\"fixture\"); }", Some("disallowed macro")),
        ("pub unsafe fn bad() {}", Some("unsafe")),
    ];
    for (source, diagnostic) in probes {
        std::fs::write(dir.join("lib.rs"), format!("#![forbid(unsafe_code)]\n{source}")).expect("probe");
        let output = std::process::Command::new("clippy-driver")
            .current_dir(&dir)
            .env("CLIPPY_CONF_DIR", root.join("core"))
            .args([
                "--edition=2021",
                "--crate-type=lib",
                "--emit=metadata",
                "lib.rs",
                "-o",
                "fixture.rmeta",
                "-Dclippy::disallowed_methods",
                "-Dclippy::disallowed_types",
                "-Dclippy::disallowed_macros",
            ])
            .output()
            .expect("run purity lint fixture");
        let err = String::from_utf8_lossy(&output.stderr);
        match diagnostic {
            Some(d) => assert!(!output.status.success() && err.contains(d), "{source}: {err}"),
            None => assert!(output.status.success(), "{err}"),
        }
    }
    std::fs::remove_dir_all(dir).expect("remove own fixture");
}
