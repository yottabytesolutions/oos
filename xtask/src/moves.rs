// SPDX-License-Identifier: GPL-2.0-or-later
//! Planned moves (`xtask/data/moves.txt`): a branch that moves code lists `<old> -> <new>`, where both sides are
//! ratchet keys (`fn ui/src/ui.rs Ui::step -> fn ui/src/step.rs Ui::step`) or file paths (`ui/src/model.rs ->
//! core/src/model.rs`, which renames the path in every key and the crate word of the clippy keys). The gate then
//! accepts a listed entry under its new key if it is not larger than under the old one; `cargo xtask ratchet --move`
//! renames the entries in the ratchet files and keeps the moves the base still needs as evidence. The integrator runs
//! it at merge.

use crate::ratchet::Ratchet;

/// One planned move.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Move {
    pub old: String,
    pub new: String,
}

/// The header of an empty moves file.
pub const HEAD: &str = "Planned moves of this branch: <old key or file> -> <new key or file> (docs/development.md).\n\
The integrator applies them with `cargo xtask ratchet --move` at merge, which empties this file.";

/// Parses `<old> -> <new>` lines; `#` comments.
pub fn parse(text: &str) -> Result<Vec<Move>, String> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (old, new) =
            line.split_once(" -> ").ok_or_else(|| format!("moves.txt line {}: no '<old> -> <new>'", i + 1))?;
        let (old, new) = (old.trim(), new.trim());
        if old.is_empty() || new.is_empty() || old.contains(' ') != new.contains(' ') {
            return Err(format!("moves.txt line {}: both sides keys or both sides file paths", i + 1));
        }
        out.push(Move { old: old.to_string(), new: new.to_string() });
    }
    Ok(out)
}

/// The crate directory of a path: its first segment.
fn crate_of(path: &str) -> &str {
    path.split('/').next().unwrap_or(path)
}

/// `key` after a file move: every word equal to the old path renamed, and a leading crate word (clippy keys) with it.
fn renamed(key: &str, m: &Move) -> Option<String> {
    let words: Vec<&str> = key.split(' ').collect();
    if !words.contains(&m.old.as_str()) {
        return None;
    }
    let (oc, nc) = (crate_of(&m.old), crate_of(&m.new));
    let out: Vec<&str> = words
        .iter()
        .enumerate()
        .map(|(i, w)| {
            if *w == m.old {
                m.new.as_str()
            } else if i == 0 && *w == oc {
                nc
            } else {
                w
            }
        })
        .collect();
    Some(out.join(" "))
}

/// `key` after the move `m`; None when `m` does not name it.
fn moved(key: &str, m: &Move) -> Option<String> {
    if m.old.contains(' ') {
        (key == m.old).then(|| m.new.clone())
    } else {
        renamed(key, m)
    }
}

/// Whether `m` names an entry of `r`: the move is still to be applied there.
pub fn pending(r: &Ratchet, m: &Move) -> bool {
    r.0.keys().any(|k| moved(k, m).is_some())
}

/// `r` with the moves applied in order: each moved entry under its new key with its own value. A key move names one
/// entry, a file move every entry with that path. A move that names no entry changes nothing, so an applied move may
/// stay listed. A move that puts two entries under one key fails: the merged entry would take the larger allowance.
pub fn apply(r: &Ratchet, moves: &[Move]) -> Result<Ratchet, String> {
    let mut cur = r.clone();
    for m in moves {
        let mut out = Ratchet::default();
        for (k, &v) in &cur.0 {
            let key = moved(k, m).unwrap_or_else(|| k.clone());
            if let Some(other) = out.0.insert(key.clone(), v) {
                return Err(format!(
                    "moves.txt: {} -> {}: {} would hold two entries ({} and {}); a move may not merge entries",
                    m.old, m.new, key, other, v
                ));
            }
        }
        cur = out;
    }
    Ok(cur)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(t: &str) -> Ratchet {
        Ratchet::parse(t).expect("ratchet")
    }

    #[test]
    fn parses_key_and_file_moves() {
        let m =
            parse("# x\nfn ui/src/a.rs f -> fn ui/src/b.rs f\nui/src/model.rs -> core/src/model.rs\n").expect("moves");
        assert_eq!(m.len(), 2);
        assert!(parse("a -> b c").is_err() && parse("a b").is_err() && parse(" -> x").is_err());
    }

    #[test]
    fn a_file_move_renames_every_key_and_the_clippy_crate() {
        let m = parse("ui/src/model.rs -> core/src/model.rs\n").expect("moves");
        let before = r("194 struct ui/src/model.rs Model\n3 ui ui/src/model.rs clippy::x\n7 fn ui/src/ui.rs f\n");
        let after = apply(&before, &m).expect("apply");
        assert_eq!(
            after,
            r("194 struct core/src/model.rs Model\n3 core core/src/model.rs clippy::x\n7 fn ui/src/ui.rs f\n")
        );
    }

    #[test]
    fn a_key_move_renames_one_entry() {
        let m = parse("fn ui/src/ui.rs Ui::step -> fn ui/src/step.rs Ui::step\n").expect("moves");
        let after = apply(&r("465 fn ui/src/ui.rs Ui::step\n90 fn ui/src/ui.rs Ui::tap\n"), &m).expect("apply");
        assert_eq!(after, r("465 fn ui/src/step.rs Ui::step\n90 fn ui/src/ui.rs Ui::tap\n"));
    }

    // The hole: A=70, B=100 and the move A -> B gave B the larger allowance, so A could grow to 100. A move that puts
    // two entries under one key is refused; the moved entry keeps its own allowance.
    #[test]
    fn a_move_onto_an_existing_entry_is_refused() {
        let base = r("70 fn ui/src/a.rs A\n100 fn ui/src/a.rs B\n");
        let m = parse("fn ui/src/a.rs A -> fn ui/src/a.rs B\n").expect("moves");
        assert!(apply(&base, &m).expect_err("collision").contains("fn ui/src/a.rs B"));
        // two files into one: their clippy counts and sizes would merge
        let files = r("3 ui ui/src/a.rs clippy::x\n2 ui ui/src/b.rs clippy::x\n");
        assert!(apply(&files, &parse("ui/src/a.rs -> ui/src/b.rs\n").expect("moves")).is_err());
        let two = parse("fn ui/src/a.rs A -> fn ui/src/c.rs C\nfn ui/src/a.rs B -> fn ui/src/c.rs C\n").expect("m");
        assert!(apply(&base, &two).is_err());
        // a move onto a free key keeps its own value
        let free = parse("fn ui/src/a.rs A -> fn ui/src/c.rs A\n").expect("moves");
        assert_eq!(apply(&base, &free).expect("apply"), r("100 fn ui/src/a.rs B\n70 fn ui/src/c.rs A\n"));
    }

    // Applied moves stay listed until the base has them: applying them again changes nothing, chains apply in order.
    #[test]
    fn moves_apply_in_order_and_again_without_effect() {
        let m = parse("fn ui/src/a.rs A -> fn ui/src/b.rs A\nfn ui/src/b.rs A -> fn core/src/b.rs A\n").expect("m");
        let once = apply(&r("70 fn ui/src/a.rs A\n"), &m).expect("apply");
        assert_eq!(once, r("70 fn core/src/b.rs A\n"));
        assert_eq!(apply(&once, &m).expect("again"), once);
        assert!(pending(&r("70 fn ui/src/a.rs A\n"), &m[0]) && !pending(&once, &m[0]));
    }
}
