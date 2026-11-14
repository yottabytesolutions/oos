//! The text backstop of the confinements: the paths spelled in a file's tokens outside test code, macro bodies
//! included, which name resolution cannot see. Comments and literals are no tokens of a path.

use super::resolve::{plain, test_only};
use proc_macro2::{Spacing, TokenStream, TokenTree};
use quote::ToTokens;

/// The attributes of an item that test code is marked with.
fn attrs(item: &syn::Item) -> &[syn::Attribute] {
    match item {
        syn::Item::Fn(i) => &i.attrs,
        syn::Item::Mod(i) => &i.attrs,
        syn::Item::Impl(i) => &i.attrs,
        syn::Item::Macro(i) => &i.attrs,
        syn::Item::Const(i) => &i.attrs,
        syn::Item::Static(i) => &i.attrs,
        syn::Item::Struct(i) => &i.attrs,
        syn::Item::Enum(i) => &i.attrs,
        syn::Item::Trait(i) => &i.attrs,
        syn::Item::Type(i) => &i.attrs,
        syn::Item::Use(i) => &i.attrs,
        _ => &[],
    }
}

/// The runs of `a::b::c` in `ts`, into nested groups; a run ends at any other token. `$crate` gives `crate`.
fn runs(ts: TokenStream, out: &mut Vec<Vec<String>>) {
    let toks: Vec<TokenTree> = ts.into_iter().collect();
    let mut cur: Vec<String> = Vec::new();
    let mut sep = false;
    let flush = |cur: &mut Vec<String>, out: &mut Vec<Vec<String>>| {
        if cur.len() > 1 {
            out.push(std::mem::take(cur));
        }
        cur.clear();
    };
    let mut i = 0;
    while i < toks.len() {
        match &toks[i] {
            TokenTree::Ident(id) => {
                if !sep {
                    flush(&mut cur, out);
                }
                cur.push(plain(id));
                sep = false;
            }
            TokenTree::Punct(p)
                if p.as_char() == ':'
                    && p.spacing() == Spacing::Joint
                    && matches!(toks.get(i + 1), Some(TokenTree::Punct(q)) if q.as_char() == ':') =>
            {
                sep = true;
                i += 1;
            }
            TokenTree::Group(g) => {
                flush(&mut cur, out);
                sep = false;
                runs(g.stream(), out);
            }
            _ => {
                flush(&mut cur, out);
                sep = false;
            }
        }
        i += 1;
    }
    flush(&mut cur, out);
}

/// The runs of `items` outside test code, each with its module; inline modules nest.
fn walk(items: &[syn::Item], here: &str, out: &mut Vec<(String, Vec<String>)>) {
    let push = |ts: TokenStream, out: &mut Vec<(String, Vec<String>)>| {
        let mut r = Vec::new();
        runs(ts, &mut r);
        out.extend(r.into_iter().map(|r| (here.to_string(), r)));
    };
    for item in items.iter().filter(|i| !test_only(attrs(i))) {
        match item {
            syn::Item::Mod(m) => {
                if let Some((_, inner)) = &m.content {
                    let name = plain(&m.ident);
                    let next = if here.is_empty() { name } else { format!("{}::{}", here, name) };
                    walk(inner, &next, out);
                }
            }
            syn::Item::Impl(i) => {
                let mut i = i.clone();
                i.items.retain(|x| !matches!(x, syn::ImplItem::Fn(f) if test_only(&f.attrs)));
                push(i.to_token_stream(), out);
            }
            other => push(other.to_token_stream(), out),
        }
    }
}

/// The paths (`a::b::c`, as segments) spelled in the tokens of `file`, the source of `module`, outside test code, each
/// with the module it is in.
pub fn spelled(file: &syn::File, module: &str) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    walk(&file.items, module, &mut out);
    out
}

/// Whether `run` spells `path` (`board::dho`) as consecutive segments.
pub fn spells(run: &[String], path: &[&str]) -> bool {
    !path.is_empty() && run.windows(path.len()).any(|w| w.iter().zip(path).all(|(a, b)| a == b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runs(src: &str, module: &str) -> Vec<(String, String)> {
        let f = syn::parse_file(src).expect("parses");
        spelled(&f, module).into_iter().map(|(m, r)| (m, r.join("::"))).collect()
    }

    // The hole: a macro body is tokens to syn, so `crate::board::dho::Dho` inside `macro_rules!` named nothing.
    #[test]
    fn paths_in_macros_are_spelled_and_test_code_comments_and_literals_are_not() {
        let src = "macro_rules! board_ty { () => { crate::board::dho::Dho } }\n\
                   fn f(_: board_ty!()) { let _ = \"board::dho\"; } // board::dho\n\
                   mod m { fn g() { m!($crate::r#board::dho::x); } }\n\
                   #[cfg(test)]\nmod tests { fn t() { crate::board::dho::y(); } }\n#[test]\nfn t() { oos_hal::dho::z(); }\n";
        let r = runs(src, "ui");
        assert!(r.contains(&("ui".into(), "crate::board::dho::Dho".into())), "{:?}", r);
        assert!(r.contains(&("ui::m".into(), "crate::board::dho::x".into())), "{:?}", r);
        assert_eq!(r.iter().filter(|(_, x)| x.contains("dho")).count(), 2, "{:?}", r);
        let segs = |s: &str| s.split("::").map(str::to_string).collect::<Vec<_>>();
        assert!(spells(&segs("crate::board::dho::Dho"), &["board", "dho"]));
        assert!(
            !spells(&segs("crate::boarding::dho"), &["board", "dho"]) && !spells(&segs("board"), &["board", "dho"])
        );
    }
}
