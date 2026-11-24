// SPDX-License-Identifier: GPL-2.0-or-later
// Host checks only. No card or network access. Signing uses test-only fixtures or keys generated in a temporary directory.
use oos_hal::update::{card, hash, kexec, pkg};
use std::path::PathBuf;
use std::process::{Command, Output};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let out = Command::new("mktemp").arg("-d").output().expect("test setup");
        assert!(out.status.success());
        let f = Self(PathBuf::from(String::from_utf8(out.stdout).expect("test setup").trim()));
        std::fs::write(f.path("image"), image()).expect("test setup");
        std::fs::write(f.path("test signing key"), include_str!("fixtures/test-signing.key")).expect("test setup");
        std::fs::write(f.path("test public key"), hash::hex(&test_key())).expect("test setup");
        f
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    fn command(&self) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_oos-update"));
        c.env("HOME", &self.0).env_remove("OOS_SIGNING_KEY").env_remove("OOS_UPDATE_KEY");
        c
    }

    fn pack(&self) -> Command {
        let mut c = self.command();
        c.args(["pack", "--image"]).arg(self.path("image"));
        c.args(["--build", "20261114-120000 kernel test", "--out"]).arg(self.path("package"));
        c
    }

    fn test_pack(&self) -> Command {
        let mut c = self.pack();
        c.arg("--key").arg(self.path("test signing key")).arg("--untrusted-ok");
        c
    }

    fn keygen(&self, name: &str) -> [u8; 32] {
        let out = self.command().args(["keygen", "--out"]).arg(self.path(name)).output().expect("test setup");
        success(&out);
        let text = String::from_utf8(out.stdout).expect("test setup");
        let keys = pkg::parse_keys(&text);
        assert_eq!(keys.len(), 1);
        keys[0]
    }

    fn verify(&self, key: [u8; 32]) {
        let mut bytes = std::fs::read(self.path("package")).expect("test setup");
        let (header, payload) = pkg::verify(&bytes, &[key]).expect("test setup");
        assert_eq!(payload, image());
        assert_eq!(header.manifest.build, "20261114-120000 kernel test");
        assert!(header.manifest.extra.contains(&(kexec::LINE.0.into(), kexec::LINE.1.into())));
        card::check_image(payload).expect("test setup");
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        assert!(pkg::verify(&bytes, &[key]).is_err(), "device verifier must reject damaged payloads");
        bytes[last] ^= 1;
        bytes[b"OOS-UPDATE 1\ntarget ".len()] ^= 1;
        assert!(pkg::verify(&bytes, &[key]).is_err(), "device verifier must reject altered signed headers");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).expect("test setup");
    }
}

fn success(out: &Output) {
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}

fn refused(out: &Output, reason: &str) {
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains(reason), "{}", String::from_utf8_lossy(&out.stderr));
}

fn test_key() -> [u8; 32] {
    oos_hal::update::ed25519::public_key(&[9; 32])
}

#[test]
fn pack_announces_staging_only_when_the_image_declares_the_signer() {
    let f = Fixture::new();
    for image in [
        image(),
        unit_identity::image_initramfs(&[(kexec::STAGED_KEYS_FILE, 0o100644, hash::hex(&[3; 32]).as_bytes())]),
    ] {
        std::fs::write(f.path("image"), image).expect("test setup");
        success(&f.test_pack().output().expect("test setup"));
        let bytes = std::fs::read(f.path("package")).expect("test setup");
        let header = pkg::verify_header(&bytes, &[test_key()]).expect("test setup");
        assert!(kexec::announced(&header.manifest));
        assert!(!kexec::announced_staged(&header.manifest));
    }
    let signing_key = hash::hex(&test_key());
    let image = unit_identity::image_initramfs(&[(kexec::STAGED_KEYS_FILE, 0o100644, signing_key.as_bytes())]);
    std::fs::write(f.path("image"), &image).expect("test setup");
    success(&f.test_pack().output().expect("test setup"));
    let mut bytes = std::fs::read(f.path("package")).expect("test setup");
    let (header, payload) = pkg::verify(&bytes, &[test_key()]).expect("older parser accepts extra signed lines");
    assert_eq!(payload, image);
    // The pre-bridge sender tests only the legacy capability. The parser preserves other signed lines.
    assert!(header.manifest.extra.iter().any(|(key, value)| key == kexec::LINE.0 && value == kexec::LINE.1));
    assert!(kexec::use_staged(&header.manifest, &test_key()).expect("staged receiver trusts this key"));
    let line = b"\nstaged-key ";
    let at = bytes[..header.data_offset].windows(line.len()).position(|part| part == line).expect("staged key");
    bytes[at + line.len()] ^= 1;
    assert!(pkg::verify(&bytes, &[test_key()]).unwrap_err().contains("SIGNATURE INVALID"));
}

fn image() -> Vec<u8> {
    let mut img = vec![0; 4096];
    img[..8].copy_from_slice(b"ANDROID!");
    for (offset, value) in [(8, 64u32), (12, 0x0028_0000), (20, 0x04bf_0000), (36, 2048)] {
        img[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    img[2048 + 0x38..2048 + 0x3c].copy_from_slice(b"ARM\x64");
    let id = hash::sha1(&[
        &img[2048..2112],
        &64u32.to_le_bytes(),
        &[],
        &0u32.to_le_bytes(),
        &[],
        &0u32.to_le_bytes(),
        &img[32..576],
    ]);
    img[576..596].copy_from_slice(&id);
    img
}

#[test]
fn explicit_key_round_trips_through_device_verifier() {
    let f = Fixture::new();
    let key = f.keygen("private key");
    let wrong_key = f.keygen("other key");
    let out = f.pack().arg("--untrusted-ok").arg("--key").arg(f.path("private key")).output().expect("test setup");
    success(&out);
    f.verify(key);
    let bytes = std::fs::read(f.path("package")).expect("test setup");
    assert!(pkg::verify(&bytes, &[wrong_key]).is_err());
    std::fs::write(f.path("public"), hash::hex(&key)).expect("test setup");
    success(
        &f.command()
            .arg("verify")
            .arg(f.path("package"))
            .arg("--keys")
            .arg(f.path("public"))
            .output()
            .expect("test setup"),
    );
}

#[test]
fn environment_selects_key_and_explicit_key_takes_precedence() {
    let f = Fixture::new();
    let key = f.keygen("private");
    success(&f.pack().arg("--untrusted-ok").env("OOS_SIGNING_KEY", f.path("private")).output().expect("test setup"));
    f.verify(key);
    success(
        &f.pack()
            .arg("--untrusted-ok")
            .env("OOS_SIGNING_KEY", f.path("missing"))
            .arg("--key")
            .arg(f.path("private"))
            .output()
            .expect("test setup"),
    );
    f.verify(key);
}

#[test]
fn no_selection_ignores_home_and_legacy_environment() {
    let f = Fixture::new();
    f.keygen(".config/oos/update-signing.key");
    refused(
        &f.pack().env("OOS_UPDATE_KEY", f.path(".config/oos/update-signing.key")).output().expect("test setup"),
        "signing requires --key",
    );
    refused(&f.pack().env("OOS_SIGNING_KEY", "").output().expect("test setup"), "signing requires --key");
    assert!(!f.path("package").exists());
}

#[test]
fn community_signing_mode_is_removed() {
    let f = Fixture::new();
    refused(&f.pack().arg("--community").output().expect("test setup"), "--community needs a value");
    assert!(!f.path("package").exists());
}

#[test]
fn missing_or_invalid_selected_key_never_falls_back() {
    let f = Fixture::new();
    refused(&f.pack().arg("--key").arg(f.path("missing")).output().expect("test setup"), "missing");
    refused(&f.pack().env("OOS_SIGNING_KEY", f.path("missing")).output().expect("test setup"), "missing");
    std::fs::write(f.path("invalid"), "invalid").expect("test setup");
    refused(&f.pack().arg("--key").arg(f.path("invalid")).output().expect("test setup"), "no 64-digit hex secret key");
    assert!(!f.path("package").exists());
}

#[test]
fn keygen_requires_explicit_output() {
    let f = Fixture::new();
    refused(&f.command().arg("keygen").output().expect("test setup"), "keygen needs --out");
    assert!(!f.path(".config").exists());
}

impl Fixture {
    fn script(&self) -> Command {
        use std::os::unix::fs::PermissionsExt;
        let out = self.path("selected target/image");
        std::fs::create_dir_all(&out).expect("test setup");
        std::fs::write(out.join("oos-dev.img"), image()).expect("test setup");
        std::fs::write(out.join("oos-dev.build"), "20261114-120000 kernel test\n").expect("test setup");
        std::fs::write(out.join("oos-dev.models"), "none\n").expect("test setup");
        std::fs::create_dir(self.path("boot")).expect("test setup");
        std::fs::write(self.path("boot/sign-dev-image.sh"), include_str!("../../boot/sign-dev-image.sh"))
            .expect("test setup");
        std::fs::create_dir(self.path("bin")).expect("test setup");
        std::fs::create_dir(self.path("tools")).expect("test setup");
        for (name, text) in
            [("output.sh", include_str!("../../tools/output.sh")), ("output.py", include_str!("../../tools/output.py"))]
        {
            std::fs::write(self.path("tools").join(name), text).expect("test setup");
        }
        // This isolated fixture has no Git checkout; keep its revision fixed, without a temporary commit.
        let git = self.path("bin/git");
        std::fs::write(&git, "#!/bin/sh\ncase \"$*\" in *rev-parse*) echo fixture;; *ls-files*) ;; *) exit 1;; esac\n")
            .expect("test setup");
        std::fs::set_permissions(git, std::fs::Permissions::from_mode(0o700)).expect("test setup");
        // Skip Cargo's rebuild and permit the untrusted test key only in this isolated host fixture.
        let cargo = self.path("bin/cargo");
        std::fs::write(
            &cargo,
            "#!/bin/sh\nwhile [ \"$1\" != -- ]; do shift; done\nshift\nexec \"$OOS_TEST_SIGNER\" \"$@\" --untrusted-ok\n",
        )
        .expect("test setup");
        std::fs::set_permissions(cargo, std::fs::Permissions::from_mode(0o700)).expect("test setup");
        let mut cmd = Command::new("sh");
        cmd.arg(self.path("boot/sign-dev-image.sh"));
        let path = std::env::join_paths(
            std::iter::once(self.path("bin"))
                .chain(std::env::split_paths(&std::env::var_os("PATH").expect("test setup"))),
        )
        .expect("test setup");
        cmd.env("PATH", &path)
            .env("CARGO_TARGET_DIR", self.path("selected target"))
            .env("OOS_TEST_SIGNER", env!("CARGO_BIN_EXE_oos-update"))
            .env_remove("OOS_SIGNING_KEY");
        self.stamp_image(&path);
        cmd
    }

    fn stamp_image(&self, path: &std::ffi::OsStr) {
        let out = self.path("selected target/image");
        let tool = self.path("tools/output.py");
        let revision = Command::new("python3")
            .arg(&tool)
            .arg("revision")
            .env("PATH", path)
            .env("CARGO_TARGET_DIR", self.path("selected target"))
            .output()
            .expect("test setup");
        success(&revision);
        let revision = String::from_utf8(revision.stdout).expect("test setup");
        for name in ["oos-dev.img", "oos-dev.build", "oos-dev.models"] {
            success(
                &Command::new("python3")
                    .arg(&tool)
                    .arg("stamp")
                    .arg(out.join(name))
                    .arg(revision.trim())
                    .env("PATH", path)
                    .env("CARGO_TARGET_DIR", self.path("selected target"))
                    .output()
                    .expect("test setup"),
            );
        }
    }
}

#[test]
fn image_signing_script_uses_saved_build_id_and_output_name() {
    let f = Fixture::new();
    let mut cmd = f.script();
    std::fs::write(f.path("private key"), include_str!("fixtures/test-signing.key")).expect("test setup");
    success(&cmd.arg("--key").arg(f.path("private key")).output().expect("test setup"));
    let bytes = std::fs::read(f.path("selected target/image/oos-dev.oosupd")).expect("test setup");
    let (header, payload) = pkg::verify(&bytes, &[test_key()]).expect("test setup");
    assert_eq!(header.manifest.build, "20261114-120000 kernel test");
    assert_eq!(payload, image());
    assert!(f.path("selected target/image/oos-dev.oosupd.oos-build.json").exists());
    assert!(!f.path("selected target/image/DHO800_DHO900_OOS_Install.GEL").exists());
    assert!(!f.path("boot/out").exists());
}

#[test]
fn trusted_public_keys_are_never_signing_seeds() {
    let f = Fixture::new();
    std::fs::write(f.path("public"), include_str!("../../keys/update.pub")).expect("test setup");
    refused(&f.pack().arg("--key").arg(f.path("public")).output().expect("test setup"), "public key");
    for key in oos_hal::update::trusted_keys() {
        std::fs::write(f.path("public"), hash::hex(&key)).expect("test setup");
        refused(
            &f.pack().arg("--key").arg(f.path("public")).arg("--untrusted-ok").output().expect("test setup"),
            "public key",
        );
        assert!(!f.path("package").exists());
    }
}

#[test]
fn untrusted_signing_keys_require_explicit_opt_in() {
    let f = Fixture::new();
    let key = f.keygen("private");
    assert!(!oos_hal::update::trusted_keys().contains(&key));
    refused(&f.pack().arg("--key").arg(f.path("private")).output().expect("test setup"), "--untrusted-ok");
    refused(&f.pack().env("OOS_SIGNING_KEY", f.path("private")).output().expect("test setup"), "--untrusted-ok");
    assert!(!f.path("package").exists());
    success(&f.pack().arg("--key").arg(f.path("private")).arg("--untrusted-ok").output().expect("test setup"));
    f.verify(key);
}

#[test]
fn signing_flags_are_pack_only() {
    let f = Fixture::new();
    for command in ["verify", "keygen", "status"] {
        refused(&f.command().args([command, "--untrusted-ok"]).output().expect("test setup"), "only valid for pack");
    }
    refused(&f.command().arg("--untrusted-ok").output().expect("test setup"), "only valid for pack");
}

#[test]
fn image_signing_script_requires_saved_build_id() {
    let f = Fixture::new();
    let mut cmd = f.script();
    std::fs::remove_file(f.path("selected target/image/oos-dev.build")).expect("test setup");
    refused(
        &cmd.env("OOS_SIGNING_KEY", f.path("test signing key")).output().expect("test setup"),
        "run boot/build-dev-image.sh first",
    );
    assert!(!f.path("selected target/image/oos-dev.oosupd").exists());
}

#[test]
fn image_signing_script_accepts_environment() {
    let f = Fixture::new();
    let mut cmd = f.script();
    refused(&cmd.output().expect("test setup"), "signing requires --key");
    assert!(!f.path("selected target/image/oos-dev.oosupd").exists());
    std::fs::write(f.path("private key"), include_str!("fixtures/test-signing.key")).expect("test setup");
    success(&cmd.env("OOS_SIGNING_KEY", f.path("private key")).output().expect("test setup"));
    let bytes = std::fs::read(f.path("selected target/image/oos-dev.oosupd")).expect("test setup");
    pkg::verify(&bytes, &[test_key()]).expect("test setup");
}

#[test]
fn image_signing_script_refuses_extra_or_incomplete_arguments() {
    let f = Fixture::new();
    let cmd = f.script();
    let path = cmd.get_envs().find(|(key, _)| *key == "PATH").expect("test setup").1.expect("test setup").to_owned();
    // A successful stand-in makes any invocation of Cargo visible as an unexpected exit 0.
    std::fs::write(f.path("bin/cargo"), "#!/bin/sh\nexit 0\n").expect("test setup");
    for args in [
        vec!["--out", "elsewhere"],
        vec!["--image", "elsewhere"],
        vec!["--build", "other"],
        vec!["--key"],
        vec!["--key", ""],
        vec!["--community"],
        vec!["--untrusted-ok"],
        vec!["pack"],
    ] {
        let out = Command::new("sh")
            .arg(f.path("boot/sign-dev-image.sh"))
            .args(&args)
            .env("PATH", &path)
            .output()
            .expect("test setup");
        refused(&out, "Usage:");
    }
    assert!(!f.path("selected target/image/oos-dev.oosupd").exists());
}

#[test]
fn image_workflow_publishes_unsigned_image_for_offline_signing() {
    let workflow = include_str!("../../.github/workflows/image.yml");
    let build = workflow.find("run: boot/build-dev-image.sh").expect("test setup");
    assert!(!workflow.contains("boot/sign-dev-image.sh"));
    assert!(!workflow.contains("boot/build-gel.py"));
    assert!(!workflow.contains("tools/build-release.py"));
    let check = workflow.find("      - name: Validate release artifacts\n        run: |\n").expect("test setup");
    let upload = workflow[check..].find("      - uses: actions/upload-artifact@").expect("test setup") + check;
    assert!(build < check && check < upload);
    let script = workflow[check..upload].split_once("run: |\n").expect("test setup").1;
    let script =
        script.lines().map(|line| line.strip_prefix("          ").expect("test setup")).collect::<Vec<_>>().join("\n");
    let f = Fixture::new();
    std::fs::create_dir_all(f.path("selected target/image")).expect("test setup");
    let required = ["oos-dev.img", "oos-dev.build", "oos-dev.models", "manifest.txt"];
    for name in required {
        assert!(workflow[upload..].lines().any(|line| line.ends_with(&format!("/image/{name}"))), "{name}");
    }
    for missing in [None].into_iter().chain(required.map(Some)) {
        for name in required {
            let path = f.path("selected target/image").join(name);
            if missing != Some(name) {
                std::fs::write(path, "artifact").expect("test setup");
            } else if path.exists() {
                std::fs::remove_file(path).expect("test setup");
            }
        }
        let out = Command::new("bash")
            .args(["-e", "-c", &script])
            .current_dir(&f.0)
            .env("CARGO_TARGET_DIR", f.path("selected target"))
            .output()
            .expect("test setup");
        assert_eq!(
            out.status.success(),
            missing.is_none(),
            "missing {missing:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn image_signing_script_rejects_unverified_inputs() {
    for name in ["oos-dev.img", "oos-dev.build", "oos-dev.models"] {
        for replace in [false, true] {
            let f = Fixture::new();
            let mut cmd = f.script();
            let input = f.path("selected target/image").join(name);
            if replace {
                std::fs::write(input, "replaced").expect("test setup");
            } else {
                std::fs::remove_file(input.with_file_name(format!("{name}.oos-build.json"))).expect("test setup");
            }
            let out = cmd.env("OOS_SIGNING_KEY", f.path("test signing key")).output().expect("test setup");
            refused(&out, if replace { "stale or replaced artifact" } else { "missing build receipt" });
            assert!(!f.path("selected target/image/oos-dev.oosupd").exists());
        }
    }
}

#[test]
fn image_signing_script_rechecks_inputs_after_pack() {
    for name in ["oos-dev.img", "oos-dev.build", "oos-dev.models"] {
        let f = Fixture::new();
        let mut cmd = f.script();
        // Simulate an input replaced while the pack command is consuming it.
        std::fs::write(
            f.path("bin/cargo"),
            format!(
                "#!/bin/sh\nprintf replaced > \"$CARGO_TARGET_DIR/image/{name}\"\nprintf package > \"$CARGO_TARGET_DIR/image/oos-dev.oosupd\"\n"
            ),
        )
        .expect("test setup");
        refused(
            &cmd.env("OOS_SIGNING_KEY", f.path("test signing key")).output().expect("test setup"),
            "stale or replaced artifact",
        );
        assert!(!f.path("selected target/image/oos-dev.oosupd.oos-build.json").exists());
    }
}

mod unit_identity;
