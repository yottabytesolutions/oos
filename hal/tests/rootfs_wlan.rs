// SPDX-License-Identifier: GPL-2.0-or-later

use std::{fs, process::Command};

#[test]
fn falls_back_only_for_a_rejected_saved_configuration() {
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
    assert!(String::from_utf8_lossy(&output.stdout).contains("next save keeps"));
}

const SCENARIOS: &str = r#"
DATA=$1
CONF=$DATA/run/wpa_supplicant.conf
SAVED=$DATA/wlan/wpa_supplicant.conf
PID=$DATA/run/wpa_supplicant.pid
LOG=$DATA/run/wpa_supplicant.log
REJECTED=$CONF.rejected
mkdir "$DATA/setups" "$DATA/wlan"
ip() { :; }
wpa_supplicant() {
    [ -s "$CONF" ] || return 1
    if grep -q bad_syntax "$CONF"; then
        printf '%s\n' 'Configuration file rejected by parser.' >> "$LOG"
        return 1
    fi
    if [ -n "${driver_failure:-}" ]; then
        printf '%s\n' 'Failed to initialize driver interface' >> "$LOG"
        return 1
    fi
}
empty='ctrl_interface=/run/wpa_supplicant
update_config=1'

# Missing file: the empty configuration exists only in RAM.
ready=
start
[ "$(cat "$CONF")" = "$empty" ]
[ ! -e "$SAVED" ]
[ ! -e "$REJECTED" ]

# Supplicant rejects a corrupt copy: the fallback leaves the original on disk.
ready=
printf 'bad_syntax\n' > "$SAVED"
start
[ "$(cat "$CONF")" = "$empty" ]
grep -qx bad_syntax "$SAVED"
[ -f "$REJECTED" ]
start
[ -f "$REJECTED" ]

# Driver failure retries the saved list even if an older log shows a rejection.
ready=
printf '%s\nnetwork={\n\tssid=74657374\n\tkey_mgmt=NONE\n}\n' "$empty" > "$SAVED"
printf '%s\n' 'Configuration file rejected by parser.' > "$LOG"
driver_failure=1
if start; then exit 1; fi
cmp "$SAVED" "$CONF"
[ ! -e "$REJECTED" ]
unset driver_failure
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
