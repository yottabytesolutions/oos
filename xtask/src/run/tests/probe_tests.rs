use super::*;
use std::os::unix::fs::PermissionsExt;

#[test]
fn toolchain_probes_ignore_inherited_override() {
    const CHILD: &str = "OOS_TOOLCHAIN_PROBE_CHILD";
    if let Some(root) = std::env::var_os(CHILD) {
        let root = PathBuf::from(root);
        let (ctx, _) = start_logs(&root).unwrap();
        let before = std::env::var_os("RUSTUP_TOOLCHAIN");
        assert!(available(&ctx, &root, "rustc", &["--version"]));
        assert!(available(&ctx, &root, "sh", &["probe.sh"]));
        assert!(crate::target::installed(&ctx, &root));
        assert!(crate::coverage::installed(&ctx, &root));
        assert_eq!(std::env::var_os("RUSTUP_TOOLCHAIN"), before);
        let inherited = capture(&ctx, &root, "sh", &["-c", "printf '%s' \"$RUSTUP_TOOLCHAIN\""]).unwrap();
        assert_eq!(inherited.stdout, "oos-nonexistent-toolchain");
        assert_eq!(logs(&ctx).len(), 5);
        return;
    }
    let fixture = Fixture::new();
    fs::write(fixture.0.join("rust-toolchain.toml"), include_str!("../../../../rust-toolchain.toml")).unwrap();
    fs::write(fixture.0.join("probe.sh"), "test -z \"${RUSTUP_TOOLCHAIN+x}\"\n").unwrap();
    let script = format!(
        "#!/bin/sh\n[ -z \"${{RUSTUP_TOOLCHAIN+x}}\" ] || exit 7\n[ \"$PWD\" = \"$OOS_TOOLCHAIN_PROBE_CHILD\" ] || exit 8\nprintf '%s\\n' {}\n",
        crate::repo::TARGET
    );
    for program in ["rustup", "cargo"] {
        let path = fixture.0.join(program);
        fs::write(&path, &script).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let out = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "run::tests::probe_tests::toolchain_probes_ignore_inherited_override", "--nocapture"])
        .env(CHILD, &fixture.0)
        .env("RUSTUP_TOOLCHAIN", "oos-nonexistent-toolchain")
        .env(
            "PATH",
            std::env::join_paths(
                std::iter::once(fixture.0.clone()).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
            )
            .unwrap(),
        )
        .output()
        .unwrap();
    assert!(out.status.success(), "{}\n{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
}

#[test]
fn fixture_git_ignores_global_and_system_configuration() {
    const CHILD: &str = "OOS_GIT_CONFIG_CHILD";
    if let Some(root) = std::env::var_os(CHILD) {
        let root = PathBuf::from(root);
        crate::testrepo::git(&root, &["init", "-q", "-b", "overhaul"]);
        crate::testrepo::commit(&root, &[("README", "fixture")], "fixture");
        return;
    }
    let fixture = Fixture::new();
    let config = fixture.0.join("invalid.gitconfig");
    fs::write(&config, "[invalid\n").unwrap();
    let out = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "run::tests::probe_tests::fixture_git_ignores_global_and_system_configuration",
            "--nocapture",
        ])
        .env(CHILD, &fixture.0)
        .env("GIT_CONFIG_GLOBAL", &config)
        .env("GIT_CONFIG_SYSTEM", &config)
        .env_remove("GIT_CONFIG_NOSYSTEM")
        .output()
        .unwrap();
    assert!(out.status.success(), "{}\n{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
}
