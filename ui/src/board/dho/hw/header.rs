// SPDX-License-Identifier: GPL-2.0-or-later

//! The frame-header log of `OOS_FRAME_HEADER` (trigger.md 8, `switches::HeaderProbe`).

use oos_hal::dho::seq::Settings;

/// One line of the header log: the 8 header words, `pre`, and the offsets (samples) of the nearest rising and falling
/// crossing of the trigger level from `pre` on the trigger channel. `only_off` (OOS_FRAME_HEADER=off): only the frames
/// whose edge (of the trigger slope) is not within 20 samples of `pre`, and a count of all frames every 10,000 (the
/// `probed` counter): a long run for rare frames such as issue #5's.
pub(super) fn log(h: &[u8], v: &[f32], s: &Settings, (pre, k): (usize, usize), only_off: bool) {
    use std::sync::atomic::{AtomicU32, Ordering};
    static LINES: AtomicU32 = AtomicU32::new(0);
    let tc = (s.trig_src.clamp(1, 4) - 1) as usize;
    let hyst = (0.3 * s.ch[tc].scale) as f32;
    let near = |rising: bool| {
        crate::fastdraw::edge_near(v, pre, s.level as f32, hyst, rising, false, v.len()).map(|t| t - pre as f64)
    };
    let mut line = String::new();
    if only_off {
        let n = crate::drawpipe::counters().losses.probed.fetch_add(1, Ordering::Relaxed) + 1;
        if n.is_multiple_of(10_000) {
            line = format!("frames {}\n", n);
        }
        let rising = !matches!(s.slope, oos_hal::dho::tpu::Slope::Falling);
        if near(rising).is_some_and(|t| t.abs() <= 20.0) {
            return append_header_log(&line);
        }
    }
    if LINES.fetch_add(1, Ordering::Relaxed) >= 20_000 {
        return;
    }
    let words: Vec<String> = h.as_chunks::<2>().0.iter().map(|b| format!("{:04x}", u16::from_le_bytes(*b))).collect();
    let off = |rising: bool| near(rising).map_or("-".to_string(), |t| format!("{:.1}", t));
    line += &format!(
        "{} pre {} k {} n {} rise {} fall {} scale {} tdiv {:e}\n",
        words.join(" "),
        pre,
        k,
        v.len(),
        off(true),
        off(false),
        s.ch[tc].scale,
        s.tdiv
    );
    append_header_log(&line);
}

fn append_header_log(line: &str) {
    use std::io::Write;
    if line.is_empty() {
        return;
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open("/tmp/oos-hdr.log") {
        let _ = f.write_all(line.as_bytes());
    }
}
