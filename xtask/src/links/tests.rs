// SPDX-License-Identifier: GPL-2.0-or-later
use super::parse::{links, Target};
use super::{anchors, broken, resolve, Tree};

fn url(s: &str) -> Target {
    Target::Url(s.into())
}

fn tree() -> Tree {
    let md = |p: &str, t: &str| (p.to_string(), Some(t.to_string()));
    Tree::new([
        md("README.md", "# OOS\n\n## Try it\n"),
        md("docs/models.md", "# Models\n\n## 2.1 Variants\n\n## Notes\n\n## Notes\n"),
        md("docs/tasks/done/0001-x.md", "# Task\n"),
        md("wiki/Home.md", "# Home\n"),
        md("wiki/Going-back-to-stock.md", "# Going back\n\n## In one-shot mode\n"),
        ("docs/images/main.png".to_string(), None),
    ])
}

#[test]
fn finds_inline_reference_html_and_wiki_links_outside_code() {
    // A reference definition cannot interrupt a paragraph (CommonMark), so it opens the text here.
    let text = "[ref]: docs/y.md\n\
                See [a](x.md) and ![b](img/c.png \"t\").\n\
                <img src=\"z.png\"> `[no](skip.md)` [[Going back to stock]] [[Text|Home]]\n\
                ```sh\n[no](fence.md)\n```\n\
                [link with (parens)](a_(b).md)";
    let found: Vec<_> = links(text).into_iter().map(|l| (l.line, l.target)).collect();
    assert_eq!(
        found,
        vec![
            (1, url("docs/y.md")),
            (2, url("x.md")),
            (2, url("img/c.png")),
            (3, url("z.png")),
            (3, Target::Wiki("Going back to stock".into())),
            (3, Target::Wiki("Home".into())),
            (7, url("a_(b).md")),
        ]
    );
}

#[test]
fn resolves_relative_paths_and_rejects_escapes() {
    assert_eq!(resolve("docs/tasks/done/a.md", "../../models.md").as_deref(), Some("docs/models.md"));
    assert_eq!(resolve("docs/a.md", "/README.md").as_deref(), Some("README.md"));
    assert_eq!(resolve("README.md", "../x"), None);
}

#[test]
fn headings_give_github_anchors() {
    assert_eq!(anchors::slug("2.1 Variants"), "21-variants");
    assert_eq!(anchors::slug("Install oos from a release!"), "install-oos-from-a-release");
    let a = anchors::of("## Install `oos` from [a release](x.md)!\n## Call `f](x)`\n");
    assert!(a.contains("install-oos-from-a-release") && a.contains("call-fx"), "{a:?}");
    let a = anchors::of("# A\n## A\n```\n# not\n```\n<a id=\"own\"></a>\n");
    assert!(a.contains("a") && a.contains("a-1") && a.contains("own") && !a.contains("not"));
}

#[test]
fn checks_paths_pages_own_urls_and_anchors() {
    let t = tree();
    let ok = [
        ("README.md", url("docs/models.md#21-variants")),
        ("README.md", url("docs/models.md#notes-1")),
        ("README.md", url("docs/models.md?plain=1#notes")),
        ("README.md", url("#try-it")),
        ("README.md", url("docs/images/")),
        ("README.md", url("https://example.invalid/x")),
        ("README.md", url("//cdn.example.invalid/image.png")),
        ("docs/tasks/done/0001-x.md", url("../../models.md")),
        ("wiki/Home.md", Target::Wiki("Going back to stock".into())),
        ("wiki/Home.md", url("Going-back-to-stock#in-one-shot-mode")),
        ("wiki/Home.md", url("https://github.com/yottabytesolutions/oos/blob/main/docs/models.md#notes")),
        ("wiki/Home.md", url("https://raw.githubusercontent.com/yottabytesolutions/oos/main/docs/images/main.png")),
        ("README.md", url("https://github.com/yottabytesolutions/oos/wiki")),
        ("README.md", url("https://github.com/yottabytesolutions/oos/wiki/Going-back-to-stock")),
    ];
    for (file, target) in ok {
        assert_eq!(broken(&t, file, &target), None, "{file}: {target:?}");
    }
    let bad = [
        ("README.md", url("docs/missing.md")),
        ("README.md", url("docs/models.md#nothing")),
        ("README.md", url("docs/models.md#Notes")),
        ("README.md", url("docs/models.md?plain=1#nothing")),
        ("README.md", url("#nothing")),
        ("README.md", url("../outside.md")),
        ("wiki/Home.md", Target::Wiki("Installation".into())),
        ("wiki/Home.md", url("Installation")),
        ("wiki/Home.md", url("https://github.com/yottabytesolutions/oos/blob/main/docs/gone.md")),
        ("README.md", url("https://github.com/yottabytesolutions/oos/wiki/Missing-page")),
    ];
    for (file, target) in bad {
        assert!(broken(&t, file, &target).is_some(), "{file}: {target:?}");
    }
}

#[test]
fn reads_html_attributes_in_every_quoting_and_case() {
    let line = "<IMG SRC='a.png'> <a href=b.md>x</a> <a class=\"c\" HREF = \"d.md\"> <p data-src=\"no.png\">";
    let found: Vec<_> = links(line).into_iter().map(|l| l.target).collect();
    assert_eq!(found, vec![url("a.png"), url("b.md"), url("d.md")]);
    assert!(anchors::of("<a name='own'></a> <span ID=other></span>\n")
        .is_superset(&["own".to_string(), "other".to_string()].into_iter().collect()));
}

#[test]
fn follows_commonmark_fences_and_split_definitions() {
    let text =
        "    ```\n[a](a.md)\n````\n```\n[no](no.md)\n````\n[b](b.md)\n\n[manual]:\n  <c.md> \"Title\"\n[x]:\n\n[d](d.md)";
    let found: Vec<_> = links(text).into_iter().map(|l| (l.line, l.target)).collect();
    assert_eq!(found, vec![(2, url("a.md")), (7, url("b.md")), (9, url("c.md")), (13, url("d.md"))]);
}

#[test]
fn skips_indented_code_but_not_list_continuations() {
    let text = "Text\n    [para](p.md)\n\n    [code](no.md)\n\n- item\n\n    [in item](i.md)\n\nEnd\n\n\t[tab](no2.md)";
    let found: Vec<_> = links(text).into_iter().map(|l| (l.line, l.target)).collect();
    assert_eq!(found, vec![(2, url("p.md")), (8, url("i.md"))]);
}

#[test]
fn reads_attributes_continued_on_the_next_line() {
    let found: Vec<_> = links("<img\nsrc=\"a.png\"\n  alt=x>").into_iter().map(|l| (l.line, l.target)).collect();
    assert_eq!(found, vec![(2, url("a.png"))]);
}

#[test]
fn setext_headings_give_anchors() {
    let a = anchors::of("Title\n=====\n\nSecond part\nof it\n---\n\n---\n\n- item\n---\n");
    let want: std::collections::BTreeSet<String> = ["title", "second-part-of-it"].map(String::from).into();
    assert_eq!(a, want);
}

#[test]
fn explicit_anchors_keep_their_case_and_atx_takes_a_tab() {
    let md = |p: &str, t: &str| (p.to_string(), Some(t.to_string()));
    let t = Tree::new([md("a.md", "<a id=\"ReleaseNotes\"></a>\n##\tTabbed Title\n")]);
    for frag in ["#ReleaseNotes", "#tabbed-title"] {
        assert_eq!(broken(&t, "a.md", &url(frag)), None, "{frag}");
    }
}

#[test]
fn skips_html_comments_and_indented_runs_inside_fences() {
    let text = "a <!-- [x](x.md) --> [b](b.md) <!--\n[y](y.md)\n--> [c](c.md)\n```\n    ```\n[z](z.md)\n```\n[d](d.md)";
    let found: Vec<_> = links(text).into_iter().map(|l| (l.line, l.target)).collect();
    assert_eq!(found, vec![(1, url("b.md")), (3, url("c.md")), (8, url("d.md"))]);
}

#[test]
fn comments_code_and_block_quotes_do_not_leak_into_each_other() {
    let text = "<!--\n```\n-->\n[a](a.md)\n`<!--` [b](b.md)\n> ```\n> [no](no.md)\n> ```\n> [c](c.md)";
    let found: Vec<_> = links(text).into_iter().map(|l| (l.line, l.target)).collect();
    assert_eq!(found, vec![(4, url("a.md")), (5, url("b.md")), (9, url("c.md"))]);
    let a = anchors::of("<!-- <a id=\"old\"></a> -->\n> ## Quoted\n`<a id=\"code\">`\n");
    let want: std::collections::BTreeSet<String> = ["quoted"].map(String::from).into();
    assert_eq!(a, want);
}

#[test]
fn raw_html_blocks_and_definitions_render_no_links_or_headings() {
    let text = "<pre>\n[no](no.md)\n</pre>\n<SCRIPT type=x>\n[no](no2.md)\n</script>\n[a](a.md)";
    let found: Vec<_> = links(text).into_iter().map(|l| (l.line, l.target)).collect();
    assert_eq!(found, vec![(7, url("a.md"))]);
    assert!(anchors::of("[label]: /url\n---\n").is_empty());
    assert!(anchors::of("<pre>\n## Not\n</pre>\n").is_empty());
}

#[test]
fn attribute_text_inside_quoted_values_is_no_attribute() {
    let found: Vec<_> =
        links("<span title=' href=\"no.md\" '>x</span> <a href=\"a.md\">a</a>").into_iter().map(|l| l.target).collect();
    assert_eq!(found, vec![url("a.md")]);
    assert!(anchors::of("<span title='id=\"fake\"'></span>\n").is_empty());
}

#[test]
fn html_values_span_lines_decode_references_and_skip_raw_text() {
    let text = "<a href=\"missing\n.md\">x</a>\n\n<img src=\"images/main&#46;png\">\n\n<script>\nvar s = '<a href=\"no.md\">';\n</script>";
    let found: Vec<_> = links(text).into_iter().map(|l| (l.line, l.target)).collect();
    assert_eq!(found, vec![(1, url("missing.md")), (4, url("images/main.png"))]);
}

#[test]
fn a_longer_tag_name_does_not_close_raw_text() {
    let text = "<script>\nvar s = '</scripture><a href=\"no.md\">';\n</script >\n<a href=\"yes.md\">y</a>";
    let found: Vec<_> = links(text).into_iter().map(|l| l.target).collect();
    assert_eq!(found, vec![url("yes.md")]);
}

#[test]
fn explicit_anchors_do_not_shift_heading_repeats() {
    let a = anchors::of("<a id=\"foo\"></a>\n\n# Foo\n");
    let want: std::collections::BTreeSet<String> = ["foo"].map(String::from).into();
    assert_eq!(a, want);
}
