// SPDX-License-Identifier: GPL-2.0-or-later

//! Raw readings from the FPGA's ADC measure unit for slot diagnostics and core-offset matching.
//!
//! The unit (docs/hardware/fpga.md "Measure unit"; self-cal.md 2.4): 0x1040 bit 31 starts a run of 2^e samples
//! per ADC core (e = 0x1040[4:0]) at 156.25 MHz, bit 30 reports it done. 0x1040[11:8] selects the core (slot) the read
//! registers show: 0x1048 = its 12-bit maximum [27:16] and minimum [11:0], 0x1044 = its mean on the 16-bit scale. The
//! values are taken after the ADC x2 of 0x105c and before the Q10 read-path gain and offset (wpu.rs), so a code c on
//! the 16-bit scale is the read-out word 32768 + (c - 32768) * dg, dg = the channel's digital gain without the x2.
//!
//! The slots of a channel (docs/hardware/fpga.md "Channel to core and slot mapping"; slot k shows core k): 1 channel
//! all eight; 2 channels the lower-numbered channel 0-3, the higher 4-7; 4 channels (also three) CH1 0/2, CH2 1/3, CH3
//! 4/6, CH4 5/7. The DDR word order (CH1 = 6/7 ...) assumed before was wrong: on the unit (2026-11-13) a signal on CH1
//! and CH2 in 4-channel mode showed in slots 2/3, which this table gives to CH1 and CH2.

use crate::dho::bus::Bus;

/// One slot of one run: 12-bit minimum and maximum, mean on the 16-bit scale.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Slot {
    pub min: u16,
    pub max: u16,
    pub mean: f64,
}

/// Exponent of a run: 2^18 samples per core, 1.68 ms at 156.25 MHz (the largest the 19-bit sample counter takes).
pub const RUN_EXP: u32 = 18;
const RUN_US: u32 = ((1u64 << RUN_EXP) * 1_000_000 / 156_250_000) as u32 + 1;

/// One run of the measure unit: start, wait for done, read every slot. None when the unit does not finish or a read
/// is not ready.
pub fn run(bus: &mut dyn Bus, sleep_us: &mut dyn FnMut(u32)) -> Option<[Slot; 8]> {
    let cfg = (bus.rd(0x1040) & 0x77ff_f0e0) | RUN_EXP;
    bus.wr(0x1040, cfg);
    bus.wr(0x1040, cfg | 0x8000_0000);
    sleep_us(RUN_US);
    let mut done = bus.rd(0x1040) & 0x4000_0000 != 0;
    for _ in 0..5 {
        if done {
            break;
        }
        sleep_us(500);
        done = bus.rd(0x1040) & 0x4000_0000 != 0;
    }
    if !done {
        return None;
    }
    let sh = bus.rd(0x1040);
    let mut out = [Slot::default(); 8];
    for (k, o) in out.iter_mut().enumerate() {
        bus.wr(0x1040, (sh & !0xf00) | (k as u32) << 8);
        let (m, p) = (bus.rd(0x1044), bus.rd(0x1048));
        if m == 0xeeee_eeee || p == 0xeeee_eeee {
            return None;
        }
        *o = Slot { min: (p & 0xfff) as u16, max: (p >> 16 & 0xfff) as u16, mean: (m & 0xffff) as f64 };
    }
    Some(out)
}

/// The slots of channel `ch` (0..3) for the channel set `mask` (module comment); empty when it is not in the set.
pub fn slots(mask: u32, ch: usize) -> Vec<usize> {
    if mask & (1 << ch) == 0 {
        return Vec::new();
    }
    match crate::dho::hz::ratio(mask) {
        1 => (0..8).collect(),
        2 => {
            let lower = (0..ch).all(|c| mask & (1 << c) == 0);
            if lower {
                vec![0, 1, 2, 3]
            } else {
                vec![4, 5, 6, 7]
            }
        }
        _ => [[0, 2], [1, 3], [4, 6], [5, 7]][ch].to_vec(),
    }
}

/// The vertical set-up of one channel the levels are converted with: V/div, offset (V), digital gain (cal record,
/// 1.0 if unknown).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vert {
    pub scale: f64,
    pub offset: f64,
    pub gain: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The measure unit of a signal per slot: slot k sees `sig(k)` = (min, max, mean) as raw 16-bit codes.
    struct Unit {
        sel: usize,
        started: bool,
        sig: Vec<(f64, f64, f64)>,
    }

    impl Bus for Unit {
        fn wr(&mut self, off: u32, v: u32) {
            if off == 0x1040 {
                self.sel = (v >> 8 & 7) as usize;
                self.started |= v & 0x8000_0000 != 0;
            }
        }
        fn rd(&mut self, off: u32) -> u32 {
            let (lo, hi, mean) = self.sig[self.sel];
            match off {
                0x1040 => (self.started as u32) << 30 | (self.sel as u32) << 8 | RUN_EXP,
                0x1044 => mean as u32,
                0x1048 => ((hi / 16.0) as u32) << 16 | (lo / 16.0) as u32,
                _ => 0,
            }
        }
    }

    #[test]
    fn raw_run_reads_every_slot() {
        let mut unit = Unit {
            sel: 0,
            started: false,
            sig: (0..8).map(|k| (16000.0 + k as f64 * 16.0, 48000.0 + k as f64 * 16.0, 32000.0 + k as f64)).collect(),
        };
        let result = run(&mut unit, &mut |_| {}).unwrap();
        for (k, slot) in result.iter().enumerate() {
            assert_eq!(*slot, Slot { min: 1000 + k as u16, max: 3000 + k as u16, mean: 32000.0 + k as f64 });
        }
    }

    #[test]
    fn slots_follow_the_core_table() {
        assert_eq!(slots(1, 0), (0..8).collect::<Vec<_>>());
        assert_eq!((slots(0b1010, 1), slots(0b1010, 3)), (vec![0, 1, 2, 3], vec![4, 5, 6, 7]));
        assert_eq!((slots(0b0101, 0), slots(0b0101, 2)), (vec![0, 1, 2, 3], vec![4, 5, 6, 7]));
        assert_eq!(
            (slots(0xf, 0), slots(0xf, 1), slots(0xf, 2), slots(0xf, 3)),
            (vec![0, 2], vec![1, 3], vec![4, 6], vec![5, 7])
        );
        assert_eq!((slots(0b0111, 2), slots(0b0111, 3), slots(0b1011, 2)), (vec![4, 6], vec![], vec![]));
    }

    #[test]
    fn unfinished_run_returns_none() {
        struct Pending;
        impl Bus for Pending {
            fn wr(&mut self, _: u32, _: u32) {}
            fn rd(&mut self, _: u32) -> u32 {
                0
            }
        }
        assert!(run(&mut Pending, &mut |_| {}).is_none());
    }
}
