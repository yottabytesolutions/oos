// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;

const HEADER: &str = "// SPDX-License-Identifier: GPL-2.0-or-later\n";

#[test]
fn a_clean_source_breaks_no_rule() {
    assert!(findings("ui/src/a.rs", format!("{HEADER}fn a() {{\n    b();\n}}\n").as_bytes()).is_empty());
}

#[test]
fn trailing_spaces_tabs_and_cr_line_ends_are_found() {
    for line in ["a \n", "a\t\n", "a\r\n", "a\n\nb \n"] {
        assert_eq!(findings("docs/a.md", line.as_bytes()), vec![Rule::TrailingSpace], "{line:?}");
    }
}

#[test]
fn a_text_file_ends_with_a_newline_and_an_empty_file_needs_none() {
    assert_eq!(findings("docs/a.md", b"a"), vec![Rule::NoFinalNewline]);
    assert!(findings("docs/a.md", b"").is_empty());
}

#[test]
fn the_spdx_line_is_needed_in_the_first_five_lines_of_our_sources() {
    let late = format!("#!/bin/sh\n#\n#\n#\n#\n{HEADER}");
    assert_eq!(findings("tools/a.sh", late.as_bytes()), vec![Rule::NoSpdx]);
    let early = format!("#!/bin/sh\n#\n#\n#\n{HEADER}");
    assert!(findings("tools/a.sh", early.as_bytes()).is_empty());
    for path in ["a.rs", "a.py", "a.c", "a.h"] {
        assert_eq!(findings(path, b"x\n"), vec![Rule::NoSpdx], "{path}");
    }
    assert!(findings("docs/a.md", b"x\n").is_empty());
}

#[test]
fn rust_is_indented_with_spaces_and_c_may_use_tabs() {
    let tabbed = format!("{HEADER}fn a() {{\n\tb();\n}}\n");
    assert_eq!(findings("a.rs", tabbed.as_bytes()), vec![Rule::RustTabs]);
    assert!(findings("gpu/a.c", tabbed.as_bytes()).is_empty());
}

#[test]
fn binary_files_have_no_line_rules() {
    assert!(findings("ui/tests/golden/a.png", b"\x89PNG\0 \r\n\0").is_empty());
}

#[test]
fn files_taken_from_elsewhere_keep_their_format() {
    for path in [
        "LICENSE",
        "kernel/tools/git-log-answers.txt",
        "kernel/modules/xdma/a.c",
        "kernel/patches/a.patch",
        "rootfs/initramfs/lib/firmware/a.txt",
        "rootfs/initramfs/etc/licenses/GPL-2.0.txt",
    ] {
        assert!(findings(path, b"a \n\tb").is_empty(), "{path}");
    }
    assert!(!findings("rootfs/initramfs/etc/licenses/THIRD-PARTY-LICENSES.md", b"a \n").is_empty());
}

#[test]
fn a_licence_copy_must_equal_its_original() {
    let dir = std::env::temp_dir().join(format!("xtask-text-copies-{}", std::process::id()));
    for (original, copy) in LICENCE_COPIES {
        for path in [original, copy] {
            let file = dir.join(path);
            std::fs::create_dir_all(file.parent().expect("parent")).expect("dir");
            std::fs::write(file, original).expect("write");
        }
    }
    assert!(copies(&dir).is_empty());
    std::fs::write(dir.join(LICENCE_COPIES[1].1), "changed").expect("write");
    std::fs::remove_file(dir.join(LICENCE_COPIES[2].1)).expect("remove");
    let found = copies(&dir);
    std::fs::remove_dir_all(&dir).expect("clean up");
    assert_eq!(found.len(), 2);
    assert!(found[0].starts_with(LICENCE_COPIES[1].1) && found[1].starts_with(LICENCE_COPIES[2].1));
}

#[test]
fn the_check_names_each_tracked_file_that_breaks_a_rule() {
    let d = crate::testrepo::repo("text-check", &[(&[("docs/a.md", "a \n"), ("ui/src/b.rs", "fn b() {}\n")], "base")]);
    let found = check(&Default::default(), &d).expect_err("two files break rules");
    std::fs::remove_dir_all(&d).expect("clean up");
    assert!(found.contains(&format!("docs/a.md: {}", Rule::TrailingSpace.says())));
    assert!(found.contains(&format!("ui/src/b.rs: {}", Rule::NoSpdx.says())));
}
