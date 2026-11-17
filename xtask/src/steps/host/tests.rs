// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use std::fs;

pub(in crate::steps) struct Fixture(pub PathBuf);

impl Fixture {
    pub(in crate::steps) fn new(name: &str) -> Self {
        Self(crate::testrepo::repo(name, &[]))
    }

    pub(in crate::steps) fn write(&self, path: &str, text: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    pub(in crate::steps) fn output(&self, failure: &str) {
        self.write(
            "tools/output.py",
            &format!(
            "import os, sys\nfrom pathlib import Path\nroot = Path(__file__).resolve().parent.parent\n\
             assert sys.flags.isolated\n\
             with (root / 'commands').open('a') as log: log.write(' '.join(sys.argv[1:]) + '\\n')\n\
             if sys.argv[1] == 'target': print(root / 'selected target')\n\
             if {failure:?} and {failure:?} in ' '.join(sys.argv[1:]):\n    sys.exit('stale receipt or failed build')\n"
        ),
        );
    }

    fn commands(&self) -> Vec<String> {
        fs::read_to_string(self.0.join("commands")).unwrap().lines().map(String::from).collect()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn arm_requires_release_link_all_binaries_and_their_receipts() {
    let f = Fixture::new("host-arm");
    f.output("");
    let dir = f.0.join("selected target").join(crate::repo::TARGET).join("release");
    fs::create_dir_all(&dir).unwrap();
    for name in ["oos-ui", "oos-fpgad", "oos-update"] {
        fs::write(dir.join(name), crate::elf::tests::executable()).unwrap();
    }
    let lines = arm(&Default::default(), &f.0).unwrap();
    assert_eq!(lines.iter().filter(|line| line.contains("ELF verified")).count(), 3);
    assert_eq!(f.commands()[0], "build --release --target aarch64-unknown-linux-musl -p oos-ui -p oos-fpgad --bins");
    assert_eq!(f.commands().iter().filter(|line| line.starts_with("verify ")).count(), 3);
    fs::remove_file(dir.join("oos-update")).unwrap();
    assert!(arm(&Default::default(), &f.0).unwrap_err().join("\n").contains("oos-update"));
    f.output("verify");
    assert!(arm(&Default::default(), &f.0).unwrap_err().join("\n").contains("stale receipt"));
    f.output("build");
    fs::remove_file(f.0.join("commands")).unwrap();
    assert!(arm(&Default::default(), &f.0).is_err());
    assert_eq!(f.commands().len(), 1);
}

#[test]
fn simulations_prepare_only_required_roles_in_order_and_stop_on_failure() {
    let f = Fixture::new("host-sim");
    let log = "printf '%s %s\\n' \"$0\" \"$*\" >> commands\n[ -z \"${OOS_OUT:-}\" ] || exit 9\n";
    for script in [
        "tools/build-env.sh",
        "rootfs/build-nft.sh",
        "rootfs/build-dropbear.sh",
        "rootfs/build-sftp-server.sh",
        "tools/ssh-sim.sh",
        "tools/fw-sim.sh",
    ] {
        f.write(script, log);
    }
    let roles: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(crate::repo::root().join("tools/container-images.json")).unwrap())
            .unwrap();
    for (runner, builders, script) in [
        (ssh as crate::inventory::Runner, vec!["nft", "dropbear", "sftp-server"], "tools/ssh-sim.sh"),
        (firewall, vec!["nft"], "tools/fw-sim.sh"),
    ] {
        runner(&Default::default(), &f.0).unwrap();
        let mut expected = vec!["tools/build-env.sh prepare initramfs".to_string()];
        for builder in &builders {
            let role = format!("rootfs-{builder}");
            assert!(!roles["roles"][&role].is_null());
            expected.push(format!("tools/build-env.sh prepare {role}"));
        }
        expected.extend(builders.iter().map(|builder| format!("rootfs/build-{builder}.sh ")));
        expected.push(format!("{script} "));
        assert_eq!(f.commands(), expected);
        fs::remove_file(f.0.join("commands")).unwrap();
    }
    f.write("rootfs/build-nft.sh", "echo FAIL; exit 7\n");
    assert!(ssh(&Default::default(), &f.0).unwrap_err().join("\n").contains("FAIL"));
    assert_eq!(f.commands().len(), 4, "no builders or simulations run after a failure");
}
