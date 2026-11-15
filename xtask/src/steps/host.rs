//! Default Python and explicit image runners. Exit status decides success, never printed PASS.
use super::Outcome;
use std::path::Path;

pub(crate) fn command(root: &Path, program: &str, args: &[&str]) -> Outcome {
    let out = crate::run::capture(root, program, args).map_err(|e| vec![e])?;
    let lines = vec![format!("{}{}", out.stdout, out.stderr)];
    if out.ok {
        Ok(lines)
    } else {
        Err(lines)
    }
}

pub(crate) fn doctor(root: &Path) -> Outcome {
    crate::doctor::inventory(root, &["--gpu", "--sim"])
}

fn host(root: &Path, id: &str) -> Outcome {
    command(root, "python3", &["-I", "tools/check_inventory.py", id])
}

pub(crate) fn python(root: &Path) -> Outcome {
    host(root, "python-tests")
}
pub(crate) fn arm(root: &Path) -> Outcome {
    host(root, "arm64-release-link")
}
pub(crate) fn ssh(root: &Path) -> Outcome {
    host(root, "ssh-sim")
}
pub(crate) fn firewall(root: &Path) -> Outcome {
    host(root, "firewall-sim")
}
pub(crate) fn gpu(root: &Path) -> Outcome {
    command(root, "sh", &["gpu/test-host.sh"])
}
