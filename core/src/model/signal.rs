// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

impl Model {
    fn dsp_slope(s: Slope) -> oos_dsp::trigger::Slope {
        match s {
            Slope::Rise => oos_dsp::trigger::Slope::Rising,
            Slope::Fall => oos_dsp::trigger::Slope::Falling,
            Slope::Either => oos_dsp::trigger::Slope::Either,
        }
    }

    /// The time condition of the "When" setting (None = Off).
    pub fn when(&self) -> Option<oos_dsp::trigger::When> {
        use oos_dsp::trigger::When;
        let (a, b) = (self.trig_t1 as f64, self.trig_t2 as f64);
        match self.trig_when {
            WhenKind::Off => None,
            WhenKind::Gt => Some(When::Gt(a)),
            WhenKind::Lt => Some(When::Lt(a)),
            WhenKind::In => Some(When::In(a.min(b), a.max(b))),
            WhenKind::Out => Some(When::Out(a.min(b), a.max(b))),
        }
    }

    /// The configured trigger for the software trigger engine.
    pub fn trigger(&self) -> oos_dsp::trigger::Trigger {
        use oos_dsp::trigger::{Trigger, WinWhen};
        let (src, lv, lv2) = (self.trig_src, self.trig_level, self.trig_level2);
        let (hi, lo) = (lv.max(lv2), lv.min(lv2));
        let when = self.when().unwrap_or(oos_dsp::trigger::When::Gt(0.0));
        let sl = Self::dsp_slope(self.slope);
        match self.trig_type {
            TrigType::Edge => Trigger::Edge { src, slope: sl, level: lv },
            // the protocol trigger is evaluated by trig::find_all; this edge only stands for it elsewhere
            TrigType::Bus => {
                Trigger::Edge { src: self.dec_src, slope: oos_dsp::trigger::Slope::Falling, level: self.dec_thr }
            }
            TrigType::Pulse => Trigger::Pulse { src, pol: self.trig_pol, level: lv, when },
            TrigType::Slope => Trigger::Slope { src, slope: sl, lo, hi, when },
            TrigType::Video => Trigger::Video { src, std: self.video_std, level: lv, sync: self.video_sync() },
            TrigType::Pattern => Trigger::Pattern { bits: self.pat, levels: self.pat_levels },
            TrigType::Duration => Trigger::Duration {
                bits: self.pat.map(|b| if matches!(b, Bit::R | Bit::F) { Bit::X } else { b }),
                levels: self.pat_levels,
                when,
            },
            TrigType::Timeout => Trigger::Timeout { src, slope: sl, level: lv, time: self.trig_t1 as f64 },
            TrigType::Runt => Trigger::Runt { src, pol: self.trig_pol, lo, hi, when: self.when() },
            TrigType::Window => Trigger::Window {
                src,
                lo,
                hi,
                when: match self.win_mode {
                    WinMode::Enter => WinWhen::Enter,
                    WinMode::Exit => WinWhen::Exit,
                    WinMode::Inside => WinWhen::Inside(when),
                },
            },
            TrigType::Delay => Trigger::Delay {
                a: src,
                slope_a: sl,
                level_a: lv,
                b: self.trig_src2,
                slope_b: Self::dsp_slope(self.trig_slope2),
                level_b: lv2,
                when,
            },
            TrigType::SetupHold => Trigger::SetupHold {
                data: src,
                clk: self.trig_src2,
                clk_slope: Self::dsp_slope(self.trig_slope2),
                data_level: lv,
                clk_level: lv2,
                setup: self.setup as f64,
                hold: self.hold as f64,
            },
            TrigType::NthEdge => {
                Trigger::NthEdge { src, slope: sl, level: lv, idle: self.nth_idle as f64, n: self.nth_n }
            }
        }
    }

    fn video_sync(&self) -> oos_dsp::trigger::VideoSync {
        use oos_dsp::trigger::VideoSync;
        match self.video_sync {
            VSync::AllLines => VideoSync::AllLines,
            VSync::Line => VideoSync::Line(self.video_line),
            VSync::Odd => VideoSync::OddField,
            VSync::Even => VideoSync::EvenField,
            VSync::AllFields => VideoSync::AllFields,
        }
    }

    /// Channels the trigger reads.
    pub fn trig_channels_in(&self, config: &Config, ctx: &Context<'_>) -> [bool; NCH] {
        let mut u = [false; NCH];
        if self.trig_line_in(config).is_some() || self.bus_on_la_in(config) {
            return u;
        }
        match self.trig_type {
            TrigType::Pattern | TrigType::Duration => {
                for c in 0..NCH {
                    u[c] = self.pat[c] != Bit::X;
                }
            }
            TrigType::Delay | TrigType::SetupHold => {
                u[self.trig_src] = true;
                u[self.trig_src2] = true;
            }
            TrigType::Bus => {
                let r = self.dec_proto.roles_in(ctx);
                u[self.dec_src] = true;
                if r[1].is_some() {
                    u[self.dec_src2] = true;
                }
                if let (Some(_), Some(c)) = (r[2], self.dec_cs) {
                    u[c] = true;
                }
                if let (Some(_), Some(c)) = (r[3], self.dec_src4) {
                    u[c] = true;
                }
                if let (true, Some(c)) = (self.dec_proto.diff_ok_in(ctx), self.dec_diff) {
                    u[c] = true;
                }
            }
            _ => u[self.trig_src] = true,
        }
        u
    }

    /// Time scale of the trigger condition (s): the search resolution must resolve it.
    pub fn trig_feature_in(&self, ctx: &Context<'_>) -> f64 {
        let t = |x: f32| x as f64;
        match self.trig_type {
            TrigType::Edge | TrigType::Pattern => f64::INFINITY,
            TrigType::Bus => self.bus_bit_in(ctx) / 20.0,
            TrigType::Video => 0.25e-6,
            TrigType::SetupHold => t(self.setup.min(self.hold)) / 4.0,
            TrigType::Timeout => t(self.trig_t1) / 20.0,
            TrigType::NthEdge => t(self.nth_idle) / 20.0,
            TrigType::Runt if self.trig_when == WhenKind::Off => f64::INFINITY,
            TrigType::Window if self.win_mode != WinMode::Inside => f64::INFINITY,
            _ => {
                t(self.trig_t1.min(if matches!(self.trig_when, WhenKind::In | WhenKind::Out) {
                    self.trig_t2
                } else {
                    f32::MAX
                })) / 20.0
            }
        }
    }

    /// Record length the trigger needs to decide (s).
    pub fn trig_span_in(&self, ctx: &Context<'_>) -> f64 {
        let t = |x: f32| x as f64;
        match self.trig_type {
            TrigType::Video => match self.video_sync {
                VSync::AllLines => 4.0 * self.video_std.line(),
                _ => 1.1 * self.video_std.lines() as f64 * self.video_std.line(),
            },
            TrigType::Timeout | TrigType::Duration => 3.0 * t(self.trig_t1.max(self.trig_t2)) + 6e-3,
            TrigType::NthEdge => 4.0 * t(self.nth_idle) + 2e-3,
            // room for a whole frame of the protocol before the trigger point (ARINC 429 auto rate: a low-speed word;
            // USB auto: low-speed packets)
            TrigType::Bus if self.dec_proto == Proto::Arinc429 && self.dec_arate == 0 => {
                200.0 / crate::decode::ARINC_RATES[1]
            }
            TrigType::Bus if self.dec_proto == Proto::Usb && self.dec_uspeed == 0 => {
                200.0 / oos_dsp::decode_ext3::USB_LOW
            }
            TrigType::Bus => match self.dec_proto {
                Proto::Module(i) => ctx.frame_span(i).unwrap_or(200.0 * self.bus_bit_in(ctx)),
                _ => 200.0 * self.bus_bit_in(ctx),
            },
            TrigType::Edge | TrigType::Pattern => 0.0,
            _ => 3.0 * t(self.trig_t1.max(self.trig_t2)) + 3e-3,
        }
    }

    /// The option settings of the decoder's protocol module, in range (zeros for a built-in protocol).
    pub fn dec_opts_in(&self, ctx: &Context<'_>) -> [usize; crate::protocols::OPTS] {
        match self.dec_proto {
            Proto::Module(i) => ctx.opt_fit(i, self.dec_opt),
            _ => [0; crate::protocols::OPTS],
        }
    }

    /// Bit time of the decoder's protocol (s), for the protocol trigger's sampling and search window.
    pub fn bus_bit_in(&self, ctx: &Context<'_>) -> f64 {
        match self.dec_proto {
            Proto::Uart | Proto::Lin | Proto::Uart9 | Proto::ModbusRtu | Proto::ModbusAscii => {
                1.0 / crate::decode::BAUDS[self.dec_baud]
            }
            Proto::Can | Proto::CanFd => 1.0 / crate::decode::CAN_RATES[self.dec_canrate],
            Proto::I2c | Proto::Ds2482 => 2.5e-6,
            Proto::I3c => 0.1e-6,
            Proto::Spi | Proto::Qspi | Proto::Swd | Proto::Jtag | Proto::Parallel => 0.5e-6,
            Proto::I2s => 0.6e-6,
            Proto::Mil1553 => 1e-6,
            Proto::FlexRay => 0.1e-6,
            Proto::OneWire => 15e-6,
            Proto::Module(i) => ctx.bit_time(i),
            // auto: high speed (the finer sampling)
            Proto::Arinc429 => {
                let r = crate::decode::ARINC_RATES[self.dec_arate];
                1.0 / if r > 0.0 { r } else { crate::decode::ARINC_RATES[2] }
            }
            // about 4 ticks per data bit; auto: the shortest tick
            Proto::Sent => {
                let t = crate::decode::SENT_TICKS[self.dec_tick];
                4.0 * if t > 0.0 { t } else { crate::decode::SENT_TICKS[0] }
            }
            // auto: full speed
            Proto::Usb => {
                use oos_dsp::decode_ext3::{UsbSpeed, USB_FULL, USB_LOW};
                1.0 / if crate::decode::USB_SPEEDS[self.dec_uspeed] == UsbSpeed::Low { USB_LOW } else { USB_FULL }
            }
        }
    }
}
