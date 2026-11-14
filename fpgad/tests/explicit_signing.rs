// SPDX-License-Identifier: GPL-2.0-or-later
// Host checks only. No card or network access. Every secret key lives in a temporary directory.
use oos_hal::update::{card, hash, kexec, pkg};
use std::path::PathBuf;
use std::process::{Command, Output};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let out = Command::new("mktemp").arg("-d").output().unwrap();
        assert!(out.status.success());
        let f = Self(PathBuf::from(String::from_utf8(out.stdout).unwrap().trim()));
        std::fs::write(f.path("image"), image()).unwrap();
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

    fn keygen(&self, name: &str) -> [u8; 32] {
        let out = self.command().args(["keygen", "--out"]).arg(self.path(name)).output().unwrap();
        success(&out);
        let text = String::from_utf8(out.stdout).unwrap();
        let keys = pkg::parse_keys(&text);
        assert_eq!(keys.len(), 1);
        keys[0]
    }

    fn verify(&self, key: [u8; 32]) {
        let mut bytes = std::fs::read(self.path("package")).unwrap();
        let (header, payload) = pkg::verify(&bytes, &[key]).unwrap();
        assert_eq!(payload, image());
        assert_eq!(header.manifest.build, "20261114-120000 kernel test");
        assert!(header.manifest.extra.contains(&(kexec::LINE.0.into(), kexec::LINE.1.into())));
        card::check_image(payload).unwrap();
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
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn success(out: &Output) {
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}

fn refused(out: &Output, reason: &str) {
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains(reason), "{}", String::from_utf8_lossy(&out.stderr));
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
    let out = f.pack().arg("--untrusted-ok").arg("--key").arg(f.path("private key")).output().unwrap();
    success(&out);
    f.verify(key);
    let bytes = std::fs::read(f.path("package")).unwrap();
    assert!(pkg::verify(&bytes, &[wrong_key]).is_err());
    std::fs::write(f.path("public"), hash::hex(&key)).unwrap();
    success(&f.command().arg("verify").arg(f.path("package")).arg("--keys").arg(f.path("public")).output().unwrap());
}

#[test]
fn environment_selects_key_and_explicit_key_takes_precedence() {
    let f = Fixture::new();
    let key = f.keygen("private");
    success(&f.pack().arg("--untrusted-ok").env("OOS_SIGNING_KEY", f.path("private")).output().unwrap());
    f.verify(key);
    success(
        &f.pack()
            .arg("--untrusted-ok")
            .env("OOS_SIGNING_KEY", f.path("missing"))
            .arg("--key")
            .arg(f.path("private"))
            .output()
            .unwrap(),
    );
    f.verify(key);
}

#[test]
fn no_selection_ignores_home_and_legacy_environment() {
    let f = Fixture::new();
    f.keygen(".config/oos/update-signing.key");
    refused(
        &f.pack().env("OOS_UPDATE_KEY", f.path(".config/oos/update-signing.key")).output().unwrap(),
        "signing requires --key",
    );
    refused(&f.pack().env("OOS_SIGNING_KEY", "").output().unwrap(), "signing requires --key");
    assert!(!f.path("package").exists());
}

#[test]
fn community_requires_opt_in_and_refuses_ambiguous_selection() {
    let f = Fixture::new();
    let text = include_str!("../../keys/community-signing.key");
    let seed: [u8; 32] =
        hash::unhex(text.lines().find(|s| !s.starts_with('#') && !s.is_empty()).unwrap()).unwrap().try_into().unwrap();
    success(&f.pack().arg("--community").output().unwrap());
    f.verify(oos_hal::update::ed25519::public_key(&seed));
    refused(&f.pack().arg("--community").arg("--key").arg(f.path("missing")).output().unwrap(), "cannot be combined");
    refused(
        &f.pack().arg("--community").env("OOS_SIGNING_KEY", f.path("missing")).output().unwrap(),
        "cannot be combined",
    );
}

#[test]
fn missing_or_invalid_selected_key_never_falls_back() {
    let f = Fixture::new();
    refused(&f.pack().arg("--key").arg(f.path("missing")).output().unwrap(), "missing");
    refused(&f.pack().env("OOS_SIGNING_KEY", f.path("missing")).output().unwrap(), "missing");
    std::fs::write(f.path("invalid"), "invalid").unwrap();
    refused(&f.pack().arg("--key").arg(f.path("invalid")).output().unwrap(), "no 64-digit hex secret key");
    assert!(!f.path("package").exists());
}

#[test]
fn keygen_requires_explicit_output() {
    let f = Fixture::new();
    refused(&f.command().arg("keygen").output().unwrap(), "keygen needs --out");
    assert!(!f.path(".config").exists());
}

impl Fixture {
    fn script(&self) -> Command {
        use std::os::unix::fs::PermissionsExt;
        let out = self.path("boot/out");
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(out.join("oos-dev.img"), image()).unwrap();
        std::fs::write(out.join("oos-dev.build"), "20261114-120000 kernel test\n").unwrap();
        std::fs::write(self.path("boot/sign-dev-image.sh"), include_str!("../../boot/sign-dev-image.sh")).unwrap();
        std::fs::create_dir(self.path("bin")).unwrap();
        // Skip Cargo's rebuild. Run the signer binary built for this host check.
        let cargo = self.path("bin/cargo");
        std::fs::write(
            &cargo,
            "#!/bin/sh\nwhile [ \"$1\" != -- ]; do shift; done\nshift\nexec \"$OOS_TEST_SIGNER\" \"$@\"\n",
        )
        .unwrap();
        std::fs::set_permissions(cargo, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut cmd = Command::new("sh");
        cmd.arg(self.path("boot/sign-dev-image.sh"));
        let path = std::env::join_paths(
            std::iter::once(self.path("bin")).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
        )
        .unwrap();
        cmd.env("PATH", path).env("OOS_TEST_SIGNER", env!("CARGO_BIN_EXE_oos-update")).env_remove("OOS_SIGNING_KEY");
        cmd
    }
}

#[test]
fn image_signing_script_uses_saved_build_id_and_output_name() {
    let f = Fixture::new();
    let mut cmd = f.script();
    std::fs::write(f.path("private key"), include_str!("../../keys/community-signing.key")).unwrap();
    success(&cmd.arg("--key").arg(f.path("private key")).output().unwrap());
    let bytes = std::fs::read(f.path("boot/out/oos-dev.oosupd")).unwrap();
    let (header, payload) = pkg::verify(&bytes, &oos_hal::update::trusted_keys()).unwrap();
    assert_eq!(header.manifest.build, "20261114-120000 kernel test");
    assert_eq!(payload, image());
    assert!(!f.path("boot/out/DHO800_DHO900_OOS_Install.GEL").exists());
}

#[test]
fn trusted_public_keys_are_never_signing_seeds() {
    let f = Fixture::new();
    std::fs::write(f.path("public"), include_str!("../../keys/update.pub")).unwrap();
    refused(&f.pack().arg("--key").arg(f.path("public")).output().unwrap(), "public key");
    for key in oos_hal::update::trusted_keys() {
        std::fs::write(f.path("public"), hash::hex(&key)).unwrap();
        refused(&f.pack().arg("--key").arg(f.path("public")).arg("--untrusted-ok").output().unwrap(), "public key");
        assert!(!f.path("package").exists());
    }
}

#[test]
fn untrusted_signing_keys_require_explicit_opt_in() {
    let f = Fixture::new();
    let key = f.keygen("private");
    assert!(!oos_hal::update::trusted_keys().contains(&key));
    refused(&f.pack().arg("--key").arg(f.path("private")).output().unwrap(), "--untrusted-ok");
    refused(&f.pack().env("OOS_SIGNING_KEY", f.path("private")).output().unwrap(), "--untrusted-ok");
    assert!(!f.path("package").exists());
    success(&f.pack().arg("--key").arg(f.path("private")).arg("--untrusted-ok").output().unwrap());
    f.verify(key);
}

#[test]
fn signing_flags_are_pack_only() {
    let f = Fixture::new();
    for flag in ["--community", "--untrusted-ok"] {
        for command in ["verify", "keygen", "status"] {
            refused(&f.command().args([command, flag]).output().unwrap(), "only valid for pack");
        }
        refused(&f.command().arg(flag).output().unwrap(), "only valid for pack");
    }
}

#[test]
fn image_signing_script_requires_saved_build_id() {
    let f = Fixture::new();
    let mut cmd = f.script();
    std::fs::remove_file(f.path("boot/out/oos-dev.build")).unwrap();
    refused(&cmd.arg("--community").output().unwrap(), "run boot/build-dev-image.sh first");
    assert!(!f.path("boot/out/oos-dev.oosupd").exists());
}

#[test]
fn image_signing_script_accepts_community_and_environment() {
    let f = Fixture::new();
    let mut cmd = f.script();
    refused(&cmd.output().unwrap(), "signing requires --key");
    assert!(!f.path("boot/out/oos-dev.oosupd").exists());
    std::fs::write(f.path("private key"), include_str!("../../keys/community-signing.key")).unwrap();
    success(&cmd.env("OOS_SIGNING_KEY", f.path("private key")).output().unwrap());
    std::fs::remove_file(f.path("boot/out/oos-dev.oosupd")).unwrap();
    success(&cmd.env_remove("OOS_SIGNING_KEY").arg("--community").output().unwrap());
    let bytes = std::fs::read(f.path("boot/out/oos-dev.oosupd")).unwrap();
    pkg::verify(&bytes, &oos_hal::update::trusted_keys()).unwrap();
}

#[test]
fn image_signing_script_refuses_extra_or_incomplete_arguments() {
    let f = Fixture::new();
    let cmd = f.script();
    let path = cmd.get_envs().find(|(key, _)| *key == "PATH").unwrap().1.unwrap().to_owned();
    // A successful stand-in makes any invocation of Cargo visible as an unexpected exit 0.
    std::fs::write(f.path("bin/cargo"), "#!/bin/sh\nexit 0\n").unwrap();
    for args in [
        vec!["--out", "elsewhere"],
        vec!["--image", "elsewhere"],
        vec!["--build", "other"],
        vec!["--key"],
        vec!["--key", ""],
        vec!["--community", "--untrusted-ok"],
        vec!["pack"],
    ] {
        let out =
            Command::new("sh").arg(f.path("boot/sign-dev-image.sh")).args(&args).env("PATH", &path).output().unwrap();
        refused(&out, "Usage:");
    }
    assert!(!f.path("boot/out/oos-dev.oosupd").exists());
}

#[test]
fn image_workflow_requires_signed_update_and_gel() {
    let workflow = include_str!("../../.github/workflows/image.yml");
    let build = workflow.find("run: boot/build-dev-image.sh").unwrap();
    let sign = workflow.find("run: boot/sign-dev-image.sh --community").unwrap();
    let gel = workflow.find("run: python3 -I boot/build-gel.py").unwrap();
    let check = workflow.find("      - name: Validate release artifacts\n        run: |\n").unwrap();
    let upload = workflow[check..].find("      - uses: actions/upload-artifact@").unwrap() + check;
    assert!(build < sign && sign < gel && gel < check && check < upload);
    let script = workflow[check..upload].split_once("run: |\n").unwrap().1;
    let script = script.lines().map(|line| line.strip_prefix("          ").unwrap()).collect::<Vec<_>>().join("\n");
    let f = Fixture::new();
    std::fs::create_dir_all(f.path("boot/out")).unwrap();
    for (update, gel) in [(false, false), (true, false), (false, true), (true, true)] {
        for (name, present) in [("oos-dev.oosupd", update), ("DHO800_DHO900_OOS_Install.GEL", gel)] {
            let path = f.path("boot/out").join(name);
            if present {
                std::fs::write(path, "artifact").unwrap();
            } else if path.exists() {
                std::fs::remove_file(path).unwrap();
            }
        }
        let out = Command::new("bash").args(["-e", "-c", &script]).current_dir(&f.0).output().unwrap();
        assert_eq!(out.status.success(), update && gel, "{}", String::from_utf8_lossy(&out.stderr));
    }
}
