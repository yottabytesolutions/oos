//! Parse the intentionally small pin-file format without adding a TOML dependency.
use serde::Deserialize;
use std::{collections::BTreeMap, fs, path::Path};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Tools {
    pub schema: u8,
    pub python: String,
    #[serde(rename = "cargo-llvm-cov")]
    pub coverage: String,
    pub stdlib: Vec<String>,
    pub profiles: BTreeMap<String, Vec<String>>,
}

#[derive(Debug)]
pub(super) struct Pins {
    pub rust: String,
    pub tools: Tools,
}

fn read(root: &Path, name: &str) -> Result<String, String> {
    fs::read_to_string(root.join(name)).map_err(|_| format!("{}: missing or unreadable", name))
}

/// Only the string and string-array TOML subset used by these configuration files.
pub(super) fn table(text: &str, section: &str) -> Result<BTreeMap<String, serde_json::Value>, String> {
    let mut active = false;
    let mut entries = BTreeMap::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        if line.starts_with('[') {
            active = line == format!("[{}]", section);
        } else if active {
            let (key, value) = line.split_once('=').ok_or("invalid pin assignment")?;
            let value = serde_json::from_str(value.trim()).map_err(|_| "invalid pin value")?;
            if entries.insert(key.trim().to_string(), value).is_some() {
                return Err("duplicate pin assignment".into());
            }
        }
    }
    Ok(entries)
}

pub(super) fn load(root: &Path) -> Result<Pins, String> {
    let t = table(&read(root, "rust-toolchain.toml")?, "toolchain")?;
    let rust = t.get("channel").and_then(|v| v.as_str()).ok_or("missing Rust channel")?.to_string();
    if !version(&rust)
        || t.len() != 4
        || t.get("profile") != Some(&serde_json::json!("minimal"))
        || t.get("components") != Some(&serde_json::json!(["rustfmt", "clippy", "llvm-tools-preview"]))
        || t.get("targets") != Some(&serde_json::json!([crate::repo::TARGET]))
    {
        return Err(
            "rust-toolchain.toml: require exact Rust, minimal, rustfmt/clippy/llvm-tools-preview and musl target"
                .into(),
        );
    }
    let tools: Tools = serde_json::from_str(&read(root, "tools/build-tools.json")?)
        .map_err(|_| "tools/build-tools.json: invalid schema")?;
    if tools.schema != 1
        || !python_minimum(&tools.python)
        || !version(&tools.coverage)
        || read(root, ".python-version")?.trim() != tools.python
        || tools.stdlib.is_empty()
        || tools.profiles.keys().map(String::as_str).collect::<Vec<_>>() != ["gpu", "image", "sim"]
    {
        return Err("tools/build-tools.json: invalid versions/profiles or Python minimum disagreement".into());
    }
    Ok(Pins { rust, tools })
}

fn python_minimum(s: &str) -> bool {
    let parts: Vec<_> = s.split('.').collect();
    parts.len() == 2 && parts.iter().all(|p| p.parse::<u32>().is_ok())
}

fn version(s: &str) -> bool {
    let parts: Vec<_> = s.split('.').collect();
    parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.bytes().all(|c| c.is_ascii_digit()))
}

pub(crate) fn check(root: &Path) -> crate::steps::Outcome {
    load(root).map_err(|e| vec![e])?;
    let out = crate::run::capture(root, "python3", &["-I", "tools/build_env.py", "validate"])
        .map_err(|_| vec!["pin syntax: install Python with tomllib, then tools/bootstrap.sh".into()])?;
    if !out.ok {
        return Err(vec!["container lock/recipe syntax: run python3 -I tools/build_env.py validate".into()]);
    }
    Ok(vec!["tool requirements and container base/recipe syntax valid".into()])
}
