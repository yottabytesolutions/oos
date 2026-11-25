// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use oos_hal::dho::afe;
use oos_hal::dho::seq::{self, Step};

pub(crate) fn run(a: &[String]) {
    super::run::acquire("plan", a);
}

pub(super) fn print(st: &[Step]) {
    for s in st {
        match s {
            Step::Wr((o, v)) => println!("  wr  {:#06x} {:#010x}", o, v),
            Step::Afe(afe::Op::Afe { ch, reg, val }) => println!("  afe ch{} {:#04x} {:#06x}", ch + 1, reg, val),
            Step::Afe(afe::Op::Gear { ch, gear }) => println!("  gear ch{} {}", ch + 1, gear),
            Step::Afe(afe::Op::Adc(w)) => println!("  wr  0x1034 {:#010x}  (ADC SPI)", w),
            Step::Poll { reg, bit, ms, what } => println!("  poll {:#06x} & {:#x} (max {} ms): {}", reg, bit, ms, what),
            Step::Read { frames, frame } => println!(
                "  read {} frames of {} bytes ({} with the stream padding) from /dev/xdma0_c2h_0",
                frames,
                frame,
                seq::read_len(*frames, *frame)
            ),
            Step::ReadPadded { frames, frame, stride } => println!(
                "  read {} frames of {} bytes ({} with the stream padding) from /dev/xdma0_c2h_0",
                frames,
                frame,
                frames * stride
            ),
            Step::Sleep { us } => println!("  sleep {} us", us),
            Step::Peek(r) => println!("  peek {:x?}", r),
            Step::Note(n) => println!("# {}", n),
        }
    }
    println!("# {} hardware writes", st.iter().filter(|s| matches!(s, Step::Wr(_) | Step::Afe(_))).count());
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_matrix_case_passes_the_playback_length_check() {
        for case in include_str!("../../../tools/plan-matrix-cases.txt").lines() {
            let args = case.split_ascii_whitespace().map(str::to_owned).collect::<Vec<_>>();
            let opt = super::super::acquisition::parse(&args).unwrap();
            let p = oos_hal::dho::seq::record_geometry(&opt.s).unwrap();
            // Geometry is independent of the AFE calibration. The matrix gate checks the full plan with calibration.
            let mut state = oos_hal::dho::seq::State::handover(opt.app_mask, None);
            state.acquire(&opt.s, &p).unwrap_or_else(|error| panic!("{case}: {error}"));
        }
    }
}
