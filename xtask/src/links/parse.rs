// SPDX-License-Identifier: GPL-2.0-or-later
//! Link targets of a Markdown text, as GitHub renders it (CommonMark with the GFM extensions, read by
//! pulldown-cmark): links and images, reference definitions, HTML `src`/`href` and wiki `[[...]]` links. Code, HTML
//! comments and the text of raw HTML blocks render no link and give none.

use super::html::{attributes, Collect};
use pulldown_cmark::{Event, LinkType, Options, Parser, Tag, TagEnd};

/// One link target and the line it is on (1-based).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Link {
    pub line: usize,
    pub target: Target,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Target {
    /// A Markdown or HTML link: a URL or a path.
    Url(String),
    /// A GitHub wiki link, `[[Page Name]]` or `[[Text|Page Name]]`: the page part.
    Wiki(String),
}

/// The Markdown dialect: GitHub's extensions, and wiki links.
pub(crate) fn options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_WIKILINKS
}

/// Every link target in `text`, in line order.
pub(crate) fn links(text: &str) -> Vec<Link> {
    let lines = Lines::new(text);
    let parser = Parser::new_ext(text, options());
    let mut out: Vec<Link> = parser
        .reference_definitions()
        .iter()
        .map(|(_, d)| Link { line: lines.at(d.span.start), target: Target::Url(d.dest.to_string()) })
        .collect();
    // GitHub's `[[Text|Page]]` names the page after the bar, where the parser (Obsidian style) puts the link text.
    let mut piped: Option<(usize, String)> = None;
    let mut html = Collect::default();
    for (event, range) in parser.into_offset_iter() {
        let line = lines.at(range.start);
        match &event {
            Event::Start(Tag::Link { link_type: LinkType::WikiLink { has_pothole: true }, .. }) => {
                piped = Some((line, String::new()));
            }
            Event::Start(Tag::Link { link_type: LinkType::WikiLink { .. }, dest_url, .. }) => {
                out.push(Link { line, target: Target::Wiki(dest_url.to_string()) });
            }
            Event::Start(Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. }) => {
                out.push(Link { line, target: Target::Url(dest_url.to_string()) });
            }
            Event::Text(t) | Event::Code(t) => piped.iter_mut().for_each(|(_, page)| page.push_str(t)),
            Event::End(TagEnd::Link) => {
                out.extend(piped.take().map(|(line, page)| Link { line, target: Target::Wiki(page) }));
            }
            _ => {}
        }
        if let Some((start, raw)) = html.feed(&event, range.start) {
            for a in attributes(&raw).into_iter().filter(|a| a.name == "src" || a.name == "href") {
                let line = lines.at(start) + raw[..a.at].matches('\n').count();
                out.push(Link { line, target: Target::Url(a.value) });
            }
        }
    }
    out.sort_by_key(|l| l.line);
    out
}

/// Line numbers of byte offsets.
struct Lines(Vec<usize>);

impl Lines {
    fn new(text: &str) -> Self {
        Lines(std::iter::once(0).chain(text.match_indices('\n').map(|(i, _)| i + 1)).collect())
    }

    /// The 1-based line of byte offset `at`.
    fn at(&self, at: usize) -> usize {
        self.0.partition_point(|start| *start <= at)
    }
}
