//! Read-only process probes. Untrusted command output is never copied into diagnostics.
use super::pins::Pins;
use crate::run::Output;
use std::path::Path;

type Exec<'a> = dyn Fn(&str, &[&str]) -> Result<Output, String> + 'a;

pub(super) struct Inventory<'a> {
    pub exec: &'a Exec<'a>,
    pub lines: Vec<String>,
    pub bad: bool,
}

impl<'a> Inventory<'a> {
    pub fn new(exec: &'a Exec<'a>) -> Self {
        Self { exec, lines: Vec::new(), bad: false }
    }

    pub fn check(&mut self, name: &str, good: bool, expected: &str, actual: &str, remedy: &str) {
        self.bad |= !good;
        self.lines.push(format!(
            "{} {}: expected {}; actual {}; setup: {}",
            if good { "ok" } else { "FAIL" },
            name,
            expected,
            actual,
            remedy
        ));
    }

    pub fn output(&self, program: &str, args: &[&str]) -> Option<String> {
        (self.exec)(program, args).ok().filter(|o| o.ok).map(|o| o.stdout.trim().to_string())
    }

    pub fn command(&mut self, name: &str, program: &str, args: &[&str], remedy: &str) {
        let good = self.output(program, args).is_some();
        self.check(name, good, "successful probe", if good { "available" } else { "missing or probe failed" }, remedy);
    }

    fn version(&mut self, program: &str, args: &[&str], expected: &str, remedy: &str) {
        let out = self.output(program, args);
        let actual = out.as_deref().and_then(|s| s.split_whitespace().nth(1)).unwrap_or("unavailable");
        // Only version-shaped tokens may enter the report, never arbitrary command output.
        let actual = if actual.bytes().all(|b| b.is_ascii_digit() || b == b'.') { actual } else { "unrecognized" };
        self.check(program, actual == expected, expected, actual, remedy);
    }

    pub fn base(&mut self, pins: &Pins) {
        let setup = "tools/bootstrap.sh";
        self.version("rustc", &["--version"], &pins.rust, setup);
        self.version("cargo", &["--version"], &pins.rust, setup);
        self.cargo_tool("llvm-cov", &pins.tools.coverage, setup);
        self.cargo_tool("nextest", &pins.tools.nextest, setup);
        self.python(&pins.tools.python);
        self.active_toolchain(pins);
        self.components(pins);
        self.toolchain_tools();
        let imports = format!("import {}", pins.tools.stdlib.join(","));
        self.command("Python stdlib", "python3", &["-I", "-c", &imports], "install Python >=3.12");
        self.command("Git", "git", &["--version"], "install Git");
        let hooks = self.output("git", &["config", "core.hooksPath"]);
        self.lines.push(format!(
            "{} hooks: expected .githooks; actual {}; setup: git config core.hooksPath .githooks",
            if hooks.as_deref() == Some(".githooks") { "ok" } else { "WARN" },
            if hooks.as_deref() == Some(".githooks") { ".githooks" } else { "mismatch/unset" },
        ));
        self.command(
            "writable isolated output",
            "python3",
            &["-I", "tools/build_env.py", "output"],
            "unset CARGO_TARGET_DIR; python3 tools/output.py target",
        );
    }

    /// The Cargo subcommand `cargo-<tool>` at the pinned version: the word after its name in `cargo <tool> --version`
    /// (`cargo-nextest 0.9.150 (d225b1daf 2026-10-26)` and more lines).
    fn cargo_tool(&mut self, tool: &str, expected: &str, remedy: &str) {
        let name = format!("cargo-{tool}");
        let out = self.output("cargo", &[tool, "--version"]);
        let mut words = out.as_deref().unwrap_or_default().split_whitespace();
        let actual = if words.next() == Some(name.as_str()) { words.next() } else { None }.unwrap_or("unavailable");
        let actual = if actual.bytes().all(|b| b.is_ascii_digit() || b == b'.') { actual } else { "unrecognized" };
        self.check(&name, actual == expected, expected, actual, remedy);
    }

    fn active_toolchain(&mut self, pins: &Pins) {
        let active = self.output("rustup", &["show", "active-toolchain"]);
        let verbose = self.output("rustc", &["-vV"]);
        let host = verbose.as_deref().and_then(|s| s.lines().find_map(|l| l.strip_prefix("host: ")));
        let good = host.is_some_and(|host| {
            active.as_deref().and_then(|s| s.split_whitespace().next())
                == Some(format!("{}-{host}", pins.rust).as_str())
        });
        self.check(
            "active toolchain",
            good,
            &pins.rust,
            if good { "pinned" } else { "override mismatch/unavailable" },
            "unset RUSTUP_TOOLCHAIN; rustup override unset; tools/bootstrap.sh",
        );
    }

    fn python(&mut self, minimum: &str) {
        let out = self.output("python3", &["--version"]);
        let actual = out.as_deref().and_then(|s| s.strip_prefix("Python ")).unwrap_or("unavailable");
        let parts: Option<Vec<u32>> = actual.split('.').map(|s| s.parse().ok()).collect();
        let required: Option<Vec<u32>> = minimum.split('.').map(|s| s.parse().ok()).collect();
        let good = parts.as_ref().zip(required.as_ref()).is_some_and(|(p, r)| p.len() == 3 && &p[..2] >= r);
        let actual = if parts.is_some() { actual } else { "unavailable/unrecognized" };
        self.check("python3", good, &format!(">={}", minimum), actual, "install Python >=3.12; tools/bootstrap.sh");
    }

    fn components(&mut self, pins: &Pins) {
        let list = self.output("rustup", &["component", "list", "--installed"]);
        for component in ["rustfmt", "clippy", "llvm-tools"] {
            let good = list.as_deref().is_some_and(|s| s.lines().any(|l| l.starts_with(&format!("{}-", component))));
            self.check(
                component,
                good,
                "pinned component",
                if good { "installed" } else { "missing" },
                "tools/bootstrap.sh",
            );
        }
        let list = self.output("rustup", &["target", "list", "--installed"]);
        let good = list.as_deref().is_some_and(|s| s.lines().any(|l| l == crate::repo::TARGET));
        self.check(
            "musl std",
            good,
            crate::repo::TARGET,
            if good { "installed" } else { "missing" },
            "tools/bootstrap.sh",
        );
        // Compare the wrapper version against the binary shipped in this sysroot below.
        self.command(
            "rustfmt",
            "rustfmt",
            &["--version"],
            &format!("rustup component add --toolchain {} rustfmt", pins.rust),
        );
        self.command("clippy", "cargo", &["clippy", "--version"], "tools/bootstrap.sh");
    }

    fn toolchain_tools(&mut self) {
        let sysroot = self.output("rustc", &["--print", "sysroot"]);
        let verbose = self.output("rustc", &["-vV"]);
        let host = verbose.as_deref().and_then(|s| s.lines().find_map(|l| l.strip_prefix("host: ")));
        for tool in ["rust-lld", "llvm-cov", "llvm-profdata", "rustfmt", "clippy-driver"] {
            let directory = if matches!(tool, "rustfmt" | "clippy-driver") {
                "bin".into()
            } else {
                format!("lib/rustlib/{}/bin", host.unwrap_or("missing-host"))
            };
            let out = sysroot.as_ref().and_then(|s| {
                let path = Path::new(s).join(&directory).join(tool);
                let args: &[&str] = if tool == "rust-lld" { &["-flavor", "gnu", "--version"] } else { &["--version"] };
                self.output(path.to_str()?, args)
            });
            let wrapper = match tool {
                "rustfmt" => self.output("rustfmt", &["--version"]),
                "clippy-driver" => self.output("cargo", &["clippy", "--version"]),
                _ => out.clone(),
            };
            let good = out.is_some() && out == wrapper;
            self.check(
                tool,
                good,
                "usable active-sysroot tool",
                if good { "matches" } else { "missing/mismatch/probe failed" },
                "tools/bootstrap.sh",
            );
        }
    }
}
