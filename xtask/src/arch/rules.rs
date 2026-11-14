//! The architecture rules file (`xtask/data/architecture.txt`).

use std::collections::{BTreeMap, BTreeSet};

/// One module rule: in crate `krate`, code under `src/<from>` must not name `crate::<to>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Forbid {
    pub krate: String,
    pub from: String,
    pub to: String,
}

/// An allow-list: in crate `krate`, code under `src/<module>` may name only `crate::<allowed>` (and itself).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layer {
    pub krate: String,
    pub module: String,
    pub allowed: BTreeSet<String>,
}

/// The parsed rules.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Rules {
    /// package name: the packages it may depend on (any kind)
    pub crates: BTreeMap<String, BTreeSet<String>>,
    pub forbid: Vec<Forbid>,
    pub layers: Vec<Layer>,
    /// the metrics allowlist: `static <file> <name>` keys of statics with interior mutability that may stay
    pub statics: BTreeSet<String>,
}

/// Parses `crate <name> -> <deps...>`, `forbid <crate dir> <from module> <to module>` and
/// `layer <crate dir> <module> -> <modules...>` and `static <file> <name>` lines; `#` starts a comment.
pub fn parse(text: &str) -> Result<Rules, String> {
    let mut r = Rules::default();
    for (i, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("").trim();
        let words: Vec<&str> = line.split_whitespace().collect();
        match words.as_slice() {
            [] => {}
            ["crate", name, "->", deps @ ..] => {
                r.crates.insert(name.to_string(), deps.iter().map(|d| d.to_string()).collect());
            }
            ["forbid", krate, from, to] => {
                r.forbid.push(Forbid { krate: krate.to_string(), from: from.to_string(), to: to.to_string() })
            }
            ["static", file, name] => {
                r.statics.insert(format!("static {} {}", file, name));
            }
            ["layer", krate, module, "->", allowed @ ..] => r.layers.push(Layer {
                krate: krate.to_string(),
                module: module.to_string(),
                allowed: allowed.iter().map(|m| m.to_string()).collect(),
            }),
            _ => return Err(format!("line {}: {:?} is no rule", i + 1, line)),
        }
    }
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_crate_and_module_rules() {
        let r = parse("# c\ncrate oos-dsp ->\ncrate oos-ui -> libc oos-hal # why\nforbid ui render model\n").unwrap();
        assert!(r.crates["oos-dsp"].is_empty());
        assert_eq!(r.crates["oos-ui"].iter().collect::<Vec<_>>(), ["libc", "oos-hal"]);
        assert_eq!(r.forbid, vec![Forbid { krate: "ui".into(), from: "render".into(), to: "model".into() }]);
        assert!(parse("crate x").is_err() && parse("forbid ui a").is_err());
        let l = parse("layer ui render -> model wave\nlayer ui leaf ->\n").unwrap().layers;
        assert_eq!(l[0].allowed.iter().collect::<Vec<_>>(), ["model", "wave"]);
        assert!(l[1].allowed.is_empty() && l[1].module == "leaf");
        let s = parse("static ui/src/pacing.rs VIEWS\n").unwrap().statics;
        assert!(s.contains("static ui/src/pacing.rs VIEWS"));
    }
}
