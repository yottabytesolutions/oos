// SPDX-License-Identifier: GPL-2.0-or-later
//! The report: Markdown tables for people and job summaries, JSON for the release workflow.

use super::Counts;
use serde::Serialize;
use std::collections::BTreeMap;

/// One name's lines now and, when compared, at the base and the change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Row {
    pub now: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<i64>,
}

impl Row {
    fn new(now: u64, base: Option<u64>) -> Row {
        Row { now, base, delta: base.map(|b| now as i64 - b as i64) }
    }
}

/// The report; `base` names the ref compared with.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Report {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    pub production: BTreeMap<String, Row>,
    pub production_total: Row,
    pub tooling: BTreeMap<String, Row>,
}

/// The rows of every name in `now` or `base`; a name missing on one side has 0 lines there.
fn rows(now: &BTreeMap<String, u64>, base: Option<&BTreeMap<String, u64>>) -> BTreeMap<String, Row> {
    let names = now.keys().chain(base.into_iter().flat_map(BTreeMap::keys));
    let at = |m: &BTreeMap<String, u64>, k: &str| m.get(k).copied().unwrap_or(0);
    names.map(|k| (k.clone(), Row::new(at(now, k), base.map(|b| at(b, k))))).collect()
}

/// One table row in Markdown.
fn line(name: &str, r: &Row) -> String {
    match (r.base, r.delta) {
        (Some(b), Some(d)) => format!("| {} | {} | {} | {:+} |\n", name, b, r.now, d),
        _ => format!("| {} | {} |\n", name, r.now),
    }
}

impl Report {
    pub fn new(now: &Counts, base: Option<(&str, &Counts)>) -> Report {
        let total = |c: &Counts| c.production.values().sum::<u64>();
        Report {
            base: base.map(|(r, _)| r.to_string()),
            production: rows(&now.production, base.map(|(_, c)| &c.production)),
            production_total: Row::new(total(now), base.map(|(_, c)| total(c))),
            tooling: rows(&now.tooling, base.map(|(_, c)| &c.tooling)),
        }
    }

    fn table(&self, title: &str, rows: &BTreeMap<String, Row>, total: Option<&Row>) -> String {
        let mut out = match &self.base {
            Some(b) => format!("| {} | {} | Now | Change |\n|---|---:|---:|---:|\n", title, b),
            None => format!("| {} | Lines |\n|---|---:|\n", title),
        };
        for (name, r) in rows.iter().map(|(n, r)| (n.as_str(), r)).chain(total.map(|t| ("Total", t))) {
            out.push_str(&line(name, r));
        }
        out
    }

    /// The production code table with its total, then the tooling table.
    pub fn markdown(&self) -> String {
        let p = self.table("Production code", &self.production, Some(&self.production_total));
        format!("{}\n{}", p, self.table("Tooling", &self.tooling, None))
    }
}
