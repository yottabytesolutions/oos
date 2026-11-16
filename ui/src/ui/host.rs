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
    time_budgets: bool,
    usb_root: PathBuf,
    switches: crate::sysconf::Switches,
    runtime: super::lifecycle::Runtime,
    firmware: String,
}

impl Host {
    pub fn system(switches: crate::sysconf::Switches) -> Self {
        let mut runtime = super::lifecycle::Runtime::new(switches.memory);
        runtime.crash = crate::store::startup::crash::latest(Path::new("/oos/crash")).ok().flatten();
        Self {
            runtime,
            firmware: String::new(),
            text: |p| std::fs::read_to_string(p).ok(),
            space: super::sysinfo::disk,
            temperatures: || board().thermal_zones(),
            unix: crate::clock::unix_now,
            directory: Path::is_dir,
            services: true,
            time_budgets: true,
            usb_root: crate::usbtmc::SYSFS.into(),
            switches,
        }
        .cache_firmware()
    }

    fn cache_firmware(mut self) -> Self {
        self.firmware = format!("oos-ui {}", env!("CARGO_PKG_VERSION"));
        if let Some(build) = self.text("/etc/oos-build") {
            // Bound retained identity metadata and preserve the SCPI field/message separators.
            if let Some(id) = build.split_whitespace().next().filter(|id| id.len() <= 128 && !id.contains([',', ';'])) {
                self.firmware.push_str(" build ");
                self.firmware.push_str(id);
            }
        }
        self
    }

    pub(super) fn runtime(&self) -> &super::lifecycle::Runtime {
        &self.runtime
    }
    pub(super) fn runtime_mut(&mut self) -> &mut super::lifecycle::Runtime {
        &mut self.runtime
    }

    pub(super) fn firmware_version(&self) -> &str {
        &self.firmware
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

    /// Hermetic off-screen execution, including the ordinary binary built without test configuration.
    pub fn headless() -> Self {
        Self {
            runtime: super::lifecycle::Runtime::new(crate::system::memory::Limits::parse("")),
            firmware: format!("oos-ui {}", env!("CARGO_PKG_VERSION")),
            text: |_| None,
            space: |_| None,
            temperatures: Vec::new,
            unix: || 1_762_862_400.0,
            directory: |_| false,
            services: false,
            time_budgets: false,
            // inside the crate and never created: no USB device of the build machine is seen
            usb_root: concat!(env!("CARGO_MANIFEST_DIR"), "/tests/no-usb-root").into(),
            switches: crate::sysconf::Switches { pipeline: crate::drawpipe::Mode::Batch, ..Default::default() },
        }
    }

    pub fn time_budgets(&self) -> bool {
        self.time_budgets
    }

    #[cfg(test)]
    pub fn fixture() -> Self {
        // the tests run the measure-unit path of Auto, which is what they check (`autoset.levels = unit`)
        let switches = crate::sysconf::Switches { autoset_unit_levels: true, ..Default::default() };
        Self { unix: || 946_684_800.0, time_budgets: true, switches, ..Self::headless() }
    }

    #[cfg(test)]
    pub fn fixture_with_text(text: fn(&str) -> Option<String>) -> Self {
        Self { text, ..Self::fixture() }.cache_firmware()
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
    mod publication;
    use super::*;
    use crate::ui::Item;
    use crate::ui::{sysinfo::Row, Ui};
    use crate::wave::Source;

    fn idn(host: Host) -> String {
        let mut ui = Ui::new(Source::Demo { t: 0.0 }, host);
        let cv = crate::gfx::Canvas::new(crate::ui::PANEL_W, crate::ui::PANEL_H);
        let mut scpi = crate::scpi::Scpi::open(0, false);
        String::from_utf8(scpi.exec_line(&mut ui, &cv, "*IDN?")).unwrap()
    }

    #[test]
    fn idn_reports_build_in_the_fourth_field() {
        let mut host = Host::fixture_with_text(|path| {
            assert_eq!(path, "/etc/oos-build");
            Some(" 20261115-143335 kernel fixture\n".into())
        });
        host.text = |_| panic!("IDN must use the identity captured at start");
        let reply = idn(host);
        let p = oos_hal::model::platform();
        assert_eq!(
            reply.split(',').collect::<Vec<_>>(),
            [
                "OOS",
                p.caps.name.as_str(),
                p.serial.as_deref().unwrap_or("0"),
                &format!("oos-ui {} build 20261115-143335", env!("CARGO_PKG_VERSION"))
            ]
        );
    }

    #[test]
    fn idn_keeps_existing_reply_without_build_file() {
        // Missing and unreadable files both become None at the Host text seam.
        for host in [
            Host::fixture(),
            Host::fixture_with_text(|_| None),
            Host::fixture_with_text(|_| Some(String::new())),
            Host::fixture_with_text(|_| Some(" \n\t".into())),
        ] {
            let reply = idn(host);
            let p = oos_hal::model::platform();
            assert_eq!(
                reply,
                format!(
                    "OOS,{},{},oos-ui {}",
                    p.caps.name,
                    p.serial.as_deref().unwrap_or("0"),
                    env!("CARGO_PKG_VERSION")
                )
            );
        }
    }

    #[test]
    fn idn_ignores_build_ids_outside_response_bounds() {
        for host in [
            Host::fixture_with_text(|_| Some("20261115,143335".into())),
            Host::fixture_with_text(|_| Some("20261115;143335".into())),
            Host::fixture_with_text(|_| Some("x".repeat(129))),
        ] {
            assert_eq!(idn(host), idn(Host::fixture()));
        }
    }

    #[test]
    fn memory_stop_still_polls_update_ntp_ssh_and_usb() {
        let mut host = Host::fixture();
        host.services = true;
        host.text = |path| (path == "/tmp/oos-update.notice").then(|| "Update completed".into());
        let mut ui = Ui::new(Source::Demo { t: 0.0 }, host);
        ui.host.runtime_mut().memory = crate::system::memory::Watchdog::new(crate::system::memory::Limits::parse(
            "mem.soft_mb = 1\nmem.hard_mb = 2\n",
        ));
        ui.host.runtime_mut().memory.sample(std::time::Duration::ZERO, Some(3 << 20));
        ui.m.run = crate::model::RunState::Stop;
        ui.ntp.st.due = Some(0.0);
        ui.sys.ntp = false;
        ui.ssh.state = "not polled".into();
        // An empty directory keeps the SSH poll hermetic; there are no keys to import.
        let root = std::env::temp_dir().join(format!("oos-stop-polls-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        ui.set_ssh_dir(root.clone());
        ui.open_menu(crate::ui::MenuId::Ssh);
        ui.step(0.5);
        assert!(ui.memory_stopped() && ui.last[0].is_none());
        assert!(ui.dialog.as_ref().unwrap().iter().any(|line| line == "Update completed"));
        assert!(ui.ntp.st.due.is_none(), "NTP off poll clears scheduled work");
        assert_ne!(ui.ssh.state, "not polled");
        assert_eq!(ui.usb_check, ui.now + 2.0, "USB poll schedules its next check");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn archived_crash_reaches_state_query_and_system_page() {
        let root = std::env::temp_dir().join(format!("oos-crash-presentation-{}", std::process::id()));
        let archive = root.join("1-20260101T000000Z-abcdef012345");
        std::fs::create_dir_all(&archive).unwrap();
        std::fs::write(archive.join("dmesg-ramoops-0"), "Panic#1 Part1\nfull kernel body").unwrap();
        let mut host = Host::fixture();
        host.runtime.crash = crate::store::startup::crash::latest(&root).unwrap();
        let ui = Ui::new(Source::Demo { t: 0.0 }, host);
        let text = crate::cmd::stat_text(&ui);
        assert!(text.contains("last_crash 20260101T000000Z Panic#1 Part1"));
        assert!(!text.contains("full kernel body"));
        assert!(ui.sysinfo_rows()[0].iter().any(|row| matches!(row,
            Row::Note(text, _) if text == "Last crash: 20260101T000000Z Panic#1 Part1")));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn headless_host_supplies_config_without_services_or_time_budgets() {
        let host = Host::headless();
        assert!(!host.services() && !host.time_budgets());
        assert!(Host::system(Default::default()).time_budgets());
        assert_eq!(host.unix_now(), 1_762_862_400.0);
        assert_eq!(host.switches().pipeline, crate::drawpipe::Mode::Batch);
        assert_eq!(host.switches().cores, Default::default());
        assert!(host.text("/etc/oos-build").is_none());
        assert!(host.disk("/").is_none());
        assert!(host.temperatures().is_empty());
    }

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
        // the fixture's switches: the defaults, with the measure-unit path of Auto the tests check
        let want = crate::sysconf::Switches { autoset_unit_levels: true, ..Default::default() };
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

#[cfg(test)]
mod auto_timeout {
    use super::Host;
    use crate::board::scripted::{self, Batch, BatchKind, BatchSource};
    use crate::board::{Frame, HwShared};
    use crate::drawpipe::{self, DecodeInto, Mode, Pipe};
    use crate::model::{RunState, TrigMode, TrigStatus};
    use crate::ui::Ui;
    use crate::wave::{Phosphor, Source};
    use oos_hal::sync::lock;
    use std::sync::{mpsc, Arc, Mutex};
    use std::time::Duration;

    const HANG_LIMIT: Duration = Duration::from_secs(30);

    fn hardware() -> (Ui, HwShared) {
        let shared: HwShared = Default::default();
        let mut host = Host::fixture();
        host.time_budgets = false;
        let mut ui = Ui::new(Source::Hw { shared: shared.clone(), seen: 0 }, host);
        ui.m.persist = 0;
        ui.m.trig_level = 0.5;
        ui.step(0.0);
        assert!(ui.fast_ok());
        (ui, shared)
    }

    fn deliver(shared: &HwShared, mut frame: Frame, tpu: bool) {
        let mut g = lock(shared);
        let id = g.acquisition.identity();
        (frame.cfg, frame.stamp) = (id.gen(), id.epoch());
        g.trig_fired = Some((frame.seq, Some(tpu)));
        g.frame = frame.clone();
        g.queue.push_back(frame);
    }

    fn crossing(ui: &Ui, seq: u64) -> Frame {
        let n = 4000;
        Frame {
            seq,
            dt: ui.m.time_per_div() as f64 * crate::model::HDIV as f64 * 1.2 / n as f64,
            pre: n / 2,
            ch: [
                (0..n).map(|k| if (k / 200) % 2 == 1 { 1.0 } else { 0.0 }).collect(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ],
            ..Default::default()
        }
    }

    // Empty screen steps advance the UI clock and WAIT deadline without inventing records or clearing the trace.
    #[test]
    fn auto_records_do_not_clock_ui() {
        let (mut ui, shared) = hardware();
        for _ in 0..2 {
            ui.step(0.125);
            assert_eq!((ui.records, ui.wfm_total), (0, 0));
        }
        let frame = crossing(&ui, 1);
        assert!(crate::wave::hw_record(&ui.m, frame.clone()).triggered, "software sees an edge");
        deliver(&shared, frame, false);
        ui.step(0.03125);
        assert_eq!((ui.records, ui.wfm_total, ui.m.status), (1, 1, TrigStatus::Auto));
        let (map, zoom) = (ui.ph.map.clone(), ui.phz.map.clone());
        assert!(map[0].iter().any(|&v| v > 0));
        for _ in 0..2 {
            ui.step(0.25);
            assert_eq!((ui.records, ui.wfm_total, ui.m.status), (1, 1, TrigStatus::Auto));
            assert_eq!((&ui.ph.map, &ui.phz.map), (&map, &zoom));
        }
        assert_eq!(ui.now - ui.last_rec_t, 0.5, "WAIT starts strictly after the deadline");
        ui.step(1.0 / 1024.0);
        assert_eq!((ui.records, ui.wfm_total, ui.m.status), (1, 1, TrigStatus::Wait));
        assert_eq!((&ui.ph.map, &ui.phz.map), (&map, &zoom));
        assert_eq!(ui.now, 0.7822265625);
        deliver(&shared, crossing(&ui, 2), true);
        ui.step(0.03125);
        assert_eq!((ui.records, ui.wfm_total, ui.m.status), (2, 2, TrigStatus::Triggered));
        ui.step(0.03125);
        assert_eq!((ui.records, ui.wfm_total, ui.m.status), (2, 2, TrigStatus::Triggered));
    }

    // Demo's immediate AUTO fallback characterizes its generator, not the hardware's SCU timers.
    #[test]
    fn demo_auto_is_immediate_but_normal_and_single_wait() {
        for mode in [TrigMode::Auto, TrigMode::Normal, TrigMode::Single] {
            let mut ui = Ui::for_tests(Source::Demo { t: 0.0 });
            ui.m.trig_mode = mode;
            ui.m.trig_level = 1e6;
            ui.step(1.0 / 1024.0);
            let count = if mode == TrigMode::Auto { 6 } else { 0 };
            assert_eq!((ui.records, ui.wfm_total), (count, count), "{:?}", mode);
            assert_eq!(ui.m.run, RunState::Run);
            if mode == TrigMode::Auto {
                assert_eq!(ui.m.status, TrigStatus::Auto);
                assert!(ui.history.iter().all(|r| !r.triggered));
                assert!(!ui.last[0].as_ref().unwrap().triggered);
            } else {
                assert_eq!(ui.m.status, TrigStatus::Wait);
                assert!(ui.last[0].is_none());
            }
            // CH1's demo square is 0..2.5 V; use a level inside that range.
            ui.m.trig_level = 1.25;
            ui.step(1.0 / 30.0);
            assert!(ui.records > count, "{:?}: a matching edge still delivers", mode);
            assert!(ui.last[0].as_ref().unwrap().triggered);
            assert_eq!((ui.m.run, ui.m.status), (RunState::Run, TrigStatus::Triggered));
            if mode == TrigMode::Single {
                let records = ui.records;
                ui.step(1.0 / 30.0);
                assert_eq!(ui.records, records, "pending SINGLE completion accepts no extra record");
                assert_eq!(ui.m.status, TrigStatus::Stop);
            }
            assert_eq!(ui.m.run, if mode == TrigMode::Single { RunState::Stop } else { RunState::Run });
        }
    }

    fn held_decode(inner: DecodeInto) -> (DecodeInto, mpsc::Receiver<()>, mpsc::Sender<()>) {
        let (entered_tx, entered) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let gate = Mutex::new(Some((entered_tx, released)));
        let decode = Arc::new(move |bytes: &[u8], frame: &mut Frame| {
            if let Some((entered, released)) = lock(&gate).take() {
                entered.send(()).unwrap();
                released.recv_timeout(HANG_LIMIT).expect("test releases the held worker");
            }
            inner(bytes, frame);
        });
        (decode, entered, release)
    }

    fn assert_backlog(
        pipe: &Pipe,
        ui: &mut Ui,
        shared: &HwShared,
        map: &mut Phosphor,
        delivered: &mpsc::Receiver<u64>,
    ) {
        for _ in 0..3 {
            ui.step(0.03125);
            assert_eq!((ui.records, ui.wfm_total), (0, 0));
            assert!(lock(shared).queue.is_empty());
            assert_eq!(pipe.merge(map, &mut true, &mut [0; 4], &mut (0.0, 0.0)).frames, 0);
            assert!(matches!(delivered.try_recv(), Err(mpsc::TryRecvError::Empty)));
        }
    }

    fn assert_picture(worker_map: &Phosphor, ui: &Ui, reference: &Ui, frames: usize) {
        for c in 0..crate::model::NCH {
            let combined: Vec<_> = worker_map.map[c].iter().zip(&ui.ph.map[c]).map(|(a, b)| a + b).collect();
            assert_eq!(combined, reference.ph.map[c], "{} frames: CH{}", frames, c + 1);
        }
    }

    // M1 has no Ui pipe injection yet. Merge the fixture-owned pipe explicitly, then consume its newest frame in
    // frames mode. Compare the sum of worker and newest-frame maps with the ordinary all-frames route.
    fn backlog_batch(frames: usize) {
        let (mut ui, shared) = hardware();
        let (mut reference, reference_shared) = hardware();
        let mut sw = *ui.switches();
        sw.pipeline = Mode::Batch;
        let mut source = scripted::source(BatchKind::Raw, 123);
        let batch = source.next_batch(&ui.m, &sw, frames, None).unwrap();
        for i in 0..frames {
            deliver(&reference_shared, batch.frame(i, ui.m.hw_stamp), false);
        }
        reference.step(0.03125);
        assert_eq!(reference.wfm_total, frames as u64);
        let newest = batch.newest(ui.m.hw_stamp);
        let (fb, decode) = (batch.frame_bytes(), batch.decode());
        let (decode, entered, release) = held_decode(decode);
        let counters: &'static drawpipe::Counters = Box::leak(Box::default());
        let pipe = Pipe::start(1, 0, counters);
        pipe.set_view(Some(&ui.m), ui.ph.w, ui.ph.h, sw.interp);
        let mut worker_map = Phosphor::new(ui.ph.w, ui.ph.h);
        let (delivered_tx, delivered) = mpsc::channel();
        let destination = shared.clone();
        pipe.set_identity(crate::ui::change::acquisition::identity(&ui));
        pipe.feed_with(
            Box::new(batch.into_bytes()),
            fb,
            frames - 1,
            crate::ui::change::acquisition::identity(&ui),
            decode,
            None,
            Box::new(move || {
                let seq = newest.seq;
                deliver(&destination, newest, false);
                delivered_tx.send(seq).unwrap();
            }),
        );
        entered.recv_timeout(HANG_LIMIT).expect("worker reached its first decode");
        assert_backlog(&pipe, &mut ui, &shared, &mut worker_map, &delivered);
        release.send(()).unwrap();
        let _returned = drawpipe::host::finish_batch(&pipe, 1).unwrap();
        assert!(delivered.try_recv().is_err(), "buffer return alone does not deliver the newest frame");
        let merged = pipe.merge(&mut worker_map, &mut true, &mut [0; 4], &mut (0.0, 0.0));
        assert_eq!(delivered.recv_timeout(HANG_LIMIT).unwrap(), frames as u64);
        ui.step(0.03125);
        assert_eq!((merged.frames, ui.records, ui.wfm_total), ((frames - 1) as u64, 1, 1));
        assert_eq!(ui.m.status, TrigStatus::Auto);
        assert!(lock(&shared).queue.is_empty());
        assert!(matches!(ui.src, Source::Hw { seen, .. } if seen == frames as u64));
        assert_picture(&worker_map, &ui, &reference, frames);
        assert_eq!(pipe.merge(&mut worker_map, &mut false, &mut [0; 4], &mut (0.0, 0.0)).frames, 0);
        ui.step(0.03125);
        assert_eq!((ui.records, ui.wfm_total, counters.read().2), (1, 1, 0));
        assert!(delivered.try_recv().is_err(), "delivered exactly once");
    }

    #[test]
    fn auto_delivery_survives_worker_backlog() {
        for frames in [3, 13, 27] {
            backlog_batch(frames);
        }
    }
}
