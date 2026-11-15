// SPDX-License-Identifier: GPL-2.0-or-later

mod support;

use std::process::Command;

#[test]
fn compare_scripted_pictures() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let inputs = support::isolated_inputs();
    let smoke = (cfg!(debug_assertions) || std::env::var_os("LLVM_PROFILE_FILE").is_some())
        && std::env::var("OOS_FULL_GOLDEN").as_deref() != Ok("1");
    let inventory = support::cases::inventory(&inputs.path("cases"), smoke);
    let mut command = Command::new("python3");
    command
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .current_dir(root)
        .arg("ui/tools/golden_run.py")
        .arg("--inventory")
        .arg(inventory)
        .arg("--output-root")
        .arg(inputs.path("").parent().unwrap().parent().unwrap())
        .arg("--binary")
        .arg(env!("CARGO_BIN_EXE_oos-ui"))
        .arg("--profile")
        .arg(if cfg!(debug_assertions) { "debug" } else { "release" });
    if smoke {
        command.arg("--subset");
    }
    let output = support::run_group(&inputs, command, 3600);
    assert!(
        output.status.success(),
        "golden runner failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    print!("{}", String::from_utf8_lossy(&output.stdout));
}

#[test]
#[ignore = "explicit golden-update command only; never replaces references itself"]
fn export_update_inventory() {
    let out = std::env::var_os("OOS_GOLDEN_INVENTORY").expect("explicit update output required");
    support::cases::inventory(std::path::Path::new(&out), false);
}
