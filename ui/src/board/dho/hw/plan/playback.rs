// SPDX-License-Identifier: GPL-2.0-or-later

use super::{batch_plan, Model, Playback, Settings, Slow};
use crate::{
    model::{AcqMode, TrigType},
    sysconf::Switches,
};
use oos_hal::dho::{acq::Mode, hz, seq};

pub(super) fn select(m: &Model, sw: &Switches, slow: Slow, s: &mut Settings) {
    if let Some(inferred) = sw.source.stock_play().filter(|_| {
        slow == Slow::Off
            && m.acq == AcqMode::Normal
            && m.mdepth == 0
            && m.trig_type == TrigType::Edge
            && m.segments == 0
            && !m.mask_on
            && !m.zones.iter().any(|zone| zone.usable())
    }) {
        s.playback = if inferred { Playback::StockInferred } else { Playback::Stock };
    }
}

impl super::super::super::Dho {
    /// Capture and play geometry associated with the current hardware frame.
    pub(crate) fn capture_state(&self, source: &crate::wave::Source) -> String {
        let crate::wave::Source::Hw { shared, .. } = source else {
            return " acq.capture=unavailable acq.capture.stored_points=0 acq.play=unavailable acq.play.frame_len=0"
                .into();
        };
        let g = crate::perfguard::deep::ui_lock(shared);
        let id = g.acquisition.identity();
        let frame = &g.frame;
        let current = frame.seq != 0 && (frame.cfg, frame.stamp) == (id.gen(), id.epoch());
        let Some(want) = g.want.as_ref().filter(|_| current) else {
            return " acq.capture=pending acq.capture.stored_points=0 acq.play=pending acq.play.frame_len=0".into();
        };
        let (rate, points) = frame.acq;
        let (play, len) = play_state(want, g.slow, g.needs_raw, (rate, points));
        let capture = match want.mode {
            Mode::Reduced(capture) => capture.to_string(),
            _ => String::from("full"),
        };
        format!(" acq.capture={capture} acq.capture.stored_points={points} acq.play={play} acq.play.frame_len={len}")
    }
}

fn play_state(want: &Settings, slow: Slow, needs_raw: bool, (rate, points): (f64, usize)) -> (&'static str, u32) {
    let tdiv = seq::tdiv_fs(want.tdiv);
    if slow != Slow::Off || points == 0 || rate <= 0.0 || tdiv <= 0 || want.la.is_some() || want.segments.is_some() {
        return ("raw", 0);
    }
    let mut want = want.clone();
    super::raw_for_ui(&mut want, needs_raw);
    let mut params = hz::prep(want.mask, tdiv, want.depth, 0);
    params.memsel = points as u32;
    params.rate = rate as u64;
    let batch = batch_plan(&want, &params, 1);
    let mode = if seq::stock_play(&batch, &params).is_some() { "stock" } else { "raw" };
    (mode, seq::frame_len(&batch, &params))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::tscales;

    #[test]
    fn stock_play_tracks_the_ui_plan_and_the_current_frame() {
        let switches = Switches::parse(&|_| None, "acq.play = stock\nacq.play.inferred = on", "", Default::default());
        for (channels, tx_len) in [(1, 13_000), (2, 7_000), (4, 4_000)] {
            let mut m = Model {
                tscale: tscales().iter().position(|&t| (t - 1e-6).abs() < 1e-12).unwrap(),
                ..Default::default()
            };
            for (index, channel) in m.ch.iter_mut().enumerate() {
                channel.on = index < channels;
            }
            let raw = super::super::settings(&m, false, &Switches::default());
            let want = super::super::settings(&m, false, &switches);
            assert_eq!(Settings { playback: Playback::Raw, ..want.clone() }, raw);
            let (_, p) = seq::State::handover(want.mask, None).configure(&want, None, None).unwrap();
            let batch = batch_plan(&want, &p, 1);
            assert_eq!(seq::frame_len(&batch, &p), tx_len);
            assert_eq!(seq::frame_bytes(&batch, &p), tx_len as usize * channels * 2);
            assert_eq!(play_state(&want, Slow::Off, true, (p.rate as f64, p.memsel as usize)), ("raw", p.memsel));

            let shared = crate::board::HwShared::default();
            let source = crate::wave::Source::Hw { shared: shared.clone(), seen: 0 };
            let status = || crate::board::board().capture_state(&source);
            assert!(status().contains("acq.play=pending acq.play.frame_len=0"));
            let mut g = crate::perfguard::deep::acq_lock(&shared);
            let id = g.acquisition.identity();
            g.want = Some(want);
            g.frame = std::sync::Arc::new(crate::board::Frame {
                seq: 1,
                cfg: id.gen(),
                stamp: id.epoch(),
                acq: (p.rate as f64, p.memsel as usize),
                ..Default::default()
            });
            drop(g);
            assert!(status().contains(&format!("acq.play=stock acq.play.frame_len={tx_len}")));
        }
    }

    #[test]
    fn special_capture_modes_keep_raw_play() {
        let switches = Switches::parse(&|_| None, "acq.play = stock\nacq.play.inferred = on", "", Default::default());
        for model in [
            Model { mdepth: 1, ..Default::default() },
            Model { acq: AcqMode::Peak, ..Default::default() },
            Model { mask_on: true, ..Default::default() },
            Model { segments: 2, ..Default::default() },
        ] {
            assert_eq!(super::super::settings(&model, false, &switches).playback, Playback::Raw);
        }
    }
}
