// SPDX-License-Identifier: GPL-2.0-or-later
//! Ordered executable check registry. Listing and dispatch use these same records.
use crate::{check, steps};
use std::path::Path;

#[cfg(test)]
mod tests;

pub(crate) type Runner = fn(&crate::run::Context, &Path) -> steps::Outcome;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Selection {
    Fast,
    Rust,
    Coverage,
    Host,
    Image,
}

#[derive(Debug)]
pub(crate) struct Check {
    pub id: &'static str,
    pub name: &'static str,
    pub command: &'static str,
    pub prerequisite: &'static str,
    pub selection: Selection,
    pub after: &'static [&'static str],
    pub run: Runner,
}

macro_rules! entry {
    ($id:literal, $name:literal, $command:literal, $prereq:literal, $selection:ident, [$($after:literal),*], $run:expr) => {
        Check { id: $id, name: $name, command: $command, prerequisite: $prereq,
            selection: Selection::$selection, after: &[$($after),*], run: $run }
    };
}

pub(crate) const CHECKS: &[Check] = &[
    entry!(
        "fmt",
        "fmt",
        "cargo fmt --all --check; static pins",
        "pinned Rust/Python",
        Fast,
        [],
        check::formatting_and_pins
    ),
    entry!("size", "size limits", "syntax and move-aware size ratchets", "workspace metadata", Fast, [], steps::size),
    entry!(
        "architecture",
        "architecture",
        "workspace completeness and architecture rules",
        "Cargo.lock",
        Fast,
        [],
        steps::arch
    ),
    entry!("mutable-statics", "mutable statics", "syntax and per-crate ratchets", "sources", Fast, [], steps::statics),
    entry!("unsafe", "unsafe", "syntax and per-crate ratchets", "sources", Fast, [], steps::unsafety),
    entry!(
        "method-exceptions",
        "disallowed-method exceptions",
        "syntax and per-crate ratchets",
        "sources",
        Fast,
        [],
        steps::allows
    ),
    entry!("known-failures", "known failures", "syntax and open-task inventory", "sources", Fast, [], steps::known),
    entry!(
        "agent-rules",
        "agent rules",
        "exact loaders and Git inventory",
        "AGENTS.md",
        Fast,
        [],
        crate::agent_rules::check
    ),
    entry!("test-time", "test timing", "test source timing ratchet", "sources", Fast, [], crate::test_time::check),
    entry!("links", "links", "links in README, wiki/ and docs/ resolve", "Git", Fast, [], crate::links::check),
    entry!("text", "text files", "ADR 0019 file rules over git ls-files", "Git", Fast, [], crate::text::check),
    entry!("doctor-base", "doctor-base", "doctor", "tools/bootstrap.sh; no Docker", Host, [], crate::doctor::check),
    entry!(
        "doctor",
        "doctor",
        "doctor --gpu --sim",
        "tools/bootstrap.sh --gpu --sim",
        Image,
        ["doctor-base"],
        steps::host::doctor
    ),
    entry!("clippy", "clippy", "cargo clippy --all-targets, host and ARM", "pinned toolchain", Rust, [], steps::clippy),
    entry!(
        "plan-matrix",
        "register plan matrix",
        "build host oos-fpgad; compare 65 plans with xtask/data/plan-matrix.txt",
        "pinned toolchain, Python, diff; synthetic calibration, no local OOS defaults",
        Rust,
        [],
        crate::plan_matrix::check
    ),
    entry!(
        "decoders",
        "standalone decoders",
        "cargo check --all-targets --no-default-features in ui; core only and each optional decoder alone",
        "pinned toolchain",
        Rust,
        [],
        steps::decoders::matrix
    ),
    entry!("tests-debug", "tests (debug)", "cargo nextest run per crate", "pinned toolchain", Rust, [], |ctx, r| {
        steps::tests(ctx, r, false)
    }),
    entry!(
        "tests-release",
        "tests (release)",
        "cargo nextest run --cargo-profile test-release per crate",
        "pinned toolchain",
        Rust,
        [],
        |ctx, r| { steps::tests(ctx, r, true) }
    ),
    entry!(
        "perf-build",
        "perf guard builds",
        "sh tools/perf-guard.sh --build-only",
        "pinned toolchain",
        Rust,
        [],
        steps::perf_build
    ),
    entry!("perf", "perf guards", "sh tools/perf-guard.sh", "pinned toolchain", Rust, [], steps::perf),
    entry!(
        "coverage",
        "coverage",
        "cargo llvm-cov nextest per crate",
        "cargo-llvm-cov, llvm-tools",
        Coverage,
        [],
        steps::coverage
    ),
    entry!(
        "script-syntax",
        "script-syntax",
        "python3 -I tools/script_syntax.py",
        "Git, Python, declared shells",
        Host,
        [],
        steps::scripts::syntax
    ),
    entry!(
        "python-tests",
        "python-tests",
        "python3 -I tools/python_tests.py (isolated discovery with count checks)",
        "Git, Python; tools/tests, tools/privacy and ui/tools",
        Host,
        [],
        steps::python::run
    ),
    entry!(
        "arm64-release-link",
        "arm64-release-link",
        "receipted ARM release build; static AArch64 ELF validation",
        "ARM musl std and rust-lld",
        Image,
        [],
        steps::host::arm
    ),
    entry!(
        "gpu-host",
        "gpu-host",
        "sh gpu/test-host.sh",
        "pinned gpu-test image",
        Image,
        ["doctor", "arm64-release-link"],
        steps::host::gpu
    ),
    entry!(
        "ssh-sim",
        "ssh-sim",
        "sh tools/ssh-sim.sh",
        "initramfs, nft, Dropbear, SFTP builders",
        Image,
        ["doctor"],
        steps::host::ssh
    ),
    entry!(
        "firewall-sim",
        "firewall-sim",
        "sh tools/fw-sim.sh",
        "initramfs and nft builders",
        Image,
        ["doctor"],
        steps::host::firewall
    ),
    entry!(
        "image-required",
        "image-required",
        "python3 -I tools/image_required.py",
        "authoritative base and Git paths",
        Image,
        [],
        steps::image::required
    ),
    entry!(
        "image-suite",
        "image-suite",
        "python3 -I tools/image_suite.py",
        "kernel source; doctor --image; Docker; QEMU",
        Image,
        ["doctor", "arm64-release-link"],
        steps::image::suite
    ),
];

/// Checks the gate runs one at a time after all others: the perf guards measure timing ratios, which other work on
/// the machine would disturb, and the image checks drive Docker and QEMU with their own time limits.
pub(crate) fn alone(c: &Check) -> bool {
    c.id == "perf" || c.selection == Selection::Image
}

pub(crate) fn selected(c: &Check, options: check::Options, image: bool) -> bool {
    match c.selection {
        Selection::Fast => true,
        Selection::Coverage => !options.fast && options.coverage,
        Selection::Image => !options.fast && image,
        _ => !options.fast,
    }
}

pub(crate) fn list(options: check::Options, image: bool) {
    println!("inventory only (not a passing gate): ID | selection | command | prerequisites");
    for c in CHECKS {
        println!(
            "{} ({}) | {} ({:?}) | {} | {}",
            c.id,
            c.name,
            if selected(c, options, image) { "required" } else { "not-selected" },
            c.selection,
            c.command,
            c.prerequisite
        );
    }
    println!(
        "--image: default gate plus Docker/emulation/image checks; coordinator runs before unit rounds and releases"
    );
}
