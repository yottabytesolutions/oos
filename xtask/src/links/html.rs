// SPDX-License-Identifier: GPL-2.0-or-later
//! Attributes of raw HTML inside Markdown. pulldown-cmark hands an HTML block over one line per event; `Collect`
//! joins it again, so the scanner always sees whole tags, comments and quoted values.

use pulldown_cmark::{Event, Tag, TagEnd};

/// Joins the raw HTML of an event stream into scannable pieces.
#[derive(Debug, Default)]
pub(crate) struct Collect {
    block: Option<(usize, String)>,
}

impl Collect {
    /// Feeds one event that starts at byte offset `at`. Returns a finished piece of raw HTML with its start offset:
    /// an inline tag at once, an HTML block at its end.
    pub(crate) fn feed(&mut self, event: &Event, at: usize) -> Option<(usize, String)> {
        match event {
            Event::Start(Tag::HtmlBlock) => self.block = Some((at, String::new())),
            Event::End(TagEnd::HtmlBlock) => return self.block.take(),
            Event::Html(h) => match self.block.as_mut() {
                Some((_, text)) => text.push_str(h),
                None => return Some((at, h.to_string())),
            },
            Event::InlineHtml(h) => return Some((at, h.to_string())),
            _ => {}
        }
        None
    }
}

/// One attribute: its byte offset in the scanned text, its name in lower case and its decoded value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Attribute {
    pub at: usize,
    pub name: String,
    pub value: String,
}

/// Elements whose content is text, not markup.
const RAW_TEXT: [&str; 4] = ["script", "style", "textarea", "title"];

/// The attributes with a value in `html`. Comments, text and the content of raw-text elements have none, and text
/// inside a quoted value is part of that value.
pub(crate) fn attributes(html: &str) -> Vec<Attribute> {
    let lower = html.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(lt) = html[i..].find('<').map(|p| i + p) {
        if html[lt..].starts_with("<!--") {
            i = html[lt + 4..].find("-->").map_or(html.len(), |p| lt + 4 + p + 3);
            continue;
        }
        let b = html.as_bytes();
        let closing = b.get(lt + 1) == Some(&b'/');
        let start = lt + 1 + usize::from(closing);
        if !b.get(start).is_some_and(u8::is_ascii_alphabetic) {
            i = lt + 1;
            continue;
        }
        let name_end = start + b[start..].iter().take_while(|c| c.is_ascii_alphanumeric() || **c == b'-').count();
        i = tag(html, name_end, &mut out);
        let name = &lower[start..name_end];
        if !closing && RAW_TEXT.contains(&name) {
            i = raw_text_end(&lower, i, name);
        }
    }
    out
}

/// The start of the `</name>` that closes a raw-text element opened before `i`. `</scripture>` does not close
/// `<script>`: the name must end at whitespace, `/` or `>`.
fn raw_text_end(lower: &str, mut i: usize, name: &str) -> usize {
    let close = format!("</{name}");
    while let Some(p) = lower[i..].find(&close).map(|p| i + p) {
        let after = lower.as_bytes().get(p + close.len());
        if after.is_none_or(|c| c.is_ascii_whitespace() || b"/>".contains(c)) {
            return p;
        }
        i = p + close.len();
    }
    lower.len()
}

/// The attributes of a tag from `i` (after its name) to its `>`; returns the position after the tag.
fn tag(html: &str, mut i: usize, out: &mut Vec<Attribute>) -> usize {
    let b = html.as_bytes();
    loop {
        i += b[i..].iter().take_while(|c| c.is_ascii_whitespace() || **c == b'/').count();
        match b.get(i) {
            None => return i,
            Some(b'>') => return i + 1,
            _ => {}
        }
        let name_end = i + b[i..].iter().take_while(|c| !c.is_ascii_whitespace() && !b"=>/".contains(c)).count();
        let after = name_end + b[name_end..].iter().take_while(|c| c.is_ascii_whitespace()).count();
        if b.get(after) != Some(&b'=') {
            i = name_end.max(i + 1);
            continue;
        }
        let v = after + 1 + b[after + 1..].iter().take_while(|c| c.is_ascii_whitespace()).count();
        let (value, next) = match b.get(v) {
            Some(&q @ (b'"' | b'\'')) => {
                let end = html[v + 1..].find(char::from(q)).map_or(html.len(), |p| v + 1 + p);
                (&html[v + 1..end], (end + 1).min(html.len()))
            }
            _ => {
                let end = v + b[v..].iter().take_while(|c| !c.is_ascii_whitespace() && **c != b'>').count();
                (&html[v..end], end)
            }
        };
        if !value.is_empty() {
            out.push(Attribute { at: i, name: html[i..name_end].to_ascii_lowercase(), value: decode(value) });
        }
        i = next;
    }
}

/// `value` with its character references decoded: numeric ones and the named ones Markdown authors use. Line ends
/// inside a value are dropped, as browsers do for URLs.
fn decode(value: &str) -> String {
    let mut out = String::new();
    let mut rest = value;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let reference = rest.find(';').map(|end| (&rest[1..end], end + 1));
        match reference.and_then(|(name, len)| character(name).map(|c| (c, len))) {
            Some((c, len)) => {
                out.push(c);
                rest = &rest[len..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out.replace(['\n', '\r'], "")
}

fn character(name: &str) -> Option<char> {
    let numeric = |digits: &str, radix| u32::from_str_radix(digits, radix).ok().and_then(char::from_u32);
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some('\u{a0}'),
        _ => match name.strip_prefix('#') {
            Some(hex) if hex.starts_with(['x', 'X']) => numeric(&hex[1..], 16),
            Some(dec) => numeric(dec, 10),
            None => None,
        },
    }
}
