// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Persistent hardware shadows and acquisition configuration.

use super::{frames, readout, settings::record_params, tdiv_fs, time_fs, Couple, ReadoutError, Settings, Step};
use crate::dho::{acq, adc, afe, auxout, hz, la, mask, scu, tpu, zone};

mod qualify;
mod trigger;

/// Everything the FPGA/ADC/AFE do not read back, kept across acquisitions.
pub struct State {
    pub sh: scu::Shadows,
    pub ts: tpu::Shadows,
    pub afe: [afe::Afe; 4],
    pub adc: adc::Adc,
    pub mode: usize,
    /// FPGA digital gain of the trigger source channel (cal_vertical record), 1.0 when OOS did not set the AFE
    pub gain: [f64; 4],
    /// the LA was on in the last set-up: 0x131c goes back when it is off again
    pub la_was_on: bool,
    /// a holdoff other than `HOLDOFF_MIN` is in the TPU
    pub holdoff_set: bool,
    /// the AUX OUT set-up last written; None = not written since the hand-over, so the first set-up writes it
    pub aux: Option<auxout::AuxOut>,
    /// the mask unit: table last loaded, (slot, samples per column, clear, stop on fail) of the running test (None =
    /// off), and the control shadow 0x1190
    pub mask_cols: Option<Vec<mask::Column>>,
    pub mask_run: Option<(u32, u32, u64, bool)>,
    pub s1190: u32,
    /// the zone unit: tables last loaded, and (slot, samples per column, control word, clear) of the running zone
    /// qualification (None = off)
    pub zone_tabs: [Option<Vec<zone::Column>>; 2],
    pub zone_run: Option<(u32, u32, u32, u64)>,
}

impl State {
    /// State after a warm hand-over: the stock values, ADC in the mode of `app_mask`.
    pub fn handover(app_mask: u32, cal_adc: Option<&adc::CalAdc>) -> State {
        let mode = adc::mode_of_mask(app_mask).unwrap_or(0);
        let mut a = adc::Adc::default();
        if let Some(c) = cal_adc {
            a.parameter(c, 0);
            if mode != 0 {
                a.channel_mode(c, mode);
            }
        }
        State {
            sh: scu::Shadows::HANDOVER,
            ts: tpu::Shadows::APP,
            afe: [0, 1, 2, 3].map(afe::Afe::new),
            adc: a,
            mode,
            gain: [1.0; 4],
            la_was_on: false,
            holdoff_set: false,
            aux: None,
            mask_cols: None,
            mask_run: None,
            s1190: 0,
            zone_tabs: [None, None],
            zone_run: None,
        }
    }

    /// Configuration steps for `s` (ADC mode if the channel set changed, AFEs, trigger, acquisition registers).
    pub fn configure(
        &mut self,
        s: &Settings,
        cal: Option<&afe::Cal>,
        cal_adc: Option<&adc::CalAdc>,
    ) -> Result<(Vec<Step>, hz::Params), String> {
        // trigger source: a channel that is acquired, or LA line 9..24 (only with the LA on)
        let on_la = (9..=24).contains(&s.trig_src) && s.la.is_some();
        if !(1..=15).contains(&s.mask)
            || (!on_la && (!(1..=4).contains(&s.trig_src) || s.mask & (1 << (s.trig_src - 1)) == 0))
        {
            return Err("channel mask 1..0xf with the trigger source enabled".into());
        }
        let mut st = Vec::new();
        let p = record_params(s, &mut st)?;
        readout::validate_readout(s, &p).map_err(|error| error.to_string())?;
        self.adc_steps(s.mask, cal_adc, &mut st)?;
        if s.afe {
            self.afe_steps(s, &p, cal, &mut st)?;
        }
        if s.blocks & 7 != 7 {
            st.push(Step::Note(format!(
                "configuration blocks {:#x} (bit0 trigger, bit1 acquisition, bit2 auto trigger)",
                s.blocks
            )));
        }
        if s.blocks & 1 != 0 {
            st.extend(trigger::steps(self, s).into_iter().map(Step::Wr));
        }
        if s.blocks & 2 != 0 {
            self.acquisition_steps(s, &p, &mut st);
        }
        if let Some(l) = &s.la {
            // thresholds with every set-up (la.md 8.1: on change and once after the start)
            st.push(Step::Wr(la::thresholds(&l.cal, l.thr[0], l.thr[1])));
        }
        if s.blocks & 4 != 0 {
            st.push(Step::Note(format!("SCU {} trigger", if s.auto { "auto" } else { "normal" })));
            // a time base of 0 gives the shortest auto times (t1 10 ms, t2 100 ms)
            let tb = if s.auto_fast { 0 } else { tdiv_fs(s.tdiv) };
            st.extend(scu::auto_trigger(&mut self.sh, s.auto, tb, time_fs(s.holdoff)).into_iter().map(Step::Wr));
        }
        Ok((st, p))
    }

    fn adc_steps(&mut self, mask: u32, cal_adc: Option<&adc::CalAdc>, st: &mut Vec<Step>) -> Result<(), String> {
        let mode = adc::mode_of_mask(mask).expect("configure validated the channel mask");
        if mode != self.mode {
            let c = cal_adc.ok_or("a channel set different from the current ADC mode needs cal_adc.hex")?;
            st.push(Step::Note(format!("ADC mode {} -> {}", self.mode, mode)));
            for w in self.adc.channel_mode(c, mode) {
                if w.pre_us > 0 {
                    st.push(Step::Sleep { us: w.pre_us });
                }
                st.push(Step::Wr((w.reg, w.val)));
                if w.post_us > 0 {
                    st.push(Step::Sleep { us: w.post_us });
                }
            }
            for (_, v) in adc::wpu_gain(mask, [1; 4], 0x5555) {
                st.push(Step::Wr((0x105c, v)));
            }
            self.mode = mode;
        }
        Ok(())
    }

    fn afe_steps(
        &mut self,
        s: &Settings,
        p: &hz::Params,
        cal: Option<&afe::Cal>,
        st: &mut Vec<Step>,
    ) -> Result<(), String> {
        let cal = cal.ok_or("AFE set-up needs the calibration files (cal_vertical/zero/bandwidth/lsb.hex)")?;
        for (c, k) in s.ch.iter().enumerate() {
            if s.mask & (1 << c) == 0 {
                continue;
            }
            // the correction is on the input side, so an inverted channel has it the other way round
            let bias = if k.invert { -k.bias } else { k.bias };
            let dc = match k.couple {
                Couple::Dc => Some(true),
                Couple::Ac => Some(false),
                Couple::Gnd => None,
            };
            let (ops, rec, code) = afe::configure_ch(
                &mut self.afe[c],
                cal,
                (k.scale / k.probe * 1e9) as i64,
                ((k.offset - bias) / k.probe * 1e9) as i64,
                k.bw,
                p.rate,
                s.mask,
                k.invert,
                dc,
            );
            st.push(Step::Note(format!(
                "AFE CH{}: {} V/div, digital gain {:.5}, offset DAC {}",
                c + 1,
                k.scale,
                rec.gain,
                code
            )));
            self.gain[c] = rec.gain;
            st.extend(ops.into_iter().map(Step::Afe));
        }
        // read-path gain/offset from the digital gains of the AFE records (else they keep the stock values)
        st.push(Step::Note("read-path gain/offset".into()));
        let gnd =
            s.ch.map(|k| (k.couple == Couple::Gnd).then(|| (k.offset / k.scale * 7500.0 + 32768.0) as i32 as u32));
        st.extend(crate::dho::wpu::analog_gnd(s.mask, self.gain, gnd).into_iter().map(Step::Wr));
        Ok(())
    }

    fn acquisition_steps(&mut self, s: &Settings, p: &hz::Params, st: &mut Vec<Step>) {
        st.push(Step::Poll { reg: 0x4034, bit: scu::status::STOPPED, ms: 30, what: "FSM stopped (stop poll)" });
        if matches!(s.mode, acq::Mode::Average { .. }) {
            // stock RunConfig: stop, average clear, then the configuration
            st.push(Step::Note("average clear".into()));
            st.extend(acq::avg_clear().into_iter().map(Step::Wr));
        }
        st.push(Step::Note("acquisition configuration".into()));
        let deskew = s.la.as_ref().map(|_| la::deskew(la::timing(p, s.mask).m));
        let skew = s.ch.map(|k| (k.skew * 1e15).round() as i64);
        let invert = (0..4).filter(|&c| s.ch[c].invert).fold(0, |a, c| a | 1 << c);
        st.extend(
            acq::configure_la(&mut self.sh, p, s.mask, invert, 0, s.mode, deskew, skew).into_iter().map(Step::Wr),
        );
        if std::mem::replace(&mut self.la_was_on, s.la.is_some()) && s.la.is_none() {
            st.push(Step::Wr(la::wave_early(None)));
        }
        if s.la.is_some() {
            let t = la::timing(p, s.mask);
            st.push(Step::Note(format!("LA on: m {}, {} Sa/s, {} LA samples, trigger at {}", t.m, t.rate, t.n, t.pre)));
            st.extend(
                la::config(&mut self.sh, p, s.mask, s.la.as_ref().is_some_and(|l| l.roll)).into_iter().map(Step::Wr),
            );
        }
        if self.aux != Some(s.aux) {
            st.push(Step::Note(format!("AUX OUT {:?}", s.aux)));
            st.extend(auxout::writes(&s.aux).into_iter().map(Step::Wr));
            self.aux = Some(s.aux);
        }
        st.extend(qualify::zone_steps(self, s, p));
        st.extend(qualify::mask_steps(self, s, p));
    }

    /// The control shadow while the zone trigger runs (for `zone::read`), None when it is off.
    pub fn zone_ctrl(&self) -> Option<u32> {
        self.zone_run.map(|_| self.s1190)
    }

    /// The control shadow while the mask test runs (for `mask::read`), None when it is off.
    pub fn mask_ctrl(&self) -> Option<u32> {
        self.mask_run.map(|_| self.s1190)
    }

    /// One acquisition with the configured registers: arm, wait, read-out window, read.
    pub fn acquire(&mut self, s: &Settings, p: &hz::Params) -> Result<Vec<Step>, ReadoutError> {
        readout::validate_readout(s, p)?;
        let nf = frames(s, p);
        let mut st = self.arm_steps(s, p);
        st.extend(self.read_steps(s, p, nf)?);
        if s.la.is_some() {
            st.extend(self.la_read_steps(s, p));
        }
        Ok(st)
    }

    /// The raw LA read after the analog read of the same acquisition (la.md 4.2, UNTESTED): pass-through LA chain,
    /// window of `la::window` words, play-last, one read, analog stream back on. Only the last frame of a batch has
    /// its LA (la.md 4.1), so the LA runs with one frame per batch.
    pub fn la_read_steps(&mut self, s: &Settings, p: &hz::Params) -> Vec<Step> {
        readout::la_steps(s, p, &mut self.sh)
    }

    /// Read-out of the `count` frames a batch recorded, all of them in every mode: pass-through, window, stop sequence,
    /// type-4 play, one read. In average mode too, as the stock system does: the FPGA's averaging sits on the play
    /// side (assumed: the next frame waits for the play-out of the one before), so a frame that is not played does not
    /// reach the average (B1, unit 2026-11-13: reading the newest frame alone left the noise near 0.5 mV at any
    /// count). The caller shows the newest.
    pub fn read_steps(&mut self, s: &Settings, p: &hz::Params, count: u32) -> Result<Vec<Step>, ReadoutError> {
        self.read_steps_at(s, p, 0, count)
    }

    /// Frame `frame` of the last batch read again, with the window and compression of `s` (STOP: another part of the
    /// record, or the whole record): the same read-out with a type-4 play of that frame only; play-last without
    /// `play4`. Nothing is armed, the ring keeps the batch (acquisition-start.md, "Reading a stored frame again").
    pub fn reread_steps(&mut self, s: &Settings, p: &hz::Params, frame: u32) -> Result<Vec<Step>, ReadoutError> {
        self.read_steps_at(s, p, frame, 1)
    }

    /// `read_steps` of `count` frames from frame `first` of the ring (the segments of a hardware segmented record,
    /// a stored frame read again).
    pub fn read_steps_at(
        &mut self,
        s: &Settings,
        p: &hz::Params,
        first: u32,
        count: u32,
    ) -> Result<Vec<Step>, ReadoutError> {
        readout::steps_at(s, p, &mut self.sh, (first, count))
    }

    /// The stamp play of segment `k`: a type-4 play of frame `k` alone with the smallest read-out window
    /// (`STAMP_WORDS` words of all channels, from the start of the record window, header on), its read, then the latch
    /// pulse. Read the stamp from 0x40b0/0x40b4 after it (`seg::read_latched`).
    pub fn segment_stamp_steps(&mut self, s: &Settings, p: &hz::Params, k: u32) -> Vec<Step> {
        readout::stamp_steps(s, p, &mut self.sh, k)
    }
}

/// Configure + acquire in one list (oos-fpgad plan/run).
pub fn plan(
    st: &mut State,
    s: &Settings,
    cal: Option<&afe::Cal>,
    cal_adc: Option<&adc::CalAdc>,
) -> Result<(Vec<Step>, hz::Params), String> {
    let (mut v, p) = st.configure(s, cal, cal_adc)?;
    v.extend(st.acquire(s, &p).map_err(|error| error.to_string())?);
    Ok((v, p))
}

#[cfg(test)]
mod tests;
