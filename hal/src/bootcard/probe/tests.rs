// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::bootcard::{read_state, write_state, CardState};

const HOST: &str = "mmc3";
const DEVICE: &str = "/sys/devices/platform/controller.example/mmc_host/mmc3/mmc3:0001";
const OF_LINK: &str = "/sys/devices/platform/controller.example/of_node";
const OF_NODE: &str = "/sys/firmware/devicetree/base/controller.example";
const SDR104_IOS: &str = "clock:\t150000000 Hz\ntiming spec:\t6 (sd uhs SDR104)\n";
const HS_IOS: &str = "timing spec:\t2 (sd high-speed)\n";

struct Fake {
    debugfs: bool,
    ios: Option<&'static str>,
    log: Option<&'static str>,
    card: bool,
    node: bool,
    properties: &'static [&'static str],
}

impl Default for Fake {
    fn default() -> Self {
        Self { debugfs: true, ios: Some(SDR104_IOS), log: None, card: true, node: true, properties: &[] }
    }
}

impl Probe for Fake {
    fn resolve(&self, path: &Path) -> io::Result<PathBuf> {
        match path.to_str().unwrap() {
            CARD_DEVICE if self.card => Ok(DEVICE.into()),
            OF_LINK if self.node => Ok(OF_NODE.into()),
            CARD_DEVICE | OF_LINK => Err(io::ErrorKind::NotFound.into()),
            _ => panic!("unexpected symlink {path:?}"),
        }
    }

    fn has_property(&self, path: &Path) -> io::Result<bool> {
        assert_eq!(path.parent(), Some(Path::new(OF_NODE)));
        Ok(self.properties.contains(&path.file_name().unwrap().to_str().unwrap()))
    }

    fn read_text(&self, path: &Path) -> io::Result<String> {
        assert_eq!(path, Path::new(DEBUGFS).join(HOST).join("ios"));
        self.ios.map(str::to_owned).ok_or_else(|| io::ErrorKind::NotFound.into())
    }

    fn ensure_debugfs(&self) -> bool {
        self.debugfs
    }

    fn kernel_log(&self) -> Option<String> {
        self.log.map(str::to_owned)
    }
}

#[test]
fn timing_number_selects_the_mode() {
    assert_eq!(timing::parse_ios(SDR104_IOS), Ok(Mode::Sdr104));
    assert_eq!(timing::parse_ios(HS_IOS), Ok(Mode::Hs));
    for (number, mode) in [(0, Mode::Legacy), (3, Mode::Sdr12), (4, Mode::Sdr25), (5, Mode::Sdr50), (7, Mode::Ddr50)] {
        assert_eq!(timing::parse_ios(&format!("timing spec:\t{number} (kernel label)")), Ok(mode));
    }
    assert_eq!(timing::parse_ios("timing spec:\t8 (sd uhs SDR104)"), Err(Reason::UnknownTiming));
    for malformed in
        ["", "timing spec: invalid (name)", "timing spec: 2", "timing spec: 2 ()", "timing spec: 2 (name) noise"]
    {
        assert_eq!(timing::parse_ios(malformed), Err(Reason::MalformedTiming), "{malformed:?}");
    }
}

#[test]
fn compares_every_sd_mode_with_each_ceiling() {
    use CardState::{Degraded, Ok, Unknown, Worse};
    for (mode, row) in [
        (Mode::Legacy, [Worse, Worse, Worse, Unknown]),
        (Mode::Hs, [Ok, Degraded, Degraded, Unknown]),
        (Mode::Sdr12, [Worse, Worse, Worse, Unknown]),
        (Mode::Sdr25, [Ok, Degraded, Degraded, Unknown]),
        (Mode::Sdr50, [Ok, Ok, Degraded, Unknown]),
        (Mode::Ddr50, [Ok, Ok, Degraded, Unknown]),
        (Mode::Sdr104, [Ok, Ok, Ok, Unknown]),
        (Mode::Unknown, [Unknown; 4]),
    ] {
        for (expected, state) in [Mode::Hs, Mode::Sdr50, Mode::Sdr104, Mode::Unknown].into_iter().zip(row) {
            assert_eq!(CardState::compare(mode, expected), state, "{mode:?} against {expected:?}");
        }
    }
}

#[test]
fn boot_card_host_and_running_tree_select_the_ceiling() {
    for (properties, expected, state) in [
        (["sd-uhs-sdr104", "sd-uhs-sdr50"].as_slice(), Mode::Sdr104, CardState::Degraded),
        (["sd-uhs-sdr50"].as_slice(), Mode::Sdr50, CardState::Degraded),
        ([].as_slice(), Mode::Hs, CardState::Ok),
    ] {
        let status = detect_with(&Fake { properties, ios: Some(HS_IOS), ..Fake::default() });
        assert_eq!((status.mode(), status.expected(), status.state()), (Mode::Hs, expected, state));
    }
}

#[test]
fn missing_tree_and_boot_card_report_unknown_with_reasons() {
    let status = detect_with(&Fake { node: false, ..Fake::default() });
    assert_eq!((status.mode(), status.expected(), status.state()), (Mode::Sdr104, Mode::Unknown, CardState::Unknown));
    assert_eq!(status.reason, Reason::DeviceTree);
    assert_eq!(detect_with(&Fake { card: false, ..Fake::default() }).reason, Reason::BootHost);
}

#[test]
fn missing_debugfs_or_ios_falls_back_only_to_the_boot_hosts_last_card_add() {
    assert!(debugfs_mounted("debugfs /sys/kernel/debug debugfs rw 0 0\n"));
    assert!(!debugfs_mounted("tmpfs /sys/kernel/debug tmpfs rw 0 0\n"));
    assert!(!debugfs_mounted("debugfs /tmp/debug debugfs rw 0 0\n"));
    const LOG: &str = concat!(
        "[    1.0] mmc3: new ultra high speed SDR104 SDHC card at address 0001\n",
        "<6>[    2.0] mmc3: new high speed SDHC card at address 0001\n",
        "mmc30: new ultra high speed SDR104 SDXC card at address 0002\n",
    );
    assert_eq!(detect_with(&Fake { debugfs: false, log: Some(LOG), ..Fake::default() }).mode(), Mode::Hs);
    assert_eq!(detect_with(&Fake { ios: None, log: Some(LOG), ..Fake::default() }).mode(), Mode::Hs);
    let status = detect_with(&Fake { debugfs: false, ..Fake::default() });
    assert_eq!(
        (status.mode(), status.state(), status.reason),
        (Mode::Unknown, CardState::Unknown, Reason::DebugfsUnavailable)
    );
    let removed = format!("{LOG}mmc3: card 0001 removed\n");
    assert_eq!(timing::from_log(&removed, HOST), None);
}

#[test]
fn unrecognized_ios_cannot_be_replaced_by_an_old_log_mode() {
    let status = detect_with(&Fake {
        ios: Some("timing spec: 11 (new timing)"),
        log: Some("mmc3: new high speed SDHC card at address 0001"),
        ..Fake::default()
    });
    assert_eq!(
        (status.mode(), status.state(), status.reason),
        (Mode::Unknown, CardState::Unknown, Reason::UnknownTiming)
    );
}

#[test]
fn detected_status_round_trips_through_the_startup_file() {
    let directory = std::env::temp_dir().join(format!("oos-sd-hal-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let path = directory.join("state");
    assert_eq!(read_state(&path), Status::default());
    let detected = detect_with(&Fake { ios: Some(HS_IOS), properties: &["sd-uhs-sdr104"], ..Fake::default() });
    write_state(&path, detected).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "mode=hs\nexpected=sdr104\nstate=degraded\nreason=none\n");
    assert_eq!(read_state(&path), detected);
    let unknown = detect_with(&Fake { debugfs: false, ..Fake::default() });
    write_state(&path, unknown).unwrap();
    assert_eq!(read_state(&path), unknown);
    for malformed in
        ["mode=hs\nexpected=hs\nstate=ok\n", "mode=hs\nexpected=hs\nstate=ok\nreason=serial\n", &"x".repeat(513)]
    {
        fs::write(&path, malformed).unwrap();
        assert_eq!(read_state(&path), Status::default());
    }
    fs::remove_dir_all(directory).unwrap();
}
