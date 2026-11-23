// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::gfx::Canvas;
use crate::scpi::Scpi;
use crate::ui::{AMBER, DIM, RED, TEXT};
use std::fs;

fn fixture_dir(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oos-sd-ui-{test}-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn boot_card_record_reaches_all_system_page_rows() {
    let directory = fixture_dir("rows");
    let input = directory.join("input");
    let output = directory.join("output");
    for (mode, expected, state, value, color) in [
        ("sdr104", "sdr104", "ok", "SDR104", TEXT),
        ("hs", "hs", "ok", "High speed", TEXT),
        ("hs", "sdr104", "degraded", "High speed (expected SDR104)", AMBER),
        ("legacy", "sdr104", "worse", "Default speed", RED),
        ("sdr12", "sdr104", "worse", "SDR12", RED),
        ("hs", "sdr104", "unverified", "Not yet seen in SDR104", DIM),
        ("unknown", "unknown", "unknown", "Unknown", DIM),
    ] {
        let streak = u32::from(matches!(state, "degraded" | "worse"));
        fs::write(
            &input,
            format!("mode={mode}\nexpected={expected}\nstate={state}\nreason=none\nceiling={expected}\ndegraded_in_a_row={streak}\nkexec=0\n"),
        )
        .unwrap();
        bootcard::write_state(&output, bootcard::read_state(&input)).unwrap();
        let host = Host::fixture().cache_sd_card(&output);
        fs::remove_file(&output).unwrap();
        let mut ui = Ui::new(Source::Demo { t: 0.0 }, host);
        let cols = ui.sysinfo_rows();
        assert!(matches!(&cols[0][0], Row::Head(label) if label == "Unit"));
        assert!(cols[0].iter().skip(1).take_while(|row| !matches!(row, Row::Head(_))).any(|row| {
            matches!(row, Row::Kv(label, text, c) if label == "SD card" && text == value && *c == color)
        }));
        let reply = Scpi::disconnected().exec_line(&mut ui, &Canvas::new(1, 1), ":SYSTem:STATe?");
        let state_fields = format!(
            "sd.mode={mode} sd.expected={expected} sd.state={state} sd.degraded_in_a_row={streak} sd.repair=none"
        );
        assert!(String::from_utf8(reply).unwrap().contains(&state_fields));
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn longest_sd_fields_fit_the_scpi_response() {
    let directory = fixture_dir("reply");
    let path = directory.join("state");
    // The fields have independent limits. Use their longest wire values together.
    fs::write(&path, "mode=unknown\nexpected=unknown\nstate=unverified\nreason=device-tree-unavailable\n").unwrap();
    let host = Host::fixture().cache_sd_card(&path);
    let mut ui = Ui::new(Source::Demo { t: 0.0 }, host);
    let mut scpi = Scpi::disconnected();
    let reply = scpi.response(&mut ui, &Canvas::new(1, 1), ":SYSTem:STATe?").unwrap();
    assert!(reply.len() < crate::scpi::RESP_MAX && reply.len() < crate::netport::RESPONSE_BYTES);
    let reply = String::from_utf8(reply).unwrap();
    assert_eq!(reply.lines().count(), 1);
    assert!(
        reply.contains("sd.mode=unknown sd.expected=unknown sd.state=unverified sd.degraded_in_a_row=0 sd.repair=none")
    );
    fs::remove_dir_all(directory).unwrap();
}
