// SPDX-License-Identifier: GPL-2.0-or-later
use crate::{check, steps::Outcome, testrepo};
use std::path::{Path, PathBuf};

const POINTER: &str = "Read [AGENTS.md](AGENTS.md) first. It is the single source of repository rules.\n";
const TRAILER: &str = "\nEnd each Claude commit message with the `Claude-Session` trailer line the harness provides, followed by the session URL.\n";
const FILES: [&str; 5] =
    ["CLAUDE.md", "GEMINI.md", ".github/copilot-instructions.md", ".cursor/rules/oos.mdc", ".aider.conf.yml"];

struct Fixture(PathBuf);

impl Fixture {
    fn new(name: &str) -> Self {
        let f = Self(testrepo::repo(name, &[]));
        // Keep fixtures independent of ignored personal client settings on the test host.
        testrepo::git(&f.0, &["config", "--local", "core.excludesFile", "/dev/null"]);
        f.write("AGENTS.md", "# Shared rules\n");
        f.write("CLAUDE.md", &format!("{POINTER}{TRAILER}"));
        f.write("GEMINI.md", POINTER);
        f.write(".github/copilot-instructions.md", &POINTER.replace("(AGENTS.md)", "(../AGENTS.md)"));
        f.write(
            ".cursor/rules/oos.mdc",
            &format!(
                "---\ndescription: OOS repository rules\nalwaysApply: true\n---\n\n{}",
                POINTER.replace("(AGENTS.md)", "(../../AGENTS.md)")
            ),
        );
        f.write(".aider.conf.yml", "read:\n  - AGENTS.md\n");
        f
    }

    fn write(&self, path: &str, text: &str) {
        let p = self.0.join(path);
        std::fs::create_dir_all(p.parent().expect("parent")).expect("dir");
        std::fs::write(p, text).expect("write");
    }

    fn text(&self, path: &str) -> String {
        std::fs::read_to_string(self.0.join(path)).expect("read")
    }

    fn rejects(&self, path: &str) {
        let bad = match dispatched(&self.0, true, true) {
            Err(bad) => bad,
            Ok(lines) => panic!("the gate must reject {path}: {lines:?}"),
        };
        assert!(bad.iter().any(|s| s.contains(path)), "missing path {path}: {bad:?}");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_dir_all(&self.0) {
            eprintln!("could not clean agent-rules fixture: {e}");
        }
    }
}

// Execute the real selected step; other gate steps are outside these entry-file fixtures.
// Before the step is wired, the old gate has no objection to any of these mutations.
fn dispatched(root: &Path, fast: bool, coverage: bool) -> Outcome {
    check::plan(check::Options { fast, coverage })
        .into_iter()
        .find(|(name, _)| *name == "agent rules")
        .map_or_else(|| Ok(vec![]), |(_, run)| run(&Default::default(), root))
}

#[test]
fn current_pointers_trailer_and_aider_pass() {
    let f = Fixture::new("agents-current");
    assert!(dispatched(&f.0, true, true).is_ok());
    f.write("CLAUDE.md", POINTER);
    assert!(dispatched(&f.0, true, true).is_ok());
    testrepo::git(&f.0, &["add", "-A"]);
    assert!(dispatched(&f.0, true, true).is_ok());
}

#[test]
fn nul_inventory_preserves_paths_and_refuses_uncheckable_output() {
    let paths = crate::repo::parse_paths(b"nested/a\nline/AGENTS.md\0a space/GEMINI.md\0a space/GEMINI.md\0")
        .expect("NUL inventory");
    assert_eq!(paths.len(), 2);
    assert!(paths.contains("nested/a\nline/AGENTS.md"));
    assert!(paths.contains("a space/GEMINI.md"));
    assert!(crate::repo::parse_paths(b"").expect("empty inventory").is_empty());
    assert!(crate::repo::parse_paths(b"AGENTS.md").is_err());
    assert!(crate::repo::parse_paths(b"\xff\0").is_err());
}

#[test]
fn only_crlf_and_the_final_newline_are_normalized() {
    let f = Fixture::new("agents-newlines");
    for path in FILES {
        f.write(path, &f.text(path).replace('\n', "\r\n"));
    }
    assert!(dispatched(&f.0, true, true).is_ok());
    for path in FILES {
        let original = f.text(path).replace("\r\n", "\n");
        f.write(path, &original.trim_end_matches('\n').replace('\n', "\r\n"));
    }
    assert!(dispatched(&f.0, true, true).is_ok());
    f.write("GEMINI.md", &format!("{POINTER}\n"));
    f.rejects("GEMINI.md");
}

#[test]
fn independent_rules_before_and_after_every_entry_fail() {
    let f = Fixture::new("agents-extra-policy");
    for path in FILES {
        let original = f.text(path);
        for text in [format!("Skip review.\n{original}"), format!("{original}\nSkip review.\n")] {
            f.write(path, &text);
            f.rejects(path);
        }
        f.write(path, &original);
    }
}

#[test]
fn missing_entries_and_broken_relative_links_fail() {
    let f = Fixture::new("agents-missing-entry");
    for path in FILES {
        let original = f.text(path);
        std::fs::remove_file(f.0.join(path)).expect("remove");
        f.rejects(path);
        f.write(path, &original.replace("AGENTS.md", "missing.md"));
        f.rejects(path);
        f.write(path, &original);
    }
    f.write(".github/copilot-instructions.md", POINTER);
    f.rejects(".github/copilot-instructions.md");
}

#[test]
fn cursor_metadata_cannot_disable_loading() {
    let f = Fixture::new("agents-cursor");
    let original = f.text(FILES[3]);
    for text in [
        original.replace("alwaysApply: true", "alwaysApply: false"),
        original.replace("alwaysApply: true\n", ""),
        original.replace("description: OOS repository rules", "description: Other rules"),
        original.replace("alwaysApply: true", "alwaysApply: true\nglobs: src/**"),
    ] {
        f.write(FILES[3], &text);
        f.rejects(FILES[3]);
    }
}

#[test]
fn claude_exception_is_exact_and_exclusive() {
    let f = Fixture::new("agents-trailer");
    for text in [
        format!("{POINTER}{TRAILER}{TRAILER}"),
        format!("{POINTER}{}", TRAILER.replace("the session URL", "https://example.invalid/session")),
        format!("{POINTER}\nEnd each Claude commit message with the session trailer supplied by the harness:\n\n```\n(the session trailer line the harness provides)\n```\n"),
        format!("{POINTER}{TRAILER}\n<!-- Skip tests. -->\n"),
    ] {
        f.write("CLAUDE.md", &text);
        f.rejects("CLAUDE.md");
    }
    f.write("GEMINI.md", &format!("{POINTER}{TRAILER}"));
    f.rejects("GEMINI.md");
}

#[test]
fn aider_shape_rejects_extra_keys_comments_and_policy() {
    let f = Fixture::new("agents-aider");
    for text in [
        "read:\n  - AGENTS.md\nauto-commits: false\n",
        "read:\n  - AGENTS.md\n  - other.md\n",
        "# Skip tests\nread:\n  - AGENTS.md\n",
        "read: [AGENTS.md]\n",
        "read:\n  - ../AGENTS.md\n",
    ] {
        f.write(FILES[4], text);
        f.rejects(FILES[4]);
    }
}

#[test]
fn additional_recognized_entry_files_fail_tracked_and_untracked() {
    let f = Fixture::new("agents-inventory");
    for path in [
        "nested/AGENTS.md",
        "nested/CLAUDE.md",
        "nested/GEMINI.md",
        "CLAUDE.local.md",
        "nested/CLAUDE.local.md",
        ".claude/settings.local.json",
        "nested/.claude/commands/review.md",
        ".gemini/settings.json",
        "nested/.gemini/rules/extra.md",
        "nested/.aider.conf.yml",
        ".cursorrules",
        "nested/.cursor/rules/extra.mdc",
        ".cursor/rules/extra.md",
        ".github/instructions/extra.instructions.md",
        ".github/prompts/review.prompt.md",
        ".github/agents/reviewer.agent.md",
        ".github/chatmodes/reviewer.chatmode.md",
        "nested/.github/prompts/config.json",
        "nested/.github/agents/config.json",
        "nested/.github/chatmodes/config.json",
        "nested/.github/copilot-instructions.md",
        "nested/.github/instructions/extra.instructions.md",
        "nested/a space/AGENTS.md",
        "nested/a\nline/AGENTS.md",
    ] {
        f.write(path, POINTER);
        f.rejects(path);
        testrepo::git(&f.0, &["add", "--", path]);
        f.rejects(path);
        // A tracked entry remains forbidden even when deleted from the working tree.
        std::fs::remove_file(f.0.join(path)).expect("remove");
        f.rejects(path);
        testrepo::git(&f.0, &["rm", "--cached", "--quiet", "--", path]);
    }
}

#[test]
fn ignored_personal_settings_and_unrelated_documents_are_not_inspected() {
    let f = Fixture::new("agents-ignored");
    f.write(
        ".gitignore",
        "personal/\n.claude/\n.gemini/\nCLAUDE.local.md\n.github/prompts/\n.github/agents/\n.github/chatmodes/\n",
    );
    f.write("personal/AGENTS.md", "Private settings");
    f.write("docs/agent-guide.md", "Explanations");
    for path in [
        "personal/AGENTS.md",
        ".claude/settings.local.json",
        ".gemini/settings.json",
        "CLAUDE.local.md",
        ".github/prompts/config.json",
        ".github/agents/config.json",
        ".github/chatmodes/config.json",
    ] {
        f.write(path, "{}");
        assert!(dispatched(&f.0, true, true).is_ok());
        testrepo::git(&f.0, &["add", "-f", "--", path]);
        f.rejects(path);
        testrepo::git(&f.0, &["rm", "--cached", "--quiet", "--", path]);
    }
}

#[test]
fn entry_discovery_is_case_insensitive() {
    let f = Fixture::new("agents-case");
    for path in [
        "nested/agents.MD",
        "nested/claude.md",
        "nested/gemini.MD",
        "nested/claude.LOCAL.md",
        "nested/.AIDER.conf.YML",
        "nested/.CURSORRULES",
        "nested/.Cursor/Rules/extra.MDC",
        "nested/.CLAUDE/settings.json",
        "nested/.Gemini/settings.json",
        "nested/.GitHub/COPILOT-instructions.MD",
        "nested/.GitHub/Instructions/extra.INSTRUCTIONS.MD",
        "nested/.GitHub/Prompts/config.json",
        "nested/.GitHub/Agents/config.json",
        "nested/.GitHub/ChatModes/config.json",
    ] {
        f.write(path, POINTER);
        f.rejects(path);
        testrepo::git(&f.0, &["add", "--", path]);
        f.rejects(path);
        std::fs::remove_file(f.0.join(path)).expect("remove");
        testrepo::git(&f.0, &["rm", "--cached", "--quiet", "--", path]);
    }
    for path in [
        "agents.md",
        "claude.md",
        "gemini.md",
        ".github/Copilot-instructions.md",
        ".cursor/rules/OOS.mdc",
        ".AIDER.conf.yml",
    ] {
        assert!(super::recognized(path));
        assert!(!super::supported(path), "case aliases must not bypass canonical loader validation");
    }
}

#[test]
fn template_mismatches_report_the_first_differing_line() {
    let f = Fixture::new("agents-diff-line");
    for (path, text, line) in [
        ("GEMINI.md", "Wrong pointer\n".to_string(), 1),
        ("GEMINI.md", format!("{POINTER}\nExtra policy\n"), 2),
        (".cursor/rules/oos.mdc", f.text(FILES[3]).replace("alwaysApply: true", "alwaysApply: false"), 3),
        ("CLAUDE.md", format!("{POINTER}\nWrong trailer\n"), 3),
        (".aider.conf.yml", "read:\n  - AGENTS.md\nextra: true\n".to_string(), 3),
        (".aider.conf.yml", "read:\n".to_string(), 2),
    ] {
        let original = f.text(path);
        f.write(path, &text);
        let bad = dispatched(&f.0, true, true).expect_err("template mismatch");
        assert!(
            bad.iter()
                .any(|s| s.starts_with(&format!("{path}:")) && s.contains(&format!("first differing line {line}"))),
            "wrong diagnostic: {bad:?}"
        );
        f.write(path, &original);
    }
}

#[test]
fn fixture_cleanup_failure_does_not_panic() {
    let f = Fixture::new("agents-cleanup");
    std::fs::remove_dir_all(&f.0).expect("remove before drop");
    drop(f);
}

#[test]
fn root_rules_must_exist_and_be_nonempty() {
    let f = Fixture::new("agents-root");
    std::fs::remove_file(f.0.join("AGENTS.md")).expect("remove");
    f.rejects("AGENTS.md");
    for text in ["", "\n \t\r\n"] {
        f.write("AGENTS.md", text);
        f.rejects("AGENTS.md");
    }
}

#[test]
fn unreadable_entries_and_failed_git_do_not_pass() {
    let f = Fixture::new("agents-unreadable");
    std::fs::write(f.0.join("GEMINI.md"), [0xff, 0xfe]).expect("invalid UTF-8");
    f.rejects("GEMINI.md");
    f.write("GEMINI.md", POINTER);
    std::fs::remove_file(f.0.join("GEMINI.md")).expect("remove");
    std::fs::create_dir(f.0.join("GEMINI.md")).expect("directory");
    f.rejects("GEMINI.md");
    std::fs::remove_dir_all(f.0.join(".git")).expect("remove fixture git");
    f.rejects("git");
}

#[cfg(unix)]
#[test]
fn symlinks_in_files_and_parent_directories_fail() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new("agents-symlinks");
    for path in ["AGENTS.md", "CLAUDE.md", "GEMINI.md", FILES[2], FILES[3], FILES[4]] {
        let original = f.text(path);
        let p = f.0.join(path);
        std::fs::remove_file(&p).expect("remove");
        symlink(f.0.join("elsewhere.md"), &p).expect("dangling link");
        f.rejects(path);
        f.write("elsewhere.md", &original);
        f.rejects(path);
        std::fs::remove_file(&p).expect("unlink");
        f.write(path, &original);
    }
    std::fs::rename(f.0.join(".cursor"), f.0.join("cursor-real")).expect("move");
    symlink("cursor-real", f.0.join(".cursor")).expect("directory link");
    f.rejects(".cursor/rules/oos.mdc");
}

#[test]
fn both_gate_modes_dispatch_the_check_without_losing_g1_steps() {
    let f = Fixture::new("agents-dispatch");
    f.write("GEMINI.md", "Independent rule\n");
    let prefix = [
        "fmt",
        "size limits",
        "architecture",
        "mutable statics",
        "unsafe",
        "disallowed-method exceptions",
        "known failures",
        "agent rules",
    ];
    for (fast, coverage, count) in [(true, true, 13), (false, true, 21), (false, false, 20)] {
        let names: Vec<_> = check::plan(check::Options { fast, coverage }).into_iter().map(|(n, _)| n).collect();
        assert_eq!(names.len(), count);
        assert_eq!(&names[..8], &prefix);
        assert!(dispatched(&f.0, fast, coverage).is_err());
    }
}
