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

/// The attributes inside a `cfg_attr` argument list (after its predicate).
fn from_cfg_attr(args: TokenStream, out: &mut Vec<String>) {
    for part in split_top(args).into_iter().skip(1) {
        let Some(TokenTree::Ident(name)) = part.first() else { continue };
        let name = name.unraw().to_string();
        match (name.as_str(), part.get(1)) {
            ("cfg_attr", Some(TokenTree::Group(g))) => from_cfg_attr(g.stream(), out),
            _ => out.push(name),
        }
    }
}

/// The names `a` applies: its own, or those inside it for `cfg_attr`.
pub fn names(a: &Attribute) -> Vec<String> {
    let own = a.path().get_ident().map_or_else(
        || a.path().segments.iter().map(|s| s.ident.unraw().to_string()).collect::<Vec<_>>().join("::"),
        |i| i.unraw().to_string(),
    );
    if own != "cfg_attr" {
        return vec![own];
    }
    let mut out = Vec::new();
    if let Ok(l) = a.meta.require_list() {
        from_cfg_attr(l.tokens.clone(), &mut out);
    }
    out
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
}
