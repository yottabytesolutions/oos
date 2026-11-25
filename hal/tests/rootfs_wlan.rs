// SPDX-License-Identifier: GPL-2.0-or-later

use std::{fs, process::Command};

#[test]
fn starts_from_saved_configuration_or_falls_back_to_empty_ram() {
    let source = include_str!("../../rootfs/initramfs/bin/oos-wlan");
    let (_, functions) = source.split_once("\nstart_supplicant()").unwrap();
    let (functions, _) = functions.split_once("\nready=\n").unwrap();
    let dir = std::env::temp_dir().join(format!("oos-wlan-start-{}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    let script = format!("start_supplicant(){functions}\n{SCENARIOS}");
    let output = Command::new("sh").args(["-ec", &script, "test-wlan"]).arg(&dir).output().unwrap();
    fs::remove_dir_all(&dir).unwrap();
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

const SCENARIOS: &str = r#"
DATA=$1
CONF=$DATA/run/wpa_supplicant.conf
SAVED=$DATA/wlan/wpa_supplicant.conf
PID=$DATA/run/wpa_supplicant.pid
mkdir "$DATA/setups" "$DATA/wlan"
ip() { :; }
wpa_supplicant() {
    [ -z "${driver_failure:-}" ] || return 1
    [ -s "$CONF" ] && ! grep -q bad_syntax "$CONF"
}
empty='ctrl_interface=/run/wpa_supplicant
update_config=1'

# Missing file: the empty configuration exists only in RAM.
ready=
start
[ "$(cat "$CONF")" = "$empty" ]
[ ! -e "$SAVED" ]

# Supplicant rejects a corrupt copy: the fallback leaves the original on disk.
ready=
printf '%s\nbad_syntax\n' "$empty" > "$SAVED"
start
[ "$(cat "$CONF")" = "$empty" ]
grep -qx bad_syntax "$SAVED"

# A valid saved copy survives the change in configuration location.
ready=
printf '%s\nnetwork={\n\tssid=74657374\n\tkey_mgmt=NONE\n}\n' "$empty" > "$SAVED"
start
cmp "$SAVED" "$CONF"

# A retry after the daemon ran must keep its last RAM save, even on startup failure.
printf '%s\n# changed by supplicant\n' "$empty" > "$CONF"
driver_failure=1
if start; then exit 1; fi
unset driver_failure
start
grep -qx '# changed by supplicant' "$CONF"
"#;
