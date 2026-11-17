// SPDX-License-Identifier: GPL-2.0-or-later
//! The anchors GitHub gives a Markdown file: one per heading, ATX or Setext (with `-1`, `-2` for repeats), and
//! explicit HTML `id`/`name` attributes outside comments.

use super::html::{attributes, Collect};
use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use std::collections::BTreeSet;

/// Every anchor of `text`.
pub(crate) fn of(text: &str) -> BTreeSet<String> {
    // Repeats are counted among the generated heading anchors only, not against explicit HTML ones.
    let mut headings = BTreeSet::new();
    let mut explicit = BTreeSet::new();
    let mut heading: Option<String> = None;
    let mut html = Collect::default();
    for event in Parser::new_ext(text, super::parse::options()) {
        match &event {
            Event::Start(Tag::Heading { .. }) => heading = Some(String::new()),
            Event::End(TagEnd::Heading(_)) => {
                if let Some(h) = heading.take() {
                    insert(&mut headings, &slug(&h));
                }
            }
            Event::Text(t) | Event::Code(t) => heading.iter_mut().for_each(|h| h.push_str(t)),
            Event::SoftBreak | Event::HardBreak => heading.iter_mut().for_each(|h| h.push(' ')),
            _ => {}
        }
        if let Some((_, raw)) = html.feed(&event, 0) {
            explicit
                .extend(attributes(&raw).into_iter().filter(|a| a.name == "id" || a.name == "name").map(|a| a.value));
        }
    }
    headings.extend(explicit);
    headings
}

/// Adds the anchor `base`, or `base-1`, `base-2`, ... when it is taken.
fn insert(out: &mut BTreeSet<String>, base: &str) {
    let mut name = base.to_string();
    let mut n = 0;
    while out.contains(&name) {
        n += 1;
        name = format!("{base}-{n}");
    }
    out.insert(name);
}

/// GitHub's anchor for the rendered text of a heading: lower case, punctuation removed, spaces as `-`.
pub(crate) fn slug(heading: &str) -> String {
    heading
        .chars()
        .flat_map(char::to_lowercase)
        .filter_map(|c| match c {
            ' ' => Some('-'),
            c if c.is_alphanumeric() || c == '-' || c == '_' => Some(c),
            _ => None,
        })
        .collect()
}
