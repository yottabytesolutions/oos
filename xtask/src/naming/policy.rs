// SPDX-License-Identifier: GPL-2.0-or-later
//! Declaration vocabulary from the domain glossary's Markdown tables.

use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Default)]
pub(super) struct Vocabulary {
    pub nouns: BTreeSet<String>,
    pub synonyms: BTreeMap<String, String>,
    pub abbreviations: BTreeSet<String>,
    pub contractions: BTreeSet<String>,
}

impl Vocabulary {
    pub fn parse(text: &str) -> Result<Self, super::Error> {
        let mut out = Self::default();
        let mut section = "";
        for line in text.lines() {
            let cells: Vec<_> = line.split('|').map(str::trim).collect();
            if cells.len() < 4 || cells[1].starts_with('-') {
                continue;
            }
            match cells[1] {
                "Canonical" | "Abbreviation" | "Contraction" => section = cells[1],
                word => match section {
                    "Canonical" => {
                        out.nouns.extend([word.to_string(), pluralize(word)]);
                        for synonym in cells[2].split(", ").filter(|s| *s != "-") {
                            out.synonyms.insert(synonym.to_string(), word.to_string());
                        }
                    }
                    "Abbreviation" => out.abbreviations.extend(word.split(", ").map(str::to_string)),
                    "Contraction" => out.contractions.extend(word.split(", ").map(str::to_string)),
                    _ => {}
                },
            }
        }
        if out.nouns.is_empty() || out.abbreviations.is_empty() || out.contractions.is_empty() {
            return Err(super::Error::MissingTables);
        }
        Ok(out)
    }
}

fn pluralize(word: &str) -> String {
    match word.strip_suffix('y') {
        Some(stem) if stem.ends_with(|letter: char| letter.is_ascii_alphabetic() && !"aeiou".contains(letter)) => {
            format!("{stem}ies")
        }
        _ if word.ends_with(['s', 'x', 'z']) || word.ends_with("ch") || word.ends_with("sh") => format!("{word}es"),
        _ => format!("{word}s"),
    }
}

/// Split `snake_case` and `CamelCase`, retaining an acronym as a word and digits with their unit/protocol.
pub(super) fn split_words(name: &str) -> Vec<String> {
    let chars: Vec<_> = name.trim_start_matches("r#").chars().collect();
    let mut out = Vec::new();
    let mut word = String::new();
    for (i, &ch) in chars.iter().enumerate() {
        let boundary = ch.is_uppercase()
            && (i > 0 && chars[i - 1].is_lowercase()
                || chars.get(i + 1).is_some_and(|c| c.is_lowercase()) && word.chars().count() > 1);
        if ch == '_' || boundary {
            if !word.is_empty() {
                out.push(std::mem::take(&mut word));
            }
            if ch == '_' {
                continue;
            }
        }
        word.push(ch);
    }
    if !word.is_empty() {
        out.push(word);
    }
    out
}
