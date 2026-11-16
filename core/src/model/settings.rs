// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

impl Model {
    /// The deposits draw dots (YT): `trace_style` for samples `x_per_sample` pixels apart.
    pub fn dots_at(&self, x_per_sample: f64) -> bool {
        trace_style(self.dots, false, x_per_sample) == TraceStyle::Dots
    }

    /// The settings of the selected generator.
    pub fn gen_read(&self) -> GenCh {
        GenCh {
            on: self.afg_on,
            arb: self.afg_arb,
            arb_src: self.afg_arb_src,
            freq: self.afg_freq,
            ampl: self.afg_ampl,
            offset: self.afg_offset,
            wave: self.afg_wave,
            duty: self.afg_duty,
            symmetry: self.afg_symmetry,
            modulation: self.afg_mod,
            mod_rate: self.afg_mod_rate,
            mod_depth: self.afg_mod_depth,
            fm_dev: self.afg_fm_dev,
            pm_dev: self.afg_pm_dev,
            mod_shape: self.afg_mod_shape,
        }
    }

    fn gen_write(&mut self, g: GenCh) {
        (self.afg_on, self.afg_arb, self.afg_arb_src) = (g.on, g.arb, g.arb_src);
        (self.afg_freq, self.afg_ampl, self.afg_offset) = (g.freq, g.ampl, g.offset);
        (self.afg_wave, self.afg_duty, self.afg_symmetry) = (g.wave, g.duty, g.symmetry);
        (self.afg_mod, self.afg_mod_rate, self.afg_mod_depth) = (g.modulation, g.mod_rate, g.mod_depth);
        (self.afg_fm_dev, self.afg_pm_dev, self.afg_mod_shape) = (g.fm_dev, g.pm_dev, g.mod_shape);
    }

    /// The settings of generator `i`.
    pub fn gen_ch(&self, i: usize) -> GenCh {
        if i == self.gen_sel {
            self.gen_read()
        } else {
            self.gen_bank[i]
        }
    }

    /// Makes generator `i` the one the page and the `afg_*` fields work on.
    pub fn gen_select(&mut self, i: usize) {
        if i != self.gen_sel && i < NGEN {
            self.gen_bank[self.gen_sel] = self.gen_read();
            self.gen_write(self.gen_bank[i]);
            self.gen_sel = i;
        }
    }

    pub fn time_per_div(&self) -> f32 {
        TIME_SCALES[self.tscale] * self.tfine
    }

    /// The time base the acquisition runs at: the step, which covers at least the screen. Display scales below
    /// 1 ns/div stay on the plot; the ADC still takes a 1 ns/div window.
    pub fn hw_time_per_div(&self) -> f32 {
        TIME_SCALES[self.tscale].max(HW_MIN_TDIV)
    }

    /// Live analog / LA cannot display below 1 ns/div; the ADC window is that floor.
    pub fn floor_live_tdiv(&mut self) {
        if TIME_SCALES[self.tscale] < HW_MIN_TDIV {
            self.tscale = TIME_SCALES.iter().position(|&t| t >= HW_MIN_TDIV).unwrap_or(0);
            self.tfine = 1.0;
        }
    }

    /// Unit of channel `c`.
    pub fn unit(&self, c: usize) -> &'static str {
        self.ch[c.min(NCH - 1)].unit.name()
    }

    /// Settings of math channel `k` (the selected one from the edit fields).
    pub fn math_ch(&self, k: usize) -> crate::math::MathCh {
        if k == self.math_sel {
            crate::math::MathCh {
                lin: self.math_lin,
                op: self.math,
                a: self.math_a,
                b: self.math_b,
                scale: self.math_scale,
                fc: self.math_fc,
                bw: self.math_bw,
                offset: self.math_offset,
                win: self.fft_win,
            }
        } else {
            self.maths[k]
        }
    }

    /// The settings of the decoder the menu edits.
    pub fn dec_cfg(&self) -> DecCfg {
        DecCfg {
            on: self.dec_on.clone(),
            src: self.dec_src.clone(),
            baud: self.dec_baud.clone(),
            canrate: self.dec_canrate.clone(),
            canh: self.dec_canh.clone(),
            fdrate: self.dec_fdrate.clone(),
            arate: self.dec_arate.clone(),
            tick: self.dec_tick.clone(),
            nibbles: self.dec_nibbles.clone(),
            uspeed: self.dec_uspeed.clone(),
            cphy_rate: self.dec_cphy_rate,
            src4: self.dec_src4.clone(),
            la_in: self.dec_la_in.clone(),
            la: self.dec_la.clone(),
            diff: self.dec_diff.clone(),
            thr: self.dec_thr.clone(),
            ascii: self.dec_ascii.clone(),
            proto: self.dec_proto.clone(),
            src2: self.dec_src2.clone(),
            cs: self.dec_cs.clone(),
            mode: self.dec_mode.clone(),
            bits: self.dec_bits.clone(),
            parity: self.dec_parity.clone(),
            opt: self.dec_opt,
        }
    }

    fn dec_load(&mut self, c: &DecCfg) {
        self.dec_on = c.on.clone();
        self.dec_src = c.src.clone();
        self.dec_baud = c.baud.clone();
        self.dec_canrate = c.canrate.clone();
        self.dec_canh = c.canh.clone();
        self.dec_fdrate = c.fdrate.clone();
        self.dec_arate = c.arate.clone();
        self.dec_tick = c.tick.clone();
        self.dec_nibbles = c.nibbles.clone();
        self.dec_uspeed = c.uspeed.clone();
        self.dec_cphy_rate = c.cphy_rate;
        self.dec_src4 = c.src4.clone();
        self.dec_la_in = c.la_in.clone();
        self.dec_la = c.la.clone();
        self.dec_diff = c.diff.clone();
        self.dec_thr = c.thr.clone();
        self.dec_ascii = c.ascii.clone();
        self.dec_proto = c.proto.clone();
        self.dec_src2 = c.src2.clone();
        self.dec_cs = c.cs.clone();
        self.dec_mode = c.mode.clone();
        self.dec_bits = c.bits.clone();
        self.dec_parity = c.parity.clone();
        self.dec_opt = c.opt;
    }

    /// Settings of decoder `k`.
    pub fn dec_slot(&self, k: usize) -> DecCfg {
        if k == self.dec_sel {
            self.dec_cfg()
        } else {
            self.decs[k].clone().unwrap_or_else(|| Model::default().dec_cfg())
        }
    }

    pub fn dec_is_on(&self, k: usize) -> bool {
        if k == self.dec_sel {
            self.dec_on
        } else {
            self.decs[k].as_ref().is_some_and(|c| c.on)
        }
    }

    /// The model as decoder `k` sees it, for the code that reads the `dec_*` fields.
    pub fn dec_view(&self, k: usize) -> Model {
        let mut m = self.clone();
        if k != self.dec_sel {
            m.dec_load(&self.dec_slot(k));
        }
        m
    }

    /// Makes decoder `k` the one the menu edits.
    pub fn dec_select(&mut self, k: usize) {
        let k = k.min(DECODERS - 1);
        if k != self.dec_sel {
            let c = self.dec_slot(k);
            self.decs[self.dec_sel] = Some(self.dec_cfg());
            self.dec_load(&c);
            self.dec_sel = k;
        }
    }

    /// Writes `c` into slot `k` (setups).
    pub fn dec_store(&mut self, k: usize, c: DecCfg) {
        if k == self.dec_sel {
            self.dec_load(&c);
        } else {
            self.decs[k] = Some(c);
        }
    }

    /// Makes math channel `k` the one the menu edits.
    pub fn math_select(&mut self, k: usize) {
        self.maths[self.math_sel] = self.math_ch(self.math_sel);
        self.math_sel = k.min(crate::math::NMATH - 1);
        let c = self.maths[self.math_sel];
        self.math_lin = c.lin;
        (
            self.math,
            self.math_a,
            self.math_b,
            self.math_scale,
            self.math_fc,
            self.math_bw,
            self.math_offset,
            self.fft_win,
        ) = (c.op, c.a, c.b, c.scale, c.fc, c.bw, c.offset, c.win);
    }

    /// Copies the selected SIM preset into the knobs. Play then uses the knobs, not the table row itself.
    pub fn apply_sim_preset(&mut self) {
        let l = oos_dsp::serial::LANES[self.sim_preset.min(oos_dsp::serial::LANES.len() - 1)];
        self.sim_rate = l.bits_per_s;
        self.sim_prbs = l.prbs;
        self.sim_vhigh = l.v_high;
        self.sim_vlow = l.v_low;
        self.sim_rise = l.rise;
        self.sim_noise = l.noise_rms;
        self.sim_bw = l.bw;
        self.sim_deemph = l.deemph_db;
        self.sim_ssc = l.ssc;
        self.sim_ssc_center = l.ssc_center;
        self.sim_ssc_deviation = l.ssc_deviation;
        self.sim_ssc_hz = l.ssc_hz;
        self.sim_rj = l.rj_rms;
        self.sim_pj = l.pj_peak;
        self.sim_pjhz = l.pj_hz;
        self.sim_dcd_pp = l.dcd_pp;
        self.sim_corr_rj = l.corr_rj_rms;
        self.sim_corr_hz = l.corr_rj_hz;
    }

    pub fn trigger_wave(&self) -> WaveId {
        self.trig_ref.map_or(WaveId::Ch(self.trig_src), WaveId::Ref)
    }

    pub fn trigger_wave2(&self) -> WaveId {
        self.trig_ref2.map_or(WaveId::Ch(self.trig_src2), WaveId::Ref)
    }

    pub fn set_trigger_wave(&mut self, source: WaveId, secondary: bool) {
        let (channel, reference) = if secondary {
            (&mut self.trig_src2, &mut self.trig_ref2)
        } else {
            (&mut self.trig_src, &mut self.trig_ref)
        };
        match source {
            WaveId::Ch(c) => {
                *channel = c.min(NCH - 1);
                *reference = None;
            }
            WaveId::Ref(i) => {
                *reference = Some(i.min(NREF - 1));
            }
        }
        if !secondary {
            self.trig_la = None;
        }
    }

    /// Keeps the lower level of a two-level type at or below the upper one. The lower level gives way: the upper is
    /// the level every type shares, the lower one is also Delay's level B and Setup/Hold's clock level.
    pub fn order_trigger_levels(&mut self) {
        if self.trig_type.two_level() && self.trig_level2 > self.trig_level {
            self.trig_level2 = self.trig_level;
        }
    }
}
