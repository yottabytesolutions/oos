// SPDX-License-Identifier: GPL-2.0-or-later

//! Run writer tests with a private data area and closing marker in a separate test process.

use std::io;
use std::path::{Path, PathBuf};

const ROOT: &str = "OOS_TEST_DATA_ADMISSION_ROOT";

pub(super) fn closing(path: &Path) -> Option<io::Result<bool>> {
    let root = PathBuf::from(std::env::var_os(ROOT)?);
    (path.starts_with("/oos") || path.starts_with(root.join("data"))).then(|| root.join("closing").try_exists())
}

/// Run a test with the data area closed and a sibling directory available for RAM files.
pub fn with_closed_data(module: &str, test: &str, verify: impl FnOnce(&Path, &Path)) {
    if let Some(root) = std::env::var_os(ROOT) {
        let root = PathBuf::from(root);
        verify(&root.join("data"), &root.join("ram"));
        return;
    }

    let (_, module) = module.split_once("::").expect("test lives in a module");
    let name = format!("{module}::{test}");
    let root = std::env::temp_dir().join(format!("oos-admission-{}-{test}", std::process::id()));
    std::fs::create_dir(&root).expect("create isolated test root");
    for directory in ["data", "ram", "closing"] {
        std::fs::create_dir(root.join(directory)).expect("create test fixture");
    }
    let output = std::process::Command::new(std::env::current_exe().expect("test binary"))
        .args(["--exact", &name])
        .env(ROOT, &root)
        .output();
    std::fs::remove_dir_all(root).expect("clean test fixture");
    let output = output.expect("run writer with admission closed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success() && stdout.contains("1 passed"), "{name}: {stdout}");
}

/// Check that the writer returned the shutdown error before reaching the filesystem.
pub fn assert_refused(error: io::Error) {
    assert_eq!(error.kind(), io::ErrorKind::ReadOnlyFilesystem);
    assert_eq!(error.to_string(), "/oos is closed for shutdown");
}
