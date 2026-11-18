// SPDX-License-Identifier: GPL-2.0-or-later
//! C lines, counted as production code per directory (ADR 0007): the GPU helper and the bring-up tool run on the
//! scope. A line counts when it holds something outside a comment. Vendored code and tests do not count.

use std::collections::BTreeMap;
use std::path::Path;

/// Vendored C: not this project's code.
const VENDORED: [&str; 1] = ["kernel/modules/xdma/"];

/// Lines with anything but blanks and comments (`//`, `/* */`); string and character literals are code.
pub fn lines(text: &str) -> u64 {
    let (mut block, mut n) = (false, 0);
    for line in text.lines() {
        let (mut code, mut quote, mut chars) = (false, None, line.chars().peekable());
        while let Some(c) = chars.next() {
            match (block, quote, c, chars.peek()) {
                (true, _, '*', Some('/')) => {
                    chars.next();
                    block = false;
                }
                (true, ..) => {}
                (_, Some(_), '\\', _) => {
                    chars.next();
                }
                (_, Some(q), c, _) if c == q => quote = None,
                (_, Some(_), ..) => {}
                (_, None, '/', Some('/')) => break,
                (_, None, '/', Some('*')) => {
                    chars.next();
                    block = true;
                }
                (_, None, '"' | '\'', _) => {
                    quote = Some(c);
                    code = true;
                }
                (_, None, c, _) => code |= !c.is_whitespace(),
            }
        }
        n += u64::from(code);
    }
    n
}

/// C lines among `files` under `root`, by directory. A missing file is skipped.
pub fn count(root: &Path, files: &[String]) -> Result<BTreeMap<String, u64>, String> {
    let mut out = BTreeMap::new();
    let own = |f: &&String| !VENDORED.iter().any(|v| f.starts_with(v)) && !super::tooling::is_test(f);
    for f in files.iter().filter(|f| f.ends_with(".c") || f.ends_with(".h")).filter(own) {
        let path = root.join(f);
        if !path.is_file() {
            continue;
        }
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {}", f, e))?;
        let dir = f.rsplit_once('/').map_or(".", |(d, _)| d);
        *out.entry(dir.to_string()).or_default() += lines(&text);
    }
    Ok(out)
}
