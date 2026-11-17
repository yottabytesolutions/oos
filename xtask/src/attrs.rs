// SPDX-License-Identifier: GPL-2.0-or-later
//! The names of the attributes on an item as the compiler applies them: `cfg_attr(pred, a, b)` contributes `a` and
//! `b` (nested `cfg_attr` too), whatever its predicate, and raw identifiers count by their name (`r#path` is `path`).

use proc_macro2::{TokenStream, TokenTree};
use syn::ext::IdentExt;
use syn::Attribute;

/// The token trees of `ts` split at its top-level commas.
fn split_top(ts: TokenStream) -> Vec<Vec<TokenTree>> {
    let mut parts = vec![Vec::new()];
    for t in ts {
        match &t {
            TokenTree::Punct(p) if p.as_char() == ',' => parts.push(Vec::new()),
            _ => parts.last_mut().map_or((), |v| v.push(t)),
        }
    }
    parts.retain(|p| !p.is_empty());
    parts
}

/// One applied attribute: its name and the tokens after the name (`(..)` group or `= value`).
#[derive(Debug, Clone)]
pub struct Applied {
    pub name: String,
    pub rest: Vec<TokenTree>,
}

/// The attributes inside a `cfg_attr` argument list (after its predicate).
fn from_cfg_attr(args: TokenStream, out: &mut Vec<Applied>) {
    for part in split_top(args).into_iter().skip(1) {
        let Some(TokenTree::Ident(name)) = part.first() else { continue };
        let name = name.unraw().to_string();
        match (name.as_str(), part.get(1)) {
            ("cfg_attr", Some(TokenTree::Group(g))) => from_cfg_attr(g.stream(), out),
            _ => out.push(Applied { name, rest: part[1..].to_vec() }),
        }
    }
}

/// The attributes `a` applies: itself, or those inside it for `cfg_attr`.
pub fn applied(a: &Attribute) -> Vec<Applied> {
    let own = a.path().get_ident().map_or_else(
        || a.path().segments.iter().map(|s| s.ident.unraw().to_string()).collect::<Vec<_>>().join("::"),
        |i| i.unraw().to_string(),
    );
    let tokens = match &a.meta {
        syn::Meta::Path(_) => TokenStream::new(),
        syn::Meta::List(l) => l.tokens.clone(),
        syn::Meta::NameValue(nv) => {
            use quote::ToTokens;
            let mut t = TokenStream::new();
            nv.eq_token.to_tokens(&mut t);
            nv.value.to_tokens(&mut t);
            return vec![Applied { name: own, rest: t.into_iter().collect() }];
        }
    };
    if own != "cfg_attr" {
        let rest = if tokens.is_empty() {
            Vec::new()
        } else {
            vec![TokenTree::Group(proc_macro2::Group::new(proc_macro2::Delimiter::Parenthesis, tokens))]
        };
        return vec![Applied { name: own, rest }];
    }
    let mut out = Vec::new();
    from_cfg_attr(tokens, &mut out);
    out
}

/// The names `a` applies: its own, or those inside it for `cfg_attr`.
pub fn names(a: &Attribute) -> Vec<String> {
    applied(a).into_iter().map(|x| x.name).collect()
}

/// The paths in the argument list of an applied attribute: `clippy::disallowed_methods` for
/// `allow(clippy::disallowed_methods, dead_code)`.
pub fn list_paths(x: &Applied) -> Vec<String> {
    let Some(TokenTree::Group(g)) = x.rest.first() else { return Vec::new() };
    split_top(g.stream())
        .into_iter()
        .map(|part| {
            part.iter()
                .map(|t| match t {
                    TokenTree::Ident(i) => i.unraw().to_string(),
                    other => other.to_string(),
                })
                .collect::<String>()
        })
        .collect()
}

/// The string value of a `name = "value"` attribute.
pub fn string_value(x: &Applied) -> Option<String> {
    match x.rest.as_slice() {
        [TokenTree::Punct(p), TokenTree::Literal(l)] if p.as_char() == '=' => {
            syn::parse2::<syn::LitStr>(TokenTree::Literal(l.clone()).into()).ok().map(|s| s.value())
        }
        _ => None,
    }
}

/// Whether any of `attrs` applies one of `wanted`.
pub fn any(attrs: &[Attribute], wanted: &[&str]) -> bool {
    attrs.iter().flat_map(names).any(|n| wanted.contains(&n.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attrs(src: &str) -> Vec<Attribute> {
        let f: syn::ItemMod = syn::parse_str(&format!("{} mod m;", src)).expect("parses");
        f.attrs
    }

    // The holes: `r#path` and `cfg_attr(.., unsafe(no_mangle))` escaped the token-string match; `doc = "a,path"`
    // was taken for a path attribute.
    #[test]
    fn cfg_attr_and_raw_identifiers_resolve_to_names() {
        assert!(any(&attrs("#[r#path = \"x.rs\"]"), &["path"]));
        assert!(any(&attrs("#[cfg_attr(all(), r#path = \"x.rs\")]"), &["path"]));
        assert!(any(&attrs("#[cfg_attr(test, inline, cfg_attr(all(), path = \"x.rs\"))]"), &["path"]));
        assert!(any(&attrs("#[cfg_attr(all(), unsafe(no_mangle))]"), &["unsafe"]));
        assert!(!any(&attrs("#[cfg_attr(all(), doc = \"a,path\")]"), &["path"]));
        assert!(!any(&attrs("#[doc = \"path = x\"]"), &["path"]));
        assert_eq!(names(&attrs("#[rustfmt::skip]")[0]), ["rustfmt::skip"]);
    }

    #[test]
    fn lists_and_values_come_through_cfg_attr() {
        let a = attrs("#[cfg_attr(all(), allow(dead_code, clippy::disallowed_methods))]");
        assert_eq!(list_paths(&applied(&a[0])[0]), ["dead_code", "clippy::disallowed_methods"]);
        let b = attrs("#[allow(clippy :: disallowed_methods)]");
        assert_eq!(list_paths(&applied(&b[0])[0]), ["clippy::disallowed_methods"]);
        let c = attrs("#[ignore = \"known: task 0012\"]");
        assert_eq!(string_value(&applied(&c[0])[0]).as_deref(), Some("known: task 0012"));
        let d = attrs("#[cfg_attr(test, ignore = \"known: task 7\")]");
        assert_eq!(string_value(&applied(&d[0])[0]).as_deref(), Some("known: task 7"));
    }
}
