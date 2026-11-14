// SPDX-License-Identifier: GPL-2.0-or-later

//! Host inputs, the switches read at start and permission to run OS services, supplied when constructing the UI.

use crate::board::{board, Board, Zone};
use std::path::{Path, PathBuf};

type Space = Option<(u64, u64)>;

pub struct Host {
    text: fn(&str) -> Option<String>,
    space: fn(&str) -> Space,
    temperatures: fn() -> Vec<Zone>,
    unix: fn() -> f64,
    directory: fn(&Path) -> bool,
    services: bool,
    usb_root: PathBuf,
    switches: crate::sysconf::Switches,
}

impl Host {
    pub fn system(switches: crate::sysconf::Switches) -> Self {
        Self {
            text: |p| std::fs::read_to_string(p).ok(),
            space: super::sysinfo::disk,
            temperatures: || board().thermal_zones(),
            unix: crate::clock::unix_now,
            directory: Path::is_dir,
            services: true,
            usb_root: crate::usbtmc::SYSFS.into(),
            switches,
        }
    }

    pub fn text(&self, path: &str) -> Option<String> {
        (self.text)(path)
    }
    pub fn disk(&self, path: &str) -> Space {
        (self.space)(path)
    }
    pub fn temperatures(&self) -> Vec<Zone> {
        (self.temperatures)()
    }
    pub fn unix_now(&self) -> f64 {
        self.switches.fixed_time.unwrap_or_else(self.unix)
    }
    pub fn is_dir(&self, path: &Path) -> bool {
        (self.directory)(path)
    }
    pub fn services(&self) -> bool {
        self.services
    }
    pub fn usb_root(&self) -> PathBuf {
        self.usb_root.clone()
    }
    pub fn switches(&self) -> &crate::sysconf::Switches {
        &self.switches
    }

    #[cfg(test)]
    pub fn fixture() -> Self {
        Self {
            text: |_| None,
            space: |_| None,
            temperatures: Vec::new,
            unix: || 946_684_800.0,
            directory: |_| false,
            services: false,
            // inside the crate and never created: no USB device of the build machine is seen
            usb_root: concat!(env!("CARGO_MANIFEST_DIR"), "/tests/no-usb-root").into(),
            switches: crate::sysconf::Switches::default(),
        }
    }
}

#[cfg(test)]
impl super::Ui {
    pub(crate) fn for_tests(src: crate::wave::Source) -> Self {
        Self::new(src, Host::fixture())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::Item;
    use crate::ui::{sysinfo::Row, Ui};
    use crate::wave::Source;

    #[test]
    fn supplied_clock_controls_ui_dates_without_changing_the_system_clock() {
        for (unix, want) in [(0.0, "1970-01-01 00:00:00"), (946_684_800.0, "2000-01-01 00:00:00")] {
            let mut host = Host::fixture();
            host.unix = if unix == 0.0 { || 0.0 } else { || 946_684_800.0 };
            let ui = Ui::new(Source::Demo { t: 0.0 }, host);
            assert_eq!(ui.local_now().text(), want);
        }
    }

    #[test]
    fn supplied_host_metadata_reaches_about_system_info_and_update() {
        let mut host = Host::fixture();
        host.text = |path| match path {
            "/etc/oos-build" => Some("20000101-000000 fixture".into()),
            "/proc/uptime" => Some("12345.0 100.0".into()),
            "/proc/meminfo" => Some("MemAvailable: 512 kB\nMemTotal: 1024 kB".into()),
            "/proc/loadavg" => Some("0.10 0.20 0.30 1/100 1".into()),
            _ => None,
        };
        host.space = |_| Some((512, 1024));
        host.temperatures = || vec![Zone { name: "cpu-thermal".into(), temp: 42.0 }];
        let mut ui = Ui::new(Source::Demo { t: 0.0 }, host);
        ui.activate(Item::About);
        assert!(ui.dialog.as_ref().unwrap().iter().any(|l| l == "Build 20000101-000000"));
        let rows = ui.sysinfo_rows();
        assert!(rows[0].iter().any(|r| matches!(r, Row::Kv(k, v, _) if k == "Uptime" && v == "3 h 25 min")));
        assert!(rows[1].iter().any(|r| matches!(r, Row::Kv(k, v, _) if k == "SoC (CPU)" && v == "42.0 \u{b0}C")));
        assert!(ui.update_lines().iter().any(|l| l == "Running 20000101-000000"));
    }

    #[test]
    fn update_directory_without_state_file_shows_the_default_state() {
        let ui = Ui::for_tests(Source::Demo { t: 0.0 });
        assert!(ui.update_lines().iter().any(|l| l == "No update installed yet"));
        let mut host = Host::fixture();
        host.directory = |d| d == Path::new(oos_hal::update::STATE_PATH).parent().unwrap_or(Path::new("/"));
        let lines = Ui::new(Source::Demo { t: 0.0 }, host).update_lines();
        for want in ["Recovery -", "Rollback image -", "Boot mode one-shot"] {
            assert!(lines.iter().any(|l| l == want), "{}: {:?}", want, lines);
        }
        assert!(!lines.iter().any(|l| l == "No update installed yet"), "{:?}", lines);
    }

    // Break it catches: the network page reading the build machine's eth0 (its MAC in a fixture picture).
    #[test]
    fn network_page_reads_the_interface_through_the_host() {
        let ui = Ui::for_tests(Source::Demo { t: 0.0 });
        assert_eq!((ui.net_value(Item::NetEth), ui.net_value(Item::NetMac)), ("no cable".into(), "-".into()));
        let mut host = Host::fixture();
        host.text = |path| match path {
            "/sys/class/net/eth0/carrier" => Some("1\n".into()),
            "/sys/class/net/eth0/address" => Some("00:00:5e:00:53:01\n".into()),
            _ => None,
        };
        let ui = Ui::new(Source::Demo { t: 0.0 }, host);
        assert_eq!(ui.net_value(Item::NetEth), "link up, no address yet");
        assert_eq!(ui.net_value(Item::NetMac), "00:00:5e:00:53:01");
    }

    #[test]
    fn mandatory_paths_ignore_the_process_environment() {
        // Set every switch the process used to read; the UI built for tests must not see them.
        let vars = [
            ("OOS_DEMO_LOOP", "15"),
            ("OOS_COMPRESS", "1"),
            ("OOS_PIPELINE", "batch"),
            ("OOS_INTERP", "fast"),
            ("OOS_DEEP_RAW", "0"),
            ("OOS_PLACE", "1"),
            ("OOS_IPV6", "1"),
            ("OOS_AUTOSET_LEVELS", "records"),
            ("OOS_HWAVG", "hold"),
            ("OOS_FRAME_HEADER", "1"),
            ("OOS_TRIG_HW_STATUS", "0"),
            ("OOS_PREARM", "1"),
            ("OOS_FIXED_TIME", "1"),
        ];
        for (k, v) in vars {
            std::env::set_var(k, v);
        }
        let ui = Ui::for_tests(Source::Demo { t: 0.0 });
        let want = crate::sysconf::Switches::default();
        assert_eq!(*ui.switches(), want);
        let hw = |sw| crate::board::dho::hw::settings(&ui.m, false, sw);
        assert!(hw(ui.switches()) == hw(&want), "the hardware settings follow the supplied switches only");
        assert_eq!(ui.local_now().text(), "2000-01-01 00:00:00", "the fixture's clock, not OOS_FIXED_TIME");
        assert!(crate::clock::unix_now() > 1e9, "the system clock does not read OOS_FIXED_TIME");
        for (k, _) in vars {
            std::env::remove_var(k);
        }
    }
}
