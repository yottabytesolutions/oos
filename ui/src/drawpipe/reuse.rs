// SPDX-License-Identifier: GPL-2.0-or-later

use crate::{board::Frame, model::*, wave::Record};

/// Inputs to normal analog-edge conversion, captured before Source::acquire.
/// Other trigger/acquisition kinds deliberately retain the independent fallback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct EdgeInputs {
    window: u64,
    first: u64,
    source: usize,
    slope: Slope,
    level: u32,
    coupling: TrigCoup,
    hysteresis: [u32; NCH],
}
impl EdgeInputs {
    pub(crate) fn of(m: &Model) -> Option<Self> {
        if m.acq != AcqMode::Normal || m.trig_type != TrigType::Edge || m.trig_line().is_some() {
            return None;
        }
        let window = m.time_per_div() as f64 * HDIV as f64;
        Some(Self {
            window: window.to_bits(),
            first: (m.delay as f64 - window / 2.0).to_bits(),
            source: m.trig_src,
            slope: m.slope,
            level: m.trig_level.to_bits(),
            coupling: m.trig_coupling(),
            hysteresis: std::array::from_fn(|c| (m.trig_hyst_div() * m.ch[c].volts_per_div()).to_bits()),
        })
    }
    pub(super) fn matches(self, m: &Model, f: &Frame, rec: &Record) -> bool {
        // Restrict reuse to envelope records: sparse::draw_with always declines them.
        // Frame identity pins dt, pre, fixed_ti, samples, LA and acquisition metadata.
        // For analog Edge, span is zero; source/slope/level/coupling and hysteresis
        // cover every remaining trigger input. Normal pins the resampling mode.
        crate::deepmeas::deep_pairs(f)
            && Some(self) == Self::of(m)
            && rec.t0.to_bits() == self.first
            && rec.dt.to_bits() == (f64::from_bits(self.window) / 3600.0).to_bits()
            && rec.ch.iter().zip(&f.ch).all(|(out, input)| out.len() == if input.len() < 2 { 0 } else { 3600 })
            && rec.raw.as_ref().is_some_and(|r| r.conditioned && std::ptr::eq(&*r.frame, f))
            && enabled()
    }
}

#[cfg(test)]
fn enabled() -> bool {
    REUSE.with(std::cell::Cell::get)
}
#[cfg(not(test))]
fn enabled() -> bool {
    true
}

#[cfg(test)]
thread_local! {
    static REUSE: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

/// Run the actual previous double-conversion path for correctness and timing controls.
#[cfg(test)]
pub(crate) fn without_reuse<T>(f: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            REUSE.with(|on| on.set(self.0));
        }
    }
    let _restore = Restore(REUSE.with(|on| on.replace(false)));
    f()
}

#[cfg(test)]
mod tests;
