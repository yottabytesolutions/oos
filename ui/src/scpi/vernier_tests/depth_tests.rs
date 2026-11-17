// SPDX-License-Identifier: GPL-2.0-or-later

//! Pin the acquired-record readback contract, including the web JSON wrapper, without listeners or hardware.

use crate::board::{Frame, HwShared};
use crate::scpi::*;
use crate::wave::Source;

struct Fixture {
    ui: Ui,
    shared: HwShared,
    cv: Canvas,
    scpi: Scpi,
}

impl Fixture {
    fn new() -> Self {
        let shared: HwShared = Default::default();
        Self {
            ui: Ui::new(Source::Hw { shared: shared.clone(), seen: 0 }, crate::ui::host::Host::fixture()),
            shared,
            cv: super::panel_canvas(),
            scpi: Scpi::disconnected(),
        }
    }

    fn query(&mut self, command: &str) -> String {
        String::from_utf8(self.scpi.exec_line(&mut self.ui, &self.cv, command)).unwrap()
    }

    fn record(&mut self, roll: bool, points: usize, acq: (f64, usize)) {
        let mut g = self.shared.lock().unwrap();
        g.frame = Frame {
            seq: g.frame.seq + 1,
            dt: 320e-6,
            // Depth is per channel, never the sum; the longest populated channel supplies the fallback.
            ch: [vec![0.0; points / 2], vec![0.0; points], Vec::new(), Vec::new()],
            roll,
            envelope: true,
            partial: roll && points < 15_625,
            acq,
            ..Default::default()
        }
        .into();
    }

    fn assert_readback(&mut self, rate: f64, depth: u64) {
        assert_eq!(self.query(":ACQuire:MDEPth?").parse::<f64>().unwrap(), depth as f64);
        assert_eq!(self.query(":ACQ:MDEP?").parse::<f64>().unwrap(), depth as f64);
        assert_eq!(self.query(":ACQuire:SRATe?").parse::<f64>().unwrap(), rate);
        // This same pair supplies the top bar and System information's Record label.
        let (shown_rate, shown_depth) = self.ui.src.rate_depth(&self.ui.m);
        assert!((shown_rate - rate).abs() <= rate * 1e-12);
        assert_eq!(shown_depth, depth);
        assert_eq!(self.query(":SYST:ERR?"), "0,\"No error\"");
    }
}

#[test]
fn roll_depth_readback_tracks_populated_points_and_survives_stop() {
    let mut f = Fixture::new();
    f.query(":TIM:SCAL 0.5;:ACQ:MDEP 10M");
    for points in [25, 15_625] {
        f.record(true, points, (0.0, 0)); // roll publications carry no normal acquisition metadata
        f.assert_readback(3125.0, points as u64);
        assert_eq!(f.ui.m.depth_label(), "10M", "the menu still edits the requested depth");
    }
    f.query(":STOP");
    f.assert_readback(3125.0, 15_625);
    f.query(":ACQ:MDEP AUTO");
    f.assert_readback(3125.0, 15_625);
    assert_eq!(f.ui.m.depth_label(), "Auto");
}

#[test]
fn normal_depth_readback_uses_acquisition_metadata_for_compressed_records() {
    let mut f = Fixture::new();
    f.query(":TIM:SCAL 0.002;:ACQ:MDEP 10M");
    f.record(false, 1000, (2_500_000.0, 100_000));
    f.assert_readback(2_500_000.0, 100_000);
    assert_eq!(f.ui.m.depth_label(), "10M");
    f.query(":STOP");
    f.assert_readback(2_500_000.0, 100_000);
}

#[test]
fn depth_readback_changes_with_the_latest_record_when_leaving_roll() {
    let mut f = Fixture::new();
    f.query(":TIM:SCAL 0.5;:ACQ:MDEP AUTO");
    f.record(true, 25, (0.0, 0));
    f.assert_readback(3125.0, 25);
    f.query(":TIM:SCAL 0.002");
    f.assert_readback(3125.0, 25); // the requested time base does not relabel the retained frame
    f.record(false, 1000, (2_500_000.0, 100_000));
    f.assert_readback(2_500_000.0, 100_000);
}

#[test]
fn depth_readback_without_a_usable_frame_keeps_the_model_estimate() {
    let mut f = Fixture::new();
    for scale in ["0.002", "0.5"] {
        f.query(&format!(":TIM:SCAL {};:ACQ:MDEP AUTO", scale));
        let (rate, depth) = f.ui.m.srate_depth();
        f.assert_readback(rate, depth);
        f.record(true, 0, (0.0, 0));
        f.assert_readback(rate, depth);
    }
}

#[test]
fn web_json_depth_readback_wraps_the_same_roll_and_normal_answers() {
    let mut f = Fixture::new();
    for (roll, acq, depth) in [(true, (0.0, 0), 25), (false, (2_500_000.0, 100_000), 100_000)] {
        f.record(roll, 25, acq);
        let reply = crate::http::mcp(
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"scpi","arguments":{"command":":ACQ:MDEP?"}}}"#,
            &mut f.ui,
            &f.cv,
            &mut f.scpi,
        )
        .unwrap();
        let json = crate::json::parse(&reply).unwrap();
        let crate::json::J::Arr(content) = json.get("result").unwrap().get("content").unwrap() else {
            panic!("MCP content must be an array");
        };
        let answer = content[0].get("text").unwrap().str().unwrap().parse::<f64>().unwrap();
        assert_eq!(answer, depth as f64);
    }
}
