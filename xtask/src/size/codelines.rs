//! The lines of a file that hold code: lines with a token on them. Comments are no tokens; doc comments become
//! `#[doc]` attributes and are left out too. A token over several lines (a string literal) covers each of them.

use proc_macro2::{Delimiter, TokenStream, TokenTree};
use quote::ToTokens;
use std::collections::BTreeSet;

fn mark(span: proc_macro2::Span, out: &mut BTreeSet<usize>) {
    out.extend(span.start().line..=span.end().line);
}

/// `#` followed by a bracket group that starts with `doc`, or `!` then that group.
fn is_doc(group: &TokenTree) -> bool {
    match group {
        TokenTree::Group(g) if g.delimiter() == Delimiter::Bracket => {
            matches!(g.stream().into_iter().next(), Some(TokenTree::Ident(i)) if i == "doc")
        }
        _ => false,
    }
}

fn walk(ts: TokenStream, out: &mut BTreeSet<usize>) {
    let tokens: Vec<TokenTree> = ts.into_iter().collect();
    let mut i = 0;
    while i < tokens.len() {
        let t = &tokens[i];
        // `#[doc = ..]` and `#![doc = ..]`: comments, not code
        if let TokenTree::Punct(p) = t {
            if p.as_char() == '#' {
                let skip = match tokens.get(i + 1) {
                    Some(TokenTree::Punct(b)) if b.as_char() == '!' => {
                        tokens.get(i + 2).filter(|g| is_doc(g)).map(|_| 3)
                    }
                    Some(g) if is_doc(g) => Some(2),
                    _ => None,
                };
                if let Some(n) = skip {
                    i += n;
                    continue;
                }
            }
        }
        match t {
            TokenTree::Group(g) => {
                mark(g.span_open(), out);
                mark(g.span_close(), out);
                walk(g.stream(), out);
            }
            TokenTree::Literal(l) => mark(l.span(), out),
            other => mark(other.span(), out),
        }
        i += 1;
    }
}

/// The lines of `file` (1-based) that hold code.
pub fn of(file: &syn::File) -> BTreeSet<usize> {
    let mut out = BTreeSet::new();
    walk(file.to_token_stream(), &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_blank_lines_and_doc_comments_are_no_code() {
        let src = "/// doc\nfn f() {\n\n    // a comment\n    /* block\n       comment */\n    let s = \"a\nb\";\n}\n";
        let lines = of(&syn::parse_file(src).expect("parses"));
        assert_eq!(lines.into_iter().collect::<Vec<_>>(), [2, 7, 8, 9]);
    }
}
