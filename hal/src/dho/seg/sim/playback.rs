// SPDX-License-Identifier: GPL-2.0-or-later

use super::{Engine, HEADER_TICK_FS};
use crate::dho::{pairs, seq};

impl Engine {
    pub(super) fn play(&mut self, first: u32, last: u32) {
        let ratio = self.ratio as usize;
        let points = self.reg(0x11f0);
        let start = self.reg(0x1108);
        let compression = self.reg(0x1170);
        let factor = compression & seq::COMPRESS_K_MAX;
        let compressed = compression & (1 << 31) != 0;
        let average = self.reg(0x1004) & 8 != 0;
        if compressed && !self.compressed_window_fits(start, points, factor) {
            return;
        }
        let header = self.reg(0x1010) & (1 << 31) == 0;
        let words = points as usize * ratio;
        let stride = seq::frame_stride(2 * words + if header { 16 } else { 0 });
        self.stream.reserve(stride * (last.saturating_sub(first) as usize + 1));
        for frame in first..=last {
            let end = self.stream.len() + stride;
            if header {
                self.play_header(frame);
            }
            if average {
                for word in self.avg_words(frame, words) {
                    self.stream.extend(word.to_le_bytes());
                }
            } else if compressed {
                self.play_pairs(frame, start as usize, points as usize, factor as usize);
            } else {
                for i in 0..words {
                    self.stream.extend(Self::word(frame, i).to_le_bytes());
                }
            }
            self.stream.resize(end, 0);
        }
        self.last_played = Some(last);
    }

    fn compressed_window_fits(&self, start: u32, points: u32, factor: u32) -> bool {
        let window = self.reg(0x1100).saturating_sub(0x48 | (self.ratio << 9));
        let record = self.reg(0x10e4);
        // The HAL permits an unaligned input length when playing the whole record.
        let full_record = start == 0 && window == record;
        start.is_multiple_of(8)
            && (full_record || window.is_multiple_of(8))
            && points >= 8
            && points.is_multiple_of(8)
            && window <= record.saturating_sub(start)
            && factor != 0
            && points <= window / factor
    }

    fn play_header(&mut self, frame: u32) {
        let h = (self.header_fs(frame) / HEADER_TICK_FS) as u64 & ((1 << 48) - 1);
        for word in [0xfa05, 0, 0x7b, 0, frame as u16, (h >> 32) as u16, (h >> 16) as u16, h as u16] {
            self.stream.extend(word.to_le_bytes());
        }
    }

    fn play_pairs(&mut self, frame: u32, start: usize, points: usize, factor: usize) {
        let ratio = self.ratio as usize;
        for output in (0..points).step_by(pairs::PAIR) {
            let lo = start + output * factor;
            let hi = lo + pairs::BLOCK_K * factor;
            let mut extrema = [[0u16; 4]; pairs::PAIR];
            let [first, last] = &mut extrema;
            for (lane, (first, last)) in first[..ratio].iter_mut().zip(&mut last[..ratio]).enumerate() {
                let samples = (lo..hi).map(|i| Self::word(frame, i * ratio + lane));
                if let Some([early, late]) = pairs::pair(samples, pairs::ORDER) {
                    *first = early;
                    *last = late;
                }
            }
            for row in extrema {
                for word in &row[..ratio] {
                    self.stream.extend(word.to_le_bytes());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compressed_play_stalls_on_invalid_windows() {
        for (start, window, points, factor) in [
            (2, 24, 8, 2),  // unaligned start
            (0, 26, 8, 2),  // unaligned input size
            (0, 24, 10, 2), // even but unaligned output size
            (48, 24, 8, 2), // input window overruns the record even when the requested output fits
            (0, 24, 16, 2), // output needs more input than the window supplies
            (0, 24, 8, 0),  // zero compression factor
        ] {
            let mut engine = Engine::new(0, 800_000, 1, 1);
            engine.regs.extend([(0x10e4, 64), (0x1100, 0x248 + window), (0x1108, start), (0x11f0, points)]);
            engine.regs.insert(0x1170, 0xc000_0000 | factor);
            engine.play(0, 0);
            assert!(engine.stream.is_empty(), "start {start}, window {window}, points {points}, factor {factor}");
            assert_eq!(engine.last_played, None);
        }
    }

    #[test]
    fn compressed_play_accepts_an_exact_end_and_an_unaligned_full_record() {
        for (start, window, record) in [(40, 24, 64), (0, 26, 26)] {
            let mut engine = Engine::new(0, 800_000, 1, 1);
            engine.regs.extend([(0x10e4, record), (0x1100, 0x248 + window), (0x1108, start), (0x11f0, 8)]);
            engine.regs.insert(0x1170, 0xc000_0002);
            engine.play(0, 0);
            assert_eq!(engine.stream.len(), 16 + 8 * 2);
            assert_eq!(engine.last_played, Some(0));
        }
    }
}
