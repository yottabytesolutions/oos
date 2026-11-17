// SPDX-License-Identifier: GPL-2.0-or-later
//! Links in README.md, SECURITY.md, wiki/ and docs/ resolve: a relative path names a file or directory of the
//! repository, a wiki link names a page in wiki/, a link to this repository on GitHub names an existing path or page,
//! and a `#fragment` into a Markdown file names one of its headings or HTML anchors.

mod anchors;
mod html;
mod parse;
#[cfg(test)]
mod tests;

use crate::steps::{outcome, Outcome};
use parse::Target;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// The repository on GitHub. Links to its files on `main` and to its wiki are checked like relative links.
const REPO: &str = "github.com/yottabytesolutions/oos";
const RAW: &str = "raw.githubusercontent.com/yottabytesolutions/oos/main/";
const WIKI: &str = "wiki/";

/// The tracked and new files, every directory above them, and the anchors of the Markdown files.
#[derive(Debug, Default)]
pub(crate) struct Tree {
    files: BTreeSet<String>,
    dirs: BTreeSet<String>,
    anchors: BTreeMap<String, BTreeSet<String>>,
}

impl Tree {
    /// `files` with the text of each Markdown file, from which its anchors come.
    pub(crate) fn new(files: impl IntoIterator<Item = (String, Option<String>)>) -> Self {
        let mut tree = Tree::default();
        for (f, text) in files {
            let mut dir = f.as_str();
            while let Some((parent, _)) = dir.rsplit_once('/') {
                tree.dirs.insert(parent.to_string());
                dir = parent;
            }
            if let Some(text) = text {
                tree.anchors.insert(f.clone(), anchors::of(&text));
            }
            tree.files.insert(f);
        }
        tree
    }

    fn exists(&self, path: &str) -> bool {
        let path = path.trim_end_matches('/');
        path.is_empty() || self.files.contains(path) || self.dirs.contains(path)
    }

    /// The file of wiki page `name` (`Page Name`, `Page-Name` or `Page-Name.md`).
    fn page(name: &str) -> String {
        let name = name.replace(' ', "-");
        format!("{WIKI}{}.md", name.strip_suffix(".md").unwrap_or(&name))
    }

    /// Why `path#fragment` does not resolve, or None.
    fn target(&self, path: &str, fragment: Option<&str>, url: &str) -> Option<String> {
        if !self.exists(path) {
            return Some(format!("{url}: no {path} in the repository"));
        }
        let fragment = fragment.filter(|f| !f.is_empty())?;
        let known = self.anchors.get(path)?;
        let fragment = decode(fragment);
        // Fragments match case-sensitively: heading anchors are lower case, explicit `id` and `name` keep theirs.
        (!known.contains(&fragment)).then(|| format!("{url}: no heading #{fragment} in {path}"))
    }
}

/// Whether the gate reads the links of `path`.
pub(crate) fn scanned(path: &str) -> bool {
    path == "README.md"
        || path == "SECURITY.md"
        || (path.ends_with(".md") && (path.starts_with("docs/") || path.starts_with(WIKI)))
}

pub fn check(ctx: &crate::run::Context, root: &Path) -> Outcome {
    let mut files = Vec::new();
    for f in inventory(ctx, root).map_err(|e| vec![e])? {
        let text = if f.ends_with(".md") {
            Some(std::fs::read_to_string(root.join(&f)).map_err(|e| vec![format!("{f}: {e}")])?)
        } else {
            None
        };
        files.push((f, text));
    }
    let texts: Vec<(String, String)> =
        files.iter().filter(|(f, _)| scanned(f)).filter_map(|(f, t)| Some((f.clone(), t.clone()?))).collect();
    let tree = Tree::new(files);
    let mut bad = Vec::new();
    let mut count = 0usize;
    for (file, text) in &texts {
        for link in parse::links(text) {
            count += 1;
            if let Some(why) = broken(&tree, file, &link.target) {
                bad.push(format!("{file}:{}: {why}", link.line));
            }
        }
    }
    outcome(vec![format!("{count} links in README.md, SECURITY.md, wiki/ and docs/ resolve")], bad)
}

/// Why `target` in `file` does not resolve, or None when it does.
pub(crate) fn broken(tree: &Tree, file: &str, target: &Target) -> Option<String> {
    match target {
        Target::Wiki(page) if page.contains("://") => None,
        Target::Wiki(page) => {
            let (name, fragment) = split(page);
            tree.target(&Tree::page(name), fragment, &format!("[[{page}]]"))
        }
        Target::Url(url) => url_broken(tree, file, url),
    }
}

/// `url` split into its path, without the query, and its fragment (`path?query#fragment`).
fn split(url: &str) -> (&str, Option<&str>) {
    let (rest, fragment) = match url.split_once('#') {
        Some((r, f)) => (r, Some(f)),
        None => (url, None),
    };
    (rest.split('?').next().unwrap_or(""), fragment)
}

fn url_broken(tree: &Tree, file: &str, url: &str) -> Option<String> {
    let (path, fragment) = split(url);
    if let Some(own) = own(path) {
        return match own {
            Own::Path(p) => tree.target(decode(p).trim_end_matches('/'), fragment, url),
            Own::Page("") => None,
            Own::Page(p) => tree.target(&Tree::page(&decode(p)), fragment, url),
        };
    }
    if scheme(url) {
        return None;
    }
    let path = decode(path);
    if path.is_empty() {
        return tree.target(file, fragment, url);
    }
    if file.starts_with(WIKI) && !path.contains('/') && !path.contains('.') {
        return tree.target(&Tree::page(&path), fragment, url);
    }
    match resolve(file, &path) {
        Some(p) => tree.target(&p, fragment, url),
        None => Some(format!("{url}: leaves the repository")),
    }
}

enum Own<'a> {
    Path(&'a str),
    Page(&'a str),
}

/// The repository path or wiki page of a link (without query and fragment) to this repository on GitHub.
fn own(url: &str) -> Option<Own<'_>> {
    let rest = url.strip_prefix("https://")?;
    if let Some(p) = rest.strip_prefix(RAW) {
        return Some(Own::Path(p));
    }
    let rest = rest.strip_prefix(REPO)?;
    for prefix in ["/blob/main/", "/tree/main/", "/raw/main/"] {
        if let Some(p) = rest.strip_prefix(prefix) {
            return Some(Own::Path(p));
        }
    }
    match rest {
        "/wiki" | "/wiki/" => Some(Own::Page("")),
        _ => rest.strip_prefix("/wiki/").map(Own::Page),
    }
}

/// Whether `url` leaves the repository by itself: a URI scheme (`https:`, `mailto:`) or a network path (`//host/`).
fn scheme(url: &str) -> bool {
    url.starts_with("//")
        || url.split_once(':').is_some_and(|(s, _)| {
            !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        })
}

/// `target` relative to the directory of `file` (or to the root when it starts with `/`), normalised; None when it
/// climbs out of the repository.
pub(crate) fn resolve(file: &str, target: &str) -> Option<String> {
    let mut parts: Vec<&str> = if target.starts_with('/') {
        Vec::new()
    } else {
        file.split('/').collect::<Vec<_>>().split_last().map(|(_, d)| d.to_vec()).unwrap_or_default()
    };
    for part in target.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            p => parts.push(p),
        }
    }
    Some(parts.join("/"))
}

/// Percent-decoded `s`; an invalid escape stays as it is.
fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes.get(i + 1..i + 3).and_then(|h| std::str::from_utf8(h).ok());
        match (bytes[i], hex.and_then(|h| u8::from_str_radix(h, 16).ok())) {
            (b'%', Some(b)) => {
                out.push(b);
                i += 3;
            }
            (b, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Tracked and new, not ignored files that exist in the work tree, as Git lists them.
fn inventory(ctx: &crate::run::Context, root: &Path) -> Result<Vec<String>, String> {
    let out = crate::run::capture_raw(
        ctx,
        root,
        "git",
        &["ls-files", "-z", "--cached", "--others", "--exclude-standard"],
        &[],
    )
    .map_err(|e| format!("git ls-files: {e}"))?;
    if !out.status.success() {
        return Err(format!("git ls-files failed: {}", String::from_utf8_lossy(&out.stderr)));
    }
    let mut paths = Vec::new();
    for p in out.stdout.split(|b| *b == 0).filter(|p| !p.is_empty()) {
        let p = String::from_utf8(p.to_vec()).map_err(|_| "git ls-files: non-UTF-8 path".to_string())?;
        if root.join(&p).is_file() {
            paths.push(p);
        }
    }
    Ok(paths)
}
