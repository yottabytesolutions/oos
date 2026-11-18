// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;

fn record(id: u16, slug: &str) -> String {
    format!("docs/adr/{id:04}-{slug}.md")
}

#[test]
fn duplicate_numbers_fail_even_with_different_slugs() {
    let paths = [record(1, "first"), record(1, "other")];
    let (_, bad) = records(&paths.iter().collect::<Vec<_>>());
    assert_eq!(bad.len(), 1);
    assert!(bad[0].contains("duplicate"));
}

#[test]
fn nested_records_cannot_reintroduce_a_second_series() {
    let flat = record(1, "first");
    let nested = flat.replace("adr/", &format!("adr/{}/", "code"));
    let (_, bad) = records(&[&flat, &nested]);
    assert_eq!(bad.len(), 2);
    assert!(bad.iter().any(|s| s.contains("duplicate")));
    assert!(bad.iter().any(|s| s.contains("directly")));
}

#[test]
fn number_citations_resolve_without_chained_renumbering() {
    let paths = [record(9, "protected"), record(11, "recovery")];
    let (records, bad) = records(&paths.iter().collect::<Vec<_>>());
    assert!(bad.is_empty());
    let tree = Tree::new(paths.into_iter().map(|p| (p, None)));
    for id in [9, 11] {
        let text = format!("// ADR {id:04}; adr/{id:04}");
        assert!(references(&tree, &records, "source.rs", &text).is_empty());
    }
    let missing = format!("# ADR {:04}", 12);
    assert_eq!(references(&tree, &records, "source.rs", &missing).len(), 1);
    assert!(citations("ADR NNNN and ADR 12345").next().is_none());
}

#[test]
fn legacy_architecture_citations_fail_even_when_the_number_has_a_current_record() {
    let paths: Vec<_> = (1..=19).map(|id| record(id, "current")).collect();
    let (records, _) = records(&paths.iter().collect::<Vec<_>>());
    let tree = Tree::new(paths.into_iter().map(|p| (p, None)));
    for id in 1..=19 {
        for text in [format!("// architecture/{id:04}"), format!("// docs/architecture/{id:04}-old.md")] {
            let bad = references(&tree, &records, "source.rs", &text);
            assert_eq!(bad, [format!("source.rs:1: legacy architecture/{id:04} citation; use ADR NNNN")]);
        }
    }
}

#[test]
fn paths_check_the_filename_and_relative_location_not_just_the_number() {
    let file = record(1, "real");
    let (records, _) = records(&[&file]);
    let tree = Tree::new([(file.clone(), None)]);
    for text in [format!("[decision](adr/{}).", file.rsplit('/').next().unwrap()), format!("`{file}`")] {
        assert!(references(&tree, &records, "docs/guide.md", &text).is_empty());
    }
    for target in [record(1, "wrong-slug"), record(2, "missing")] {
        assert!(!references(&tree, &records, "code.rs", &format!("// {target}")).is_empty());
    }
    let obsolete = file.replace("adr/", &format!("adr/{}/", "code"));
    for target in [obsolete.clone(), obsolete.trim_end_matches(".md").into(), format!("docs/adr/{}/", "code")] {
        assert!(!references(&tree, &records, "code.rs", &target).is_empty());
    }
}

#[test]
fn inventory_checks_new_records_and_source_comments_and_ignores_binary_data() {
    let dir = std::env::temp_dir().join(format!("xtask-adr-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("docs/adr")).unwrap();
    let mut paths = vec![record(1, "real"), "image.png".into()];
    std::fs::write(dir.join(&paths[0]), "decision\n").unwrap();
    std::fs::write(dir.join(&paths[1]), format!("\0ADR {:04}", 3)).unwrap();
    let sources = ["source.rs", "helper.sh", "probe.py", "gpu.c", "gpu.h", "other.cpp"];
    for source in sources {
        std::fs::write(dir.join(source), format!("// ADR {:04}\n", 2)).unwrap();
        paths.push(source.into());
    }
    for dotfile in [".editorconfig", ".gitignore"] {
        std::fs::write(dir.join(dotfile), format!("# docs/adr/{}/{:04}\n", "process", 1)).unwrap();
        paths.push(dotfile.into());
    }
    let bad = check(&dir, &paths.iter().collect::<Vec<_>>());
    std::fs::remove_dir_all(dir).unwrap();
    assert_eq!(bad.len(), sources.len() + 2);
    for source in sources.into_iter().chain([".editorconfig", ".gitignore"]) {
        assert!(bad.iter().any(|finding| finding.starts_with(&format!("{source}:1:"))));
    }
}

#[test]
fn check_paragraphs_name_inventory_steps_including_continuation_lines() {
    for check in crate::inventory::CHECKS {
        assert!(check_steps("record.md", &format!("Check: `{}`; review.\n", check.id)).is_empty());
    }
    let bad = check_steps("record.md", "Check: `architecture`,\n`arch` and `ratchet`.\n\n`not-a-step`\n");
    assert_eq!(bad.len(), 2);
    assert!(bad[0].contains("record.md:2: unknown gate step arch"));
    assert!(bad[1].contains("record.md:2: unknown gate step ratchet"));
    assert!(check_steps("record.md", "Check: review only; the pr-meta CI check.\n").is_empty());
}
