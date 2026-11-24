// SPDX-License-Identifier: GPL-2.0-or-later

use super::{
    acq, compress_k, compress_word, compressed_len, frame_bytes, hz, scu, stock_play, window, Settings, Step, StockPlay,
};

pub(super) fn steps_at(
    s: &Settings,
    p: &hz::Params,
    shadows: &mut scu::Shadows,
    (first, count): (u32, u32),
) -> Vec<Step> {
    let nf = if s.playback.is_play4() { count.max(1) } else { 1 };
    let mut st = Vec::new();
    let stock = stock_play(s, p);
    let (n, start) = stock.map_or_else(|| window(s, p), StockPlay::window);
    let win = readout_window(s, p, (n, start));
    st.push(Step::Note(format!("read-out: {} points from memory point {} (trigger at {})", n, start, p.pre)));
    let mut ro = if s.playback.is_play4() {
        play4(s, p, &win, stock, shadows, (first, nf))
    } else {
        scu::readout(&win, shadows)
    };
    if s.header {
        // frame header on: the FPGA puts a 16-byte header in front of the frame
        for w in ro.iter_mut().filter(|w| w.0 == 0x1010) {
            w.1 &= !(1 << 31);
        }
        shadows.s1010 &= !(1 << 31);
    }
    st.extend(ro.into_iter().map(Step::Wr));
    st.push(Step::Peek(&[0x4034, 0x4080]));
    st.push(Step::Read { frames: nf as usize, frame: frame_bytes(s, p) });
    st
}

fn readout_window(s: &Settings, p: &hz::Params, (n, start): (u32, u32)) -> scu::Window {
    scu::Window {
        n: n * p.ratio,
        start: start * p.ratio,
        ratio: p.ratio,
        fsm: s.mask,
        la: s.la.is_some(),
        avg: matches!(s.mode, acq::Mode::Average { .. }),
    }
}

fn play4(
    s: &Settings,
    p: &hz::Params,
    win: &scu::Window,
    stock: Option<StockPlay>,
    shadows: &mut scu::Shadows,
    (first, count): (u32, u32),
) -> Vec<crate::dho::Write> {
    let mut writes = scu::passthrough();
    let mut read = scu::readout_play4(win, shadows, first, count);
    if let Some(play) = stock {
        let play_writes = play.writes();
        writes.reserve(read.len() + play_writes.len());
        writes.extend(play_writes);
        for write in &mut read {
            match write.0 {
                0x1100 => write.1 += 1, // Add the stock display path's k = 1.
                0x11f0 => write.1 = play.frame_len(),
                _ => {}
            }
        }
    } else if s.compress > 1 {
        let word = compress_word(compress_k(s, p));
        let len = compressed_len(s, p);
        for write in writes.iter_mut().filter(|write| write.0 == 0x1170) {
            write.1 = word;
        }
        for write in read.iter_mut().filter(|write| write.0 == 0x11f0) {
            write.1 = len;
        }
    }
    writes.extend(read);
    writes
}

/// Main TX length register for the selected read path, in points per channel.
pub fn frame_len(s: &Settings, p: &hz::Params) -> u32 {
    if let Some(play) = stock_play(s, p) {
        play.frame_len()
    } else if s.compress > 1 && s.playback.is_play4() {
        compressed_len(s, p)
    } else {
        scu::tx_points(&readout_window(s, p, window(s, p)))
    }
}
