// SPDX-License-Identifier: GPL-2.0-or-later
//! Names checked syntactically; meaning and composition remain review obligations.

mod policy;
#[cfg(test)]
mod tests;

use crate::ratchet::Ratchet;
use crate::size::test_item;
use crate::sources::{visit_production, Source};
use policy::{split_words, Vocabulary};
use std::path::Path;
use syn::ext::IdentExt;
use syn::visit::{self, Visit};

const VAGUE: &[&str] =
    &["manager", "helper", "util", "utils", "misc", "data", "info", "handler", "stuff", "thing", "impl"];

struct Declarations<'a> {
    vocabulary: &'a Vocabulary,
    file: &'a str,
    findings: Ratchet,
}

impl Declarations<'_> {
    fn check_name(&mut self, ident: &syn::Ident, ty: bool) {
        let name = ident.unraw().to_string();
        let parts = split_words(&name);
        let lower: Vec<_> = parts.iter().map(|p| p.to_lowercase()).collect();
        let mut rules = Vec::new();
        if lower.iter().any(|p| VAGUE.contains(&p.as_str()) || VAGUE.contains(&p.trim_end_matches('s'))) {
            rules.push("vague".to_string());
        }
        if !ty
            && lower.first().is_some_and(|p| ["do", "handle", "process"].contains(&p.as_str()))
            && !lower.iter().skip(1).any(|p| self.vocabulary.nouns.contains(p))
        {
            rules.push("domain-verb".to_string());
        }
        for (part, word) in parts.iter().zip(&lower) {
            let singular = word.strip_suffix('s').unwrap_or(word);
            if ty {
                if let Some(canonical) = self.vocabulary.synonyms.get(singular) {
                    rules.push(format!("use-{canonical}"));
                }
            }
            let short = word.chars().filter(|c| c.is_alphabetic()).count() <= 3;
            let acronym = part.chars().filter(|c| c.is_alphabetic()).all(char::is_uppercase);
            if !self.vocabulary.abbreviations.contains(word)
                && (self.vocabulary.contractions.contains(word) || short || acronym)
                && !VAGUE.contains(&word.as_str())
                && !self.vocabulary.nouns.contains(word)
            {
                rules.push(format!("abbreviation-{word}"));
            }
        }
        rules.sort();
        rules.dedup();
        for rule in rules {
            *self.findings.0.entry(format!("name {} {name} {rule}", self.file)).or_default() += 1;
        }
    }
}

impl<'ast> Visit<'ast> for Declarations<'_> {
    visit_production!('ast, |self, node| test_item(node));

    fn visit_item_struct(&mut self, item: &'ast syn::ItemStruct) {
        self.check_name(&item.ident, true);
        visit::visit_item_struct(self, item);
    }

    fn visit_item_enum(&mut self, item: &'ast syn::ItemEnum) {
        self.check_name(&item.ident, true);
        visit::visit_item_enum(self, item);
    }

    fn visit_item_type(&mut self, item: &'ast syn::ItemType) {
        self.check_name(&item.ident, true);
        visit::visit_item_type(self, item);
    }

    fn visit_trait_item_type(&mut self, item: &'ast syn::TraitItemType) {
        self.check_name(&item.ident, true);
        visit::visit_trait_item_type(self, item);
    }

    fn visit_impl_item_type(&mut self, item: &'ast syn::ImplItemType) {
        self.check_name(&item.ident, true);
        visit::visit_impl_item_type(self, item);
    }

    fn visit_item_trait(&mut self, item: &'ast syn::ItemTrait) {
        self.check_name(&item.ident, true);
        visit::visit_item_trait(self, item);
    }

    fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
        // Trait member names are fixed by their trait, including external Rust and adapter APIs.
        if item.trait_.is_none() {
            visit::visit_item_impl(self, item);
        } else {
            for member in item.items.iter().filter(|member| !test_item(*member)) {
                match member {
                    syn::ImplItem::Fn(function) => visit::visit_impl_item_fn(self, function),
                    syn::ImplItem::Type(ty) => visit::visit_impl_item_type(self, ty),
                    _ => visit::visit_impl_item(self, member),
                }
            }
        }
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        self.check_name(&item.sig.ident, false);
        visit::visit_item_fn(self, item);
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        self.check_name(&item.sig.ident, false);
        visit::visit_impl_item_fn(self, item);
    }

    fn visit_trait_item_fn(&mut self, item: &'ast syn::TraitItemFn) {
        self.check_name(&item.sig.ident, false);
        visit::visit_trait_item_fn(self, item);
    }
}

#[derive(Debug)]
pub enum Error {
    Read(std::io::Error),
    MissingTables,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read(error) => write!(f, "glossary: {error}"),
            Self::MissingTables => f.write_str("glossary needs Canonical, Abbreviation and Contraction tables"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(error) => Some(error),
            Self::MissingTables => None,
        }
    }
}

pub fn measure(root: &Path, sources: &[Source]) -> Result<Ratchet, Error> {
    let text = std::fs::read_to_string(root.join("docs/architecture/glossary.md")).map_err(Error::Read)?;
    let vocabulary = Vocabulary::parse(&text)?;
    let mut all = Ratchet::default();
    for source in sources.iter().filter(|s| !s.test) {
        let mut declarations =
            Declarations { vocabulary: &vocabulary, file: &source.rel, findings: Ratchet::default() };
        declarations.visit_file(&source.file);
        for (key, count) in declarations.findings.0 {
            *all.0.entry(key).or_default() += count;
        }
    }
    Ok(all)
}
